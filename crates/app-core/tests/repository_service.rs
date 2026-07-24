use app_core::RepositoryService;
use git_domain::HeadState;
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
