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
