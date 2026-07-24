//! Application use cases and errors shared by every UI adapter.

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Repository could not be found")]
    RepositoryNotFound,
    #[error(
        "The request used repository revision {expected}, but the current revision is {actual}"
    )]
    StaleRevision { expected: u64, actual: u64 },
    #[error("Git operation failed (diagnostic {diagnostic_id})")]
    GitFailed { diagnostic_id: Uuid },
    #[error("Operation was cancelled")]
    Cancelled,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub schema_version: u16,
    pub code: &'static str,
    pub message: String,
    pub recovery_actions: Vec<&'static str>,
}

impl From<&AppError> for AppErrorDto {
    fn from(error: &AppError) -> Self {
        let (code, recovery_actions) = match error {
            AppError::RepositoryNotFound => ("repositoryNotFound", vec!["chooseRepository"]),
            AppError::StaleRevision { .. } => ("staleRevision", vec!["refresh"]),
            AppError::GitFailed { .. } => ("gitFailed", vec!["retry", "copyDiagnostics"]),
            AppError::Cancelled => ("cancelled", Vec::new()),
        };

        Self {
            schema_version: 1,
            code,
            message: error.to_string(),
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
}
