use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git_cli::{CancellationToken, GitExecutionError, GitExecutor, GitOutput, GitRequest};
use git_domain::{
    DiffDocument, DiffLineKind, FileBlame, HistoryPage, PathHistory, RepoId, RepositoryDescriptor,
    RepositoryOperation, RepositorySnapshot, WorktreeId, parse_blame_porcelain,
    parse_history_records, parse_path_history, parse_porcelain_v2, parse_unified_diff,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySidebar {
    pub worktrees: Vec<WorktreeSummary>,
    pub submodules: Vec<SubmoduleSummary>,
    pub branches: RefSummary,
    pub remote_branches: RefSummary,
    pub tags: RefSummary,
    pub stashes: Vec<StashSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmoduleSummary {
    pub path: String,
    pub absolute_path: String,
    pub initialized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSummary {
    pub id: WorktreeId,
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_current: bool,
    pub is_locked: bool,
    pub is_prunable: bool,
    pub is_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCreateRequest {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub start_point: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTagSummary {
    pub remote: String,
    pub name: String,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitIdentitySettings {
    pub global: GitIdentity,
    pub local: GitIdentity,
    pub effective: GitIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitReference {
    pub full_name: String,
    pub short_name: String,
    pub oid: String,
    pub kind: ReferenceKind,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryFilter {
    pub cursor: Option<String>,
    pub reference: Option<String>,
    pub query: Option<String>,
    pub author: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRequest {
    pub name: String,
    pub start_point: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTarget {
    Unstaged,
    Staged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparePatch {
    pub bytes: Vec<u8>,
    pub file_count: usize,
    pub is_binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDiffTool {
    pub configured: Option<String>,
    pub merge_configured: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDiffResult {
    pub tool: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryPreview {
    pub old_path: String,
    pub new_path: String,
    pub mime_type: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub old_bytes: Option<Vec<u8>>,
    pub new_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfsOperationKind {
    Fetch,
    Pull,
    Prune,
}

impl LfsOperationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fetch => "lfs-fetch",
            Self::Pull => "lfs-pull",
            Self::Prune => "lfs-prune",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsRequest {
    pub kind: LfsOperationKind,
    pub remote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsFileStatus {
    pub path: String,
    pub oid: Option<String>,
    pub size: Option<u64>,
    pub downloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsStatus {
    pub installed: bool,
    pub tracked: Vec<LfsFileStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
    pub locked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureStatus {
    pub revision: String,
    pub kind: String,
    pub status: String,
    pub signer: Option<String>,
    pub key_id: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureSettings {
    pub commit_sign: bool,
    pub tag_sign: bool,
    pub format: Option<String>,
    pub signing_key: Option<String>,
    pub ssh_allowed_signers_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchSelection {
    pub hunk_index: usize,
    pub line_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub summary: String,
    pub description: String,
    pub amend: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFile {
    pub path: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractiveRebaseAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

impl InteractiveRebaseAction {
    fn as_todo_command(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword | Self::Edit => "edit",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveRebaseItem {
    pub oid: String,
    pub action: InteractiveRebaseAction,
    pub summary: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveRebaseRequest {
    pub base_oid: String,
    pub expected_head_oid: String,
    pub items: Vec<InteractiveRebaseItem>,
    pub auto_stash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveRebaseCommit {
    pub oid: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveRebasePreview {
    pub base_oid: String,
    pub head_oid: String,
    pub branch: String,
    pub commits: Vec<InteractiveRebaseCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractiveRebaseSession {
    items: Vec<InteractiveRebaseItem>,
}

const INTERACTIVE_REBASE_SESSION_FILE: &str = "git-acorn-interactive-rebase.json";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOperation {
    CherryPick,
    Revert,
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RepositoryService {
    pub(crate) executor: GitExecutor,
}

impl RepositoryService {
    pub fn new(executor: GitExecutor) -> Self {
        Self { executor }
    }

    pub fn detect_git(&self) -> Result<GitVersion, AppError> {
        let output = self.run(GitRequest::new(["--version"]))?;
        ensure_success(output).and_then(|output| parse_git_version(&output.stdout))
    }

    pub fn global_identity(&self) -> Result<GitIdentity, AppError> {
        Ok(GitIdentity {
            name: self.config_value(None, "--global", "user.name")?,
            email: self.config_value(None, "--global", "user.email")?,
        })
    }

    pub fn repository_identity(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<GitIdentitySettings, AppError> {
        Ok(GitIdentitySettings {
            global: self.global_identity()?,
            local: GitIdentity {
                name: self.config_value(Some(repository), "--local", "user.name")?,
                email: self.config_value(Some(repository), "--local", "user.email")?,
            },
            effective: GitIdentity {
                name: self.config_value(Some(repository), "--get", "user.name")?,
                email: self.config_value(Some(repository), "--get", "user.email")?,
            },
        })
    }

    pub fn update_global_identity(
        &self,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<GitIdentity, AppError> {
        self.update_config_value(None, "--global", "user.name", name)?;
        self.update_config_value(None, "--global", "user.email", email)?;
        self.global_identity()
    }

    pub fn update_repository_identity(
        &self,
        repository: &RepositoryDescriptor,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<GitIdentitySettings, AppError> {
        self.update_config_value(Some(repository), "--local", "user.name", name)?;
        self.update_config_value(Some(repository), "--local", "user.email", email)?;
        self.repository_identity(repository)
    }

    pub fn lfs_status(&self, repository: &RepositoryDescriptor) -> Result<LfsStatus, AppError> {
        let version = self.lfs_request(
            repository,
            [OsString::from("version")],
            Duration::from_secs(10),
        );
        let installed = matches!(version, Ok(output) if output.exit_code == 0);
        if !installed {
            return Ok(LfsStatus {
                installed: false,
                tracked: Vec::new(),
            });
        }
        let output = self.lfs_request(
            repository,
            [OsString::from("ls-files"), OsString::from("--long")],
            Duration::from_secs(30),
        )?;
        let tracked = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_lfs_file_status)
            .collect();
        Ok(LfsStatus { installed, tracked })
    }

    pub fn lfs_sync(
        &self,
        repository: &RepositoryDescriptor,
        request: &LfsRequest,
        cancellation: &CancellationToken,
        mut progress: impl FnMut(crate::RemoteProgress),
    ) -> Result<(), AppError> {
        let mut args = vec![OsString::from(match request.kind {
            LfsOperationKind::Fetch => "fetch",
            LfsOperationKind::Pull => "pull",
            LfsOperationKind::Prune => "prune",
        })];
        if let Some(remote) = request.remote.as_deref() {
            let remote = validate_lfs_remote(remote)?;
            args.push(OsString::from(remote));
        }
        let mut git_request = GitRequest::new(args);
        git_request.working_directory = Some(repository.worktree_path.clone());
        git_request.timeout = Duration::from_secs(30 * 60);
        let output = self
            .executor
            .execute_streaming(git_request, cancellation, |is_stderr, chunk| {
                let message = String::from_utf8_lossy(chunk)
                    .split(['\r', '\n'])
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !message.is_empty() {
                    progress(crate::RemoteProgress {
                        stream: if is_stderr { "stderr" } else { "stdout" },
                        message,
                    });
                }
            })
            .map_err(map_execution_error)?;
        if output.exit_code == 0 {
            Ok(())
        } else {
            Err(AppError::GitFailed {
                diagnostic_id: Uuid::new_v4(),
                detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            })
        }
    }

    pub fn lfs_locks(&self, repository: &RepositoryDescriptor) -> Result<Vec<LfsLock>, AppError> {
        if !self.lfs_status(repository)?.installed {
            return Ok(Vec::new());
        }
        let output = self.lfs_request(
            repository,
            [OsString::from("locks"), OsString::from("--json")],
            Duration::from_secs(30),
        )?;
        let payload: LfsLocksPayload = serde_json::from_slice(&output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        Ok(payload
            .locks
            .into_iter()
            .map(|lock| LfsLock {
                id: lock.id,
                path: lock.path,
                owner: lock.owner.name,
                locked_at: lock.locked_at,
            })
            .collect())
    }

    pub fn lfs_lock(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
    ) -> Result<Vec<LfsLock>, AppError> {
        let path = validate_relative_file_path(path)?;
        self.lfs_unit(
            repository,
            [
                OsString::from("lock"),
                OsString::from("--"),
                OsString::from(path),
            ],
        )?;
        self.lfs_locks(repository)
    }

    pub fn lfs_unlock(
        &self,
        repository: &RepositoryDescriptor,
        path: Option<&str>,
        lock_id: Option<&str>,
    ) -> Result<Vec<LfsLock>, AppError> {
        let mut args = vec![OsString::from("unlock")];
        if let Some(lock_id) = lock_id.map(str::trim).filter(|value| !value.is_empty()) {
            if lock_id.starts_with('-') || lock_id.contains(['\0', '\r', '\n']) {
                return Err(AppError::InvalidRequest(
                    "LFS lock ID is invalid".to_owned(),
                ));
            }
            args.extend([OsString::from("--id"), OsString::from(lock_id)]);
        } else if let Some(path) = path {
            args.extend([
                OsString::from("--"),
                OsString::from(validate_relative_file_path(path)?),
            ]);
        } else {
            return Err(AppError::InvalidRequest(
                "Provide an LFS lock path or ID".to_owned(),
            ));
        }
        self.lfs_unit(repository, args)?;
        self.lfs_locks(repository)
    }

    pub fn signature_status(
        &self,
        repository: &RepositoryDescriptor,
        revision: &str,
        kind: &str,
    ) -> Result<SignatureStatus, AppError> {
        let kind = match kind {
            "commit" => "commit",
            "tag" => "tag",
            _ => {
                return Err(AppError::InvalidRequest(
                    "Signature kind must be commit or tag".to_owned(),
                ));
            }
        };
        let revision = revision.trim();
        if kind == "tag" {
            if revision.is_empty()
                || revision.starts_with('-')
                || revision.contains(['\0', '\r', '\n'])
            {
                return Err(AppError::InvalidRequest(
                    "Signature revision is invalid".to_owned(),
                ));
            }
            let mut request = GitRequest::new([
                OsString::from("verify-tag"),
                OsString::from("--raw"),
                OsString::from("--"),
                OsString::from(revision),
            ]);
            request.working_directory = Some(repository.worktree_path.clone());
            request.timeout = Duration::from_secs(15);
            let output = self.run(request)?;
            let diagnostic = String::from_utf8_lossy(&output.stderr).into_owned();
            let status = if output.exit_code == 0 {
                "G"
            } else if diagnostic.to_ascii_lowercase().contains("badsig") {
                "B"
            } else {
                "N"
            };
            let signer = diagnostic
                .lines()
                .find(|line| line.to_ascii_lowercase().contains("good signature"))
                .and_then(|line| line.split(" is ").nth(1))
                .map(str::trim)
                .map(str::to_owned);
            return Ok(SignatureStatus {
                revision: revision.to_owned(),
                kind: kind.to_owned(),
                status: status.to_owned(),
                signer,
                key_id: None,
                fingerprint: None,
            });
        }
        let resolved = self
            .git_text(
                repository,
                [
                    OsString::from("rev-parse"),
                    OsString::from("--verify"),
                    OsString::from("--end-of-options"),
                    OsString::from(format!("{revision}^{{commit}}")),
                ],
            )?
            .trim()
            .to_owned();
        let output = self.git_text(
            repository,
            [
                OsString::from("show"),
                OsString::from("-s"),
                OsString::from("--format=%H%x00%G?%x00%GS%x00%GK%x00%GF"),
                OsString::from(&resolved),
            ],
        )?;
        let mut fields = output.trim_end_matches(['\r', '\n']).split('\0');
        let _formatted_revision = fields.next();
        Ok(SignatureStatus {
            revision: resolved,
            kind: kind.to_owned(),
            status: fields.next().unwrap_or_default().to_owned(),
            signer: non_empty(fields.next()),
            key_id: non_empty(fields.next()),
            fingerprint: non_empty(fields.next()),
        })
    }

    pub fn signature_settings(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<SignatureSettings, AppError> {
        Ok(SignatureSettings {
            commit_sign: self
                .config_value(Some(repository), "--get", "commit.gpgsign")?
                .is_some_and(|value| value == "true"),
            tag_sign: self
                .config_value(Some(repository), "--get", "tag.gpgSign")?
                .is_some_and(|value| value == "true"),
            format: self.config_value(Some(repository), "--get", "gpg.format")?,
            signing_key: self.config_value(Some(repository), "--get", "user.signingkey")?,
            ssh_allowed_signers_file: self.config_value(
                Some(repository),
                "--get",
                "gpg.ssh.allowedSignersFile",
            )?,
        })
    }

    pub fn update_signature_settings(
        &self,
        repository: &RepositoryDescriptor,
        settings: &SignatureSettings,
    ) -> Result<SignatureSettings, AppError> {
        self.update_config_bool(repository, "commit.gpgsign", settings.commit_sign)?;
        self.update_config_bool(repository, "tag.gpgSign", settings.tag_sign)?;
        self.update_config_value(
            Some(repository),
            "--local",
            "gpg.format",
            settings.format.as_deref(),
        )?;
        self.update_config_value(
            Some(repository),
            "--local",
            "user.signingkey",
            settings.signing_key.as_deref(),
        )?;
        self.update_config_value(
            Some(repository),
            "--local",
            "gpg.ssh.allowedSignersFile",
            settings.ssh_allowed_signers_file.as_deref(),
        )?;
        self.signature_settings(repository)
    }

    fn lfs_request<I, S>(
        &self,
        repository: &RepositoryDescriptor,
        args: I,
        timeout: Duration,
    ) -> Result<GitOutput, AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut request = GitRequest::new(args);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = timeout;
        self.run(request)
    }

    fn lfs_unit<I, S>(&self, repository: &RepositoryDescriptor, args: I) -> Result<(), AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        ensure_success(self.lfs_request(repository, args, Duration::from_secs(30))?).map(|_| ())
    }

    fn update_config_bool(
        &self,
        repository: &RepositoryDescriptor,
        key: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        self.update_config_value(
            Some(repository),
            "--local",
            key,
            Some(if enabled { "true" } else { "false" }),
        )
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
            OsString::from("--untracked-files=all"),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(15);
        let output = ensure_success(self.run(request)?)?;
        let status = parse_porcelain_v2(&output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        let autostash_conflict_marker = repository.git_dir.join("git-acorn-autostash-conflict");
        let operation = if self.rebase_in_progress(repository) {
            if !status.changes.iter().any(|change| change.is_conflict)
                && self
                    .paused_rebase_item(repository)
                    .is_some_and(|item| item.action == InteractiveRebaseAction::Edit)
            {
                Some(RepositoryOperation::RebaseEdit)
            } else {
                Some(RepositoryOperation::Rebase)
            }
        } else if repository.git_dir.join("CHERRY_PICK_HEAD").is_file() {
            Some(RepositoryOperation::CherryPick)
        } else if repository.git_dir.join("REVERT_HEAD").is_file() {
            Some(RepositoryOperation::Revert)
        } else if autostash_conflict_marker.is_file()
            && status.changes.iter().any(|change| change.is_conflict)
        {
            Some(RepositoryOperation::AutostashConflict)
        } else {
            if autostash_conflict_marker.is_file() {
                let _ = fs::remove_file(autostash_conflict_marker);
            }
            None
        };

        Ok(RepositorySnapshot {
            revision,
            repository: repository.clone(),
            status,
            operation,
        })
    }

    pub fn sidebar(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<RepositorySidebar, AppError> {
        let worktrees = self.git_text(repository, ["worktree", "list", "--porcelain", "-z"])?;
        let submodules = self.git_text(repository, ["submodule", "status"])?;
        let branches = self.git_text(
            repository,
            [
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname:short)",
                "refs/heads",
            ],
        )?;
        let remote_branches = self.git_text(
            repository,
            [
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname:short)",
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
            submodules: parse_submodules(&submodules, &repository.worktree_path),
            branches: summarize_refs(&branches),
            remote_branches: summarize_refs(&remote_branches),
            tags: summarize_refs(&tags),
            stashes: parse_stashes(&stashes),
        })
    }

    pub fn create_worktree(
        &self,
        repository: &RepositoryDescriptor,
        request: &WorktreeCreateRequest,
    ) -> Result<(), AppError> {
        if request.path.as_os_str().is_empty() || !request.path.is_absolute() {
            return Err(AppError::InvalidRequest(
                "Worktree path must be an absolute path".to_owned(),
            ));
        }
        if request.path.exists() {
            return Err(AppError::InvalidRequest(
                "Worktree path already exists".to_owned(),
            ));
        }
        let Some(parent) = request.path.parent() else {
            return Err(AppError::InvalidPath);
        };
        if !parent.is_dir() {
            return Err(AppError::InvalidPath);
        }
        if let Some(branch) = request.branch.as_deref() {
            self.validate_branch_name(repository, branch)?;
        }
        if let Some(start_point) = request.start_point.as_deref() {
            self.git_text(
                repository,
                [
                    "rev-parse",
                    "--verify",
                    &format!("{start_point}^{{commit}}"),
                ],
            )?;
        }

        let mut args = vec![OsString::from("worktree"), OsString::from("add")];
        if let Some(branch) = request.branch.as_deref() {
            args.extend([OsString::from("-b"), OsString::from(branch)]);
        }
        args.push(request.path.clone().into_os_string());
        if let Some(start_point) = request.start_point.as_deref() {
            args.push(OsString::from(start_point));
        }
        self.git_unit(repository, args)
    }

    pub fn remove_worktree(
        &self,
        repository: &RepositoryDescriptor,
        path: &Path,
        force: bool,
    ) -> Result<(), AppError> {
        if fs::canonicalize(path).ok().as_ref() == Some(&repository.worktree_path) {
            return Err(AppError::InvalidRequest(
                "The current worktree cannot be removed".to_owned(),
            ));
        }
        let mut args = vec![OsString::from("worktree"), OsString::from("remove")];
        if force {
            args.push(OsString::from("--force"));
        }
        args.push(path.to_path_buf().into_os_string());
        self.git_unit(repository, args)
    }

    pub fn lock_worktree(
        &self,
        repository: &RepositoryDescriptor,
        path: &Path,
        reason: Option<&str>,
    ) -> Result<(), AppError> {
        let mut args = vec![OsString::from("worktree"), OsString::from("lock")];
        if let Some(reason) = reason.filter(|reason| !reason.trim().is_empty()) {
            args.extend([OsString::from("--reason"), OsString::from(reason)]);
        }
        args.push(path.to_path_buf().into_os_string());
        self.git_unit(repository, args)
    }

    pub fn unlock_worktree(
        &self,
        repository: &RepositoryDescriptor,
        path: &Path,
    ) -> Result<(), AppError> {
        self.git_unit(
            repository,
            [
                OsString::from("worktree"),
                OsString::from("unlock"),
                path.to_path_buf().into_os_string(),
            ],
        )
    }

    pub fn references(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<Vec<GitReference>, AppError> {
        let output = self.git_bytes(
            repository,
            [
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)%00%(refname:short)%00%(objectname)%00%(upstream:short)%00%(upstream:track)%00%1e",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
        )?;
        parse_references(&output)
    }

    pub fn reflog(
        &self,
        repository: &RepositoryDescriptor,
        limit: usize,
    ) -> Result<Vec<ReflogEntry>, AppError> {
        let output = self.git_bytes(
            repository,
            [
                OsString::from("reflog"),
                OsString::from("show"),
                OsString::from("--all"),
                OsString::from(format!("--max-count={}", limit.clamp(1, 500))),
                OsString::from(
                    "--format=%gD%x00%H%x00%gs%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00%x1e",
                ),
            ],
        )?;
        let mut entries = parse_reflog_entries(&output)?;
        let reachable = self
            .git_text(repository, ["rev-list", "--all"])?
            .lines()
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        for entry in &mut entries {
            entry.reflog_only = !reachable.contains(&entry.oid);
        }
        Ok(entries)
    }

    pub fn restore_reflog_reference(
        &self,
        repository: &RepositoryDescriptor,
        oid: &str,
        name: &str,
        is_tag: bool,
    ) -> Result<(), AppError> {
        validate_object_id(oid)?;
        self.git_unit(repository, ["cat-file", "-e", &format!("{oid}^{{commit}}")])?;
        if is_tag {
            self.create_tag(repository, name, oid)
        } else {
            self.create_branch(
                repository,
                &BranchRequest {
                    name: name.to_owned(),
                    start_point: Some(oid.to_owned()),
                },
            )
        }
    }

    pub fn remote_names(&self, repository: &RepositoryDescriptor) -> Result<Vec<String>, AppError> {
        let output = self.git_text(repository, ["remote"])?;
        Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    pub fn remotes(&self, repository: &RepositoryDescriptor) -> Result<Vec<GitRemote>, AppError> {
        self.remote_names(repository)?
            .into_iter()
            .map(|name| {
                let url = self.git_text(
                    repository,
                    [
                        OsString::from("remote"),
                        OsString::from("get-url"),
                        OsString::from(&name),
                    ],
                )?;
                Ok(GitRemote {
                    name,
                    url: url.trim().to_owned(),
                })
            })
            .collect()
    }

    pub fn add_remote(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
        url: &str,
    ) -> Result<(), AppError> {
        let (name, url) = validate_remote(name, url)?;
        self.git_unit(
            repository,
            [
                OsString::from("remote"),
                OsString::from("add"),
                OsString::from(name),
                OsString::from(url),
            ],
        )
    }

    pub fn update_remote(
        &self,
        repository: &RepositoryDescriptor,
        existing_name: &str,
        name: &str,
        url: &str,
    ) -> Result<(), AppError> {
        let existing_name = validate_remote_name(existing_name)?;
        let (name, url) = validate_remote(name, url)?;
        if existing_name != name {
            self.git_unit(
                repository,
                [
                    OsString::from("remote"),
                    OsString::from("rename"),
                    OsString::from(existing_name),
                    OsString::from(name),
                ],
            )?;
        }
        self.git_unit(
            repository,
            [
                OsString::from("remote"),
                OsString::from("set-url"),
                OsString::from(name),
                OsString::from(url),
            ],
        )
    }

    pub fn remove_remote(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<(), AppError> {
        let name = validate_remote_name(name)?;
        self.git_unit(
            repository,
            [
                OsString::from("remote"),
                OsString::from("remove"),
                OsString::from(name),
            ],
        )
    }

    pub fn add_submodule(
        &self,
        repository: &RepositoryDescriptor,
        url: &str,
        path: &str,
    ) -> Result<(), AppError> {
        let url = validate_submodule_url(url)?;
        let path = validate_submodule_path(path)?;
        let mut request = GitRequest::new([
            OsString::from("-c"),
            OsString::from("protocol.file.allow=always"),
            OsString::from("submodule"),
            OsString::from("add"),
            OsString::from("--"),
            OsString::from(url),
            OsString::from(&path),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(300);
        ensure_success(self.run(request)?).map(|_| ())
    }

    pub fn initialize_submodule(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
    ) -> Result<(), AppError> {
        let path = self.managed_submodule_path(repository, path)?;
        let mut request = GitRequest::new([
            OsString::from("-c"),
            OsString::from("protocol.file.allow=always"),
            OsString::from("submodule"),
            OsString::from("update"),
            OsString::from("--init"),
            OsString::from("--"),
            OsString::from(path),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(300);
        ensure_success(self.run(request)?).map(|_| ())
    }

    pub fn deinitialize_submodule(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
    ) -> Result<(), AppError> {
        let path = self.managed_submodule_path(repository, path)?;
        let submodule = self
            .sidebar(repository)?
            .submodules
            .into_iter()
            .find(|submodule| submodule.path == path)
            .ok_or_else(|| {
                AppError::InvalidRequest("The selected path is not a managed submodule".to_owned())
            })?;
        if !submodule.initialized {
            return Err(AppError::InvalidRequest(
                "The selected submodule is already deinitialized".to_owned(),
            ));
        }
        let mut status_request = GitRequest::new([
            OsString::from("status"),
            OsString::from("--porcelain=v1"),
            OsString::from("--untracked-files=normal"),
        ]);
        status_request.working_directory = Some(PathBuf::from(submodule.absolute_path));
        let status = ensure_success(self.run(status_request)?)?;
        if !status.stdout.is_empty() {
            return Err(AppError::InvalidRequest(
                "The submodule has local changes and cannot be deinitialized".to_owned(),
            ));
        }
        let mut request = GitRequest::new([
            OsString::from("submodule"),
            OsString::from("deinit"),
            OsString::from("-f"),
            OsString::from("--"),
            OsString::from(path),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(300);
        ensure_success(self.run(request)?).map(|_| ())
    }

    pub fn remove_submodule(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
    ) -> Result<(), AppError> {
        let path = self.managed_submodule_path(repository, path)?;
        let initialized = self
            .sidebar(repository)?
            .submodules
            .into_iter()
            .find(|submodule| submodule.path == path)
            .is_some_and(|submodule| submodule.initialized);
        if initialized {
            self.deinitialize_submodule(repository, &path)?;
        }
        self.git_unit(
            repository,
            [
                OsString::from("rm"),
                OsString::from("-f"),
                OsString::from("--"),
                OsString::from(path),
            ],
        )
    }

    pub fn with_submodule_update<T>(
        &self,
        repository: &RepositoryDescriptor,
        supports_native_recurse: bool,
        operation: impl FnOnce(bool) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let previous_head = self.current_head_oid(repository)?;
        let clean_submodules = self.clean_initialized_submodules(repository)?;
        let initialized_submodules = self
            .sidebar(repository)?
            .submodules
            .into_iter()
            .filter(|submodule| submodule.initialized)
            .count();
        let use_native_recurse =
            supports_native_recurse && initialized_submodules == clean_submodules.len();
        let result = operation(use_native_recurse)?;
        let current_head = self.current_head_oid(repository)?;
        if use_native_recurse
            || previous_head == current_head
            || current_head.is_none()
            || self.rebase_in_progress(repository)
            || repository.git_dir.join("MERGE_HEAD").is_file()
        {
            return Ok(result);
        }

        for submodule in self.sidebar(repository)?.submodules {
            if !submodule.initialized || !clean_submodules.contains(&submodule.path) {
                continue;
            }
            let mut request = GitRequest::new([
                OsString::from("-c"),
                OsString::from("protocol.file.allow=always"),
                OsString::from("submodule"),
                OsString::from("update"),
                OsString::from("--"),
                OsString::from(&submodule.path),
            ]);
            request.working_directory = Some(repository.worktree_path.clone());
            request.timeout = Duration::from_secs(300);
            ensure_success(self.run(request)?)?;
        }
        Ok(result)
    }

    pub fn current_head_oid(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<Option<String>, AppError> {
        let mut request = GitRequest::new(["rev-parse", "--verify", "HEAD^{commit}"]);
        request.working_directory = Some(repository.worktree_path.clone());
        let output = self.run(request)?;
        if output.exit_code != 0 {
            return Ok(None);
        }
        let oid = String::from_utf8(output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        Ok(Some(oid.trim().to_owned()))
    }

    pub fn current_head_ref(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<Option<String>, AppError> {
        let mut request = GitRequest::new(["symbolic-ref", "--quiet", "--short", "HEAD"]);
        request.working_directory = Some(repository.worktree_path.clone());
        let output = self.run(request)?;
        if output.exit_code != 0 {
            return Ok(None);
        }
        let reference = String::from_utf8(output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        Ok(Some(reference.trim().to_owned()))
    }

    pub fn is_worktree_clean(&self, repository: &RepositoryDescriptor) -> Result<bool, AppError> {
        Ok(self
            .git_bytes(
                repository,
                ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )?
            .is_empty())
    }

    pub fn move_head_soft(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<(), AppError> {
        let current_head = self.current_head_oid(repository)?;
        if current_head.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        self.git_unit(repository, ["reset", "--soft", target_head_oid])
    }

    pub fn move_head_mixed(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<(), AppError> {
        let current_head = self.current_head_oid(repository)?;
        if current_head.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        self.git_unit(repository, ["reset", "--mixed", target_head_oid])
    }

    pub fn reset_head(
        &self,
        repository: &RepositoryDescriptor,
        target_head_oid: &str,
        mode: &str,
    ) -> Result<(), AppError> {
        validate_object_id(target_head_oid)?;
        if self.current_head_ref(repository)?.is_none() {
            return Err(AppError::InvalidRequest(
                "Reset requires a checked out local branch".to_owned(),
            ));
        }
        self.git_unit(
            repository,
            ["cat-file", "-e", &format!("{target_head_oid}^{{commit}}")],
        )?;
        let flag = match mode {
            "soft" => "--soft",
            "mixed" => "--mixed",
            "hard" => "--hard",
            _ => {
                return Err(AppError::InvalidRequest(
                    "Reset mode must be soft, mixed, or hard".to_owned(),
                ));
            }
        };
        self.git_unit(repository, ["reset", flag, target_head_oid])
    }

    pub fn cherry_pick(
        &self,
        repository: &RepositoryDescriptor,
        oids: &[String],
    ) -> Result<(), AppError> {
        self.validate_linear_history_commits(repository, oids)?;
        let mut args = vec![OsString::from("cherry-pick"), OsString::from("--no-edit")];
        args.extend(oids.iter().map(OsString::from));
        self.run_history_operation(repository, args, HistoryOperation::CherryPick)
    }

    pub fn revert(
        &self,
        repository: &RepositoryDescriptor,
        oids: &[String],
    ) -> Result<(), AppError> {
        self.validate_linear_history_commits(repository, oids)?;
        let mut args = vec![OsString::from("revert"), OsString::from("--no-edit")];
        args.extend(oids.iter().map(OsString::from));
        self.run_history_operation(repository, args, HistoryOperation::Revert)
    }

    pub fn continue_history_operation(
        &self,
        repository: &RepositoryDescriptor,
        operation: HistoryOperation,
    ) -> Result<(), AppError> {
        let command = match operation {
            HistoryOperation::CherryPick => "cherry-pick",
            HistoryOperation::Revert => "revert",
        };
        self.run_history_operation(
            repository,
            [OsString::from(command), OsString::from("--continue")],
            operation,
        )
    }

    pub fn abort_history_operation(
        &self,
        repository: &RepositoryDescriptor,
        operation: HistoryOperation,
    ) -> Result<(), AppError> {
        let command = match operation {
            HistoryOperation::CherryPick => "cherry-pick",
            HistoryOperation::Revert => "revert",
        };
        self.git_unit(repository, [command, "--abort"])
    }

    pub fn skip_history_operation(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<(), AppError> {
        self.git_unit(repository, ["cherry-pick", "--skip"])
    }

    fn validate_linear_history_commits(
        &self,
        repository: &RepositoryDescriptor,
        oids: &[String],
    ) -> Result<(), AppError> {
        if oids.is_empty() {
            return Err(AppError::InvalidRequest(
                "Select at least one commit".to_owned(),
            ));
        }
        if !self.is_worktree_clean(repository)? {
            return Err(AppError::InvalidRequest(
                "Commit history operations require a clean worktree".to_owned(),
            ));
        }
        for oid in oids {
            validate_object_id(oid)?;
            let parents = self.git_text(repository, ["rev-list", "--parents", "-n", "1", oid])?;
            if parents.split_whitespace().count() != 2 {
                return Err(AppError::InvalidRequest(
                    "Merge commits are not supported by this operation yet".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn run_history_operation<I, S>(
        &self,
        repository: &RepositoryDescriptor,
        args: I,
        operation: HistoryOperation,
    ) -> Result<(), AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut request = GitRequest::new(args);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(300);
        let output = self.run(request)?;
        if output.exit_code == 0 || self.history_operation_in_progress(repository, operation) {
            Ok(())
        } else {
            ensure_success(output).map(|_| ())
        }
    }

    fn history_operation_in_progress(
        &self,
        repository: &RepositoryDescriptor,
        operation: HistoryOperation,
    ) -> bool {
        match operation {
            HistoryOperation::CherryPick => repository.git_dir.join("CHERRY_PICK_HEAD").is_file(),
            HistoryOperation::Revert => repository.git_dir.join("REVERT_HEAD").is_file(),
        }
    }

    pub fn move_head_hard(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        target_head_oid: &str,
    ) -> Result<(), AppError> {
        if self.current_head_oid(repository)?.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        if !self.is_worktree_clean(repository)? {
            return Err(AppError::InvalidRequest(
                "The working tree changed after this operation; stash or commit changes before recovery"
                    .to_owned(),
            ));
        }
        self.git_unit(repository, ["reset", "--hard", target_head_oid])
    }

    pub fn checkout_for_recovery(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        target_head_oid: &str,
        target_head_ref: Option<&str>,
    ) -> Result<(), AppError> {
        if self.current_head_oid(repository)?.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        if !self.is_worktree_clean(repository)? {
            return Err(AppError::InvalidRequest(
                "The working tree changed after this operation; stash or commit changes before recovery"
                    .to_owned(),
            ));
        }
        match target_head_ref {
            Some(reference) => {
                if self.local_branch_oid(repository, reference)?.as_deref() != Some(target_head_oid)
                {
                    return Err(AppError::InvalidRequest(format!(
                        "Branch {reference} changed after this operation"
                    )));
                }
                self.git_unit(repository, ["switch", reference])
            }
            None => self.git_unit(repository, ["switch", "--detach", target_head_oid]),
        }
    }

    fn clean_initialized_submodules(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<HashSet<String>, AppError> {
        let mut clean = HashSet::new();
        for submodule in self.sidebar(repository)?.submodules {
            if !submodule.initialized
                || !self.submodule_matches_index(repository, &submodule.path, false)?
                || !self.submodule_matches_index(repository, &submodule.path, true)?
            {
                continue;
            }
            let mut request = GitRequest::new([
                OsString::from("status"),
                OsString::from("--porcelain=v1"),
                OsString::from("--untracked-files=normal"),
            ]);
            request.working_directory = Some(PathBuf::from(&submodule.absolute_path));
            let output = ensure_success(self.run(request)?)?;
            if output.stdout.is_empty() {
                clean.insert(submodule.path);
            }
        }
        Ok(clean)
    }

    fn submodule_matches_index(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
        cached: bool,
    ) -> Result<bool, AppError> {
        let mut args = vec![OsString::from("diff")];
        if cached {
            args.push(OsString::from("--cached"));
        }
        args.extend([
            OsString::from("--quiet"),
            OsString::from("--ignore-submodules=none"),
            OsString::from("--"),
            OsString::from(path),
        ]);
        let mut request = GitRequest::new(args);
        request.working_directory = Some(repository.worktree_path.clone());
        let output = self.run(request)?;
        match output.exit_code {
            0 => Ok(true),
            1 => Ok(false),
            _ => ensure_success(output).map(|_| true),
        }
    }

    pub fn remote_tags(
        &self,
        repository: &RepositoryDescriptor,
        remote: Option<&str>,
    ) -> Result<Vec<RemoteTagSummary>, AppError> {
        let remotes = self.remote_names(repository)?;
        let mut results = Vec::new();
        let target_remotes: Vec<&str> = match remote {
            Some(r) => vec![r],
            None => remotes.iter().map(String::as_str).collect(),
        };

        for r in target_remotes {
            let output = self.git_text(repository, ["ls-remote", "--tags", "--refs", r])?;
            for line in output.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let oid = parts[0].to_owned();
                    let refname = parts[1];
                    let name = refname
                        .strip_prefix("refs/tags/")
                        .unwrap_or(refname)
                        .to_owned();
                    results.push(RemoteTagSummary {
                        remote: r.to_owned(),
                        name,
                        oid,
                    });
                }
            }
        }
        Ok(results)
    }

    pub fn history(
        &self,
        repository: &RepositoryDescriptor,
        filter: &HistoryFilter,
    ) -> Result<HistoryPage, AppError> {
        let limit = filter.limit.clamp(1, 200);
        let offset = parse_history_cursor(filter.cursor.as_deref())?;
        let mut args = vec![
            OsString::from("log"),
            OsString::from("--topo-order"),
            OsString::from("--date-order"),
            OsString::from("--decorate=full"),
            OsString::from("--decorate-refs=refs/heads/*"),
            OsString::from("--decorate-refs=refs/remotes/*"),
            OsString::from("--decorate-refs=refs/tags/*"),
            OsString::from("--branches"),
            OsString::from("--remotes"),
            OsString::from("--tags"),
            OsString::from("--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00%D%x00%x1e"),
            OsString::from(format!("--skip={offset}")),
            OsString::from(format!("--max-count={}", limit + 1)),
        ];
        if let Some(query) = filter
            .query
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            args.push(OsString::from("--regexp-ignore-case"));
            args.push(OsString::from(format!("--grep={}", query.trim())));
        }
        if let Some(author) = filter
            .author
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            args.push(OsString::from(format!("--author={}", author.trim())));
        }
        if let Some(reference) = filter
            .reference
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            args.retain(|arg| arg != "--branches" && arg != "--remotes" && arg != "--tags");
            args.push(OsString::from(reference));
        }
        let output = self.git_bytes(repository, args)?;
        let mut commits = parse_history_records(&output).map_err(AppError::InvalidGitOutput)?;
        let has_more = commits.len() > limit;
        commits.truncate(limit);
        if offset == 0
            && filter.reference.as_deref().is_none_or(str::is_empty)
            && filter.query.as_deref().is_none_or(str::is_empty)
            && filter.author.as_deref().is_none_or(str::is_empty)
        {
            self.prepend_missing_remote_tips(repository, &mut commits)?;
        }
        self.mark_remote_only_commits(repository, &mut commits)?;
        Ok(HistoryPage {
            commits,
            next_cursor: has_more.then(|| format!("offset:{}", offset + limit)),
        })
    }

    pub fn blame(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        revision: Option<&str>,
    ) -> Result<FileBlame, AppError> {
        let raw_path = path.to_vec();
        let path = path_argument(path)?;
        let mut args = vec![
            OsString::from("blame"),
            OsString::from("--line-porcelain"),
            OsString::from("--date=unix"),
        ];
        if let Some(revision) = revision {
            validate_object_id(revision)?;
            args.push(OsString::from(revision));
        }
        args.extend([OsString::from("--"), path]);
        let output = self.git_bytes(repository, args)?;
        let lines = parse_blame_porcelain(&output).map_err(AppError::InvalidGitOutput)?;
        Ok(FileBlame {
            path: raw_path,
            revision: revision.map(str::to_owned),
            lines,
        })
    }

    pub fn path_history(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        is_directory: bool,
        query: Option<&str>,
        limit: usize,
    ) -> Result<PathHistory, AppError> {
        let raw_path = path.to_vec();
        let path_argument = path_argument(path)?;
        let limit = limit.clamp(1, 200);
        let mut args = vec![OsString::from("log")];
        if !is_directory {
            args.push(OsString::from("--follow"));
        }
        args.extend([
            OsString::from("--find-renames"),
            OsString::from("--name-status"),
            OsString::from("--format=%x1e%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00"),
            OsString::from(format!("--max-count={limit}")),
            OsString::from("--"),
            path_argument,
        ]);
        let output = self.git_bytes(repository, args)?;
        let mut entries = parse_path_history(&output).map_err(AppError::InvalidGitOutput)?;
        if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
            let query = query.to_lowercase();
            entries.retain(|entry| {
                String::from_utf8_lossy(&entry.path)
                    .to_lowercase()
                    .contains(&query)
                    || entry.subject.to_lowercase().contains(&query)
            });
        }
        let next_cursor = (entries.len() == limit).then(|| format!("offset:{limit}"));
        Ok(PathHistory {
            path: raw_path,
            is_directory,
            entries,
            next_cursor,
        })
    }

    fn prepend_missing_remote_tips(
        &self,
        repository: &RepositoryDescriptor,
        commits: &mut Vec<git_domain::CommitSummary>,
    ) -> Result<(), AppError> {
        let refs = self.git_text(
            repository,
            [
                "for-each-ref",
                "--format=%(objectname)%00%(refname)",
                "refs/remotes",
            ],
        )?;
        let existing: std::collections::HashSet<&str> =
            commits.iter().map(|commit| commit.oid.as_str()).collect();
        let mut missing = Vec::new();
        for line in refs.lines() {
            let Some((oid, reference)) = line.split_once('\0') else {
                continue;
            };
            if !reference.ends_with("/HEAD")
                && !existing.contains(oid)
                && !missing.iter().any(|item| item == oid)
            {
                missing.push(oid.to_owned());
            }
        }
        if missing.is_empty() {
            return Ok(());
        }

        let mut tips = Vec::new();
        for chunk in missing.chunks(128) {
            let mut args = vec![
                OsString::from("log"),
                OsString::from("--no-walk=unsorted"),
                OsString::from("--decorate=full"),
                OsString::from("--decorate-refs=refs/heads/*"),
                OsString::from("--decorate-refs=refs/remotes/*"),
                OsString::from("--decorate-refs=refs/tags/*"),
                OsString::from("--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00%D%x00%x1e"),
            ];
            args.extend(chunk.iter().map(OsString::from));
            let output = self.git_bytes(repository, args)?;
            tips.extend(parse_history_records(&output).map_err(AppError::InvalidGitOutput)?);
        }
        tips.sort_by(|left, right| right.authored_at.cmp(&left.authored_at));
        tips.append(commits);
        *commits = tips;
        Ok(())
    }

    fn mark_remote_only_commits(
        &self,
        repository: &RepositoryDescriptor,
        commits: &mut [git_domain::CommitSummary],
    ) -> Result<(), AppError> {
        if commits.is_empty() {
            return Ok(());
        }
        for chunk in commits.chunks_mut(128) {
            let mut args = vec![
                OsString::from("name-rev"),
                OsString::from("--name-only"),
                OsString::from("--refs=refs/heads/*"),
            ];
            args.extend(chunk.iter().map(|commit| OsString::from(&commit.oid)));
            let names = self.git_text(repository, args)?;
            let names: Vec<&str> = names.lines().collect();
            if names.len() != chunk.len() {
                return Err(AppError::InvalidGitOutput(
                    "git name-rev returned an unexpected number of records".to_owned(),
                ));
            }
            for (commit, name) in chunk.iter_mut().zip(names) {
                commit.remote_only = name.trim() == "undefined";
            }
        }
        Ok(())
    }

    pub fn create_branch(
        &self,
        repository: &RepositoryDescriptor,
        request: &BranchRequest,
    ) -> Result<(), AppError> {
        let name = self.validate_branch_name(repository, &request.name)?;
        let mut args = vec![OsString::from("branch"), OsString::from(name)];
        if let Some(start_point) = request
            .start_point
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            args.push(OsString::from(start_point));
        }
        self.git_unit(repository, args)
    }

    pub fn checkout_branch(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
        is_remote: bool,
        is_tag: bool,
        auto_stash: bool,
    ) -> Result<(), AppError> {
        self.with_submodule_update(repository, true, |recurse_submodules| {
            self.checkout_branch_inner(
                repository,
                name,
                is_remote,
                is_tag,
                auto_stash,
                recurse_submodules,
            )
        })
    }

    fn checkout_branch_inner(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
        is_remote: bool,
        is_tag: bool,
        auto_stash: bool,
        recurse_submodules: bool,
    ) -> Result<(), AppError> {
        let references = self.references(repository)?;
        let reference_kind = if is_tag {
            ReferenceKind::Tag
        } else if is_remote {
            ReferenceKind::RemoteBranch
        } else {
            ReferenceKind::LocalBranch
        };
        if !references
            .iter()
            .any(|reference| reference.kind == reference_kind && reference.short_name == name)
        {
            return Err(AppError::InvalidRequest(format!(
                "Branch {name} does not exist"
            )));
        }

        let stash_before = auto_stash
            .then(|| self.git_text(repository, ["stash", "list", "-1", "--format=%H"]))
            .transpose()?;
        if auto_stash {
            self.create_stash(
                repository,
                &crate::StashRequest {
                    message: "GitAcorn automatic checkout stash".to_owned(),
                    include_untracked: true,
                    paths: Vec::new(),
                },
            )?;
        }
        let stash_after = auto_stash
            .then(|| self.git_text(repository, ["stash", "list", "-1", "--format=%H"]))
            .transpose()?;
        let created_stash =
            auto_stash && stash_before != stash_after && stash_after.as_deref() != Some("");

        let recurse_arg = recurse_submodules.then_some("--recurse-submodules");
        let checkout_result = if is_tag {
            let mut args = vec!["switch"];
            args.extend(recurse_arg);
            args.extend(["--detach", name]);
            self.git_unit(repository, args)
        } else if is_remote {
            self.checkout_remote_branch(repository, name, &references, recurse_submodules)
        } else {
            let mut args = vec!["switch"];
            args.extend(recurse_arg);
            args.push(name);
            self.git_unit(repository, args)
        };
        if let Err(error) = checkout_result {
            if created_stash {
                let _ = self.apply_stash(repository, "stash@{0}");
                if self
                    .git_text(repository, ["diff", "--name-only", "--diff-filter=U"])
                    .is_ok_and(|output| output.trim().is_empty())
                {
                    let _ = self.drop_stash(repository, "stash@{0}");
                }
            }
            return Err(error);
        }

        if created_stash {
            self.apply_stash(repository, "stash@{0}")?;
            if self
                .git_text(repository, ["diff", "--name-only", "--diff-filter=U"])?
                .trim()
                .is_empty()
            {
                self.drop_stash(repository, "stash@{0}")?;
            }
        }
        Ok(())
    }

    fn checkout_remote_branch(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
        references: &[GitReference],
        recurse_submodules: bool,
    ) -> Result<(), AppError> {
        let recurse_arg = recurse_submodules.then_some("--recurse-submodules");
        if let Some(local) = references.iter().find(|reference| {
            reference.kind == ReferenceKind::LocalBranch
                && reference.upstream.as_deref() == Some(name)
        }) {
            let mut args = vec!["switch"];
            args.extend(recurse_arg);
            args.push(&local.short_name);
            return self.git_unit(repository, args);
        }
        if self
            .remote_names(repository)?
            .into_iter()
            .filter(|remote| name.starts_with(&format!("{remote}/")))
            .max_by_key(String::len)
            .is_none()
        {
            return Err(AppError::InvalidRequest(format!(
                "Remote branch {name} is invalid"
            )));
        }
        let mut args = vec!["switch"];
        args.extend(recurse_arg);
        args.extend(["--track", name]);
        self.git_unit(repository, args)
    }

    pub fn delete_branch(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<(), AppError> {
        let name = self.validate_branch_name(repository, name)?;
        self.git_unit(repository, ["branch", "--delete", &name])
    }

    pub fn local_branch_oid(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<Option<String>, AppError> {
        let name = self.validate_branch_name(repository, name)?;
        Ok(self
            .references(repository)?
            .into_iter()
            .find(|reference| {
                reference.kind == ReferenceKind::LocalBranch && reference.short_name == name
            })
            .map(|reference| reference.oid))
    }

    pub fn restore_deleted_branch(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        name: &str,
        oid: &str,
    ) -> Result<(), AppError> {
        if self.current_head_oid(repository)?.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        if self.local_branch_oid(repository, name)?.is_some() {
            return Err(AppError::InvalidRequest(format!(
                "Branch {name} was recreated or changed after deletion"
            )));
        }
        self.create_branch(
            repository,
            &BranchRequest {
                name: name.to_owned(),
                start_point: Some(oid.to_owned()),
            },
        )
    }

    pub fn delete_restored_branch(
        &self,
        repository: &RepositoryDescriptor,
        expected_head_oid: &str,
        name: &str,
        oid: &str,
    ) -> Result<(), AppError> {
        if self.current_head_oid(repository)?.as_deref() != Some(expected_head_oid) {
            return Err(AppError::InvalidRequest(
                "HEAD changed after this operation; refresh before attempting recovery".to_owned(),
            ));
        }
        if self.local_branch_oid(repository, name)?.as_deref() != Some(oid) {
            return Err(AppError::InvalidRequest(format!(
                "Branch {name} changed after it was restored"
            )));
        }
        self.delete_branch(repository, name)
    }

    pub fn delete_remote_branch(
        &self,
        repository: &RepositoryDescriptor,
        remote: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let remote = validate_remote_name(remote)?;
        let name = self.validate_branch_name(repository, name)?;
        self.git_unit(
            repository,
            [
                OsString::from("push"),
                OsString::from("--delete"),
                OsString::from(remote),
                OsString::from(format!("refs/heads/{name}")),
            ],
        )
    }

    pub fn rename_branch(
        &self,
        repository: &RepositoryDescriptor,
        old_name: &str,
        new_name: &str,
        rename_remote: bool,
    ) -> Result<(), AppError> {
        let old_name = self.validate_branch_name(repository, old_name)?;
        let new_name = self.validate_branch_name(repository, new_name)?;
        let upstream = if rename_remote {
            self.references(repository)?
                .into_iter()
                .find(|reference| {
                    reference.kind == ReferenceKind::LocalBranch && reference.short_name == old_name
                })
                .and_then(|reference| reference.upstream)
        } else {
            None
        };

        self.git_unit(
            repository,
            [
                OsString::from("branch"),
                OsString::from("--move"),
                OsString::from(&old_name),
                OsString::from(&new_name),
            ],
        )?;

        let Some(upstream) = upstream else {
            return Ok(());
        };
        let Some(remote) = self
            .remote_names(repository)?
            .into_iter()
            .filter(|remote| upstream.starts_with(&format!("{remote}/")))
            .max_by_key(String::len)
        else {
            return Ok(());
        };
        let remote_branch = upstream
            .strip_prefix(&format!("{remote}/"))
            .expect("remote prefix checked above");
        let push_result = self.git_unit(
            repository,
            [
                OsString::from("push"),
                OsString::from("--atomic"),
                OsString::from(&remote),
                OsString::from(format!("refs/heads/{new_name}:refs/heads/{new_name}")),
                OsString::from(format!(":refs/heads/{remote_branch}")),
            ],
        );
        if let Err(error) = push_result {
            let _ = self.git_unit(
                repository,
                [
                    OsString::from("branch"),
                    OsString::from("--move"),
                    OsString::from(&new_name),
                    OsString::from(&old_name),
                ],
            );
            return Err(error);
        }
        self.git_unit(
            repository,
            [
                OsString::from("branch"),
                OsString::from("--set-upstream-to"),
                OsString::from(format!("{remote}/{new_name}")),
                OsString::from(&new_name),
            ],
        )
    }

    pub fn rebase_onto(
        &self,
        repository: &RepositoryDescriptor,
        reference: &str,
    ) -> Result<(), AppError> {
        let reference = self.validate_branch_name(repository, reference)?;
        let mut request = GitRequest::new([
            OsString::from("rebase"),
            OsString::from("--"),
            OsString::from(reference),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(120);
        let output = self.run(request)?;
        if output.exit_code == 0
            || repository.git_dir.join("rebase-merge").is_dir()
            || repository.git_dir.join("rebase-apply").is_dir()
        {
            Ok(())
        } else {
            ensure_success(output).map(|_| ())
        }
    }

    pub fn interactive_rebase_preview(
        &self,
        repository: &RepositoryDescriptor,
        base_oid: &str,
    ) -> Result<InteractiveRebasePreview, AppError> {
        if self.rebase_in_progress(repository) {
            return Err(AppError::InvalidRequest(
                "A rebase is already in progress".to_owned(),
            ));
        }
        validate_object_id(base_oid)?;

        let head_oid = self
            .git_text(repository, ["rev-parse", "--verify", "HEAD^{commit}"])?
            .trim()
            .to_owned();
        let mut branch_request = GitRequest::new(["symbolic-ref", "--quiet", "--short", "HEAD"]);
        branch_request.working_directory = Some(repository.worktree_path.clone());
        let branch_output = self.run(branch_request)?;
        if branch_output.exit_code != 0 {
            return Err(AppError::InvalidRequest(
                "Interactive rebase requires a checked out local branch".to_owned(),
            ));
        }
        let branch = String::from_utf8(branch_output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?
            .trim()
            .to_owned();

        let mut ancestor_request =
            GitRequest::new(["merge-base", "--is-ancestor", base_oid, head_oid.as_str()]);
        ancestor_request.working_directory = Some(repository.worktree_path.clone());
        let ancestor_output = self.run(ancestor_request)?;
        if ancestor_output.exit_code != 0 {
            return Err(AppError::InvalidRequest(
                "The selected commit is not an ancestor of the current branch".to_owned(),
            ));
        }

        let range = format!("{base_oid}..{head_oid}");
        let output = self.git_bytes(
            repository,
            [
                "log",
                "--reverse",
                "--topo-order",
                "--format=%H%x00%s%x00%P%x1e",
                range.as_str(),
            ],
        )?;
        let commits = parse_interactive_rebase_commits(&output)?;
        if commits.is_empty() {
            return Err(AppError::InvalidRequest(
                "There are no commits after the selected commit".to_owned(),
            ));
        }

        Ok(InteractiveRebasePreview {
            base_oid: base_oid.to_owned(),
            head_oid,
            branch,
            commits,
        })
    }

    pub fn start_interactive_rebase(
        &self,
        repository: &RepositoryDescriptor,
        request: &InteractiveRebaseRequest,
        sequence_editor_executable: &Path,
    ) -> Result<(), AppError> {
        let preview = self.interactive_rebase_preview(repository, &request.base_oid)?;
        if preview.head_oid != request.expected_head_oid {
            return Err(AppError::InvalidRequest(
                "The branch changed after the rebase plan was opened".to_owned(),
            ));
        }
        validate_rebase_plan(&preview, &request.items)?;
        self.save_interactive_rebase_session(repository, &request.items)?;

        let mut plan = tempfile::Builder::new()
            .prefix("git-acorn-rebase-")
            .suffix(".todo")
            .tempfile_in(&repository.git_dir)
            .map_err(|error| AppError::InvalidRequest(error.to_string()))?;
        for item in &request.items {
            writeln!(plan, "{} {}", item.action.as_todo_command(), item.oid)
                .map_err(|error| AppError::InvalidRequest(error.to_string()))?;
        }
        plan.flush()
            .map_err(|error| AppError::InvalidRequest(error.to_string()))?;

        let editor = format!(
            "{} --git-acorn-sequence-editor {}",
            shell_quote(sequence_editor_executable),
            shell_quote(plan.path()),
        );
        let mut args = vec![OsString::from("rebase"), OsString::from("-i")];
        if request.auto_stash {
            args.push(OsString::from("--autostash"));
        }
        args.extend([OsString::from("--"), OsString::from(&request.base_oid)]);
        let mut git_request = GitRequest::new(args);
        git_request.working_directory = Some(repository.worktree_path.clone());
        git_request.timeout = Duration::from_secs(120);
        git_request.environment.insert(
            OsString::from("GIT_SEQUENCE_EDITOR"),
            OsString::from(editor),
        );
        git_request
            .environment
            .insert(OsString::from("GIT_EDITOR"), OsString::from("true"));

        let output = self.run(git_request)?;
        if output.exit_code != 0 && !self.rebase_in_progress(repository) {
            self.remove_interactive_rebase_session(repository);
        }
        if output.exit_code == 0 || self.rebase_in_progress(repository) {
            self.advance_automatic_rewords(repository)?;
        }
        let autostash_conflict = request.auto_stash
            && !self.rebase_in_progress(repository)
            && self
                .git_text(repository, ["diff", "--name-only", "--diff-filter=U"])?
                .lines()
                .any(|line| !line.trim().is_empty());
        if autostash_conflict {
            fs::write(repository.git_dir.join("git-acorn-autostash-conflict"), [])
                .map_err(|error| AppError::InvalidRequest(error.to_string()))?;
        }
        if output.exit_code == 0 || self.rebase_in_progress(repository) || autostash_conflict {
            if !self.rebase_in_progress(repository) {
                self.remove_interactive_rebase_session(repository);
            }
            Ok(())
        } else {
            ensure_success(output).map(|_| ())
        }
    }

    pub fn continue_rebase(&self, repository: &RepositoryDescriptor) -> Result<(), AppError> {
        self.run_rebase_control(repository, "--continue")?;
        self.advance_automatic_rewords(repository)
    }

    pub fn skip_rebase(&self, repository: &RepositoryDescriptor) -> Result<(), AppError> {
        self.run_rebase_control(repository, "--skip")?;
        self.advance_automatic_rewords(repository)
    }

    pub fn abort_rebase(&self, repository: &RepositoryDescriptor) -> Result<(), AppError> {
        self.run_rebase_control(repository, "--abort")?;
        self.remove_interactive_rebase_session(repository);
        Ok(())
    }

    fn run_rebase_control(
        &self,
        repository: &RepositoryDescriptor,
        action: &str,
    ) -> Result<(), AppError> {
        if !self.rebase_in_progress(repository) {
            return Err(AppError::InvalidRequest(
                "There is no rebase in progress".to_owned(),
            ));
        }
        let mut request = GitRequest::new(["rebase", action]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(120);
        request
            .environment
            .insert(OsString::from("GIT_EDITOR"), OsString::from("true"));
        let output = self.run(request)?;
        if output.exit_code == 0 || self.rebase_in_progress(repository) {
            Ok(())
        } else {
            ensure_success(output).map(|_| ())
        }
    }

    fn advance_automatic_rewords(&self, repository: &RepositoryDescriptor) -> Result<(), AppError> {
        while self.rebase_in_progress(repository)
            && !self.rebase_has_unmerged_entries(repository)?
        {
            let Some(item) = self.paused_rebase_item(repository) else {
                break;
            };
            if item.action != InteractiveRebaseAction::Reword {
                break;
            }
            let summary = item.summary.as_deref().unwrap_or_default();
            let description = item.description.as_deref().unwrap_or_default();
            self.commit(
                repository,
                &CommitRequest {
                    summary: summary.to_owned(),
                    description: description.to_owned(),
                    amend: true,
                },
            )?;
            self.run_rebase_control(repository, "--continue")?;
        }
        if !self.rebase_in_progress(repository) {
            self.remove_interactive_rebase_session(repository);
        }
        Ok(())
    }

    fn rebase_has_unmerged_entries(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<bool, AppError> {
        Ok(self
            .git_text(repository, ["diff", "--name-only", "--diff-filter=U"])?
            .lines()
            .any(|line| !line.trim().is_empty()))
    }

    fn interactive_rebase_session_path(&self, repository: &RepositoryDescriptor) -> PathBuf {
        repository.git_dir.join(INTERACTIVE_REBASE_SESSION_FILE)
    }

    fn save_interactive_rebase_session(
        &self,
        repository: &RepositoryDescriptor,
        items: &[InteractiveRebaseItem],
    ) -> Result<(), AppError> {
        let contents = serde_json::to_vec(&InteractiveRebaseSession {
            items: items.to_vec(),
        })
        .map_err(|error| AppError::InvalidRequest(error.to_string()))?;
        fs::write(self.interactive_rebase_session_path(repository), contents)
            .map_err(|error| AppError::InvalidRequest(error.to_string()))
    }

    fn load_interactive_rebase_session(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Option<InteractiveRebaseSession> {
        fs::read(self.interactive_rebase_session_path(repository))
            .ok()
            .and_then(|contents| serde_json::from_slice(&contents).ok())
    }

    fn remove_interactive_rebase_session(&self, repository: &RepositoryDescriptor) {
        let _ = fs::remove_file(self.interactive_rebase_session_path(repository));
    }

    fn paused_rebase_item(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Option<InteractiveRebaseItem> {
        let stopped_oid =
            fs::read_to_string(repository.git_dir.join("rebase-merge/stopped-sha")).ok()?;
        let stopped_oid = stopped_oid.trim();
        self.load_interactive_rebase_session(repository)?
            .items
            .into_iter()
            .find(|item| item.oid == stopped_oid)
    }

    fn rebase_in_progress(&self, repository: &RepositoryDescriptor) -> bool {
        repository.git_dir.join("rebase-merge").is_dir()
            || repository.git_dir.join("rebase-apply").is_dir()
    }

    pub fn create_tag(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
        target: &str,
    ) -> Result<(), AppError> {
        let name = self.validate_tag_name(repository, name)?;
        self.git_unit(repository, ["tag", &name, target])
    }

    pub fn delete_tag(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<(), AppError> {
        let name = self.validate_tag_name(repository, name)?;
        self.git_unit(repository, ["tag", "--delete", &name])
    }

    pub fn delete_remote_tag(
        &self,
        repository: &RepositoryDescriptor,
        remote: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let remote = validate_remote_name(remote)?;
        let name = self.validate_tag_name(repository, name)?;
        self.git_unit(
            repository,
            [
                OsString::from("push"),
                OsString::from("--delete"),
                OsString::from(remote),
                OsString::from(format!("refs/tags/{name}")),
            ],
        )
    }

    pub fn merge_reference(
        &self,
        repository: &RepositoryDescriptor,
        reference: &str,
    ) -> Result<(), AppError> {
        let mut request = GitRequest::new(["merge", "--no-edit", "--", reference]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(120);
        let output = self.run(request)?;
        if output.exit_code == 0 || repository.git_dir.join("MERGE_HEAD").is_file() {
            Ok(())
        } else {
            ensure_success(output).map(|_| ())
        }
    }

    pub fn fast_forward_branch(
        &self,
        repository: &RepositoryDescriptor,
        branch: &str,
    ) -> Result<(), AppError> {
        let branch = self.validate_branch_name(repository, branch)?;
        self.git_unit(
            repository,
            [
                OsString::from("merge"),
                OsString::from("--ff-only"),
                OsString::from(format!("refs/remotes/origin/{branch}")),
            ],
        )
    }

    fn validate_branch_name(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<String, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidRequest(
                "Branch name cannot be empty".to_owned(),
            ));
        }
        self.git_unit(repository, ["check-ref-format", "--branch", name])?;
        Ok(name.to_owned())
    }

    fn validate_tag_name(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<String, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidRequest(
                "Tag name cannot be empty".to_owned(),
            ));
        }
        self.git_unit(
            repository,
            [
                OsString::from("check-ref-format"),
                OsString::from(format!("refs/tags/{name}")),
            ],
        )?;
        Ok(name.to_owned())
    }

    fn managed_submodule_path(
        &self,
        repository: &RepositoryDescriptor,
        path: &str,
    ) -> Result<String, AppError> {
        let path = validate_submodule_path(path)?;
        self.sidebar(repository)?
            .submodules
            .into_iter()
            .find(|submodule| {
                normalized_path(Path::new(&submodule.path)) == normalized_path(Path::new(&path))
            })
            .map(|submodule| submodule.path)
            .ok_or_else(|| {
                AppError::InvalidRequest("The selected path is not a managed submodule".to_owned())
            })
    }

    pub fn diff(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        target: DiffTarget,
    ) -> Result<DiffDocument, AppError> {
        let output = self.diff_bytes(repository, path, target, 3)?;
        parse_unified_diff(&output).map_err(|error| AppError::InvalidGitOutput(error.to_string()))
    }

    pub fn compare(
        &self,
        repository: &RepositoryDescriptor,
        left: &str,
        right: &str,
    ) -> Result<DiffDocument, AppError> {
        let left = validate_compare_ref(self, repository, left)?;
        let right = validate_compare_ref(self, repository, right)?;
        if left.is_none() && right.is_none() {
            return Err(AppError::InvalidRequest(
                "At least one comparison side must be a commit or reference".to_owned(),
            ));
        }
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-color"),
            OsString::from("--binary"),
            OsString::from("--find-renames"),
            OsString::from("--unified=3"),
        ];
        if let Some(left) = left {
            args.push(left);
        }
        if let Some(right) = right {
            args.push(right);
        }
        args.push(OsString::from("--"));
        let output = self.git_bytes(repository, args)?;
        parse_unified_diff(&output).map_err(|error| AppError::InvalidGitOutput(error.to_string()))
    }

    pub fn compare_patch(
        &self,
        repository: &RepositoryDescriptor,
        left: &str,
        right: &str,
    ) -> Result<ComparePatch, AppError> {
        let args = self.compare_args(repository, left, right, true)?;
        let bytes = self.git_bytes(repository, args)?;
        let document = parse_unified_diff(&bytes)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        let is_binary = bytes
            .windows(b"GIT binary patch".len())
            .any(|window| window == b"GIT binary patch")
            || document.files.iter().any(|file| file.binary);
        Ok(ComparePatch {
            bytes,
            file_count: document.files.len(),
            is_binary,
        })
    }

    pub fn validate_patch(
        &self,
        repository: &RepositoryDescriptor,
        patch: &[u8],
    ) -> Result<(), AppError> {
        if patch.is_empty() {
            return Err(AppError::InvalidRequest("Patch cannot be empty".to_owned()));
        }
        let mut request = GitRequest::new([
            OsString::from("apply"),
            OsString::from("--check"),
            OsString::from("--recount"),
            OsString::from("--whitespace=error-all"),
            OsString::from("--"),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.stdin = Some(patch.to_vec());
        request.timeout = Duration::from_secs(15);
        ensure_success(self.run(request)?).map(|_| ())
    }

    pub fn apply_patch(
        &self,
        repository: &RepositoryDescriptor,
        patch: &[u8],
    ) -> Result<(), AppError> {
        self.validate_patch(repository, patch)?;
        let mut request = GitRequest::new([
            OsString::from("apply"),
            OsString::from("--index"),
            OsString::from("--recount"),
            OsString::from("--whitespace=error-all"),
            OsString::from("--"),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.stdin = Some(patch.to_vec());
        request.timeout = Duration::from_secs(15);
        ensure_success(self.run(request)?).map(|_| ())
    }

    pub fn save_patch(&self, path: &Path, patch: &[u8]) -> Result<(), AppError> {
        if patch.is_empty() {
            return Err(AppError::InvalidRequest("Patch cannot be empty".to_owned()));
        }
        if path.as_os_str().is_empty() || path.to_string_lossy().contains(['\0', '\r', '\n']) {
            return Err(AppError::InvalidRequest(
                "Patch path must be a valid file path".to_owned(),
            ));
        }
        if path.is_dir() {
            return Err(AppError::InvalidRequest(
                "Patch path must point to a file".to_owned(),
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::InvalidRequest(format!("Could not create patch folder: {error}"))
            })?;
        }
        fs::write(path, patch)
            .map_err(|error| AppError::InvalidRequest(format!("Could not save patch: {error}")))
    }

    pub fn external_diff_tool(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<ExternalDiffTool, AppError> {
        Ok(ExternalDiffTool {
            configured: self.config_value(Some(repository), "--get", "diff.tool")?,
            merge_configured: self.config_value(Some(repository), "--get", "merge.tool")?,
        })
    }

    pub fn set_external_tools(
        &self,
        repository: &RepositoryDescriptor,
        diff_tool: Option<&str>,
        merge_tool: Option<&str>,
    ) -> Result<ExternalDiffTool, AppError> {
        let diff_tool = validate_tool_name(diff_tool)?;
        let merge_tool = validate_tool_name(merge_tool)?;
        self.update_config_value(
            Some(repository),
            "--local",
            "diff.tool",
            diff_tool.as_deref(),
        )?;
        self.update_config_value(
            Some(repository),
            "--local",
            "merge.tool",
            merge_tool.as_deref(),
        )?;
        self.external_diff_tool(repository)
    }

    pub fn run_external_diff(
        &self,
        repository: &RepositoryDescriptor,
        left: &str,
        right: &str,
    ) -> Result<ExternalDiffResult, AppError> {
        let tool = self
            .external_diff_tool(repository)?
            .configured
            .ok_or_else(|| {
                AppError::InvalidRequest(
                    "Configure a Git diff.tool before launching an external diff".to_owned(),
                )
            })?;
        let left = validate_compare_ref(self, repository, left)?;
        let right = validate_compare_ref(self, repository, right)?;
        if left.is_none() || right.is_none() {
            return Err(AppError::InvalidRequest(
                "External diff tools require two commit or reference sides".to_owned(),
            ));
        }
        let mut request = GitRequest::new([
            OsString::from("difftool"),
            OsString::from("--no-prompt"),
            OsString::from(format!("--tool={tool}")),
            left.expect("validated compare side"),
            right.expect("validated compare side"),
            OsString::from("--"),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(60);
        let output = self.run(request)?;
        Ok(ExternalDiffResult {
            tool,
            exit_code: output.exit_code,
        })
    }

    pub fn run_external_merge(
        &self,
        repository: &RepositoryDescriptor,
    ) -> Result<ExternalDiffResult, AppError> {
        let tool = self
            .external_diff_tool(repository)?
            .merge_configured
            .ok_or_else(|| {
                AppError::InvalidRequest(
                    "Configure a Git merge.tool before launching an external merge".to_owned(),
                )
            })?;
        let mut request = GitRequest::new([
            OsString::from("mergetool"),
            OsString::from("--no-prompt"),
            OsString::from(format!("--tool={tool}")),
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(60);
        let output = self.run(request)?;
        Ok(ExternalDiffResult {
            tool,
            exit_code: output.exit_code,
        })
    }

    pub fn binary_preview(
        &self,
        repository: &RepositoryDescriptor,
        left: &str,
        right: &str,
        old_path: &str,
        new_path: &str,
    ) -> Result<BinaryPreview, AppError> {
        let old_path = validate_relative_file_path(old_path)?;
        let new_path = validate_relative_file_path(new_path)?;
        let left_ref = validate_compare_ref(self, repository, left)?;
        let right_ref = validate_compare_ref(self, repository, right)?;
        let old = self.binary_side(repository, left_ref.as_ref(), &old_path)?;
        let new = self.binary_side(repository, right_ref.as_ref(), &new_path)?;
        let mime_type = image_mime_type(&new_path).or_else(|| image_mime_type(&old_path));
        const PREVIEW_LIMIT: usize = 8 * 1024 * 1024;
        Ok(BinaryPreview {
            old_path,
            new_path,
            mime_type,
            old_size: old.as_ref().map(|bytes| bytes.len() as u64),
            new_size: new.as_ref().map(|bytes| bytes.len() as u64),
            old_bytes: old.filter(|bytes| bytes.len() <= PREVIEW_LIMIT),
            new_bytes: new.filter(|bytes| bytes.len() <= PREVIEW_LIMIT),
        })
    }

    fn compare_args(
        &self,
        repository: &RepositoryDescriptor,
        left: &str,
        right: &str,
        binary: bool,
    ) -> Result<Vec<OsString>, AppError> {
        let left = validate_compare_ref(self, repository, left)?;
        let right = validate_compare_ref(self, repository, right)?;
        if left.is_none() && right.is_none() {
            return Err(AppError::InvalidRequest(
                "At least one comparison side must be a commit or reference".to_owned(),
            ));
        }
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-color"),
            OsString::from("--find-renames"),
            OsString::from("--unified=3"),
        ];
        if binary {
            args.push(OsString::from("--binary"));
        }
        if let Some(left) = left {
            args.push(left);
        }
        if let Some(right) = right {
            args.push(right);
        }
        args.push(OsString::from("--"));
        Ok(args)
    }

    fn binary_side(
        &self,
        repository: &RepositoryDescriptor,
        reference: Option<&OsString>,
        path: &str,
    ) -> Result<Option<Vec<u8>>, AppError> {
        if let Some(reference) = reference {
            let spec = OsString::from(format!("{}:{path}", reference.to_string_lossy()));
            let mut request = GitRequest::new([OsString::from("show"), spec]);
            request.working_directory = Some(repository.worktree_path.clone());
            request.timeout = Duration::from_secs(15);
            let output = self.run(request)?;
            if output.exit_code != 0 {
                return Ok(None);
            }
            return Ok(Some(output.stdout));
        }
        let absolute = repository.worktree_path.join(path);
        match fs::read(absolute) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(AppError::InvalidRequest(format!(
                "Could not read binary preview: {error}"
            ))),
        }
    }

    pub fn commit_files(
        &self,
        repository: &RepositoryDescriptor,
        revision: &str,
    ) -> Result<Vec<CommitFile>, AppError> {
        let (commit, parent) = self.commit_and_parent(repository, revision)?;
        let output = if let Some(parent) = parent {
            self.git_bytes(
                repository,
                [
                    OsString::from("diff"),
                    OsString::from("--name-only"),
                    OsString::from("-z"),
                    parent,
                    commit,
                    OsString::from("--"),
                ],
            )?
        } else {
            self.git_bytes(
                repository,
                [
                    OsString::from("diff-tree"),
                    OsString::from("--root"),
                    OsString::from("--no-commit-id"),
                    OsString::from("--name-only"),
                    OsString::from("-r"),
                    OsString::from("-z"),
                    commit,
                    OsString::from("--"),
                ],
            )?
        };
        Ok(output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| CommitFile {
                path: path.to_vec(),
            })
            .collect())
    }

    pub fn commit_diff(
        &self,
        repository: &RepositoryDescriptor,
        revision: &str,
        path: &[u8],
    ) -> Result<DiffDocument, AppError> {
        let (commit, parent) = self.commit_and_parent(repository, revision)?;
        let mut args = if let Some(parent) = parent {
            vec![
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-color"),
                OsString::from("--unified=3"),
                parent,
                commit,
            ]
        } else {
            vec![
                OsString::from("diff-tree"),
                OsString::from("--root"),
                OsString::from("--no-commit-id"),
                OsString::from("-p"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-color"),
                OsString::from("--unified=3"),
                commit,
            ]
        };
        args.push(OsString::from("--"));
        args.push(path_argument(path)?);
        let output = self.git_bytes(repository, args)?;
        parse_unified_diff(&output).map_err(|error| AppError::InvalidGitOutput(error.to_string()))
    }

    pub fn stage_paths(
        &self,
        repository: &RepositoryDescriptor,
        paths: &[Vec<u8>],
    ) -> Result<(), AppError> {
        let mut args = vec![OsString::from("add"), OsString::from("--")];
        args.extend(
            paths
                .iter()
                .map(|path| path_argument(path))
                .collect::<Result<Vec<_>, _>>()?,
        );
        self.git_unit(repository, args)
    }

    pub fn unstage_paths(
        &self,
        repository: &RepositoryDescriptor,
        paths: &[Vec<u8>],
        unborn: bool,
    ) -> Result<(), AppError> {
        let mut args = if unborn {
            vec![
                OsString::from("rm"),
                OsString::from("--cached"),
                OsString::from("--ignore-unmatch"),
                OsString::from("--"),
            ]
        } else {
            vec![
                OsString::from("restore"),
                OsString::from("--staged"),
                OsString::from("--"),
            ]
        };
        args.extend(
            paths
                .iter()
                .map(|path| path_argument(path))
                .collect::<Result<Vec<_>, _>>()?,
        );
        self.git_unit(repository, args)
    }

    pub fn apply_selection(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        target: DiffTarget,
        selections: &[PatchSelection],
    ) -> Result<(), AppError> {
        if selections.is_empty() {
            return Err(AppError::InvalidRequest(
                "Select at least one changed line or hunk".to_owned(),
            ));
        }
        let output = self.diff_bytes(repository, path, target, 3)?;
        let document = parse_unified_diff(&output)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        let reverse = target == DiffTarget::Staged;
        let patch = build_selected_patch(&document, selections, reverse)?;
        self.apply_cached_patch(repository, patch, reverse)
    }

    pub fn discard_path(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        untracked: bool,
    ) -> Result<(), AppError> {
        let path = path_argument(path)?;
        if untracked {
            self.git_unit(
                repository,
                [
                    OsString::from("clean"),
                    OsString::from("-f"),
                    OsString::from("-d"),
                    OsString::from("--"),
                    path,
                ],
            )
        } else {
            self.git_unit(
                repository,
                [
                    OsString::from("restore"),
                    OsString::from("--worktree"),
                    OsString::from("--"),
                    path,
                ],
            )
        }
    }

    pub fn commit(
        &self,
        repository: &RepositoryDescriptor,
        request: &CommitRequest,
    ) -> Result<(), AppError> {
        let summary = request.summary.trim();
        if summary.is_empty() {
            return Err(AppError::InvalidRequest(
                "Commit summary cannot be empty".to_owned(),
            ));
        }
        if summary.contains(['\r', '\n']) {
            return Err(AppError::InvalidRequest(
                "Commit summary must be a single line".to_owned(),
            ));
        }
        let mut message = summary.to_owned();
        if !request.description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(request.description.trim());
        }
        message.push('\n');
        let mut args = vec![OsString::from("commit"), OsString::from("--file=-")];
        if request.amend {
            args.push(OsString::from("--amend"));
        }
        let mut git_request = GitRequest::new(args);
        git_request.working_directory = Some(repository.worktree_path.clone());
        git_request.timeout = Duration::from_secs(60);
        git_request.stdin = Some(message.into_bytes());
        ensure_success(self.run(git_request)?)?;
        Ok(())
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

    fn config_value(
        &self,
        repository: Option<&RepositoryDescriptor>,
        scope_or_action: &str,
        key: &str,
    ) -> Result<Option<String>, AppError> {
        let mut args = vec![OsString::from("config")];
        if scope_or_action == "--get" {
            args.push(OsString::from("--get"));
        } else {
            args.push(OsString::from(scope_or_action));
            args.push(OsString::from("--get"));
        }
        args.push(OsString::from(key));
        let mut request = GitRequest::new(args);
        request.working_directory = repository.map(|value| value.worktree_path.clone());
        let output = self.run(request)?;
        if output.exit_code == 1 {
            return Ok(None);
        }
        let output = ensure_success(output)?;
        let value = String::from_utf8(output.stdout)
            .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
        Ok(Some(value.trim_end_matches(['\r', '\n']).to_owned()))
    }

    fn update_config_value(
        &self,
        repository: Option<&RepositoryDescriptor>,
        scope: &str,
        key: &str,
        value: Option<&str>,
    ) -> Result<(), AppError> {
        let value = validate_identity_value(value)?;
        let removing = value.is_none();
        let mut args = vec![
            OsString::from("config"),
            OsString::from(scope),
            OsString::from(if value.is_some() {
                "--replace-all"
            } else {
                "--unset-all"
            }),
            OsString::from(key),
        ];
        if let Some(value) = value {
            args.push(OsString::from(value));
        }
        let mut request = GitRequest::new(args);
        request.working_directory = repository.map(|value| value.worktree_path.clone());
        let output = self.run(request)?;
        if removing && matches!(output.exit_code, 1 | 5) {
            return Ok(());
        }
        ensure_success(output).map(|_| ())
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

    fn git_unit<I, S>(&self, repository: &RepositoryDescriptor, args: I) -> Result<(), AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.git_bytes(repository, args).map(|_| ())
    }

    fn commit_and_parent(
        &self,
        repository: &RepositoryDescriptor,
        revision: &str,
    ) -> Result<(OsString, Option<OsString>), AppError> {
        let commit = self
            .git_text(
                repository,
                [
                    OsString::from("rev-parse"),
                    OsString::from("--verify"),
                    OsString::from("--end-of-options"),
                    OsString::from(format!("{revision}^{{commit}}")),
                ],
            )?
            .trim()
            .to_owned();
        let parents = self.git_text(
            repository,
            [
                OsString::from("rev-list"),
                OsString::from("--parents"),
                OsString::from("-n"),
                OsString::from("1"),
                OsString::from(&commit),
            ],
        )?;
        let parent = parents.split_whitespace().nth(1).map(OsString::from);
        Ok((OsString::from(commit), parent))
    }

    fn diff_bytes(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        target: DiffTarget,
        context: usize,
    ) -> Result<Vec<u8>, AppError> {
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-color"),
            OsString::from(format!("--unified={context}")),
        ];
        if target == DiffTarget::Staged {
            args.push(OsString::from("--cached"));
        }
        args.push(OsString::from("--"));
        args.push(path_argument(path)?);
        let output = self.git_bytes(repository, args)?;
        if !output.is_empty()
            || target == DiffTarget::Staged
            || !self.is_untracked(repository, path)?
        {
            return Ok(output);
        }

        let path = path_argument(path)?;
        let mut request = GitRequest::new([
            OsString::from("diff"),
            OsString::from("--no-index"),
            OsString::from("--no-color"),
            OsString::from(format!("--unified={context}")),
            OsString::from("--"),
            OsString::from("/dev/null"),
            path,
        ]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(10);
        let output = self.run(request)?;
        if matches!(output.exit_code, 0 | 1) {
            Ok(output.stdout)
        } else {
            ensure_success(output).map(|output| output.stdout)
        }
    }

    fn is_untracked(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
    ) -> Result<bool, AppError> {
        let output = self.git_bytes(
            repository,
            [
                OsString::from("ls-files"),
                OsString::from("--others"),
                OsString::from("--exclude-standard"),
                OsString::from("-z"),
                OsString::from("--"),
                path_argument(path)?,
            ],
        )?;
        Ok(output
            .split(|byte| *byte == 0)
            .any(|candidate| candidate == path))
    }

    fn apply_cached_patch(
        &self,
        repository: &RepositoryDescriptor,
        patch: Vec<u8>,
        reverse: bool,
    ) -> Result<(), AppError> {
        let run = |check: bool| {
            let mut args = vec![
                OsString::from("apply"),
                OsString::from("--cached"),
                OsString::from("--recount"),
            ];
            if check {
                args.push(OsString::from("--check"));
            }
            if reverse {
                args.push(OsString::from("--reverse"));
            }
            let mut request = GitRequest::new(args);
            request.working_directory = Some(repository.worktree_path.clone());
            request.stdin = Some(patch.clone());
            request.timeout = Duration::from_secs(15);
            ensure_success(self.run(request)?)?;
            Ok(())
        };
        run(true)?;
        run(false)
    }
}

fn validate_remote<'a>(name: &'a str, url: &'a str) -> Result<(&'a str, &'a str), AppError> {
    let name = validate_remote_name(name)?;
    let url = url.trim();
    if url.is_empty() || url.contains(['\r', '\n']) {
        return Err(AppError::InvalidRequest(
            "Remote URL must not be empty or contain line breaks".to_owned(),
        ));
    }
    Ok((name, url))
}

fn validate_remote_name(name: &str) -> Result<&str, AppError> {
    let name = name.trim();
    if name.is_empty()
        || name.contains(['\r', '\n'])
        || name.starts_with('-')
        || name.chars().any(char::is_whitespace)
    {
        return Err(AppError::InvalidRequest(
            "Remote name must not be empty, start with '-', or contain whitespace".to_owned(),
        ));
    }
    Ok(name)
}

fn validate_submodule_url(url: &str) -> Result<&str, AppError> {
    let url = url.trim();
    if url.is_empty() || url.contains(['\0', '\r', '\n']) || url.starts_with('-') {
        return Err(AppError::InvalidRequest(
            "Submodule URL must not be empty, start with '-', or contain line breaks".to_owned(),
        ));
    }
    Ok(url)
}

fn validate_submodule_path(path: &str) -> Result<String, AppError> {
    use std::path::Component;

    let path = path.trim().trim_end_matches(['/', '\\']);
    let value = Path::new(path);
    if path.is_empty()
        || path.contains(['\0', '\r', '\n'])
        || path.starts_with('-')
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::InvalidRequest(
            "Submodule path must be a relative path inside the repository".to_owned(),
        ));
    }
    Ok(path.to_owned())
}

fn build_selected_patch(
    document: &DiffDocument,
    selections: &[PatchSelection],
    reverse: bool,
) -> Result<Vec<u8>, AppError> {
    let file = document
        .files
        .first()
        .ok_or_else(|| AppError::InvalidRequest("No text diff is available".to_owned()))?;
    if file.binary {
        return Err(AppError::InvalidRequest(
            "Binary files can only be staged or unstaged as a whole".to_owned(),
        ));
    }
    let mut patch = file.header.clone();
    let mut selected_change_count = 0;

    for selection in selections {
        let hunk = file.hunks.get(selection.hunk_index).ok_or_else(|| {
            AppError::InvalidRequest("The selected hunk is no longer available".to_owned())
        })?;
        let select_whole_hunk = selection.line_indices.is_empty();
        let changes_before = selected_change_count;
        let mut body = Vec::new();
        let mut old_count = 0_u64;
        let mut new_count = 0_u64;
        let mut kept_previous_line = false;
        for (line_index, line) in hunk.lines.iter().enumerate() {
            let selected = select_whole_hunk || selection.line_indices.contains(&line_index);
            match line.kind {
                DiffLineKind::Context => {
                    body.extend_from_slice(b" ");
                    body.extend_from_slice(&line.raw_content);
                    body.push(b'\n');
                    old_count += 1;
                    new_count += 1;
                    kept_previous_line = true;
                }
                DiffLineKind::Deletion if selected => {
                    body.push(b'-');
                    body.extend_from_slice(&line.raw_content);
                    body.push(b'\n');
                    old_count += 1;
                    selected_change_count += 1;
                    kept_previous_line = true;
                }
                DiffLineKind::Deletion if !reverse => {
                    body.push(b' ');
                    body.extend_from_slice(&line.raw_content);
                    body.push(b'\n');
                    old_count += 1;
                    new_count += 1;
                    kept_previous_line = true;
                }
                DiffLineKind::Deletion => {
                    kept_previous_line = false;
                }
                DiffLineKind::Addition if selected => {
                    body.push(b'+');
                    body.extend_from_slice(&line.raw_content);
                    body.push(b'\n');
                    new_count += 1;
                    selected_change_count += 1;
                    kept_previous_line = true;
                }
                DiffLineKind::Addition if !reverse => {
                    kept_previous_line = false;
                }
                DiffLineKind::Addition => {
                    body.push(b' ');
                    body.extend_from_slice(&line.raw_content);
                    body.push(b'\n');
                    old_count += 1;
                    new_count += 1;
                    kept_previous_line = true;
                }
                DiffLineKind::NoNewline if kept_previous_line => {
                    body.extend_from_slice(b"\\ No newline at end of file\n");
                }
                DiffLineKind::NoNewline => {}
            }
        }
        if selected_change_count > changes_before {
            patch.extend_from_slice(
                format!(
                    "@@ -{},{} +{},{} @@\n",
                    hunk.old_start, old_count, hunk.new_start, new_count
                )
                .as_bytes(),
            );
            patch.extend_from_slice(&body);
        }
    }
    if selected_change_count == 0 {
        return Err(AppError::InvalidRequest(
            "Select at least one added or deleted line".to_owned(),
        ));
    }
    Ok(patch)
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
        .map_err(|_| AppError::InvalidRequest("File path is not valid UTF-8 on Windows".to_owned()))
}

fn parse_worktrees(output: &str, current_path: &Path) -> Vec<WorktreeSummary> {
    output
        .split("\0\0")
        .filter_map(|record| {
            let mut path = None;
            let mut head = None;
            let mut branch = None;
            let mut locked = false;
            let mut prunable = false;
            for field in record.split('\0').flat_map(str::lines) {
                if let Some(value) = field.strip_prefix("worktree ") {
                    path = Some(value.to_owned());
                } else if let Some(value) = field.strip_prefix("HEAD ") {
                    head = Some(value.to_owned());
                } else if let Some(value) = field.strip_prefix("branch ") {
                    branch = Some(value.trim_start_matches("refs/heads/").to_owned());
                } else if field == "locked" || field.starts_with("locked ") {
                    locked = true;
                } else if field == "prunable" || field.starts_with("prunable ") {
                    prunable = true;
                }
            }
            path.map(|path| WorktreeSummary {
                id: WorktreeId::from_canonical_path(
                    &fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path)),
                ),
                is_current: normalized_path(Path::new(&path)) == normalized_path(current_path),
                is_missing: !Path::new(&path).is_dir(),
                path,
                head,
                branch,
                is_locked: locked,
                is_prunable: prunable,
            })
        })
        .collect()
}

fn parse_submodules(output: &str, worktree_path: &Path) -> Vec<SubmoduleSummary> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            let state = line.chars().next()?;
            let initialized = state != '-';
            let remainder = if matches!(state, ' ' | '-' | '+' | 'U') {
                line.get(state.len_utf8()..)?
            } else {
                line
            };
            let (_, path_and_description) = remainder.split_once(char::is_whitespace)?;
            let path = path_and_description
                .rsplit_once(" (")
                .map_or(path_and_description, |(path, _)| path)
                .trim();
            if path.is_empty() {
                return None;
            }
            Some(SubmoduleSummary {
                path: path.to_owned(),
                absolute_path: worktree_path.join(path).to_string_lossy().into_owned(),
                initialized,
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
        items: refs,
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
        .collect()
}

fn parse_history_cursor(cursor: Option<&str>) -> Result<usize, AppError> {
    match cursor {
        None | Some("") => Ok(0),
        Some(cursor) => cursor
            .strip_prefix("offset:")
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| AppError::InvalidRequest("History cursor is invalid".to_owned())),
    }
}

fn parse_references(output: &[u8]) -> Result<Vec<GitReference>, AppError> {
    output
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.iter().all(u8::is_ascii_whitespace))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let fields: Vec<&[u8]> = record.split(|byte| *byte == 0).collect();
            if fields.len() < 5 {
                return Err(AppError::InvalidGitOutput(
                    "reference record has too few fields".to_owned(),
                ));
            }
            let text = |field: &[u8]| String::from_utf8_lossy(field).trim().to_owned();
            let full_name = text(fields[0]);
            let kind = if full_name.starts_with("refs/heads/") {
                ReferenceKind::LocalBranch
            } else if full_name.starts_with("refs/remotes/") {
                ReferenceKind::RemoteBranch
            } else {
                ReferenceKind::Tag
            };
            let tracking = text(fields[4]);
            let count = |label: &str| {
                tracking
                    .split([',', '[', ']'])
                    .map(str::trim)
                    .find_map(|part| {
                        part.strip_prefix(label)
                            .and_then(|value| value.trim().parse::<u64>().ok())
                    })
                    .unwrap_or(0)
            };
            let upstream = text(fields[3]);
            Ok(GitReference {
                full_name,
                short_name: text(fields[1]),
                oid: text(fields[2]),
                kind,
                upstream: (!upstream.is_empty()).then_some(upstream),
                ahead: count("ahead"),
                behind: count("behind"),
            })
        })
        .collect()
}

fn parse_reflog_entries(output: &[u8]) -> Result<Vec<ReflogEntry>, AppError> {
    output
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.iter().all(u8::is_ascii_whitespace))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
            if fields.len() < 9 {
                return Err(AppError::InvalidGitOutput(
                    "reflog record is incomplete".to_owned(),
                ));
            }
            let selector = String::from_utf8_lossy(fields[0]).trim().to_owned();
            let oid = String::from_utf8_lossy(fields[1]).trim().to_owned();
            validate_object_id(&oid)?;
            let authored_at = String::from_utf8_lossy(fields[6])
                .trim()
                .parse::<i64>()
                .map_err(|_| {
                    AppError::InvalidGitOutput("reflog timestamp is invalid".to_owned())
                })?;
            Ok(ReflogEntry {
                selector,
                oid,
                message: String::from_utf8_lossy(fields[2]).trim().to_owned(),
                parents: String::from_utf8_lossy(fields[3])
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
                author_name: String::from_utf8_lossy(fields[4]).trim().to_owned(),
                author_email: String::from_utf8_lossy(fields[5]).trim().to_owned(),
                authored_at,
                subject: String::from_utf8_lossy(fields[7]).trim().to_owned(),
                body: String::from_utf8_lossy(fields[8]).trim().to_owned(),
                reflog_only: false,
            })
        })
        .collect()
}

fn validate_object_id(oid: &str) -> Result<(), AppError> {
    if matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(AppError::InvalidRequest(
            "The selected commit ID is invalid".to_owned(),
        ))
    }
}

fn validate_compare_ref(
    service: &RepositoryService,
    repository: &RepositoryDescriptor,
    reference: &str,
) -> Result<Option<OsString>, AppError> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(AppError::InvalidRequest(
            "Comparison references must not be empty".to_owned(),
        ));
    }
    if reference.eq_ignore_ascii_case("WORKTREE") {
        return Ok(None);
    }
    if reference.starts_with('-') || reference.contains(['\0', '\r', '\n']) {
        return Err(AppError::InvalidRequest(
            "Comparison reference contains an unsafe character".to_owned(),
        ));
    }
    let revision = service
        .git_text(
            repository,
            [
                OsString::from("rev-parse"),
                OsString::from("--verify"),
                OsString::from("--end-of-options"),
                OsString::from(format!("{reference}^{{commit}}")),
            ],
        )?
        .trim()
        .to_owned();
    if revision.is_empty() {
        return Err(AppError::InvalidRequest(format!(
            "Comparison reference {reference} did not resolve to a commit"
        )));
    }
    Ok(Some(OsString::from(reference)))
}

fn validate_tool_name(tool: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(tool) = tool.map(str::trim).filter(|tool| !tool.is_empty()) else {
        return Ok(None);
    };
    if tool.starts_with('-')
        || tool.contains(['\0', '\r', '\n'])
        || tool.chars().any(char::is_whitespace)
    {
        return Err(AppError::InvalidRequest(
            "External tool name must not contain whitespace or control characters".to_owned(),
        ));
    }
    Ok(Some(tool.to_owned()))
}

#[derive(Debug, Deserialize)]
struct LfsLocksPayload {
    #[serde(default)]
    locks: Vec<LfsLockPayload>,
}

#[derive(Debug, Deserialize)]
struct LfsLockPayload {
    id: String,
    path: String,
    owner: LfsLockOwnerPayload,
    #[serde(default)]
    locked_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LfsLockOwnerPayload {
    #[serde(default)]
    name: String,
}

fn parse_lfs_file_status(line: &str) -> Option<LfsFileStatus> {
    let mut parts = line
        .splitn(3, char::is_whitespace)
        .filter(|part| !part.is_empty());
    let oid = parts.next()?.to_owned();
    let marker = parts.next()?;
    let path = parts.next()?.trim().to_owned();
    if path.is_empty() {
        return None;
    }
    let size = oid
        .strip_prefix("size:")
        .and_then(|value| value.parse::<u64>().ok());
    let oid = (!oid.starts_with("-") && !oid.starts_with("size:")).then_some(oid);
    Some(LfsFileStatus {
        path,
        oid,
        size,
        downloaded: marker == "*",
    })
}

fn validate_lfs_remote(remote: &str) -> Result<&str, AppError> {
    let remote = remote.trim();
    if remote.is_empty() || remote.starts_with('-') || remote.contains(['\0', '\r', '\n']) {
        return Err(AppError::InvalidRequest(
            "LFS remote name is invalid".to_owned(),
        ));
    }
    Ok(remote)
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_relative_file_path(path: &str) -> Result<String, AppError> {
    use std::path::Component;

    let path = path.trim();
    let candidate = Path::new(path);
    if path.is_empty()
        || path.contains(['\0', '\r', '\n'])
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::InvalidRequest(
            "Binary preview path must be a relative file path".to_owned(),
        ));
    }
    Ok(path.replace('\\', "/"))
}

fn image_mime_type(path: &str) -> Option<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => return None,
    };
    Some(mime.to_owned())
}

fn parse_interactive_rebase_commits(
    output: &[u8],
) -> Result<Vec<InteractiveRebaseCommit>, AppError> {
    output
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.iter().all(u8::is_ascii_whitespace))
        .map(|record| {
            let record = record.strip_prefix(b"\n").unwrap_or(record);
            let fields: Vec<&[u8]> = record.split(|byte| *byte == 0).collect();
            if fields.len() < 3 {
                return Err(AppError::InvalidGitOutput(
                    "Interactive rebase history record is incomplete".to_owned(),
                ));
            }
            let oid = String::from_utf8(fields[0].to_vec())
                .map_err(|error| AppError::InvalidGitOutput(error.to_string()))?;
            validate_object_id(&oid)?;
            let subject = String::from_utf8_lossy(fields[1]).into_owned();
            let parents = String::from_utf8_lossy(fields[2]);
            if parents.split_whitespace().count() > 1 {
                return Err(AppError::InvalidRequest(
                    "Interactive rebase does not yet support merge commits".to_owned(),
                ));
            }
            Ok(InteractiveRebaseCommit { oid, subject })
        })
        .collect()
}

fn validate_rebase_plan(
    preview: &InteractiveRebasePreview,
    items: &[InteractiveRebaseItem],
) -> Result<(), AppError> {
    if items.len() != preview.commits.len() {
        return Err(AppError::InvalidRequest(
            "The rebase plan must contain every commit exactly once".to_owned(),
        ));
    }
    let expected: HashSet<&str> = preview
        .commits
        .iter()
        .map(|commit| commit.oid.as_str())
        .collect();
    let actual: HashSet<&str> = items.iter().map(|item| item.oid.as_str()).collect();
    if actual.len() != items.len() || actual != expected {
        return Err(AppError::InvalidRequest(
            "The rebase plan contains an unknown or duplicate commit".to_owned(),
        ));
    }

    let mut has_previous = false;
    for item in items {
        if item.action == InteractiveRebaseAction::Reword {
            let summary = item.summary.as_deref().unwrap_or_default().trim();
            if summary.is_empty() || summary.contains(['\r', '\n']) {
                return Err(AppError::InvalidRequest(
                    "A reword action requires a single-line commit summary".to_owned(),
                ));
            }
        }
        match item.action {
            InteractiveRebaseAction::Squash | InteractiveRebaseAction::Fixup if !has_previous => {
                return Err(AppError::InvalidRequest(
                    "Squash and fixup require a preceding commit".to_owned(),
                ));
            }
            InteractiveRebaseAction::Drop => {}
            _ => has_previous = true,
        }
    }
    if !has_previous {
        return Err(AppError::InvalidRequest(
            "The rebase plan cannot drop every commit".to_owned(),
        ));
    }
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn validate_identity_value(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.contains(['\0', '\r', '\n']) {
        return Err(AppError::InvalidRequest(
            "Git identity values cannot contain line breaks".to_owned(),
        ));
    }
    Ok(Some(value.to_owned()))
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

    use super::{
        GitVersion, InteractiveRebaseAction, InteractiveRebaseItem, InteractiveRebasePreview,
        ReferenceKind, parse_git_version, parse_interactive_rebase_commits, parse_references,
        parse_stashes, parse_submodules, parse_worktrees, summarize_refs, validate_rebase_plan,
    };

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
            "worktree C:/repo\0HEAD abc\0branch refs/heads/main\0\0worktree C:/other\0HEAD def\0detached\0locked reason\0prunable stale\0\0",
            Path::new("C:/repo"),
        );
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_current);
        assert!(worktrees[1].is_locked);
        assert!(worktrees[1].is_prunable);
        assert!(worktrees[1].is_missing);

        let refs = summarize_refs("main\nfeature\none\ntwo\nthree\nfour\n");
        assert_eq!(refs.total, 6);
        assert_eq!(refs.items.len(), 6);

        let stashes = parse_stashes(b"stash@{0}\0WIP one\0\nstash@{1}\0WIP two\0\n");
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].reference, "stash@{0}");
    }

    #[test]
    fn parses_initialized_and_uninitialized_submodules() {
        let submodules = parse_submodules(
            " 1111111111111111111111111111111111111111 vendor/ready (heads/main)\n-2222222222222222222222222222222222222222 vendor/not ready\n+3333333333333333333333333333333333333333 vendor/changed (v1.0-2-g3333333)\n",
            Path::new("C:/repo"),
        );

        assert_eq!(submodules.len(), 3);
        assert_eq!(submodules[0].path, "vendor/ready");
        assert!(submodules[0].initialized);
        assert_eq!(submodules[1].path, "vendor/not ready");
        assert!(!submodules[1].initialized);
        assert_eq!(
            submodules[2].absolute_path,
            Path::new("C:/repo")
                .join("vendor/changed")
                .to_string_lossy()
        );
    }

    #[test]
    fn parses_detailed_refs_and_tracking_counts() {
        let refs =
            parse_references(b"refs/heads/main\0main\0abc\0origin/main\0[ahead 2, behind 3]\0\x1e")
                .expect("valid refs");
        assert_eq!(refs[0].kind, ReferenceKind::LocalBranch);
        assert_eq!(refs[0].ahead, 2);
        assert_eq!(refs[0].behind, 3);
    }

    #[test]
    fn parses_linear_interactive_rebase_commits() {
        let first = "1".repeat(40);
        let second = "2".repeat(40);
        let output = format!(
            "{first}\0First\0{}\x1e\n{second}\0Second\0{first}\x1e\n",
            "0".repeat(40)
        );
        let commits = parse_interactive_rebase_commits(output.as_bytes()).expect("linear history");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[1].subject, "Second");
    }

    #[test]
    fn rejects_merge_commits_from_interactive_rebase() {
        let oid = "1".repeat(40);
        let parents = format!("{} {}", "2".repeat(40), "3".repeat(40));
        let output = format!("{oid}\0Merge\0{parents}\x1e\n");
        assert!(parse_interactive_rebase_commits(output.as_bytes()).is_err());
    }

    #[test]
    fn validates_reordered_rebase_plan_and_rejects_leading_squash() {
        let first = "1".repeat(40);
        let second = "2".repeat(40);
        let preview = InteractiveRebasePreview {
            base_oid: "0".repeat(40),
            head_oid: second.clone(),
            branch: "main".to_owned(),
            commits: vec![
                super::InteractiveRebaseCommit {
                    oid: first.clone(),
                    subject: "First".to_owned(),
                },
                super::InteractiveRebaseCommit {
                    oid: second.clone(),
                    subject: "Second".to_owned(),
                },
            ],
        };
        let reordered = vec![
            InteractiveRebaseItem {
                oid: second.clone(),
                action: InteractiveRebaseAction::Pick,
                summary: None,
                description: None,
            },
            InteractiveRebaseItem {
                oid: first.clone(),
                action: InteractiveRebaseAction::Fixup,
                summary: None,
                description: None,
            },
        ];
        assert!(validate_rebase_plan(&preview, &reordered).is_ok());

        let invalid = vec![
            InteractiveRebaseItem {
                oid: first,
                action: InteractiveRebaseAction::Squash,
                summary: None,
                description: None,
            },
            InteractiveRebaseItem {
                oid: second,
                action: InteractiveRebaseAction::Pick,
                summary: None,
                description: None,
            },
        ];
        assert!(validate_rebase_plan(&preview, &invalid).is_err());
    }

    #[test]
    fn maps_reword_and_edit_to_pausing_todo_commands() {
        assert_eq!(InteractiveRebaseAction::Reword.as_todo_command(), "edit");
        assert_eq!(InteractiveRebaseAction::Edit.as_todo_command(), "edit");
    }

    #[test]
    fn requires_a_valid_summary_for_reword() {
        let oid = "1".repeat(40);
        let preview = InteractiveRebasePreview {
            base_oid: "0".repeat(40),
            head_oid: oid.clone(),
            branch: "main".to_owned(),
            commits: vec![super::InteractiveRebaseCommit {
                oid: oid.clone(),
                subject: "Original".to_owned(),
            }],
        };
        let invalid = [InteractiveRebaseItem {
            oid,
            action: InteractiveRebaseAction::Reword,
            summary: Some("".to_owned()),
            description: None,
        }];
        assert!(validate_rebase_plan(&preview, &invalid).is_err());
    }
}
