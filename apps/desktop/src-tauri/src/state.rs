use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use app_core::{
    AppError, BranchRequest, CloneRequest, CommitRequest, ConflictResolution, DiffTarget,
    FileBlame, GitIdentity, GitIdentitySettings, GitReference, GitRemote, HistoryFilter,
    HistoryOperation, InteractiveRebasePreview, InteractiveRebaseRequest, PatchSelection,
    PathHistory, ReflogEntry, RemoteProgress, RemoteRequest, RemoteTagSummary, RepositoryScheduler,
    RepositoryService, RepositorySidebar, StashRequest,
};
use git_cli::CancellationToken;
use git_domain::{
    DiffDocument, HeadState, HistoryPage, RepoId, RepositoryDescriptor, RepositorySnapshot,
    WorktreeId,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use persistence::{OperationRecord, OperationRecovery, SessionStore, SessionTab};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

struct OpenRepository {
    descriptor: RepositoryDescriptor,
    revision: u64,
}

pub struct SessionTabState {
    pub stored: SessionTab,
    pub snapshot: Option<RepositorySnapshot>,
    pub unavailable: bool,
    pub loading: bool,
}

pub struct SessionTabUpdate {
    pub page: String,
    pub selected_path: Option<String>,
    pub selected_diff: String,
    pub panel_width: f64,
    pub history_cursor: Option<String>,
    pub selected_commit: Option<String>,
    pub history_filter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepositoryOpenSource {
    pub repository_name: String,
    pub worktree_path: String,
}

pub struct RepositoryIdentityState {
    pub repo_id: String,
    pub repository_name: String,
    pub settings: GitIdentitySettings,
}

#[derive(Debug, Clone)]
pub struct OperationRecoveryData {
    pub action: String,
    pub before_head_oid: Option<String>,
    pub after_head_oid: Option<String>,
    pub before_head_ref: Option<String>,
    pub after_head_ref: Option<String>,
    pub recovery_ref: Option<String>,
    pub recovery_oid: Option<String>,
}

pub struct ApplicationState {
    service: RepositoryService,
    scheduler: RepositoryScheduler,
    repositories: Mutex<HashMap<RepoId, OpenRepository>>,
    watchers: Mutex<HashMap<WorktreeId, RecommendedWatcher>>,
    operations: Mutex<HashMap<Uuid, CancellationToken>>,
    identity_lock: Mutex<()>,
    session: SessionStore,
}

impl ApplicationState {
    pub fn new(session: SessionStore) -> Self {
        Self {
            service: RepositoryService::default(),
            scheduler: RepositoryScheduler::default(),
            repositories: Mutex::default(),
            watchers: Mutex::default(),
            operations: Mutex::default(),
            identity_lock: Mutex::default(),
            session,
        }
    }

    pub async fn open_repository(
        &self,
        path: &Path,
        opened_from: Option<RepositoryOpenSource>,
        app: &AppHandle,
    ) -> Result<Vec<SessionTabState>, AppError> {
        let descriptor = self.service.discover(path)?;
        let repo_id = descriptor.id;
        let snapshot = self
            .scheduler
            .read(repo_id, || self.service.snapshot(&descriptor, 1))?;
        self.install_watcher(&descriptor, app)?;
        self.repositories
            .lock()
            .expect("repository registry lock poisoned")
            .insert(
                repo_id,
                OpenRepository {
                    descriptor: descriptor.clone(),
                    revision: 1,
                },
            );

        let tabs = self.load_stored_tabs().await?;
        let existing = tabs.iter().find(|tab| tab.repo_id == repo_id.to_string());
        self.session
            .upsert_tab(&SessionTab {
                repo_id: repo_id.to_string(),
                worktree_id: descriptor.worktree_id.to_string(),
                worktree_path: descriptor.worktree_path.to_string_lossy().into_owned(),
                opened_from_repository_name: opened_from
                    .as_ref()
                    .map(|source| source.repository_name.clone()),
                opened_from_worktree_path: opened_from.map(|source| source.worktree_path),
                tab_order: existing
                    .map(|tab| tab.tab_order)
                    .unwrap_or(tabs.len() as i64),
                active: true,
                page: existing
                    .map(|tab| tab.page.clone())
                    .unwrap_or_else(|| "changes".to_owned()),
                selected_path: existing.and_then(|tab| tab.selected_path.clone()),
                selected_diff: existing
                    .map(|tab| tab.selected_diff.clone())
                    .unwrap_or_else(|| "unstaged".to_owned()),
                panel_width: existing.map(|tab| tab.panel_width).unwrap_or(280.0),
                history_cursor: existing.and_then(|tab| tab.history_cursor.clone()),
                selected_commit: existing.and_then(|tab| tab.selected_commit.clone()),
                history_filter: existing.and_then(|tab| tab.history_filter.clone()),
            })
            .await
            .map_err(persistence_error)?;
        self.session_tabs(Some((repo_id, snapshot)), false).await
    }

    pub async fn restore_session(&self, app: &AppHandle) -> Result<Vec<SessionTabState>, AppError> {
        let tabs = self.load_stored_tabs().await?;
        for mut tab in tabs {
            let result = self.service.discover(Path::new(&tab.worktree_path));
            match result {
                Ok(descriptor) => {
                    let repo_id = descriptor.id;
                    if tab.repo_id != repo_id.to_string() {
                        self.session
                            .close(&tab.repo_id)
                            .await
                            .map_err(persistence_error)?;
                        tab.repo_id = repo_id.to_string();
                    }
                    tab.worktree_id = descriptor.worktree_id.to_string();
                    tab.worktree_path = descriptor.worktree_path.to_string_lossy().into_owned();
                    self.session
                        .upsert_tab(&tab)
                        .await
                        .map_err(persistence_error)?;
                    self.install_watcher(&descriptor, app)?;
                    self.repositories
                        .lock()
                        .expect("repository registry lock poisoned")
                        .insert(
                            repo_id,
                            OpenRepository {
                                descriptor,
                                revision: 1,
                            },
                        );
                }
                Err(AppError::InvalidPath | AppError::RepositoryNotFound) => {}
                Err(error) => return Err(error),
            }
        }
        self.session_tabs(None, true).await
    }

    pub(crate) async fn backfill_submodule_sources(&self) -> Result<(), AppError> {
        let repositories = self
            .repositories
            .lock()
            .expect("repository registry lock poisoned")
            .values()
            .map(|repository| repository.descriptor.clone())
            .collect::<Vec<_>>();
        let mut sources_by_path = HashMap::new();
        for repository in repositories {
            let Ok(sidebar) = self.service.sidebar(&repository) else {
                continue;
            };
            for submodule in sidebar.submodules {
                sources_by_path.insert(
                    worktree_path_key(Path::new(&submodule.absolute_path)),
                    RepositoryOpenSource {
                        repository_name: repository.name.clone(),
                        worktree_path: repository.worktree_path.to_string_lossy().into_owned(),
                    },
                );
            }
        }

        for mut tab in self.load_stored_tabs().await? {
            if tab.opened_from_repository_name.is_some() {
                continue;
            }
            let Some(source) =
                sources_by_path.get(&worktree_path_key(Path::new(&tab.worktree_path)))
            else {
                continue;
            };
            tab.opened_from_repository_name = Some(source.repository_name.clone());
            tab.opened_from_worktree_path = Some(source.worktree_path.clone());
            self.session
                .upsert_tab(&tab)
                .await
                .map_err(persistence_error)?;
        }
        Ok(())
    }

    pub fn repository_snapshot(&self, repo_id: &str) -> Result<RepositorySnapshot, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let (descriptor, revision) = {
            let mut repositories = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned");
            let repository = repositories
                .get_mut(&repo_id)
                .ok_or(AppError::RepositoryNotOpen)?;
            repository.revision += 1;
            (repository.descriptor.clone(), repository.revision)
        };
        self.scheduler
            .read(repo_id, || self.service.snapshot(&descriptor, revision))
    }

    pub fn repository_sidebar(&self, repo_id: &str) -> Result<RepositorySidebar, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self
            .repositories
            .lock()
            .expect("repository registry lock poisoned")
            .get(&repo_id)
            .ok_or(AppError::RepositoryNotOpen)?
            .descriptor
            .clone();
        self.scheduler
            .read(repo_id, || self.service.sidebar(&descriptor))
    }

    pub fn repository_history(
        &self,
        repo_id: &str,
        filter: &HistoryFilter,
    ) -> Result<HistoryPage, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.history(&descriptor, filter))
    }

    pub fn repository_references(&self, repo_id: &str) -> Result<Vec<GitReference>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.references(&descriptor))
    }

    pub fn repository_reflog(&self, repo_id: &str) -> Result<Vec<ReflogEntry>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.reflog(&descriptor, 200))
    }

    pub fn restore_reflog_reference(
        &self,
        repo_id: &str,
        revision: u64,
        oid: &str,
        name: &str,
        is_tag: bool,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.restore_reflog_reference(repository, oid, name, is_tag)
        })
    }

    pub fn repository_remote_tags(
        &self,
        repo_id: &str,
        remote: Option<&str>,
    ) -> Result<Vec<RemoteTagSummary>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.remote_tags(&descriptor, remote))
    }

    pub fn repository_remotes(&self, repo_id: &str) -> Result<Vec<GitRemote>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.remotes(&descriptor))
    }

    pub fn global_identity(&self) -> Result<GitIdentity, AppError> {
        let _guard = self
            .identity_lock
            .lock()
            .expect("Git identity lock poisoned");
        self.service.global_identity()
    }

    pub fn repository_identity(&self, repo_id: &str) -> Result<RepositoryIdentityState, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        let settings = self
            .scheduler
            .read(repo_id, || self.service.repository_identity(&descriptor))?;
        Ok(RepositoryIdentityState {
            repo_id: repo_id.to_string(),
            repository_name: descriptor.name,
            settings,
        })
    }

    pub fn update_global_identity(
        &self,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<GitIdentity, AppError> {
        let _guard = self
            .identity_lock
            .lock()
            .expect("Git identity lock poisoned");
        self.service.update_global_identity(name, email)
    }

    pub fn update_repository_identity(
        &self,
        repo_id: &str,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<RepositoryIdentityState, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        let settings = self.scheduler.write(repo_id, || {
            self.service
                .update_repository_identity(&descriptor, name, email)
        })?;
        Ok(RepositoryIdentityState {
            repo_id: repo_id.to_string(),
            repository_name: descriptor.name,
            settings,
        })
    }

    pub fn add_remote(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
        url: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.add_remote(repository, name, url)
        })
    }

    pub fn update_remote(
        &self,
        repo_id: &str,
        revision: u64,
        existing_name: &str,
        name: &str,
        url: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.update_remote(repository, existing_name, name, url)
        })
    }

    pub fn remove_remote(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.remove_remote(repository, name)
        })
    }

    pub fn add_submodule(
        &self,
        repo_id: &str,
        revision: u64,
        url: &str,
        path: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.add_submodule(repository, url, path)
        })
    }

    pub fn initialize_submodule(
        &self,
        repo_id: &str,
        revision: u64,
        path: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.initialize_submodule(repository, path)
        })
    }

    pub fn deinitialize_submodule(
        &self,
        repo_id: &str,
        revision: u64,
        path: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.deinitialize_submodule(repository, path)
        })
    }

    pub fn remove_submodule(
        &self,
        repo_id: &str,
        revision: u64,
        path: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.remove_submodule(repository, path)
        })
    }

    pub fn repository_diff(
        &self,
        repo_id: &str,
        path: &[u8],
        target: DiffTarget,
    ) -> Result<DiffDocument, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.read(repo_id, || {
            let repositories = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned");
            let repository = repositories
                .get(&repo_id)
                .ok_or(AppError::RepositoryNotOpen)?;
            self.service.diff(&repository.descriptor, path, target)
        })
    }

    pub fn repository_blame(
        &self,
        repo_id: &str,
        path: &[u8],
        target_revision: Option<&str>,
    ) -> Result<FileBlame, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler.read(repo_id, || {
            self.service.blame(&descriptor, path, target_revision)
        })
    }

    pub fn repository_path_history(
        &self,
        repo_id: &str,
        path: &[u8],
        is_directory: bool,
        query: Option<&str>,
        limit: usize,
    ) -> Result<PathHistory, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler.read(repo_id, || {
            self.service
                .path_history(&descriptor, path, is_directory, query, limit)
        })
    }

    pub fn repository_commit_files(
        &self,
        repo_id: &str,
        revision: &str,
    ) -> Result<Vec<app_core::CommitFile>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler
            .read(repo_id, || self.service.commit_files(&descriptor, revision))
    }

    pub fn repository_commit_diff(
        &self,
        repo_id: &str,
        revision: &str,
        path: &[u8],
    ) -> Result<DiffDocument, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let descriptor = self.descriptor(repo_id)?;
        self.scheduler.read(repo_id, || {
            self.service.commit_diff(&descriptor, revision, path)
        })
    }

    pub fn stage_paths(
        &self,
        repo_id: &str,
        revision: u64,
        paths: &[Vec<u8>],
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.stage_paths(repository, paths)
        })
    }

    pub fn unstage_paths(
        &self,
        repo_id: &str,
        revision: u64,
        paths: &[Vec<u8>],
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            let snapshot = service.snapshot(repository, revision)?;
            service.unstage_paths(
                repository,
                paths,
                matches!(snapshot.status.head, HeadState::Unborn),
            )
        })
    }

    pub fn apply_selection(
        &self,
        repo_id: &str,
        revision: u64,
        path: &[u8],
        target: DiffTarget,
        selections: &[PatchSelection],
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.apply_selection(repository, path, target, selections)
        })
    }

    pub fn discard_path(
        &self,
        repo_id: &str,
        revision: u64,
        path: &[u8],
        untracked: bool,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.discard_path(repository, path, untracked)
        })
    }

    pub fn commit_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        request: &CommitRequest,
    ) -> Result<(RepositorySnapshot, Option<(String, String)>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let before = self.service.current_head_oid(&descriptor)?;
            self.service.commit(&descriptor, request)?;
            let after = self.service.current_head_oid(&descriptor)?;
            let next_revision = {
                let mut repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get_mut(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                repository.revision += 1;
                repository.revision
            };
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            Ok((snapshot, before.zip(after)))
        })
    }

    pub fn move_head_soft(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.move_head_soft(repository, expected_head_oid, target_head_oid)
        })
    }

    pub fn move_head_mixed(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.move_head_mixed(repository, expected_head_oid, target_head_oid)
        })
    }

    pub fn move_head_hard(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.move_head_hard(repository, expected_head_oid, target_head_oid)
        })
    }

    pub fn create_branch(
        &self,
        repo_id: &str,
        revision: u64,
        request: &BranchRequest,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.create_branch(repository, request)
        })
    }

    pub fn checkout_branch_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
        is_remote: bool,
        is_tag: bool,
        auto_stash: bool,
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let clean_before = self.service.is_worktree_clean(&descriptor)?;
            let before_head_oid = self.service.current_head_oid(&descriptor)?;
            let before_head_ref = self.service.current_head_ref(&descriptor)?;
            self.service
                .checkout_branch(&descriptor, name, is_remote, is_tag, auto_stash)?;
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let after_head_ref = self.service.current_head_ref(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let changed = before_head_oid != after_head_oid || before_head_ref != after_head_ref;
            let recovery =
                (clean_before && changed && snapshot.status.changes.is_empty()).then(|| {
                    OperationRecoveryData {
                        action: "checkout".to_owned(),
                        before_head_oid,
                        after_head_oid,
                        before_head_ref,
                        after_head_ref,
                        recovery_ref: None,
                        recovery_oid: None,
                    }
                });
            Ok((snapshot, recovery))
        })
    }

    pub fn delete_branch_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
        remote_references: &[(String, String)],
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let head_oid = self.service.current_head_oid(&descriptor)?;
            let branch_oid = self.service.local_branch_oid(&descriptor, name)?;
            for (remote, remote_name) in remote_references {
                self.service
                    .delete_remote_branch(&descriptor, remote, remote_name)?;
            }
            self.service.delete_branch(&descriptor, name)?;
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let recovery = if remote_references.is_empty() {
                head_oid
                    .clone()
                    .zip(branch_oid)
                    .map(|(head_oid, branch_oid)| OperationRecoveryData {
                        action: "branch-delete".to_owned(),
                        before_head_oid: Some(head_oid),
                        after_head_oid,
                        before_head_ref: None,
                        after_head_ref: None,
                        recovery_ref: Some(name.to_owned()),
                        recovery_oid: Some(branch_oid),
                    })
            } else {
                None
            };
            Ok((snapshot, recovery))
        })
    }

    pub fn checkout_for_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        target_head_oid: &str,
        target_head_ref: Option<&str>,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.checkout_for_recovery(
                repository,
                expected_head_oid,
                target_head_oid,
                target_head_ref,
            )
        })
    }

    pub fn restore_deleted_branch(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        name: &str,
        oid: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.restore_deleted_branch(repository, expected_head_oid, name, oid)
        })
    }

    pub fn delete_restored_branch(
        &self,
        repo_id: &str,
        revision: u64,
        expected_head_oid: &str,
        name: &str,
        oid: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.delete_restored_branch(repository, expected_head_oid, name, oid)
        })
    }

    pub fn rename_branch(
        &self,
        repo_id: &str,
        revision: u64,
        old_name: &str,
        new_name: &str,
        rename_remote: bool,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.rename_branch(repository, old_name, new_name, rename_remote)
        })
    }

    pub fn rebase_onto_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        reference: &str,
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let clean_before = self.service.is_worktree_clean(&descriptor)?;
            let before_head_oid = self.service.current_head_oid(&descriptor)?;
            self.service
                .with_submodule_update(&descriptor, false, |_| {
                    self.service.rebase_onto(&descriptor, reference)
                })?;
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let recovery = (clean_before
                && before_head_oid != after_head_oid
                && snapshot.operation.is_none()
                && snapshot.status.changes.is_empty())
            .then(|| OperationRecoveryData {
                action: "rebase".to_owned(),
                before_head_oid,
                after_head_oid,
                before_head_ref: None,
                after_head_ref: None,
                recovery_ref: None,
                recovery_oid: None,
            });
            Ok((snapshot, recovery))
        })
    }

    pub fn reset_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        target_head_oid: &str,
        mode: &str,
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let clean_before = self.service.is_worktree_clean(&descriptor)?;
            let before_head_oid = self.service.current_head_oid(&descriptor)?;
            let before_head_ref = self.service.current_head_ref(&descriptor)?;
            self.service
                .with_submodule_update(&descriptor, false, |_| {
                    self.service.reset_head(&descriptor, target_head_oid, mode)
                })?;
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let after_head_ref = self.service.current_head_ref(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let recovery = (clean_before
                && before_head_oid != after_head_oid
                && before_head_ref == after_head_ref)
                .then(|| OperationRecoveryData {
                    action: format!("reset-{mode}"),
                    before_head_oid,
                    after_head_oid,
                    before_head_ref,
                    after_head_ref,
                    recovery_ref: None,
                    recovery_oid: None,
                });
            Ok((snapshot, recovery))
        })
    }

    pub fn history_mutation_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        operation: HistoryOperation,
        oids: &[String],
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let clean_before = self.service.is_worktree_clean(&descriptor)?;
            let before_head_oid = self.service.current_head_oid(&descriptor)?;
            let before_head_ref = self.service.current_head_ref(&descriptor)?;
            match operation {
                HistoryOperation::CherryPick => self.service.cherry_pick(&descriptor, oids)?,
                HistoryOperation::Revert => self.service.revert(&descriptor, oids)?,
            }
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let after_head_ref = self.service.current_head_ref(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let action = match operation {
                HistoryOperation::CherryPick => "cherry-pick",
                HistoryOperation::Revert => "revert",
            };
            let recovery = (clean_before
                && before_head_oid != after_head_oid
                && before_head_ref == after_head_ref
                && snapshot.operation.is_none()
                && snapshot.status.changes.is_empty())
            .then(|| OperationRecoveryData {
                action: action.to_owned(),
                before_head_oid,
                after_head_oid,
                before_head_ref,
                after_head_ref,
                recovery_ref: None,
                recovery_oid: None,
            });
            Ok((snapshot, recovery))
        })
    }

    pub fn continue_history_operation(
        &self,
        repo_id: &str,
        revision: u64,
        operation: HistoryOperation,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.continue_history_operation(repository, operation)
        })
    }

    pub fn abort_history_operation(
        &self,
        repo_id: &str,
        revision: u64,
        operation: HistoryOperation,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.abort_history_operation(repository, operation)
        })
    }

    pub fn skip_history_operation(
        &self,
        repo_id: &str,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.skip_history_operation(repository)
        })
    }

    pub fn interactive_rebase_preview(
        &self,
        repo_id: &str,
        revision: u64,
        base_oid: &str,
    ) -> Result<InteractiveRebasePreview, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.read(repo_id, || {
            let repositories = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned");
            let repository = repositories
                .get(&repo_id)
                .ok_or(AppError::RepositoryNotOpen)?;
            ensure_revision(revision, repository.revision)?;
            self.service
                .interactive_rebase_preview(&repository.descriptor, base_oid)
        })
    }

    pub fn start_interactive_rebase_with_recovery(
        &self,
        repo_id: &str,
        revision: u64,
        request: &InteractiveRebaseRequest,
        sequence_editor_executable: &Path,
    ) -> Result<(RepositorySnapshot, Option<OperationRecoveryData>), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            let clean_before = self.service.is_worktree_clean(&descriptor)?;
            let before_head_oid = self.service.current_head_oid(&descriptor)?;
            let before_head_ref = self.service.current_head_ref(&descriptor)?;
            self.service.start_interactive_rebase(
                &descriptor,
                request,
                sequence_editor_executable,
            )?;
            let after_head_oid = self.service.current_head_oid(&descriptor)?;
            let after_head_ref = self.service.current_head_ref(&descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            let snapshot = self.service.snapshot(&descriptor, next_revision)?;
            let recovery = (clean_before
                && before_head_oid != after_head_oid
                && before_head_ref == after_head_ref
                && snapshot.operation.is_none()
                && snapshot.status.changes.is_empty())
            .then(|| OperationRecoveryData {
                action: "interactive-rebase".to_owned(),
                before_head_oid,
                after_head_oid,
                before_head_ref,
                after_head_ref,
                recovery_ref: None,
                recovery_oid: None,
            });
            Ok((snapshot, recovery))
        })
    }

    pub fn continue_rebase(
        &self,
        repo_id: &str,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.continue_rebase(repository)
        })
    }

    pub fn skip_rebase(
        &self,
        repo_id: &str,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.skip_rebase(repository)
        })
    }

    pub fn abort_rebase(
        &self,
        repo_id: &str,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.abort_rebase(repository)
        })
    }

    pub fn create_tag(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
        target: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.create_tag(repository, name, target)
        })
    }

    pub fn delete_tag(
        &self,
        repo_id: &str,
        revision: u64,
        name: &str,
        remote_references: &[(String, String)],
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            for (remote, remote_name) in remote_references {
                service.delete_remote_tag(repository, remote, remote_name)?;
            }
            service.delete_tag(repository, name)
        })
    }

    pub fn merge_reference(
        &self,
        repo_id: &str,
        revision: u64,
        reference: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.merge_reference(repository, reference)
        })
    }

    pub fn fast_forward_branch(
        &self,
        repo_id: &str,
        revision: u64,
        branch: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate_head(repo_id, revision, |service, repository| {
            service.fast_forward_branch(repository, branch)
        })
    }

    pub fn create_stash(
        &self,
        repo_id: &str,
        revision: u64,
        request: &StashRequest,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.create_stash(repository, request)
        })
    }

    pub fn apply_stash(
        &self,
        repo_id: &str,
        revision: u64,
        reference: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.apply_stash(repository, reference)
        })
    }

    pub fn drop_stash(
        &self,
        repo_id: &str,
        revision: u64,
        reference: &str,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.drop_stash(repository, reference)
        })
    }

    pub fn resolve_conflict(
        &self,
        repo_id: &str,
        revision: u64,
        path: &[u8],
        resolution: ConflictResolution,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.resolve_conflict(repository, path, resolution)
        })
    }

    pub fn abort_merge(
        &self,
        repo_id: &str,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.abort_merge(repository)
        })
    }

    pub async fn start_operation_record(
        &self,
        id: &str,
        repo_id: Option<&str>,
        kind: &str,
        summary: &str,
    ) -> Result<(), AppError> {
        self.session
            .start_operation(id, repo_id, kind, summary)
            .await
            .map_err(persistence_error)
    }

    pub async fn finish_operation_record(
        &self,
        id: &str,
        state: &str,
        summary: &str,
        diagnostic: Option<&str>,
    ) -> Result<(), AppError> {
        self.session
            .finish_operation(id, state, summary, diagnostic)
            .await
            .map_err(persistence_error)
    }

    pub async fn operation_history(&self) -> Result<Vec<OperationRecord>, AppError> {
        self.session
            .list_operations(100)
            .await
            .map_err(persistence_error)
    }

    pub async fn operation_record(&self, id: &str) -> Result<Option<OperationRecord>, AppError> {
        self.session.operation(id).await.map_err(persistence_error)
    }

    pub async fn attach_operation_recovery(
        &self,
        id: &str,
        recovery: &OperationRecoveryData,
    ) -> Result<(), AppError> {
        self.session
            .attach_recovery(
                id,
                &OperationRecovery {
                    action: &recovery.action,
                    before_head_oid: recovery.before_head_oid.as_deref(),
                    after_head_oid: recovery.after_head_oid.as_deref(),
                    before_head_ref: recovery.before_head_ref.as_deref(),
                    after_head_ref: recovery.after_head_ref.as_deref(),
                    recovery_ref: recovery.recovery_ref.as_deref(),
                    recovery_oid: recovery.recovery_oid.as_deref(),
                },
            )
            .await
            .map_err(persistence_error)
    }

    pub async fn set_operation_recovery_state(
        &self,
        id: &str,
        recovery_state: &str,
    ) -> Result<(), AppError> {
        self.session
            .set_recovery_state(id, recovery_state)
            .await
            .map_err(persistence_error)
    }

    pub async fn activate_worktree(
        &self,
        repo_id: &str,
        worktree_id: &str,
        app: &AppHandle,
    ) -> Result<Vec<SessionTabState>, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        let current = self
            .repositories
            .lock()
            .expect("repository registry lock poisoned")
            .get(&repo_id)
            .ok_or(AppError::RepositoryNotOpen)?
            .descriptor
            .clone();
        let worktree = self
            .scheduler
            .read(repo_id, || self.service.sidebar(&current))?
            .worktrees
            .into_iter()
            .find(|worktree| worktree.id.to_string() == worktree_id)
            .ok_or(AppError::WorktreeNotFound)?;
        let descriptor = self.service.discover(Path::new(&worktree.path))?;
        if descriptor.id != repo_id {
            return Err(AppError::WorktreeNotFound);
        }
        self.install_watcher(&descriptor, app)?;
        let revision = {
            let mut repositories = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned");
            let revision = repositories
                .get(&repo_id)
                .map(|repository| repository.revision + 1)
                .unwrap_or(1);
            repositories.insert(
                repo_id,
                OpenRepository {
                    descriptor: descriptor.clone(),
                    revision,
                },
            );
            revision
        };
        let snapshot = self
            .scheduler
            .read(repo_id, || self.service.snapshot(&descriptor, revision))?;

        let tabs = self.load_stored_tabs().await?;
        let mut tab = tabs
            .into_iter()
            .find(|tab| tab.repo_id == repo_id.to_string())
            .ok_or(AppError::RepositoryNotOpen)?;
        tab.worktree_id = descriptor.worktree_id.to_string();
        tab.worktree_path = descriptor.worktree_path.to_string_lossy().into_owned();
        tab.selected_path = None;
        tab.selected_diff = "unstaged".to_owned();
        self.session
            .upsert_tab(&tab)
            .await
            .map_err(persistence_error)?;
        if current.worktree_id != descriptor.worktree_id {
            self.watchers
                .lock()
                .expect("watcher registry lock poisoned")
                .remove(&current.worktree_id);
        }
        self.session_tabs(Some((repo_id, snapshot)), false).await
    }

    pub async fn activate_tab(&self, repo_id: &str) -> Result<(), AppError> {
        self.session
            .activate(repo_id)
            .await
            .map_err(persistence_error)
    }

    pub async fn close_tab(&self, repo_id: &str) -> Result<Vec<SessionTabState>, AppError> {
        self.session
            .close(repo_id)
            .await
            .map_err(persistence_error)?;
        if let Ok(repo_id) = parse_repo_id(repo_id)
            && let Some(repository) = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned")
                .remove(&repo_id)
        {
            self.watchers
                .lock()
                .expect("watcher registry lock poisoned")
                .remove(&repository.descriptor.worktree_id);
        }
        let tabs = self.load_stored_tabs().await?;
        if !tabs.is_empty() && !tabs.iter().any(|tab| tab.active) {
            let next = tabs.last().expect("non-empty tabs");
            self.session
                .activate(&next.repo_id)
                .await
                .map_err(persistence_error)?;
        }
        self.session_tabs(None, false).await
    }

    pub async fn reorder_tabs(&self, repo_ids: &[String]) -> Result<(), AppError> {
        self.session
            .reorder(repo_ids)
            .await
            .map_err(persistence_error)
    }

    pub async fn update_tab(
        &self,
        repo_id: &str,
        update: SessionTabUpdate,
    ) -> Result<(), AppError> {
        let tabs = self.load_stored_tabs().await?;
        let mut tab = tabs
            .into_iter()
            .find(|tab| tab.repo_id == repo_id)
            .ok_or(AppError::RepositoryNotOpen)?;
        tab.page = update.page;
        tab.selected_path = update.selected_path;
        tab.selected_diff = update.selected_diff;
        tab.panel_width = update.panel_width;
        tab.history_cursor = update.history_cursor;
        tab.selected_commit = update.selected_commit;
        tab.history_filter = update.history_filter;
        self.session
            .upsert_tab(&tab)
            .await
            .map_err(persistence_error)
    }

    async fn load_stored_tabs(&self) -> Result<Vec<SessionTab>, AppError> {
        self.session.load_tabs().await.map_err(persistence_error)
    }

    fn advance_revision(&self, repo_id: RepoId) -> Result<u64, AppError> {
        let mut repositories = self
            .repositories
            .lock()
            .expect("repository registry lock poisoned");
        let repository = repositories
            .get_mut(&repo_id)
            .ok_or(AppError::RepositoryNotOpen)?;
        repository.revision += 1;
        Ok(repository.revision)
    }

    fn mutate(
        &self,
        repo_id: &str,
        revision: u64,
        operation: impl FnOnce(&RepositoryService, &RepositoryDescriptor) -> Result<(), AppError>,
    ) -> Result<RepositorySnapshot, AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.scheduler.write(repo_id, || {
            let descriptor = {
                let repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                ensure_revision(revision, repository.revision)?;
                repository.descriptor.clone()
            };
            operation(&self.service, &descriptor)?;
            let next_revision = self.advance_revision(repo_id)?;
            self.service.snapshot(&descriptor, next_revision)
        })
    }

    fn mutate_head(
        &self,
        repo_id: &str,
        revision: u64,
        operation: impl FnOnce(&RepositoryService, &RepositoryDescriptor) -> Result<(), AppError>,
    ) -> Result<RepositorySnapshot, AppError> {
        self.mutate(repo_id, revision, |service, repository| {
            service.with_submodule_update(repository, false, |_| operation(service, repository))
        })
    }

    fn descriptor(&self, repo_id: RepoId) -> Result<RepositoryDescriptor, AppError> {
        self.repositories
            .lock()
            .expect("repository registry lock poisoned")
            .get(&repo_id)
            .ok_or(AppError::RepositoryNotOpen)
            .map(|repository| repository.descriptor.clone())
    }

    pub fn begin_remote_operation(&self, repo_id: &str) -> Result<(Uuid, RepoId), AppError> {
        let repo_id = parse_repo_id(repo_id)?;
        self.descriptor(repo_id)?;
        let operation_id = Uuid::new_v4();
        self.operations
            .lock()
            .expect("operation registry lock poisoned")
            .insert(operation_id, CancellationToken::default());
        Ok((operation_id, repo_id))
    }

    pub fn begin_clone_operation(&self) -> Uuid {
        let operation_id = Uuid::new_v4();
        self.operations
            .lock()
            .expect("operation registry lock poisoned")
            .insert(operation_id, CancellationToken::default());
        operation_id
    }

    pub fn run_remote_operation(
        &self,
        operation_id: Uuid,
        repo_id: RepoId,
        request: &RemoteRequest,
        progress: impl FnMut(RemoteProgress),
    ) -> Result<RepositorySnapshot, AppError> {
        let cancellation = self.operation_token(operation_id)?;
        let result = self.scheduler.write(repo_id, || {
            let descriptor = self.descriptor(repo_id)?;
            self.service
                .remote_sync(&descriptor, request, &cancellation, progress)?;
            let next_revision = {
                let mut repositories = self
                    .repositories
                    .lock()
                    .expect("repository registry lock poisoned");
                let repository = repositories
                    .get_mut(&repo_id)
                    .ok_or(AppError::RepositoryNotOpen)?;
                repository.revision += 1;
                repository.revision
            };
            self.service.snapshot(&descriptor, next_revision)
        });
        self.finish_operation(operation_id);
        result
    }

    pub fn run_clone_operation(
        &self,
        operation_id: Uuid,
        request: &CloneRequest,
        progress: impl FnMut(RemoteProgress),
    ) -> Result<(), AppError> {
        let cancellation = self.operation_token(operation_id)?;
        let result = self
            .service
            .clone_repository(request, &cancellation, progress);
        self.finish_operation(operation_id);
        result
    }

    pub fn cancel_operation(&self, operation_id: &str) -> Result<(), AppError> {
        let operation_id = Uuid::parse_str(operation_id)
            .map_err(|_| AppError::InvalidRequest("Operation ID is invalid".to_owned()))?;
        let operations = self
            .operations
            .lock()
            .expect("operation registry lock poisoned");
        operations
            .get(&operation_id)
            .ok_or_else(|| AppError::InvalidRequest("Operation is no longer running".to_owned()))?
            .cancel();
        Ok(())
    }

    fn operation_token(&self, operation_id: Uuid) -> Result<CancellationToken, AppError> {
        self.operations
            .lock()
            .expect("operation registry lock poisoned")
            .get(&operation_id)
            .cloned()
            .ok_or_else(|| AppError::InvalidRequest("Operation is no longer running".to_owned()))
    }

    fn finish_operation(&self, operation_id: Uuid) {
        self.operations
            .lock()
            .expect("operation registry lock poisoned")
            .remove(&operation_id);
    }

    async fn session_tabs(
        &self,
        known_snapshot: Option<(RepoId, RepositorySnapshot)>,
        defer_inactive_snapshots: bool,
    ) -> Result<Vec<SessionTabState>, AppError> {
        let tabs = self.load_stored_tabs().await?;
        let mut result = Vec::with_capacity(tabs.len());
        for tab in tabs {
            let repo_id = parse_repo_id(&tab.repo_id)?;
            let registered = self
                .repositories
                .lock()
                .expect("repository registry lock poisoned")
                .contains_key(&repo_id);
            let loading = defer_inactive_snapshots && !tab.active && registered;
            let snapshot = if loading {
                None
            } else if known_snapshot
                .as_ref()
                .is_some_and(|(known_id, _)| *known_id == repo_id)
            {
                known_snapshot
                    .as_ref()
                    .map(|(_, snapshot)| snapshot.clone())
            } else {
                self.repository_snapshot(&tab.repo_id).ok()
            };
            result.push(SessionTabState {
                unavailable: !loading && snapshot.is_none(),
                loading,
                snapshot,
                stored: tab,
            });
        }
        Ok(result)
    }

    fn install_watcher(
        &self,
        repository: &RepositoryDescriptor,
        app: &AppHandle,
    ) -> Result<(), AppError> {
        let repo_id = repository.id;
        let worktree_id = repository.worktree_id;
        if self
            .watchers
            .lock()
            .expect("watcher registry lock poisoned")
            .contains_key(&worktree_id)
        {
            return Ok(());
        }
        let payload = repo_id.to_string();
        let app = app.clone();
        let (change_sender, change_receiver) = mpsc::channel();
        thread::spawn(move || {
            while change_receiver.recv().is_ok() {
                let burst_started = Instant::now();
                loop {
                    let remaining = Duration::from_secs(1).saturating_sub(burst_started.elapsed());
                    if remaining.is_zero() {
                        break;
                    }
                    let quiet_period = remaining.min(Duration::from_millis(250));
                    match change_receiver.recv_timeout(quiet_period) {
                        Ok(()) => {}
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                let _ = app.emit("repository-changed", &payload);
            }
        });
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if matches!(
                    event,
                    Ok(ref event) if !matches!(event.kind, notify::EventKind::Access(_))
                ) {
                    let _ = change_sender.send(());
                }
            })
            .map_err(watcher_error)?;
        watcher
            .watch(&repository.worktree_path, RecursiveMode::Recursive)
            .map_err(watcher_error)?;
        self.watchers
            .lock()
            .expect("watcher registry lock poisoned")
            .insert(worktree_id, watcher);
        Ok(())
    }
}

fn parse_repo_id(repo_id: &str) -> Result<RepoId, AppError> {
    RepoId::from_str(repo_id).map_err(|_| AppError::RepositoryNotOpen)
}

fn worktree_path_key(path: &Path) -> String {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let key = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        key.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        key
    }
}

fn ensure_revision(expected: u64, actual: u64) -> Result<(), AppError> {
    if expected == actual {
        Ok(())
    } else {
        Err(AppError::StaleRevision { expected, actual })
    }
}

fn persistence_error(error: sqlx::Error) -> AppError {
    AppError::Persistence {
        detail: error.to_string(),
    }
}

fn watcher_error(error: notify::Error) -> AppError {
    AppError::GitFailed {
        diagnostic_id: Uuid::new_v4(),
        detail: format!("Could not watch repository: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use app_core::AppError;

    use super::ensure_revision;

    #[test]
    fn rejects_a_stale_write_revision() {
        let error = ensure_revision(4, 5).expect_err("stale revision");
        assert!(matches!(
            error,
            AppError::StaleRevision {
                expected: 4,
                actual: 5
            }
        ));
    }
}
