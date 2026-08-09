use app_core::{AppError, ConflictResolution, ConflictSegment, RepositoryService, StashRequest};
use test_support::TestRepository;

#[test]
fn creates_applies_and_drops_a_stash_with_untracked_files() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "stashed change\n");
    fixture.write("untracked.txt", "new file\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .create_stash(
            &repository,
            &StashRequest {
                message: "alpha backup".to_owned(),
                include_untracked: true,
                paths: Vec::new(),
            },
        )
        .expect("create stash");
    assert!(!fixture.path().join("untracked.txt").exists());

    service
        .apply_stash(&repository, "stash@{0}")
        .expect("apply stash");
    let tracked =
        std::fs::read_to_string(fixture.path().join("tracked.txt")).expect("tracked content");
    assert_eq!(tracked.replace("\r\n", "\n"), "stashed change\n");
    assert!(fixture.path().join("untracked.txt").is_file());

    service
        .drop_stash(&repository, "stash@{0}")
        .expect("drop stash");
    assert!(
        service
            .sidebar(&repository)
            .expect("sidebar")
            .stashes
            .is_empty()
    );
}

#[test]
fn creates_a_stash_for_only_the_selected_paths() {
    let fixture = TestRepository::init();
    fixture.write("selected.txt", "initial\n");
    fixture.write("remaining.txt", "initial\n");
    fixture.git(["add", "selected.txt", "remaining.txt"]);
    fixture.git(["commit", "-m", "add selectable files"]);
    fixture.write("selected.txt", "selected change\n");
    fixture.write("remaining.txt", "remaining change\n");
    fixture.write("selected-untracked.txt", "selected untracked\n");
    fixture.write("remaining-untracked.txt", "remaining untracked\n");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .create_stash(
            &repository,
            &StashRequest {
                message: "selected files".to_owned(),
                include_untracked: true,
                paths: vec![b"selected.txt".to_vec(), b"selected-untracked.txt".to_vec()],
            },
        )
        .expect("create selected stash");

    assert_eq!(
        std::fs::read_to_string(fixture.path().join("selected.txt"))
            .expect("selected content")
            .replace("\r\n", "\n"),
        "initial\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("remaining.txt"))
            .expect("remaining content")
            .replace("\r\n", "\n"),
        "remaining change\n"
    );
    assert!(!fixture.path().join("selected-untracked.txt").exists());
    assert!(fixture.path().join("remaining-untracked.txt").is_file());
}

#[test]
fn loads_and_applies_multiple_conflict_hunks_from_real_index_stages() {
    let fixture = TestRepository::init();
    let base = (1..=24)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    fixture.write("multi.txt", &base);
    fixture.git(["add", "multi.txt"]);
    fixture.git(["commit", "-m", "multi base"]);

    fixture.git(["switch", "-c", "topic"]);
    let topic = base
        .replace("line 2\n", "topic two\n")
        .replace("line 22\n", "topic twenty-two\n");
    fixture.write("multi.txt", &topic);
    fixture.git(["commit", "-am", "topic changes"]);

    fixture.git(["switch", "main"]);
    let current = base
        .replace("line 2\n", "main two\n")
        .replace("line 22\n", "main twenty-two\n");
    fixture.write("multi.txt", &current);
    fixture.git(["commit", "-am", "main changes"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .merge_reference(&repository, "topic")
        .expect("enter conflict");

    let conflict = service
        .conflict_file(&repository, b"multi.txt")
        .expect("load conflict stages");
    assert!(conflict.editable);
    assert_eq!(conflict.base.as_deref(), Some(base.as_str()));
    assert_eq!(conflict.ours.as_deref(), Some(current.as_str()));
    assert_eq!(conflict.theirs.as_deref(), Some(topic.as_str()));
    assert_eq!(
        conflict
            .segments
            .iter()
            .filter(|segment| matches!(segment, ConflictSegment::Conflict { .. }))
            .count(),
        2
    );

    let resolved = base
        .replace("line 2\n", "resolved two\n")
        .replace("line 22\n", "resolved twenty-two\n");
    service
        .apply_conflict_content(&repository, b"multi.txt", &conflict.worktree_oid, &resolved)
        .expect("apply resolved result");
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("multi.txt")).expect("resolved content"),
        resolved
    );
    assert!(
        !service
            .snapshot(&repository, 2)
            .expect("resolved snapshot")
            .status
            .changes
            .iter()
            .any(|change| change.is_conflict)
    );
}

#[test]
fn rejects_a_stale_conflict_editor_without_overwriting_external_edits() {
    let fixture = TestRepository::init();
    fixture.git(["switch", "-c", "topic"]);
    fixture.write("tracked.txt", "topic\n");
    fixture.git(["commit", "-am", "topic"]);
    fixture.git(["switch", "main"]);
    fixture.write("tracked.txt", "main\n");
    fixture.git(["commit", "-am", "main"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .merge_reference(&repository, "topic")
        .expect("enter conflict");
    let conflict = service
        .conflict_file(&repository, b"tracked.txt")
        .expect("load conflict");
    fixture.write("tracked.txt", "external edit\n");

    let error = service
        .apply_conflict_content(
            &repository,
            b"tracked.txt",
            &conflict.worktree_oid,
            "resolved\n",
        )
        .expect_err("stale editor must be rejected");
    assert!(matches!(error, AppError::InvalidRequest(_)));
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("tracked.txt")).expect("external content"),
        "external edit\n"
    );
    assert!(
        service
            .snapshot(&repository, 2)
            .expect("conflicted snapshot")
            .status
            .changes
            .iter()
            .any(|change| change.is_conflict)
    );
}

#[test]
fn resolves_each_side_of_a_merge_conflict_and_can_abort() {
    let fixture = TestRepository::init();
    fixture.git(["switch", "-c", "topic"]);
    fixture.write("tracked.txt", "topic\n");
    fixture.git(["commit", "-am", "topic"]);
    fixture.git(["switch", "main"]);
    fixture.write("tracked.txt", "main\n");
    fixture.git(["commit", "-am", "main"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .merge_reference(&repository, "topic")
        .expect("enter conflict");
    service
        .resolve_conflict(&repository, b"tracked.txt", ConflictResolution::Theirs)
        .expect("resolve theirs");
    let resolved =
        std::fs::read_to_string(fixture.path().join("tracked.txt")).expect("resolved content");
    assert_eq!(resolved.replace("\r\n", "\n"), "topic\n");
    assert!(
        !service
            .snapshot(&repository, 2)
            .expect("resolved snapshot")
            .status
            .changes
            .iter()
            .any(|change| change.is_conflict)
    );

    service.abort_merge(&repository).expect("abort merge");
    let restored =
        std::fs::read_to_string(fixture.path().join("tracked.txt")).expect("restored content");
    assert_eq!(restored.replace("\r\n", "\n"), "main\n");
}
