use app_core::{
    BranchRequest, CommitRequest, DiffTarget, HistoryFilter, PatchSelection, ReferenceKind,
    RepositoryService,
};
use git_domain::{DiffLineKind, HeadState};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use test_support::TestRepository;

#[test]
fn discovers_repository_and_reads_real_status() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "modified\n");
    fixture.write("staged file.txt", "staged\n");
    fixture.git(["add", "staged file.txt"]);
    fixture.write("한글 파일.txt", "untracked\n");

    let service = RepositoryService::default();
    let repository = service
        .discover(fixture.path())
        .expect("discover repository");
    let snapshot = service.snapshot(&repository, 1).expect("read status");

    assert_eq!(snapshot.revision, 1);
    assert!(matches!(
        snapshot.status.head,
        HeadState::Branch { ref name, .. } if name == "main"
    ));
    assert!(
        snapshot
            .status
            .changes
            .iter()
            .any(|change| change.path == b"tracked.txt" && change.worktree_status == b'M')
    );
    assert!(
        snapshot
            .status
            .changes
            .iter()
            .any(|change| change.path == b"staged file.txt" && change.index_status == b'A')
    );
    assert!(
        snapshot
            .status
            .changes
            .iter()
            .any(|change| String::from_utf8_lossy(&change.path) == "한글 파일.txt")
    );
}

#[test]
fn reads_real_repository_sidebar_context() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "feature/sidebar"]);
    fixture.git(["tag", "v0.1.0"]);
    fixture.write("stash-me.txt", "stashed\n");
    fixture.git(["stash", "push", "-u", "-m", "sidebar fixture"]);

    let service = RepositoryService::default();
    let repository = service
        .discover(fixture.path())
        .expect("discover repository");
    let sidebar = service.sidebar(&repository).expect("read sidebar");

    assert_eq!(sidebar.worktrees.len(), 1);
    assert!(
        sidebar.worktrees[0].is_current,
        "worktree {:?} did not match {:?}",
        sidebar.worktrees[0].path, repository.worktree_path
    );
    assert!(sidebar.branches.items.iter().any(|name| name == "main"));
    assert!(
        sidebar
            .branches
            .items
            .iter()
            .any(|name| name == "feature/sidebar")
    );
    assert_eq!(sidebar.tags.items, ["v0.1.0"]);
    assert_eq!(sidebar.stashes[0].reference, "stash@{0}");
    assert!(sidebar.stashes[0].message.contains("sidebar fixture"));
}

#[test]
fn pages_history_and_manages_branches_without_implicit_checkout() {
    let fixture = TestRepository::init();
    for index in 1..=4 {
        fixture.write("tracked.txt", &format!("commit {index}\n"));
        fixture.git(["add", "tracked.txt"]);
        fixture.git(["commit", "-m", &format!("history {index}")]);
    }
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    let first = service
        .history(
            &repository,
            &HistoryFilter {
                limit: 2,
                ..HistoryFilter::default()
            },
        )
        .expect("first history page");
    assert_eq!(first.commits.len(), 2);
    assert_eq!(first.commits[0].subject, "history 4");
    let second = service
        .history(
            &repository,
            &HistoryFilter {
                cursor: first.next_cursor,
                limit: 2,
                ..HistoryFilter::default()
            },
        )
        .expect("second history page");
    assert_eq!(second.commits[0].subject, "history 2");

    service
        .create_branch(
            &repository,
            &BranchRequest {
                name: "feature/history".to_owned(),
                start_point: None,
            },
        )
        .expect("create branch");
    assert_eq!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim(),
        "main",
        "creating or selecting a ref must not checkout"
    );
    let refs = service.references(&repository).expect("refs");
    assert!(refs.iter().any(|reference| {
        reference.kind == ReferenceKind::LocalBranch && reference.short_name == "feature/history"
    }));
    service
        .checkout_branch(&repository, "feature/history", false, false, false)
        .expect("explicit checkout");
    assert_eq!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim(),
        "feature/history"
    );
}

#[test]
fn renames_rebases_and_manages_tags_for_local_branches() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    fixture.write("tracked.txt", "main change\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "main change"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .rename_branch(&repository, "topic", "feature/renamed", false)
        .expect("rename local branch");
    service
        .create_tag(&repository, "v1.0.0", "feature/renamed")
        .expect("create tag at branch");
    service
        .rebase_onto(&repository, "feature/renamed")
        .expect("rebase current branch");

    let references = service.references(&repository).expect("read references");
    assert!(references.iter().any(|reference| {
        reference.kind == ReferenceKind::LocalBranch && reference.short_name == "feature/renamed"
    }));
    assert!(references.iter().any(|reference| {
        reference.kind == ReferenceKind::Tag && reference.short_name == "v1.0.0"
    }));

    service
        .delete_tag(&repository, "v1.0.0")
        .expect("delete tag");
    service
        .delete_branch(&repository, "feature/renamed")
        .expect("delete merged branch");
}

