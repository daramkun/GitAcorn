use app_core::{
    AppErrorDto, BranchRequest, CloneRequest, CommitFile, CommitRequest, GitIdentity, GitReference,
    GitRemote, InteractiveRebaseAction, InteractiveRebaseItem, InteractiveRebasePreview,
    InteractiveRebaseRequest, PatchSelection, ReferenceKind, ReflogEntry, RemoteOperationKind,
    RemoteRequest, RemoteTagSummary, RepositorySidebar, WorktreeCreateRequest,
};
use git_domain::{
    BlameLine, CommitSummary, DiffDocument, DiffLineKind, FileBlame, FileChange, HeadState,
    HistoryPage, PathHistory, PathHistoryEntry, RepositoryOperation, RepositorySnapshot,
};
use persistence::OperationRecord;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::path_display::display_path;
use crate::state::{RepositoryIdentityState, SessionTabState};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentityUpdateDto {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentityDto {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl From<GitIdentity> for GitIdentityDto {
    fn from(identity: GitIdentity) -> Self {
        Self {
            name: identity.name,
            email: identity.email,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGitIdentityDto {
    pub repo_id: String,
    pub repository_name: String,
    pub local: GitIdentityDto,
    pub effective: GitIdentityDto,
}

impl From<RepositoryIdentityState> for RepositoryGitIdentityDto {
    fn from(state: RepositoryIdentityState) -> Self {
        Self {
            repo_id: state.repo_id,
            repository_name: state.repository_name,
            local: state.settings.local.into(),
            effective: state.settings.effective.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentitySettingsDto {
    pub schema_version: u16,
    pub global: GitIdentityDto,
    pub repository: Option<RepositoryGitIdentityDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRequestDto {
    pub kind: String,
    pub remote: Option<String>,
    #[serde(default)]
    pub fetch_tags: bool,
    #[serde(default)]
    pub auto_stash: bool,
    pub fast_forward_only: Option<bool>,
    #[serde(default)]
    pub force_with_lease: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMutationRequestDto {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleAddRequestDto {
    pub url: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReferenceDeleteDto {
    pub remote: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoteDto {
    pub name: String,
    pub url: String,
}

impl From<GitRemote> for GitRemoteDto {
    fn from(remote: GitRemote) -> Self {
        Self {
            name: remote.name,
            url: remote.url,
        }
    }
}

impl TryFrom<RemoteRequestDto> for RemoteRequest {
    type Error = app_core::AppError;

    fn try_from(request: RemoteRequestDto) -> Result<Self, Self::Error> {
        let kind = match request.kind.as_str() {
            "fetch" => RemoteOperationKind::Fetch,
            "pull" => RemoteOperationKind::Pull,
            "push" => RemoteOperationKind::Push,
            _ => {
                return Err(app_core::AppError::InvalidRequest(
                    "Remote operation must be fetch, pull, or push".to_owned(),
                ));
            }
        };
        Ok(Self {
            kind,
            remote: request.remote,
            fetch_tags: request.fetch_tags,
            auto_stash: request.auto_stash,
            fast_forward_only: request
                .fast_forward_only
                .unwrap_or(kind == RemoteOperationKind::Pull),
            force_with_lease: request.force_with_lease,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequestDto {
    pub remote_url: String,
    pub destination: String,
}

impl From<CloneRequestDto> for CloneRequest {
    fn from(request: CloneRequestDto) -> Self {
        Self {
            remote_url: request.remote_url,
            destination: request.destination.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStartedDto {
    pub schema_version: u16,
    pub operation_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationEventDto {
    pub schema_version: u16,
    pub operation_id: String,
    pub repo_id: Option<String>,
    pub kind: String,
    pub state: &'static str,
    pub message: Option<String>,
    pub stream: Option<&'static str>,
    pub snapshot: Option<RepositorySnapshotDto>,
    pub destination: Option<String>,
    pub error: Option<AppErrorDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashRequestDto {
    pub message: String,
    #[serde(default)]
    pub include_untracked: bool,
    #[serde(default)]
    pub paths: Vec<Vec<u8>>,
}

impl From<StashRequestDto> for app_core::StashRequest {
    fn from(request: StashRequestDto) -> Self {
        Self {
            message: request.message,
            include_untracked: request.include_untracked,
            paths: request.paths,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecordDto {
    pub schema_version: u16,
    pub id: String,
    pub repo_id: Option<String>,
    pub kind: String,
    pub state: String,
    pub summary: String,
    pub diagnostic: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub recovery_action: Option<String>,
    pub recovery_state: Option<String>,
}

impl From<OperationRecord> for OperationRecordDto {
    fn from(record: OperationRecord) -> Self {
        Self {
            schema_version: 1,
            id: record.id,
            repo_id: record.repo_id,
            kind: record.kind,
            state: record.state,
            summary: record.summary,
            diagnostic: record.diagnostic,
            started_at: record.started_at,
            finished_at: record.finished_at,
            recovery_action: record.recovery_action,
            recovery_state: record.recovery_state,
        }
    }
}

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
    pub operation: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveRebaseRequestDto {
    pub base_oid: String,
    pub expected_head_oid: String,
    pub items: Vec<InteractiveRebaseItemDto>,
    #[serde(default)]
    pub auto_stash: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveRebaseItemDto {
    pub oid: String,
    pub action: String,
    pub summary: Option<String>,
    pub description: Option<String>,
}

impl TryFrom<InteractiveRebaseRequestDto> for InteractiveRebaseRequest {
    type Error = app_core::AppError;

    fn try_from(request: InteractiveRebaseRequestDto) -> Result<Self, Self::Error> {
        let items = request
            .items
            .into_iter()
            .map(|item| {
                let action = match item.action.as_str() {
                    "pick" => InteractiveRebaseAction::Pick,
                    "reword" => InteractiveRebaseAction::Reword,
                    "edit" => InteractiveRebaseAction::Edit,
                    "squash" => InteractiveRebaseAction::Squash,
                    "fixup" => InteractiveRebaseAction::Fixup,
                    "drop" => InteractiveRebaseAction::Drop,
                    _ => {
                        return Err(app_core::AppError::InvalidRequest(
                            "Rebase action must be pick, reword, edit, squash, fixup, or drop"
                                .to_owned(),
                        ));
                    }
                };
                Ok(InteractiveRebaseItem {
                    oid: item.oid,
                    action,
                    summary: item.summary,
                    description: item.description,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            base_oid: request.base_oid,
            expected_head_oid: request.expected_head_oid,
            items,
            auto_stash: request.auto_stash,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveRebasePreviewDto {
    pub schema_version: u16,
    pub base_oid: String,
    pub head_oid: String,
    pub branch: String,
    pub commits: Vec<InteractiveRebaseCommitDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveRebaseCommitDto {
    pub oid: String,
    pub subject: String,
}

impl From<InteractiveRebasePreview> for InteractiveRebasePreviewDto {
    fn from(preview: InteractiveRebasePreview) -> Self {
        Self {
            schema_version: 1,
            base_oid: preview.base_oid,
            head_oid: preview.head_oid,
            branch: preview.branch,
            commits: preview
                .commits
                .into_iter()
                .map(|commit| InteractiveRebaseCommitDto {
                    oid: commit.oid,
                    subject: commit.subject,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub schema_version: u16,
    pub tabs: Vec<SessionTabDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOpenSourceDto {
    pub repository_name: String,
    pub worktree_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedFromRepositoryDto {
    pub repository_name: String,
    pub worktree_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTabDto {
    pub repo_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub opened_from: Option<OpenedFromRepositoryDto>,
    pub active: bool,
    pub page: String,
    pub selected_path: Option<String>,
    pub selected_diff: String,
    pub panel_width: f64,
    pub history_cursor: Option<String>,
    pub selected_commit: Option<String>,
    pub history_filter: Option<String>,
    pub unavailable: bool,
    pub loading: bool,
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
                worktree_path: display_path(&snapshot.repository.worktree_path),
                git_dir: display_path(&snapshot.repository.git_dir),
            },
            head,
            upstream: snapshot.status.upstream,
            ahead: snapshot.status.ahead,
            behind: snapshot.status.behind,
            stash_count: snapshot.status.stash_count,
            operation: snapshot.operation.map(|operation| match operation {
                RepositoryOperation::Rebase => "rebase",
                RepositoryOperation::RebaseEdit => "rebaseEdit",
                RepositoryOperation::AutostashConflict => "autostashConflict",
                RepositoryOperation::CherryPick => "cherryPick",
                RepositoryOperation::Revert => "revert",
            }),
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
pub struct CommitFileDto {
    pub path: String,
    pub path_bytes: Vec<u8>,
}

impl From<CommitFile> for CommitFileDto {
    fn from(file: CommitFile) -> Self {
        Self {
            path: String::from_utf8_lossy(&file.path).into_owned(),
            path_bytes: file.path,
        }
    }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRequestDto {
    pub name: String,
    pub start_point: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTabUpdateDto {
    pub page: String,
    pub selected_path: Option<String>,
    pub selected_diff: String,
    pub panel_width: f64,
    pub history_cursor: Option<String>,
    pub selected_commit: Option<String>,
    pub history_filter: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageDto {
    pub schema_version: u16,
    pub commits: Vec<CommitDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDto {
    pub oid: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub subject: String,
    pub body: String,
    pub references: Vec<String>,
    pub remote_only: bool,
    pub lane: usize,
    pub lane_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceDto {
    pub full_name: String,
    pub short_name: String,
    pub oid: String,
    pub kind: &'static str,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTagDto {
    pub remote: String,
    pub name: String,
    pub oid: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntryDto {
    pub schema_version: u16,
    pub selector: String,
    pub oid: String,
    pub message: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub subject: String,
    pub body: String,
    pub reflog_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySidebarDto {
    pub schema_version: u16,
    pub worktrees: Vec<WorktreeDto>,
    pub submodules: Vec<SubmoduleDto>,
    pub branches: RefSummaryDto,
    pub remote_branches: RefSummaryDto,
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
    pub is_prunable: bool,
    pub is_missing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateRequestDto {
    pub path: String,
    pub branch: Option<String>,
    pub start_point: Option<String>,
}

impl TryFrom<WorktreeCreateRequestDto> for WorktreeCreateRequest {
    type Error = AppErrorDto;

    fn try_from(request: WorktreeCreateRequestDto) -> Result<Self, Self::Error> {
        let path = std::path::PathBuf::from(request.path.trim());
        if path.as_os_str().is_empty() {
            return Err(AppErrorDto::from(&app_core::AppError::InvalidRequest(
                "Worktree path is required".to_owned(),
            )));
        }
        Ok(Self {
            path,
            branch: request
                .branch
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            start_point: request
                .start_point
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleDto {
    pub path: String,
    pub absolute_path: String,
    pub initialized: bool,
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
                    opened_from: tab
                        .stored
                        .opened_from_repository_name
                        .zip(tab.stored.opened_from_worktree_path)
                        .map(|(repository_name, worktree_path)| OpenedFromRepositoryDto {
                            repository_name,
                            worktree_path: display_path(Path::new(&worktree_path)),
                        }),
                    repo_id: tab.stored.repo_id,
                    worktree_id: tab.stored.worktree_id,
                    worktree_path: display_path(Path::new(&tab.stored.worktree_path)),
                    active: tab.stored.active,
                    page: tab.stored.page,
                    selected_path: tab.stored.selected_path,
                    selected_diff: tab.stored.selected_diff,
                    panel_width: tab.stored.panel_width,
                    history_cursor: tab.stored.history_cursor,
                    selected_commit: tab.stored.selected_commit,
                    history_filter: tab.stored.history_filter,
                    unavailable: tab.unavailable,
                    loading: tab.loading,
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
                    path: display_path(Path::new(&worktree.path)),
                    head: worktree.head,
                    branch: worktree.branch,
                    is_current: worktree.is_current,
                    is_locked: worktree.is_locked,
                    is_prunable: worktree.is_prunable,
                    is_missing: worktree.is_missing,
                })
                .collect(),
            submodules: sidebar
                .submodules
                .into_iter()
                .map(|submodule| SubmoduleDto {
                    path: submodule.path,
                    absolute_path: display_path(Path::new(&submodule.absolute_path)),
                    initialized: submodule.initialized,
                })
                .collect(),
            branches: RefSummaryDto {
                total: sidebar.branches.total,
                items: sidebar.branches.items,
            },
            remote_branches: RefSummaryDto {
                total: sidebar.remote_branches.total,
                items: sidebar.remote_branches.items,
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

impl From<BranchRequestDto> for BranchRequest {
    fn from(request: BranchRequestDto) -> Self {
        Self {
            name: request.name,
            start_point: request.start_point,
        }
    }
}

impl From<HistoryPage> for HistoryPageDto {
    fn from(page: HistoryPage) -> Self {
        Self {
            schema_version: 1,
            commits: page.commits.into_iter().map(CommitDto::from).collect(),
            next_cursor: page.next_cursor,
        }
    }
}

impl From<CommitSummary> for CommitDto {
    fn from(commit: CommitSummary) -> Self {
        Self {
            oid: commit.oid,
            parents: commit.parents,
            author_name: commit.author_name,
            author_email: commit.author_email,
            authored_at: commit.authored_at,
            subject: commit.subject,
            body: commit.body,
            references: commit.references,
            remote_only: commit.remote_only,
            lane: commit.lane,
            lane_count: commit.lane_count,
        }
    }
}

impl From<GitReference> for ReferenceDto {
    fn from(reference: GitReference) -> Self {
        Self {
            full_name: reference.full_name,
            short_name: reference.short_name,
            oid: reference.oid,
            kind: match reference.kind {
                ReferenceKind::LocalBranch => "localBranch",
                ReferenceKind::RemoteBranch => "remoteBranch",
                ReferenceKind::Tag => "tag",
            },
            upstream: reference.upstream,
            ahead: reference.ahead,
            behind: reference.behind,
        }
    }
}

impl From<RemoteTagSummary> for RemoteTagDto {
    fn from(tag: RemoteTagSummary) -> Self {
        Self {
            remote: tag.remote,
            name: tag.name,
            oid: tag.oid,
        }
    }
}

impl From<ReflogEntry> for ReflogEntryDto {
    fn from(entry: ReflogEntry) -> Self {
        Self {
            schema_version: 1,
            selector: entry.selector,
            oid: entry.oid,
            message: entry.message,
            parents: entry.parents,
            author_name: entry.author_name,
            author_email: entry.author_email,
            authored_at: entry.authored_at,
            subject: entry.subject,
            body: entry.body,
            reflog_only: entry.reflog_only,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLineDto {
    pub line: usize,
    pub commit_oid: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBlameDto {
    pub schema_version: u16,
    pub path: Vec<u8>,
    pub revision: Option<String>,
    pub lines: Vec<BlameLineDto>,
}

impl From<BlameLine> for BlameLineDto {
    fn from(line: BlameLine) -> Self {
        Self {
            line: line.line,
            commit_oid: line.commit_oid,
            author_name: line.author_name,
            author_email: line.author_email,
            authored_at: line.authored_at,
            content: line.content,
        }
    }
}

impl From<FileBlame> for FileBlameDto {
    fn from(blame: FileBlame) -> Self {
        Self {
            schema_version: 1,
            path: blame.path,
            revision: blame.revision,
            lines: blame.lines.into_iter().map(BlameLineDto::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHistoryEntryDto {
    pub oid: String,
    pub parent_oid: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub subject: String,
    pub path: Vec<u8>,
    pub previous_path: Option<Vec<u8>>,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHistoryDto {
    pub schema_version: u16,
    pub path: Vec<u8>,
    pub is_directory: bool,
    pub entries: Vec<PathHistoryEntryDto>,
    pub next_cursor: Option<String>,
}

impl From<PathHistoryEntry> for PathHistoryEntryDto {
    fn from(entry: PathHistoryEntry) -> Self {
        Self {
            oid: entry.oid,
            parent_oid: entry.parent_oid,
            author_name: entry.author_name,
            author_email: entry.author_email,
            authored_at: entry.authored_at,
            subject: entry.subject,
            path: entry.path,
            previous_path: entry.previous_path,
            status: entry.status,
        }
    }
}

impl From<PathHistory> for PathHistoryDto {
    fn from(history: PathHistory) -> Self {
        Self {
            schema_version: 1,
            path: history.path,
            is_directory: history.is_directory,
            entries: history
                .entries
                .into_iter()
                .map(PathHistoryEntryDto::from)
                .collect(),
            next_cursor: history.next_cursor,
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
