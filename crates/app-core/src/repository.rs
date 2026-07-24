use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git_cli::{CancellationToken, GitExecutionError, GitExecutor, GitOutput, GitRequest};
use git_domain::{RepoId, RepositoryDescriptor, RepositorySnapshot, parse_porcelain_v2};
use uuid::Uuid;

use crate::AppError;

const MINIMUM_GIT_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 40,
    patch: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RepositoryService {
    executor: GitExecutor,
}

impl RepositoryService {
    pub fn new(executor: GitExecutor) -> Self {
        Self { executor }
    }

    pub fn detect_git(&self) -> Result<GitVersion, AppError> {
        let output = self.run(GitRequest::new(["--version"]))?;
        ensure_success(output).and_then(|output| parse_git_version(&output.stdout))
    }

    pub fn discover(&self, selected_path: &Path) -> Result<RepositoryDescriptor, AppError> {
        let version = self.detect_git()?;
        if version < MINIMUM_GIT_VERSION {
            return Err(AppError::UnsupportedGitVersion {
                found: version.to_string(),
                minimum: MINIMUM_GIT_VERSION.to_string(),
            });
        }

        let selected_path = fs::canonicalize(selected_path).map_err(|_| AppError::InvalidPath)?;
        if !selected_path.is_dir() {
            return Err(AppError::InvalidPath);
        }

        let worktree = self.rev_parse_path(&selected_path, "--show-toplevel")?;
        let git_dir = self.rev_parse_path(&selected_path, "--absolute-git-dir")?;
        let worktree_path = fs::canonicalize(worktree).map_err(|_| AppError::InvalidPath)?;
        let git_dir = fs::canonicalize(git_dir).map_err(|_| AppError::InvalidPath)?;
        let name = worktree_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Repository")
            .to_owned();

        Ok(RepositoryDescriptor {
            id: RepoId::from_canonical_path(&worktree_path),
            name,
            worktree_path,
            git_dir,
        })
    }

    pub fn snapshot(
        &self,
        repository: &RepositoryDescriptor,
        revision: u64,
    ) -> Result<RepositorySnapshot, AppError> {
        let mut request = GitRequest::new([
            OsString::from("--no-optional-locks"),
            OsString::from("status"),
            OsString::from("--porcelain=v2"),
            OsString::from("-z"),
            OsString::from("--branch"),
            OsString::from("--show-stash"),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(15);
        let output = ensure_success(self.run(request)?)?;
        let status = parse_porcelain_v2(&output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;

        Ok(RepositorySnapshot {
            revision,
            repository: repository.clone(),
            status,
        })
    }

    fn rev_parse_path(&self, selected_path: &Path, argument: &str) -> Result<PathBuf, AppError> {
        let mut request = GitRequest::new([
            OsString::from("-C"),
            selected_path.as_os_str().to_owned(),
            OsString::from("rev-parse"),
            OsString::from("--path-format=absolute"),
            OsString::from(argument),
        ]);
        request.timeout = Duration::from_secs(5);
        let output = ensure_success(self.run(request)?)?;
        let value = String::from_utf8(output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        Ok(PathBuf::from(value.trim_end_matches(['\r', '\n'])))
    }

    fn run(&self, request: GitRequest) -> Result<GitOutput, AppError> {
        self.executor
            .execute(request, &CancellationToken::default())
            .map_err(map_execution_error)
    }
}

fn ensure_success(output: GitOutput) -> Result<GitOutput, AppError> {
    if output.exit_code == 0 {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.contains("not a git repository") {
        return Err(AppError::RepositoryNotFound);
    }
    Err(AppError::GitFailed {
        diagnostic_id: Uuid::new_v4(),
        detail: stderr,
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

fn parse_git_version(output: &[u8]) -> Result<GitVersion, AppError> {
    let text = std::str::from_utf8(output)
        .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
    let version = text
        .strip_prefix("git version ")
        .ok_or_else(|| AppError::InvalidGitOutput(text.trim().to_owned()))?;
    let mut parts = version.trim().split('.');
    let parse_part = |part: Option<&str>| {
        part.and_then(|value| {
            value
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
    };

    Ok(GitVersion {
        major: parse_part(parts.next())
            .ok_or_else(|| AppError::InvalidGitOutput(version.to_owned()))?,
        minor: parse_part(parts.next())
            .ok_or_else(|| AppError::InvalidGitOutput(version.to_owned()))?,
        patch: parse_part(parts.next())
            .ok_or_else(|| AppError::InvalidGitOutput(version.to_owned()))?,
    })
}

#[cfg(test)]
mod tests {
    use super::{GitVersion, parse_git_version};

    #[test]
    fn parses_windows_git_version_suffix() {
        assert_eq!(
            parse_git_version(b"git version 2.55.0.windows.3\n").expect("valid version"),
            GitVersion {
                major: 2,
                minor: 55,
                patch: 0
            }
        );
    }
}