#[test]
fn optionally_renames_the_upstream_branch_with_the_local_branch() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    let bare = tempfile::tempdir().expect("bare remote directory");
    let initialized = Command::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .output()
        .expect("initialize bare remote");
    assert!(initialized.status.success());
    fixture.git([
        "remote",
        "add",
        "origin",
        bare.path().to_str().expect("UTF-8 remote path"),
    ]);
    fixture.git(["push", "--set-upstream", "origin", "topic"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .rename_branch(&repository, "topic", "topic-renamed", true)
        .expect("rename local and upstream branches");

    let references = service.references(&repository).expect("read references");
    let renamed = references
        .iter()
        .find(|reference| {
            reference.kind == ReferenceKind::LocalBranch && reference.short_name == "topic-renamed"
        })
        .expect("renamed local branch");
    assert_eq!(renamed.upstream.as_deref(), Some("origin/topic-renamed"));
    let remote_refs = String::from_utf8(fixture.git_output(["ls-remote", "--heads", "origin"]))
        .expect("UTF-8 remote refs");
    assert!(remote_refs.contains("refs/heads/topic-renamed"));
    assert!(!remote_refs.contains("refs/heads/topic\n"));
}

#[test]
fn checkout_can_stash_and_reapply_staged_and_untracked_changes() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    fixture.write("tracked.txt", "staged change\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.write("untracked.txt", "untracked change\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .checkout_branch(&repository, "topic", false, false, true)
        .expect("checkout with automatic stash");

    assert_eq!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim(),
        "topic"
    );
    assert_eq!(
        String::from_utf8(fixture.git_output(["diff", "--cached", "--name-only"]))
            .expect("staged paths")
            .trim(),
        "tracked.txt"
    );
    assert!(fixture.path().join("untracked.txt").is_file());
    assert!(
        String::from_utf8(fixture.git_output(["stash", "list"]))
            .expect("stash list")
            .trim()
            .is_empty()
    );
}

#[test]
fn checkout_remote_branch_creates_a_tracking_local_branch() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    let bare = tempfile::tempdir().expect("bare remote directory");
    let initialized = Command::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .output()
        .expect("initialize bare remote");
    assert!(initialized.status.success());
    fixture.git([
        "remote",
        "add",
        "origin",
        bare.path().to_str().expect("UTF-8 remote path"),
    ]);
    fixture.git(["push", "--set-upstream", "origin", "topic"]);
    fixture.git(["branch", "--delete", "topic"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .checkout_branch(&repository, "origin/topic", true, false, false)
        .expect("checkout remote branch");

    assert_eq!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim(),
        "topic"
    );
    let upstream = String::from_utf8(fixture.git_output([
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ]))
    .expect("upstream text");
    assert_eq!(upstream.trim(), "origin/topic");
}

#[test]
fn checkout_tag_enters_detached_head_at_the_tagged_commit() {
    let fixture = TestRepository::init();
    fixture.git(["tag", "v1.0.0"]);
    let tagged_oid =
        String::from_utf8(fixture.git_output(["rev-parse", "v1.0.0"])).expect("tagged oid");
    fixture.write("tracked.txt", "newer main\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "newer main"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .checkout_branch(&repository, "v1.0.0", false, true, false)
        .expect("checkout tag");

    assert!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim()
            .is_empty()
    );
    let head_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"])).expect("head oid");
    assert_eq!(head_oid.trim(), tagged_oid.trim());
}

