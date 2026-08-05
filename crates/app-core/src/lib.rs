//! Application use cases and errors shared by every UI adapter.

mod remote;
mod repository;
mod scheduler;
mod workspace;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

pub use git_domain::{FileBlame, PathHistory};
pub use remote::{CloneRequest, RemoteOperationKind, RemoteProgress, RemoteRequest};
pub use repository::{
    BinaryPreview, BranchRequest, CommitFile, CommitRequest, ComparePatch, DiffTarget,
    ExternalDiffResult, ExternalDiffTool, GitIdentity, GitIdentitySettings, GitReference,
    GitRemote, GitVersion, HistoryFilter, HistoryOperation, InteractiveRebaseAction,
    InteractiveRebaseCommit, InteractiveRebaseItem, InteractiveRebasePreview,
    InteractiveRebaseRequest, PatchSelection, RefSummary, ReferenceKind, ReflogEntry,
    RemoteTagSummary, RepositoryService, RepositorySidebar, StashSummary, WorktreeCreateRequest,
    WorktreeSummary,
};
pub use scheduler::RepositoryScheduler;
pub use workspace::{ConflictResolution, StashRequest};
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Git is not installed or is not available on PATH")]
    GitNotFound,
    #[error("Git {found} is not supported; Git {minimum} or newer is required")]
    UnsupportedGitVersion { found: String, minimum: String },
    #[error("The selected path does not exist or cannot be accessed")]
    InvalidPath,
    #[error("The selected folder is not inside a Git working tree")]
    RepositoryNotFound,
    #[error("Repository is not open in this session")]
    RepositoryNotOpen,
    #[error("Worktree is not available in this repository")]
    WorktreeNotFound,
    #[error(
        "The request used repository revision {expected}, but the current revision is {actual}"
    )]
    StaleRevision { expected: u64, actual: u64 },
    #[error("Git operation timed out")]
    TimedOut,
    #[error("Git operation failed (diagnostic {diagnostic_id})")]
    GitFailed { diagnostic_id: Uuid, detail: String },
    #[error("Git output could not be parsed: {0}")]
    InvalidGitOutput(String),
    #[error("Operation was cancelled")]
    Cancelled,
    #[error("The remote could not be reached")]
    Offline,
    #[error("Authentication failed; check your credential helper or SSH agent")]
    AuthenticationFailed,
    #[error("Push was rejected because the remote contains newer commits")]
    NonFastForward,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("Application session could not be saved")]
    Persistence { detail: String },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub schema_version: u16,
    pub code: &'static str,
    pub message: String,
    pub details: Option<String>,
    pub recovery_actions: Vec<&'static str>,
}

impl From<&AppError> for AppErrorDto {
    fn from(error: &AppError) -> Self {
        let (code, recovery_actions) = match error {
            AppError::GitNotFound => ("gitNotFound", vec!["installGit", "retry"]),
            AppError::UnsupportedGitVersion { .. } => ("unsupportedGitVersion", vec!["updateGit"]),
            AppError::InvalidPath | AppError::RepositoryNotFound => {
                ("repositoryNotFound", vec!["chooseRepository"])
            }
            AppError::RepositoryNotOpen => ("repositoryNotOpen", vec!["chooseRepository"]),
            AppError::WorktreeNotFound => ("worktreeNotFound", vec!["refresh"]),
            AppError::StaleRevision { .. } => ("staleRevision", vec!["refresh"]),
            AppError::TimedOut => ("timedOut", vec!["retry"]),
            AppError::GitFailed { .. } => ("gitFailed", vec!["retry", "copyDiagnostics"]),
            AppError::InvalidGitOutput(_) => ("invalidGitOutput", vec!["retry", "copyDiagnostics"]),
            AppError::Cancelled => ("cancelled", Vec::new()),
            AppError::Offline => ("offline", vec!["retry"]),
            AppError::AuthenticationFailed => {
                ("authenticationFailed", vec!["checkCredentials", "retry"])
            }
            AppError::NonFastForward => ("nonFastForward", vec!["fetch", "pull", "retry"]),
            AppError::InvalidRequest(_) => ("invalidRequest", vec!["editRequest", "refresh"]),
            AppError::Persistence { .. } => ("persistenceFailed", vec!["retry"]),
        };
        let details = match error {
            AppError::GitFailed {
                diagnostic_id,
                detail,
            } => Some(format!("{diagnostic_id}: {detail}")),
            AppError::InvalidGitOutput(detail) => Some(detail.clone()),
            AppError::Persistence { detail } => Some(detail.clone()),
            _ => None,
        };

        Self {
            schema_version: 1,
            code,
            message: error.to_string(),
            details,
            recovery_actions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, AppErrorDto};

    #[test]
    fn stale_revision_has_a_refresh_recovery_action() {
        let dto = AppErrorDto::from(&AppError::StaleRevision {
            expected: 4,
            actual: 5,
        });

        assert_eq!(dto.code, "staleRevision");
        assert_eq!(dto.recovery_actions, vec!["refresh"]);
    }

    #[test]
    fn missing_repository_can_reopen_picker() {
        let dto = AppErrorDto::from(&AppError::RepositoryNotFound);
        assert_eq!(dto.recovery_actions, vec!["chooseRepository"]);
    }
}
