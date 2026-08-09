use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictFile {
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub segments: Vec<ConflictSegment>,
    pub worktree_oid: String,
    pub editable: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictSegment {
    Common {
        content: String,
    },
    Conflict {
        index: usize,
        ours: String,
        base: Option<String>,
        theirs: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConflictStageEntry {
    mode: String,
    oid: String,
}

impl RepositoryService {
    pub fn conflict_file(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
    ) -> Result<ConflictFile, AppError> {
        let stages = self.conflict_stage_entries(repository, path)?;
        let worktree_path = repository.worktree_path.join(path_argument(path)?);
        let worktree = fs::read(&worktree_path).map_err(|error| AppError::GitFailed {
            diagnostic_id: Uuid::new_v4(),
            detail: error.to_string(),
        })?;
        let worktree_oid = self.hash_bytes(repository, &worktree)?;

        let regular_file = stages
            .values()
            .all(|entry| entry.mode == "100644" || entry.mode == "100755");
        let text = String::from_utf8(worktree).ok();
        let segments = text
            .as_deref()
            .and_then(parse_conflict_segments)
            .unwrap_or_default();
        let editable = regular_file
            && !segments.is_empty()
            && segments
                .iter()
                .any(|segment| matches!(segment, ConflictSegment::Conflict { .. }));
        let unavailable_reason = (!editable).then(|| {
            if !regular_file {
                "This conflict is binary, a symlink, or another non-regular file.".to_owned()
            } else if text.is_none() {
                "This file is not valid UTF-8 and cannot be edited safely in the built-in editor."
                    .to_owned()
            } else {
                "Git did not leave conflict markers in the worktree for hunk editing.".to_owned()
            }
        });

        Ok(ConflictFile {
            base: self.conflict_stage_text(repository, stages.get(&1))?,
            ours: self.conflict_stage_text(repository, stages.get(&2))?,
            theirs: self.conflict_stage_text(repository, stages.get(&3))?,
            segments,
            worktree_oid,
            editable,
            unavailable_reason,
        })
    }

    pub fn apply_conflict_content(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        expected_worktree_oid: &str,
        content: &str,
    ) -> Result<(), AppError> {
        if content.len() > 4 * 1024 * 1024 {
            return Err(AppError::InvalidRequest(
                "Conflict result exceeds the 4 MiB editor limit".to_owned(),
            ));
        }
        if content.lines().any(|line| {
            line.starts_with("<<<<<<< ")
                || line.starts_with("||||||| ")
                || line == "======="
                || line.starts_with(">>>>>>> ")
        }) {
            return Err(AppError::InvalidRequest(
                "Resolve every conflict marker before applying the result".to_owned(),
            ));
        }

        self.conflict_stage_entries(repository, path)?;
        let path_argument = path_argument(path)?;
        let worktree_path = repository.worktree_path.join(&path_argument);
        let original = fs::read(&worktree_path).map_err(|error| AppError::GitFailed {
            diagnostic_id: Uuid::new_v4(),
            detail: error.to_string(),
        })?;
        let actual_oid = self.hash_bytes(repository, &original)?;
        if actual_oid != expected_worktree_oid {
            return Err(AppError::InvalidRequest(
                "The conflicted file changed after the editor was opened; reload before applying"
                    .to_owned(),
            ));
        }

        fs::write(&worktree_path, content.as_bytes()).map_err(|error| AppError::GitFailed {
            diagnostic_id: Uuid::new_v4(),
            detail: error.to_string(),
        })?;
        if let Err(error) = self.workspace_git_unit(
            repository,
            [OsString::from("add"), OsString::from("--"), path_argument],
        ) {
            let _ = fs::write(&worktree_path, original);
            return Err(error);
        }
        Ok(())
    }
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

    fn conflict_stage_entries(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
    ) -> Result<BTreeMap<u8, ConflictStageEntry>, AppError> {
        let path_argument = path_argument(path)?;
        let output = ensure_success(self.workspace_git(
            repository,
            [
                OsString::from("ls-files"),
                OsString::from("-u"),
                OsString::from("-z"),
                OsString::from("--"),
                path_argument,
            ],
        )?)?;
        let mut stages = BTreeMap::new();
        for record in output.stdout.split(|byte| *byte == 0) {
            if record.is_empty() {
                continue;
            }
            let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
                continue;
            };
            if &record[tab + 1..] != path {
                continue;
            }
            let header = String::from_utf8_lossy(&record[..tab]);
            let mut fields = header.split_ascii_whitespace();
            let (Some(mode), Some(oid), Some(stage)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let Ok(stage) = stage.parse::<u8>() else {
                continue;
            };
            stages.insert(
                stage,
                ConflictStageEntry {
                    mode: mode.to_owned(),
                    oid: oid.to_owned(),
                },
            );
        }
        if stages.is_empty() {
            return Err(AppError::InvalidRequest(
                "The selected file is no longer conflicted".to_owned(),
            ));
        }
        Ok(stages)
    }

    fn conflict_stage_text(
        &self,
        repository: &RepositoryDescriptor,
        entry: Option<&ConflictStageEntry>,
    ) -> Result<Option<String>, AppError> {
        let Some(entry) = entry else {
            return Ok(None);
        };
        if entry.mode != "100644" && entry.mode != "100755" {
            return Ok(None);
        }
        let output = ensure_success(
            self.workspace_git(repository, ["cat-file", "blob", entry.oid.as_str()])?,
        )?;
        Ok(String::from_utf8(output.stdout).ok())
    }

    fn hash_bytes(
        &self,
        repository: &RepositoryDescriptor,
        bytes: &[u8],
    ) -> Result<String, AppError> {
        let mut request = GitRequest::new(["hash-object", "--stdin"]);
        request.working_directory = Some(repository.worktree_path.clone());
        request.timeout = Duration::from_secs(10);
        request.stdin = Some(bytes.to_vec());
        let output = self
            .executor
            .execute(request, &CancellationToken::default())
            .map_err(map_execution_error)?;
        let output = ensure_success(output)?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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

fn parse_conflict_segments(content: &str) -> Option<Vec<ConflictSegment>> {
    let lines = content.split_inclusive('\n').collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut common = String::new();
    let mut index = 0;
    let mut cursor = 0;

    while cursor < lines.len() {
        if !marker_line(lines[cursor]).starts_with("<<<<<<< ") {
            common.push_str(lines[cursor]);
            cursor += 1;
            continue;
        }
        if !common.is_empty() {
            segments.push(ConflictSegment::Common {
                content: std::mem::take(&mut common),
            });
        }
        cursor += 1;
        let mut ours = String::new();
        while cursor < lines.len()
            && !marker_line(lines[cursor]).starts_with("||||||| ")
            && marker_line(lines[cursor]) != "======="
        {
            ours.push_str(lines[cursor]);
            cursor += 1;
        }
        let mut base = None;
        if cursor < lines.len() && marker_line(lines[cursor]).starts_with("||||||| ") {
            cursor += 1;
            let mut base_content = String::new();
            while cursor < lines.len() && marker_line(lines[cursor]) != "=======" {
                base_content.push_str(lines[cursor]);
                cursor += 1;
            }
            base = Some(base_content);
        }
        if cursor >= lines.len() || marker_line(lines[cursor]) != "=======" {
            return None;
        }
        cursor += 1;
        let mut theirs = String::new();
        while cursor < lines.len() && !marker_line(lines[cursor]).starts_with(">>>>>>> ") {
            theirs.push_str(lines[cursor]);
            cursor += 1;
        }
        if cursor >= lines.len() {
            return None;
        }
        cursor += 1;
        segments.push(ConflictSegment::Conflict {
            index,
            ours,
            base,
            theirs,
        });
        index += 1;
    }
    if !common.is_empty() {
        segments.push(ConflictSegment::Common { content: common });
    }
    (index > 0).then_some(segments)
}

fn marker_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
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