#[test]
fn merge_enters_conflict_state_without_losing_the_repository_snapshot() {
    let fixture = TestRepository::init();
    fixture.git(["switch", "-c", "topic"]);
    fixture.write("tracked.txt", "topic\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "topic change"]);
    fixture.git(["switch", "main"]);
    fixture.write("tracked.txt", "main\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "main change"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .merge_reference(&repository, "topic")
        .expect("conflicted merge is a valid operation state");
    let snapshot = service.snapshot(&repository, 2).expect("conflict snapshot");

    assert!(
        snapshot
            .status
            .changes
            .iter()
            .any(|change| change.is_conflict)
    );
    assert!(repository.git_dir.join("MERGE_HEAD").is_file());
}

#[test]
#[ignore = "100k-commit performance fixture; run at milestone and release gates"]
fn first_history_page_meets_the_large_repository_target() {
    let directory = tempfile::tempdir().expect("large fixture directory");
    let init = Command::new("git")
        .current_dir(directory.path())
        .args(["init", "-b", "main"])
        .output()
        .expect("initialize large fixture");
    assert!(init.status.success());

    let mut import = Command::new("git")
        .current_dir(directory.path())
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("start fast-import");
    let stdin = import.stdin.as_mut().expect("fast-import stdin");
    stdin
        .write_all(b"blob\nmark :1\ndata 8\ninitial\n")
        .expect("write blob");
    for index in 0..100_000_u32 {
        writeln!(
            stdin,
            "commit refs/heads/main\nmark :{}\nauthor Fixture <fixture@gitacorn.local> {} +0000\ncommitter Fixture <fixture@gitacorn.local> {} +0000\ndata {}\ncommit {}",
            index + 2,
            1_700_000_000_u64 + index as u64,
            1_700_000_000_u64 + index as u64,
            format!("commit {index}").len(),
            index,
        )
        .expect("write commit");
        if index == 0 {
            writeln!(stdin, "M 100644 :1 tracked.txt").expect("write initial tree");
        } else {
            writeln!(stdin, "from :{}", index + 1).expect("write parent");
        }
        writeln!(stdin).expect("finish commit");
    }
    drop(import.stdin.take());
    assert!(import.wait().expect("wait for fast-import").success());

    let service = RepositoryService::default();
    let repository = service.discover(directory.path()).expect("discover");
    let started = Instant::now();
    let page = service
        .history(
            &repository,
            &HistoryFilter {
                limit: 100,
                ..HistoryFilter::default()
            },
        )
        .expect("first history page");
    let elapsed = started.elapsed();

    assert_eq!(page.commits.len(), 100);
    assert!(page.next_cursor.is_some());
    assert!(
        elapsed < Duration::from_secs(1),
        "first 100k history page took {elapsed:?}"
    );
}

#[test]
fn assigns_one_repository_id_and_distinct_ids_to_linked_worktrees() {
    let fixture = TestRepository::init();
    let linked_root = tempfile::tempdir().expect("create linked worktree parent");
    let linked_path = linked_root.path().join("feature-worktree");
    fixture.git([
        "worktree",
        "add",
        "-b",
        "feature/worktree-id",
        linked_path.to_str().expect("UTF-8 fixture path"),
    ]);

    let service = RepositoryService::default();
    let primary = service
        .discover(fixture.path())
        .expect("discover primary worktree");
    let linked = service
        .discover(&linked_path)
        .expect("discover linked worktree");
    let sidebar = service.sidebar(&primary).expect("read worktrees");

    assert_eq!(primary.id, linked.id);
    assert_ne!(primary.worktree_id, linked.worktree_id);
    assert_eq!(sidebar.worktrees.len(), 2);
    assert!(
        sidebar
            .worktrees
            .iter()
            .any(|worktree| worktree.id == primary.worktree_id && worktree.is_current)
    );
    assert!(
        sidebar
            .worktrees
            .iter()
            .any(|worktree| worktree.id == linked.worktree_id && !worktree.is_current)
    );
}

#[test]
fn stages_selected_lines_and_unstages_them_without_touching_other_changes() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "first\nsecond\nthird\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "baseline"]);
    fixture.write("tracked.txt", "first\nSECOND\nthird\nfourth\n");

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let diff = service
        .diff(&repository, b"tracked.txt", DiffTarget::Unstaged)
        .expect("unstaged diff");
    let hunk = &diff.files[0].hunks[0];
    let selected = hunk
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (matches!(line.kind, DiffLineKind::Addition | DiffLineKind::Deletion)
                && line.content != "fourth")
                .then_some(index)
        })
        .collect();

    service
        .apply_selection(
            &repository,
            b"tracked.txt",
            DiffTarget::Unstaged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: selected,
            }],
        )
        .expect("stage selected replacement");

    let staged = String::from_utf8(fixture.git_output([
        "diff",
        "--cached",
        "--no-color",
        "--",
        "tracked.txt",
    ]))
    .expect("UTF-8 staged diff");
    let unstaged =
        String::from_utf8(fixture.git_output(["diff", "--no-color", "--", "tracked.txt"]))
            .expect("UTF-8 unstaged diff");
    assert!(staged.contains("SECOND"));
    assert!(!staged.contains("fourth"));
    assert!(unstaged.contains("fourth"));

    let staged_diff = service
        .diff(&repository, b"tracked.txt", DiffTarget::Staged)
        .expect("staged diff");
    let staged_hunk = &staged_diff.files[0].hunks[0];
    let staged_lines = staged_hunk
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.kind.ne(&DiffLineKind::Context).then_some(index))
        .collect();
    service
        .apply_selection(
            &repository,
            b"tracked.txt",
            DiffTarget::Staged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: staged_lines,
            }],
        )
        .expect("unstage selected replacement");

    assert!(
        fixture
            .git_output(["diff", "--cached", "--", "tracked.txt"])
            .is_empty()
    );
    let snapshot = service.snapshot(&repository, 2).expect("status");
    let change = snapshot
        .status
        .changes
        .iter()
        .find(|change| change.path == b"tracked.txt")
        .expect("tracked change");
    assert_eq!(change.index_status, b'.');
    assert_eq!(change.worktree_status, b'M');
}

