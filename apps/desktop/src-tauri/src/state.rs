use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

use app_core::{AppError, RepositoryScheduler, RepositoryService};
use git_domain::{RepoId, RepositoryDescriptor, RepositorySnapshot};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

struct OpenRepository {
    descriptor: RepositoryDescriptor,
    revision: u64,
}

#[derive(Default)]
pub struct ApplicationState {
    service: RepositoryService,
    scheduler: RepositoryScheduler,
    repositories: Mutex<HashMap<RepoId, OpenRepository>>,
    watchers: Mutex<HashMap<RepoId, RecommendedWatcher>>,
}

impl ApplicationState {
    pub fn open_repository(
        &self,
        path: &Path,
        app: &AppHandle,
    ) -> Result<RepositorySnapshot, AppError> {
        let descriptor = self.service.discover(path)?;
        let snapshot = self
            .scheduler
            .read(descriptor.id, || self.service.snapshot(&descriptor, 1))?;
        self.install_watcher(&descriptor, app)?;
        self.repositories
            .lock()
            .expect("repository registry lock poisoned")
            .insert(
                descriptor.id,
                OpenRepository {
                    descriptor,
                    revision: 1,
                },
            );
        Ok(snapshot)
    }

    pub fn repository_snapshot(&self, repo_id: &str) -> Result<RepositorySnapshot, AppError> {
        let repo_id = RepoId::from_str(repo_id).map_err(|_| AppError::RepositoryNotOpen)?;
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

    fn install_watcher(
        &self,
        repository: &RepositoryDescriptor,
        app: &AppHandle,
    ) -> Result<(), AppError> {
        let repo_id = repository.id;
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
            .insert(repo_id, watcher);
        Ok(())
    }
}

fn watcher_error(error: notify::Error) -> AppError {
    AppError::GitFailed {
        diagnostic_id: Uuid::new_v4(),
        detail: format!("Could not watch repository: {error}"),
    }
}
