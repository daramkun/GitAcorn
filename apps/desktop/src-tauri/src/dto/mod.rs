use app_core::{AppErrorDto, CommitRequest, PatchSelection, RepositorySidebar};
use git_domain::{DiffDocument, DiffLineKind, FileChange, HeadState, RepositorySnapshot};
use serde::{Deserialize, Serialize};

use crate::state::SessionTabState;

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
pub struct SessionDto {
    pub schema_version: u16,
    pub tabs: Vec<SessionTabDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTabDto {
    pub repo_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub active: bool,
    pub page: String,
    pub selected_path: Option<String>,
    pub selected_diff: String,
    pub panel_width: f64,
    pub unavailable: bool,
    pub snapshot: Option<RepositorySnapshotDto>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffDto {
    pub schema_version: u16,
    pub binary: bool,
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<DiffHunkDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunkDto {
    pub index: usize,
    pub header: String,
    pub old_start: u64,
    pub old_count: u64,
    pub new_start: u64,
    pub new_count: u64,
    pub lines: Vec<DiffLineDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLineDto {
    pub index: usize,
    pub kind: &'static str,
    pub old_line: Option<u64>,
    pub new_line: Option<u64>,
    pub content: String,
    pub selectable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSelectionDto {
    pub hunk_index: usize,
    #[serde(default)]
    pub line_indices: Vec<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequestDto {
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub amend: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySidebarDto {
    pub schema_version: u16,
    pub worktrees: Vec<WorktreeDto>,
    pub branches: RefSummaryDto,
    pub tags: RefSummaryDto,
    pub stashes: Vec<StashDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeDto {
    pub id: String,
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_current: bool,
    pub is_locked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefSummaryDto {
    pub total: usize,
    pub items: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashDto {
    pub reference: String,
    pub message: String,
}

impl From<Vec<SessionTabState>> for SessionDto {
    fn from(tabs: Vec<SessionTabState>) -> Self {
        Self {
            schema_version: 1,
            tabs: tabs
                .into_iter()
                .map(|tab| SessionTabDto {
                    repo_id: tab.stored.repo_id,
                    worktree_id: tab.stored.worktree_id,
                    worktree_path: tab.stored.worktree_path,
                    active: tab.stored.active,
                    page: tab.stored.page,
                    selected_path: tab.stored.selected_path,
                    selected_diff: tab.stored.selected_diff,
                    panel_width: tab.stored.panel_width,
                    unavailable: tab.unavailable,
                    snapshot: tab.snapshot.map(RepositorySnapshotDto::from),
                })
                .collect(),
        }
    }
}

impl From<RepositorySidebar> for RepositorySidebarDto {
    fn from(sidebar: RepositorySidebar) -> Self {
        Self {
            schema_version: 1,
            worktrees: sidebar
                .worktrees
                .into_iter()
                .map(|worktree| WorktreeDto {
                    id: worktree.id.to_string(),
                    path: worktree.path,
                    head: worktree.head,
                    branch: worktree.branch,
                    is_current: worktree.is_current,
                    is_locked: worktree.is_locked,
                })
                .collect(),
            branches: RefSummaryDto {
                total: sidebar.branches.total,
                items: sidebar.branches.items,
            },
            tags: RefSummaryDto {
                total: sidebar.tags.total,
                items: sidebar.tags.items,
            },
            stashes: sidebar
                .stashes
                .into_iter()
                .map(|stash| StashDto {
                    reference: stash.reference,
                    message: stash.message,
                })
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

impl From<DiffDocument> for DiffDto {
    fn from(document: DiffDocument) -> Self {
        let file = document.files.into_iter().next();
        Self {
            schema_version: 1,
            binary: file.as_ref().is_some_and(|file| file.binary),
            old_path: file
                .as_ref()
                .map(|file| file.old_path.clone())
                .unwrap_or_default(),
            new_path: file
                .as_ref()
                .map(|file| file.new_path.clone())
                .unwrap_or_default(),
            hunks: file
                .map(|file| {
                    file.hunks
                        .into_iter()
                        .map(|hunk| DiffHunkDto {
                            index: hunk.index,
                            header: hunk.header,
                            old_start: hunk.old_start,
                            old_count: hunk.old_count,
                            new_start: hunk.new_start,
                            new_count: hunk.new_count,
                            lines: hunk
                                .lines
                                .into_iter()
                                .enumerate()
                                .map(|(index, line)| {
                                    let (kind, selectable) = match line.kind {
                                        DiffLineKind::Context => ("context", false),
                                        DiffLineKind::Addition => ("addition", true),
                                        DiffLineKind::Deletion => ("deletion", true),
                                        DiffLineKind::NoNewline => ("noNewline", false),
                                    };
                                    DiffLineDto {
                                        index,
                                        kind,
                                        old_line: line.old_line,
                                        new_line: line.new_line,
                                        content: line.content,
                                        selectable,
                                    }
                                })
                                .collect(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

impl From<PatchSelectionDto> for PatchSelection {
    fn from(selection: PatchSelectionDto) -> Self {
        Self {
            hunk_index: selection.hunk_index,
            line_indices: selection.line_indices,
        }
    }
}

impl From<CommitRequestDto> for CommitRequest {
    fn from(request: CommitRequestDto) -> Self {
        Self {
            summary: request.summary,
            description: request.description,
            amend: request.amend,
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
