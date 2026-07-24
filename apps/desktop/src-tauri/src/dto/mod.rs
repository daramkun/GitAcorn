use app_core::AppErrorDto;
use git_domain::{FileChange, HeadState, RepositorySnapshot};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub schema_version: u16,
    pub name: &'static str,
    pub version: &'static str,
    pub runtime: &'static str,
}

impl AppInfoDto {
    pub const fn current() -> Self {
        Self {
            schema_version: 1,
            name: "GitAcorn",
            version: env!("CARGO_PKG_VERSION"),
            runtime: "Tauri 2",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySnapshotDto {
    pub schema_version: u16,
    pub revision: u64,
    pub repository: RepositoryDto,
    pub head: HeadDto,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub stash_count: u64,
    pub changes: Vec<FileChangeDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDto {
    pub id: String,
    pub name: String,
    pub worktree_path: String,
    pub git_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadDto {
    pub kind: &'static str,
    pub name: Option<String>,
    pub oid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeDto {
    pub path: String,
    pub path_bytes: Vec<u8>,
    pub original_path: Option<String>,
    pub index_status: String,
    pub worktree_status: String,
    pub conflict: bool,
    pub submodule: bool,
}

impl From<RepositorySnapshot> for RepositorySnapshotDto {
    fn from(snapshot: RepositorySnapshot) -> Self {
        let head = match snapshot.status.head {
            HeadState::Unborn => HeadDto {
                kind: "unborn",
                name: None,
                oid: None,
            },
            HeadState::Detached { oid } => HeadDto {
                kind: "detached",
                name: None,
                oid,
            },
            HeadState::Branch { name, oid } => HeadDto {
                kind: "branch",
                name: Some(name),
                oid,
            },
        };

        Self {
            schema_version: 1,
            revision: snapshot.revision,
            repository: RepositoryDto {
                id: snapshot.repository.id.to_string(),
                name: snapshot.repository.name,
                worktree_path: snapshot
                    .repository
                    .worktree_path
                    .to_string_lossy()
                    .into_owned(),
                git_dir: snapshot.repository.git_dir.to_string_lossy().into_owned(),
            },
            head,
            upstream: snapshot.status.upstream,
            ahead: snapshot.status.ahead,
            behind: snapshot.status.behind,
            stash_count: snapshot.status.stash_count,
            changes: snapshot
                .status
                .changes
                .into_iter()
                .map(FileChangeDto::from)
                .collect(),
        }
    }
}

impl From<FileChange> for FileChangeDto {
    fn from(change: FileChange) -> Self {
        Self {
            path: String::from_utf8_lossy(&change.path).into_owned(),
            path_bytes: change.path,
            original_path: change
                .original_path
                .map(|path| String::from_utf8_lossy(&path).into_owned()),
            index_status: char::from(change.index_status).to_string(),
            worktree_status: char::from(change.worktree_status).to_string(),
            conflict: change.is_conflict,
            submodule: change.is_submodule,
        }
    }
}

pub type CommandResult<T> = Result<T, AppErrorDto>;

#[cfg(test)]
mod tests {
    use super::AppInfoDto;

    #[test]
    fn app_info_contract_uses_versioned_camel_case_fields() {
        let value = serde_json::to_value(AppInfoDto::current()).expect("serializable DTO");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["name"], "GitAcorn");
        assert_eq!(value["runtime"], "Tauri 2");
        assert!(value.get("schema_version").is_none());
    }
}
