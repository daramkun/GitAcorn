use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git_cli::{CancellationToken, GitExecutionError, GitExecutor, GitOutput, GitRequest};
use git_domain::{
    DiffDocument, DiffLineKind, HistoryPage, RepoId, RepositoryDescriptor, RepositorySnapshot,
    WorktreeId, parse_history_records, parse_porcelain_v2, parse_unified_diff,
};
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySidebar {
    pub worktrees: Vec<WorktreeSummary>,
    pub branches: RefSummary,
    pub remote_branches: RefSummary,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTagSummary {
    pub remote: String,
    pub name: String,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
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
            branches: summarize_refs(&branches),
            remote_branches: summarize_refs(&remote_branches),
            tags: summarize_refs(&tags),
            stashes: parse_stashes(&stashes),
        })
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

        let checkout_result = if is_tag {
            self.git_unit(repository, ["switch", "--detach", name])
        } else if is_remote {
            self.checkout_remote_branch(repository, name, &references)
        } else {
            self.git_unit(repository, ["switch", name])
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
    ) -> Result<(), AppError> {
        if let Some(local) = references.iter().find(|reference| {
            reference.kind == ReferenceKind::LocalBranch
                && reference.upstream.as_deref() == Some(name)
        }) {
            return self.git_unit(repository, ["switch", &local.short_name]);
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
        self.git_unit(repository, ["switch", "--track", name])
    }

    pub fn delete_branch(
        &self,
        repository: &RepositoryDescriptor,
        name: &str,
    ) -> Result<(), AppError> {
        let name = self.validate_branch_name(repository, name)?;
        self.git_unit(repository, ["branch", "--delete", &name])
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

    pub fn diff(
        &self,
        repository: &RepositoryDescriptor,
        path: &[u8],
        target: DiffTarget,
    ) -> Result<DiffDocument, AppError> {
        let output = self.diff_bytes(repository, path, target, 3)?;
        parse_unified_diff(&output).map_err(|error| AppError::InvalidGitOutput(error.to_string()))
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
        GitVersion, ReferenceKind, parse_git_version, parse_references, parse_stashes,
        parse_worktrees, summarize_refs,
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
            "worktree C:/repo\0HEAD abc\0branch refs/heads/main\0\0worktree C:/other\0HEAD def\0detached\0locked reason\0\0",
            Path::new("C:/repo"),
        );
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_current);
        assert!(worktrees[1].is_locked);

        let refs = summarize_refs("main\nfeature\none\ntwo\nthree\nfour\n");
        assert_eq!(refs.total, 6);
        assert_eq!(refs.items.len(), 6);

        let stashes = parse_stashes(b"stash@{0}\0WIP one\0\nstash@{1}\0WIP two\0\n");
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].reference, "stash@{0}");
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
}
