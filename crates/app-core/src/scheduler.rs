use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use git_domain::RepoId;

#[derive(Debug, Default)]
pub struct RepositoryScheduler {
    locks: Mutex<HashMap<RepoId, Arc<RwLock<()>>>>,
}

impl RepositoryScheduler {
    pub fn read<T>(&self, repo_id: RepoId, operation: impl FnOnce() -> T) -> T {
        let lock = self.lock_for(repo_id);
        let _read = lock.read().expect("repository read lock poisoned");
        operation()
    }

    pub fn write<T>(&self, repo_id: RepoId, operation: impl FnOnce() -> T) -> T {
        let lock = self.lock_for(repo_id);
        let _write = lock.write().expect("repository write lock poisoned");
        operation()
    }

    fn lock_for(&self, repo_id: RepoId) -> Arc<RwLock<()>> {
        self.locks
            .lock()
            .expect("scheduler registry lock poisoned")
            .entry(repo_id)
            .or_default()
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use git_domain::RepoId;

    use super::RepositoryScheduler;

    #[test]
    fn returns_operation_results() {
        let scheduler = RepositoryScheduler::default();
        let repo_id = RepoId::from_canonical_path(Path::new("C:/repo"));

        assert_eq!(scheduler.read(repo_id, || 41) + 1, 42);
        assert_eq!(scheduler.write(repo_id, || "written"), "written");
    }
}
