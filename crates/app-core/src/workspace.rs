use std::ffi::OsString;
use std::time::Duration;

use git_cli::{CancellationToken, GitExecutionError, GitOutput, GitRequest};
use git_domain::RepositoryDescriptor;
use uuid::Uuid;

use crate::{AppError, RepositoryService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashRequest {
    pub message: String,
    pub include_untracked: bool,
    pub paths: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    Ours,
    Theirs,
    MarkResolved,
}

impl RepositoryService {
    pub fn create_stash(
        &self,
        repository: &RepositoryDescriptor,
        request: &StashRequest,
    ) -> Result<(), AppError> {
        if request.message.contains(['\r', '\n']) {
            return Err(AppError::InvalidRequest(
                "Stash message must be a single line".to_owned(),
            ));
        }
        let mut args = vec![OsString::from("stash"), OsString::from("push")];
        if request.include_untracked {
            args.push(OsString::from("--include-untracked"));
        }
        let message = request.message.trim();
        if !message.is_empty() {
            args.push(OsString::from("--message"));
            args.push(OsString::from(message));
        }
        if !request.paths.is_empty() {
            args.push(OsString::from("--"));
            args.extend(
                request
                    .paths
                    .iter()
                    .map(|path| path_argument(path))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        self.workspace_git_unit(repository, args)
    }

    pub fn apply_stash(
        &self,
        repository: &RepositoryDescriptor,
        reference: &str,
    ) -> Result<(), AppError> {
        let reference = validate_stash_reference(reference)?;
        let output = self.workspace_git(repository, ["stash", "apply", "--index", reference])?;
        if output.exit_code == 0 || self.has_unmerged_entries(repository)? {
            Ok(())
        } else {
            Err(git_failed(output))
        }
    }

    pub fn drop_stash(
        &self,
        repository: &RepositoryDescriptor,
        reference: &str,
    ) -> Result<(), AppError> {
        let reference = validate_stash_reference(reference)?;
        self.workspace_git_unit(repository, ["stash", "drop", reference])
    }

    pub fn resolve_conflict(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        resolution: ConflictResolution,
    ) -> Result<(), AppError> {
        let path = path_argument(path)?;
        let entries = self.workspace_git(
            repository,
            [
                OsString::from("ls-files"),
                OsString::from("-u"),
                OsString::from("--"),
                path.clone(),
            ],
        )?;
        ensure_success(entries.clone())?;
        if entries.stdout.is_empty() {
            return Err(AppError::InvalidRequest(
                "The selected file is no longer conflicted".to_owned(),
            ));
        }
        if let Some(flag) = match resolution {
            ConflictResolution::Ours => Some("--ours"),
            ConflictResolution::Theirs => Some("--theirs"),
            ConflictResolution::MarkResolved => None,
        } {
            self.workspace_git_unit(
                repository,
                [
                    OsString::from("checkout"),
                    OsString::from(flag),
                    OsString::from("--"),
                    path.clone(),
                ],
            )?;
        }
        self.workspace_git_unit(
            repository,
            [OsString::from("add"), OsString::from("--"), path],
        )
    }

    pub fn abort_merge(&self, repository: &RepositoryDescriptor) -> Result<(), AppError> {
        if !repository.git_dir.join("MERGE_HEAD").is_file() {
            return Err(AppError::InvalidRequest(
                "There is no merge in progress".to_owned(),
            ));
        }
        self.workspace_git_unit(repository, ["merge", "--abort"])
    }

    fn has_unmerged_entries(&self, repository: &RepositoryDescriptor) -> Result<bool, AppError> {
        let output = ensure_success(self.workspace_git(repository, ["ls-files", "-u"])?)?;
        Ok(!output.stdout.is_empty())
    }

    fn workspace_git_unit(
        &self,
        repository: &RepositoryDescriptor,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<(), AppError> {
        ensure_success(self.workspace_git(repository, args)?).map(|_| ())
    }

    fn workspace_git(
        &self,
        repository: &RepositoryDescriptor,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<GitOutput, AppError> {
        let mut request = GitRequest::new(args);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(120);
        self.executor
            .execute(request, &CancellationToken::default())
            .map_err(map_execution_error)
    }
}

fn validate_stash_reference(reference: &str) -> Result<&str, AppError> {
    let Some(index) = reference
        .strip_prefix("stash@{")
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err(AppError::InvalidRequest(
            "Stash reference is invalid".to_owned(),
        ));
    };
    if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::InvalidRequest(
            "Stash reference is invalid".to_owned(),
        ));
    }
    Ok(reference)
}

fn ensure_success(output: GitOutput) -> Result<GitOutput, AppError> {
    if output.exit_code == 0 {
        Ok(output)
    } else {
        Err(git_failed(output))
    }
}

fn git_failed(output: GitOutput) -> AppError {
    AppError::GitFailed {
        diagnostic_id: Uuid::new_v4(),
        detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
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

#[cfg(unix)]
fn path_argument(path: &[u8]) -> Result<OsString, AppError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(path.to_vec()))
}

#[cfg(windows)]
fn path_argument(path: &[u8]) -> Result<OsString, AppError> {
    String::from_utf8(path.to_vec())
        .map(OsString::from)
        .map_err(|_| AppError::InvalidRequest("Path is not valid UTF-8 on Windows".to_owned()))
}

#[cfg(not(any(unix, windows)))]
fn path_argument(path: &[u8]) -> Result<OsString, AppError> {
    String::from_utf8(path.to_vec())
        .map(OsString::from)
        .map_err(|_| AppError::InvalidRequest("Path is not valid UTF-8".to_owned()))
}
