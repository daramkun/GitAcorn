use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git_cli::{CancellationToken, GitExecutionError, GitExecutor, GitOutput, GitRequest};
use git_domain::{
    RepoId, RepositoryDescriptor, RepositorySnapshot, WorktreeId, parse_porcelain_v2,
};
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySidebar {
    pub worktrees: Vec<WorktreeSummary>,
    pub branches: RefSummary,
    pub tags: RefSummary,
    pub stashes: Vec<StashSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSummary {
    pub id: WorktreeId,
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_current: bool,
    pub is_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSummary {
    pub total: usize,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashSummary {
    pub reference: String,
    pub message: String,
}

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
        let common_git_dir = self.rev_parse_path(&selected_path, "--git-common-dir")?;
        let worktree_path = fs::canonicalize(worktree).map_err(|_| AppError::InvalidPath)?;
        let git_dir = fs::canonicalize(git_dir).map_err(|_| AppError::InvalidPath)?;
        let common_git_dir = fs::canonicalize(common_git_dir).map_err(|_| AppError::InvalidPath)?;
        let name = worktree_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Repository")
            .to_owned();

        Ok(RepositoryDescriptor {
            id: RepoId::from_canonical_path(&common_git_dir),
            worktree_id: WorktreeId::from_canonical_path(&worktree_path),
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

    pub fn sidebar(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<RepositorySidebar, AppError> {
        let worktrees = self.git_text(repository, ["worktree", "list", "--porcelain", "-z"])?;
        let branches = self.git_text(
            repository,
            [
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
            ],
        )?;
        let tags = self.git_text(
            repository,
            [
                "for-each-ref",
                "--sort=-creatordate",
                "--format=%(refname:short)",
                "refs/tags",
            ],
        )?;
        let stashes = self.git_bytes(repository, ["stash", "list", "--format=%gd%x00%s%x00"])?;

        Ok(RepositorySidebar {
            worktrees: parse_worktrees(&worktrees, &repository.worktree_path),
            branches: summarize_refs(&branches),
            tags: summarize_refs(&tags),
            stashes: parse_stashes(&stashes),
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

    fn git_bytes<I, S>(
        &self,
        repository: &RepositoryDescriptor,
        args: I,
    ) -> Result<Vec<u8>, AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut request = GitRequest::new(args);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(10);
        Ok(ensure_success(self.run(request)?)?.stdout)
    }

    fn git_text<I, S>(&self, repository: &RepositoryDescriptor, args: I) -> Result<String, AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        String::from_utf8(self.git_bytes(repository, args)?)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))
    }
}

fn parse_worktrees(output: &str, current_path: &Path) -> Vec<WorktreeSummary> {
    output
        .split("\0\0")
        .filter_map(|record| {
            let mut path = None;
            let mut head = None;
            let mut branch = None;
            let mut locked = false;
            for field in record.split('\0').flat_map(str::lines) {
                if let Some(value) = field.strip_prefix("worktree ") {
                    path = Some(value.to_owned());
                } else if let Some(value) = field.strip_prefix("HEAD ") {
                    head = Some(value.to_owned());
                } else if let Some(value) = field.strip_prefix("branch ") {
                    branch = Some(value.trim_start_matches("refs/heads/").to_owned());
                } else if field == "locked" || field.starts_with("locked ") {
                    locked = true;
                }
            }
            path.map(|path| WorktreeSummary {
                id: WorktreeId::from_canonical_path(
                    &fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path)),
                ),
                is_current: normalized_path(Path::new(&path)) == normalized_path(current_path),
                path,
                head,
                branch,
                is_locked: locked,
            })
        })
        .collect()
}

fn normalized_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    let value = value.strip_prefix("//?/").unwrap_or(&value).to_owned();
    #[cfg(windows)]
    let value = value.to_lowercase();
    value.trim_end_matches('/').to_owned()
}

fn summarize_refs(output: &str) -> RefSummary {
    let refs: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.ends_with("/HEAD"))
        .map(ToOwned::to_owned)
        .collect();
    RefSummary {
        total: refs.len(),
        items: refs.into_iter().take(5).collect(),
    }
}

fn parse_stashes(output: &[u8]) -> Vec<StashSummary> {
    output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty() && *field != b"\n")
        .collect::<Vec<_>>()
        .chunks(2)
        .filter_map(|fields| {
            let reference = String::from_utf8_lossy(fields.first()?).trim().to_owned();
            let message = String::from_utf8_lossy(fields.get(1)?).trim().to_owned();
            Some(StashSummary { reference, message })
        })
        .take(5)
        .collect()
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
    use std::path::Path;

    use super::{GitVersion, parse_git_version, parse_stashes, parse_worktrees, summarize_refs};

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

    #[test]
    fn parses_sidebar_machine_formats_and_limits_refs() {
        let worktrees = parse_worktrees(
            "worktree C:/repo\0HEAD abc\0branch refs/heads/main\0\0worktree C:/other\0HEAD def\0detached\0locked reason\0\0",
            Path::new("C:/repo"),
        );
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_current);
        assert!(worktrees[1].is_locked);

        let refs = summarize_refs("main\nfeature\none\ntwo\nthree\nfour\n");
        assert_eq!(refs.total, 6);
        assert_eq!(refs.items.len(), 5);

        let stashes = parse_stashes(b"stash@{0}\0WIP one\0\nstash@{1}\0WIP two\0\n");
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].reference, "stash@{0}");
    }
}