#[test]
fn stages_selected_lines_in_a_crlf_file() {
    let fixture = TestRepository::init();
    fixture.git(["config", "core.autocrlf", "false"]);
    fixture.write("crlf.h", "first\r\nsecond\r\n");
    fixture.git(["add", "crlf.h"]);
    fixture.git(["commit", "-m", "add CRLF file"]);
    fixture.write("crlf.h", "FIRST\r\nsecond\r\nthird\r\n");

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let diff = service
        .diff(&repository, b"crlf.h", DiffTarget::Unstaged)
        .expect("unstaged CRLF diff");
    let selected = diff.files[0].hunks[0]
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (matches!(line.kind, DiffLineKind::Addition | DiffLineKind::Deletion)
                && line.content != "third")
                .then_some(index)
        })
        .collect();

    service
        .apply_selection(
            &repository,
            b"crlf.h",
            DiffTarget::Unstaged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: selected,
            }],
        )
        .expect("stage selected CRLF replacement");

    assert_eq!(
        fixture.git_output(["show", ":crlf.h"]),
        b"FIRST\r\nsecond\r\n"
    );
    assert!(
        fixture
            .git_output(["diff", "--", "crlf.h"])
            .windows(b"+third\r".len())
            .any(|window| window == b"+third\r")
    );
}

#[test]
fn unstages_each_side_of_a_staged_crlf_replacement_independently() {
    let fixture = TestRepository::init();
    fixture.git(["config", "core.autocrlf", "false"]);
    fixture.write("crlf.h", "first\r\nsecond\r\n");
    fixture.git(["add", "crlf.h"]);
    fixture.git(["commit", "-m", "add CRLF file"]);
    fixture.write("crlf.h", "FIRST\r\nsecond\r\n");
    fixture.git(["add", "crlf.h"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let staged_diff = service
        .diff(&repository, b"crlf.h", DiffTarget::Staged)
        .expect("staged CRLF diff");
    let addition = staged_diff.files[0].hunks[0]
        .lines
        .iter()
        .position(|line| line.kind == DiffLineKind::Addition)
        .expect("staged addition");

    service
        .apply_selection(
            &repository,
            b"crlf.h",
            DiffTarget::Staged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: vec![addition],
            }],
        )
        .expect("unstage only the added replacement line");
    assert_eq!(fixture.git_output(["show", ":crlf.h"]), b"second\r\n");

    let staged_diff = service
        .diff(&repository, b"crlf.h", DiffTarget::Staged)
        .expect("remaining staged CRLF diff");
    let deletion = staged_diff.files[0].hunks[0]
        .lines
        .iter()
        .position(|line| line.kind == DiffLineKind::Deletion)
        .expect("staged deletion");

    service
        .apply_selection(
            &repository,
            b"crlf.h",
            DiffTarget::Staged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: vec![deletion],
            }],
        )
        .expect("unstage only the deleted replacement line");
    assert_eq!(
        fixture.git_output(["show", ":crlf.h"]),
        b"first\r\nsecond\r\n"
    );
    assert!(fixture.git_output(["diff", "--cached"]).is_empty());
}

#[test]
fn rejects_an_invalid_patch_before_changing_the_index() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "changed\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    let result = service.apply_selection(
        &repository,
        b"tracked.txt",
        DiffTarget::Unstaged,
        &[PatchSelection {
            hunk_index: 99,
            line_indices: Vec::new(),
        }],
    );

    assert!(result.is_err());
    assert!(fixture.git_output(["diff", "--cached"]).is_empty());
}

