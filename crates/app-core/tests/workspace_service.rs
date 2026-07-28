use app_core::{ConflictResolution, RepositoryService, StashRequest};
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
