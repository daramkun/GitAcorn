use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

use app_core::{AppError, RepositoryScheduler, RepositoryService, RepositorySidebar};
use git_domain::{RepoId, RepositoryDescriptor, RepositorySnapshot, WorktreeId};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use persistence::{SessionStore, SessionTab};
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
}

pub struct ApplicationState {
    service: RepositoryService,
    scheduler: RepositoryScheduler,
    repositories: Mutex<HashMap<RepoId, OpenRepository>>,
    watchers: Mutex<HashMap<WorktreeId, RecommendedWatcher>>,
    session: SessionStore,
}

impl ApplicationState {
    pub fn new(session: SessionStore) -> Self {
        Self {
            service: RepositoryService::default(),
            scheduler: RepositoryScheduler::default(),
            repositories: Mutex::default(),
            watchers: Mutex::default(),
            session,
        }
    }

    pub async fn open_repository(
        &self,
        path: &Path,
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
                tab_order: existing
                    .map(|tab| tab.tab_order)
                    .unwrap_or(tabs.len() as i64),
                active: true,
                page: existing
                    .map(|tab| tab.page.clone())
                    .unwrap_or_else(|| "changes".to_owned()),
                selected_path: existing.and_then(|tab| tab.selected_path.clone()),
                panel_width: existing.map(|tab| tab.panel_width).unwrap_or(280.0),
            })
            .await
            .map_err(persistence_error)?;
        self.session_tabs(Some((repo_id, snapshot))).await
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
        self.session_tabs(None).await
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
        self.session
            .upsert_tab(&tab)
            .await
            .map_err(persistence_error)?;
        self.session_tabs(Some((repo_id, snapshot))).await
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
        let tabs = self.load_stored_tabs().await?;
        if !tabs.is_empty() && !tabs.iter().any(|tab| tab.active) {
            let next = tabs.last().expect("non-empty tabs");
            self.session
                .activate(&next.repo_id)
                .await
                .map_err(persistence_error)?;
        }
        self.session_tabs(None).await
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
        page: String,
        selected_path: Option<String>,
        panel_width: f64,
    ) -> Result<(), AppError> {
        let tabs = self.load_stored_tabs().await?;
        let mut tab = tabs
            .into_iter()
            .find(|tab| tab.repo_id == repo_id)
            .ok_or(AppError::RepositoryNotOpen)?;
        tab.page = page;
        tab.selected_path = selected_path;
        tab.panel_width = panel_width;
        self.session
            .upsert_tab(&tab)
            .await
            .map_err(persistence_error)
    }

    async fn load_stored_tabs(&self) -> Result<Vec<SessionTab>, AppError> {
        self.session.load_tabs().await.map_err(persistence_error)
    }

    async fn session_tabs(
        &self,
        known_snapshot: Option<(RepoId, RepositorySnapshot)>,
    ) -> Result<Vec<SessionTabState>, AppError> {
        let tabs = self.load_stored_tabs().await?;
        let mut result = Vec::with_capacity(tabs.len());
        for tab in tabs {
            let repo_id = parse_repo_id(&tab.repo_id)?;
            let snapshot = if known_snapshot
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
                unavailable: snapshot.is_none(),
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
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok() {
                    let _ = app.emit("repository-changed", &payload);
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
