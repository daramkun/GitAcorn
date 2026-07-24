use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git_cli::{
    CancellationToken, GitExecutionError, GitExecutor, GitOutput, GitRequest, redact_remote,
};
use git_domain::RepositoryDescriptor;
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteOperationKind {
    Fetch,
    Pull,
    Push,
}

impl RemoteOperationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Pull => "pull",
            Self::Push => "push",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRequest {
    pub kind: RemoteOperationKind,
    pub force_with_lease: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneRequest {
    pub remote_url: String,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteProgress {
    pub stream: &'static str,
    pub message: String,
}

impl super::RepositoryService {
    pub fn remote_sync(
        &self,
        repository: &RepositoryDescriptor,
        request: &RemoteRequest,
        cancellation: &CancellationToken,
        progress: impl FnMut(RemoteProgress),
    ) -> Result<(), AppError> {
        if request.force_with_lease && request.kind != RemoteOperationKind::Push {
            return Err(AppError::InvalidRequest(
                "Force with lease is only valid for push".to_owned(),
            ));
        }
        let mut args = vec![OsString::from(request.kind.label())];
        if request.kind == RemoteOperationKind::Pull {
            args.push(OsString::from("--ff-only"));
        }
        if request.force_with_lease {
            args.push(OsString::from("--force-with-lease"));
        }
        let mut git_request = GitRequest::new(args);
        git_request.working_directory = Some(repository.worktree_path.clone());
        git_request.timeout = Duration::from_secs(15 * 60);
        run_remote(&self.executor, git_request, cancellation, progress)
    }

    pub fn clone_repository(
        &self,
        request: &CloneRequest,
        cancellation: &CancellationToken,
        progress: impl FnMut(RemoteProgress),
    ) -> Result<(), AppError> {
        validate_clone_request(request)?;
        let mut git_request = GitRequest::new([
            OsString::from("clone"),
            OsString::from("--progress"),
            OsString::from("--"),
            OsString::from(&request.remote_url),
            request.destination.as_os_str().to_owned(),
        ]);
        git_request.timeout = Duration::from_secs(30 * 60);
        run_remote(&self.executor, git_request, cancellation, progress)
    }
}

fn validate_clone_request(request: &CloneRequest) -> Result<(), AppError> {
    let remote = request.remote_url.trim();
    if remote.is_empty() || remote.contains(['\r', '\n']) {
        return Err(AppError::InvalidRequest(
            "Enter a valid repository URL".to_owned(),
        ));
    }
    let parent = request
        .destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(AppError::InvalidPath);
    }
    if request.destination.exists() {
        return Err(AppError::InvalidRequest(
            "Clone destination already exists".to_owned(),
        ));
    }
    Ok(())
}

fn run_remote(
    executor: &GitExecutor,
    request: GitRequest,
    cancellation: &CancellationToken,
    mut progress: impl FnMut(RemoteProgress),
) -> Result<(), AppError> {
    let output = executor
        .execute_streaming(request, cancellation, |is_stderr, chunk| {
            let message = sanitize_progress(chunk);
            if !message.is_empty() {
                progress(RemoteProgress {
                    stream: if is_stderr { "stderr" } else { "stdout" },
                    message,
                });
            }
        })
        .map_err(map_execution_error)?;
    ensure_remote_success(output)
}

fn sanitize_progress(chunk: &[u8]) -> String {
    let value = String::from_utf8_lossy(chunk);
    value
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(redact_remote)
        .map(|line| redact_query_secret(&line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_query_secret(value: &str) -> String {
    let mut redacted = value.to_owned();
    for key in ["token=", "access_token=", "password="] {
        let mut search_from = 0;
        while let Some(relative) = redacted[search_from..].to_ascii_lowercase().find(key) {
            let start = search_from + relative + key.len();
            let end = redacted[start..]
                .find(['&', ' ', '\t'])
                .map_or(redacted.len(), |offset| start + offset);
            redacted.replace_range(start..end, "***");
            search_from = start + 3;
        }
    }
    redacted
}

fn ensure_remote_success(output: GitOutput) -> Result<(), AppError> {
    if output.exit_code == 0 {
        return Ok(());
    }
    let detail = sanitize_progress(&output.stderr);
    let lower = detail.to_ascii_lowercase();
    if lower.contains("authentication failed")
        || lower.contains("permission denied")
        || lower.contains("could not read username")
        || lower.contains("publickey")
    {
        return Err(AppError::AuthenticationFailed);
    }
    if lower.contains("non-fast-forward")
        || lower.contains("fetch first")
        || lower.contains("rejected")
    {
        return Err(AppError::NonFastForward);
    }
    if lower.contains("could not resolve host")
        || lower.contains("network is unreachable")
        || lower.contains("unable to access")
        || lower.contains("connection timed out")
    {
        return Err(AppError::Offline);
    }
    Err(AppError::GitFailed {
        diagnostic_id: Uuid::new_v4(),
        detail,
    })
}

fn map_execution_error(error: GitExecutionError) -> AppError {
    match error {
        GitExecutionError::ExecutableNotFound => AppError::GitNotFound,
        GitExecutionError::TimedOut => AppError::TimedOut,
        GitExecutionError::Cancelled => AppError::Cancelled,
        GitExecutionError::Io(error) => AppError::GitFailed {
            diagnostic_id: Uuid::new_v4(),
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{redact_query_secret, sanitize_progress};

    #[test]
    fn progress_removes_inline_credentials_and_tokens() {
        let value = sanitize_progress(
            b"fatal: https://alice:secret@example.com/x?access_token=private failed\n",
        );
        assert!(!value.contains("alice"));
        assert!(!value.contains("secret"));
        assert!(!value.contains("private"));
        assert!(value.contains("***"));
    }

    #[test]
    fn redacts_secrets_until_query_separator() {
        assert_eq!(
            redact_query_secret("https://host/x?token=secret&ref=main"),
            "https://host/x?token=***&ref=main"
        );
    }
}