#[test]
fn stages_commits_and_discards_with_real_git() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "ready to commit\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .stage_paths(&repository, &[b"tracked.txt".to_vec()])
        .expect("stage file");
    service
        .commit(
            &repository,
            &CommitRequest {
                summary: "Commit from GitAcorn".to_owned(),
                description: "M3 integration flow".to_owned(),
                amend: false,
            },
        )
        .expect("commit");
    assert_eq!(
        String::from_utf8(fixture.git_output(["log", "-1", "--pretty=%s"]))
            .expect("UTF-8 subject")
            .trim(),
        "Commit from GitAcorn"
    );

    fixture.write("tracked.txt", "amended content\n");
    service
        .stage_paths(&repository, &[b"tracked.txt".to_vec()])
        .expect("stage amend");
    service
        .commit(
            &repository,
            &CommitRequest {
                summary: "Amended from GitAcorn".to_owned(),
                description: String::new(),
                amend: true,
            },
        )
        .expect("amend commit");
    assert_eq!(
        String::from_utf8(fixture.git_output(["log", "-1", "--pretty=%s"]))
            .expect("UTF-8 amended subject")
            .trim(),
        "Amended from GitAcorn"
    );
    assert_eq!(
        String::from_utf8(fixture.git_output(["rev-list", "--count", "HEAD"]))
            .expect("UTF-8 commit count")
            .trim(),
        "2"
    );

    fixture.write("tracked.txt", "discard me\n");
    service
        .discard_path(&repository, b"tracked.txt", false)
        .expect("discard tracked change");
    assert!(
        service
            .snapshot(&repository, 3)
            .expect("clean status")
            .status
            .changes
            .is_empty()
    );
}

#[test]
fn renders_and_partially_stages_an_untracked_file() {
    let fixture = TestRepository::init();
    fixture.write("new file.txt", "one\ntwo\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let diff = service
        .diff(&repository, b"new file.txt", DiffTarget::Unstaged)
        .expect("untracked diff");
    let hunk = &diff.files[0].hunks[0];
    let first_addition = hunk
        .lines
        .iter()
        .position(|line| line.kind == DiffLineKind::Addition && line.content == "one")
        .expect("first addition");

    service
        .apply_selection(
            &repository,
            b"new file.txt",
            DiffTarget::Unstaged,
            &[PatchSelection {
                hunk_index: 0,
                line_indices: vec![first_addition],
            }],
        )
        .expect("partially stage untracked file");

    let staged = String::from_utf8(fixture.git_output(["show", ":new file.txt"]))
        .expect("UTF-8 index content");
    assert_eq!(staged, "one\n");
    let snapshot = service.snapshot(&repository, 4).expect("partial status");
    let change = snapshot
        .status
        .changes
        .iter()
        .find(|change| change.path == b"new file.txt")
        .expect("partial new file");
    assert_eq!(change.index_status, b'A');
    assert_eq!(change.worktree_status, b'M');
}

#[test]
fn expands_untracked_directories_into_diffable_file_entries() {
    let fixture = TestRepository::init();
    std::fs::create_dir_all(fixture.path().join("nested/deeper"))
        .expect("create nested fixture directory");
    fixture.write("nested/deeper/new.txt", "nested content\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    let snapshot = service.snapshot(&repository, 1).expect("status");
    assert!(
        snapshot
            .status
            .changes
            .iter()
            .any(|change| change.path == b"nested/deeper/new.txt")
    );
    assert!(
        !snapshot
            .status
            .changes
            .iter()
            .any(|change| change.path.ends_with(b"/"))
    );

    let diff = service
        .diff(&repository, b"nested/deeper/new.txt", DiffTarget::Unstaged)
        .expect("nested untracked diff");
    assert_eq!(diff.files[0].hunks[0].lines[0].content, "nested content");
}

#[test]
fn reads_configured_remotes_list() {
    let fixture = TestRepository::init();
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    let remotes = service.remotes(&repository).expect("read remotes");
    assert!(remotes.is_empty());

    service
        .add_remote(&repository, "origin", "https://example.com/acorn.git")
        .expect("add remote");
    assert_eq!(
        service.remotes(&repository).expect("read added remote"),
        [app_core::GitRemote {
            name: "origin".to_owned(),
            url: "https://example.com/acorn.git".to_owned(),
        }]
    );

    service
        .update_remote(
            &repository,
            "origin",
            "upstream",
            "ssh://git@example.com/acorn.git",
        )
        .expect("update remote");
    assert_eq!(
        service.remotes(&repository).expect("read updated remote"),
        [app_core::GitRemote {
            name: "upstream".to_owned(),
            url: "ssh://git@example.com/acorn.git".to_owned(),
        }]
    );

    service
        .remove_remote(&repository, "upstream")
        .expect("remove remote");
    assert!(
        service
            .remotes(&repository)
            .expect("read remotes")
            .is_empty()
    );
}
