//! Pure Git domain types. Tauri and process execution do not belong in this crate.

mod status;

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use status::{FileChange, HeadState, StatusParseError, StatusSnapshot, parse_porcelain_v2};

const REPOSITORY_NAMESPACE: Uuid = Uuid::from_u128(0xa6a5_6f5f_d466_45de_a2af_82cd_987d_1552);
const WORKTREE_NAMESPACE: Uuid = Uuid::from_u128(0xd33a_1ae7_e5bc_4a6d_9e7c_4208_62ca_63d1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(Uuid);

impl RepoId {
    pub fn from_canonical_path(path: &Path) -> Self {
        let normalized = path.to_string_lossy().replace('\\', "/");
        #[cfg(windows)]
        let normalized = normalized.to_lowercase();
        Self(Uuid::new_v5(&REPOSITORY_NAMESPACE, normalized.as_bytes()))
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RepoId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorktreeId(Uuid);

impl WorktreeId {
    pub fn from_canonical_path(path: &Path) -> Self {
        let normalized = normalized_path(path);
        Self(Uuid::new_v5(&WORKTREE_NAMESPACE, normalized.as_bytes()))
    }
}

impl fmt::Display for WorktreeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for WorktreeId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

fn normalized_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let normalized = normalized.to_lowercase();
    normalized
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDescriptor {
    pub id: RepoId,
    pub worktree_id: WorktreeId,
    pub name: String,
    pub worktree_path: PathBuf,
    pub git_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    pub revision: u64,
    pub repository: RepositoryDescriptor,
    pub status: StatusSnapshot,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{RepoId, WorktreeId};

    #[test]
    #[cfg(windows)]
    fn repository_id_is_stable_for_equivalent_path_spelling() {
        let first = RepoId::from_canonical_path(Path::new("C:\\Code\\Acorn"));
        let second = RepoId::from_canonical_path(Path::new("c:/code/acorn"));

        assert_eq!(first, second);
    }

    #[test]
    fn worktree_id_is_stable_and_distinct_from_repository_id() {
        let path = Path::new("C:/Code/Acorn");
        assert_eq!(
            WorktreeId::from_canonical_path(path),
            WorktreeId::from_canonical_path(path)
        );
        assert_ne!(
            RepoId::from_canonical_path(path).to_string(),
            WorktreeId::from_canonical_path(path).to_string()
        );
    }
}
