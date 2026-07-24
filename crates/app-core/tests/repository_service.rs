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
        .checkout_branch(&repository, "feature/history")
        .expect("explicit checkout");
    assert_eq!(
        String::from_utf8(fixture.git_output(["branch", "--show-current"]))
            .expect("branch text")
            .trim(),
        "feature/history"
    );
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
