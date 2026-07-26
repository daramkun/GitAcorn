use std::process::Command;

use app_core::{CloneRequest, RemoteOperationKind, RemoteRequest, RepositoryService};
use git_cli::CancellationToken;
use tempfile::tempdir;
use test_support::TestRepository;

fn run_git(directory: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .expect("run fixture Git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn clones_fetches_pulls_and_pushes_against_a_local_remote() {
    let source = TestRepository::init();
    let remote_dir = tempdir().expect("remote parent");
    let remote_path = remote_dir.path().join("remote.git");
    run_git(remote_dir.path(), &["init", "--bare", "remote.git"]);
    source.git([
        "remote",
        "add",
        "origin",
        remote_path.to_str().expect("utf8 remote"),
    ]);
    source.git(["push", "-u", "origin", "main"]);

    let clone_parent = tempdir().expect("clone parent");
    let clone_path = clone_parent.path().join("clone");
    let service = RepositoryService::default();
    let cancellation = CancellationToken::default();
    let mut progress = Vec::new();
    service
        .clone_repository(
            &CloneRequest {
                remote_url: remote_path.to_string_lossy().into_owned(),
                destination: clone_path.clone(),
            },
            &cancellation,
            |event| progress.push(event.message),
        )
        .expect("clone");
    assert!(clone_path.join(".git").is_dir());
    assert!(!progress.is_empty());

    source.write("tracked.txt", "from source\n");
    source.git(["add", "tracked.txt"]);
    source.git(["commit", "-m", "source update"]);
    source.git(["push"]);

    run_git(&clone_path, &["config", "user.name", "GitAcorn Test"]);
    run_git(
        &clone_path,
        &["config", "user.email", "test@gitacorn.local"],
    );
    let repository = service.discover(&clone_path).expect("discover clone");
    service
        .remote_sync(
            &repository,
            &RemoteRequest {
                kind: RemoteOperationKind::Fetch,
                remote: None,
                fetch_tags: false,
                auto_stash: false,
                fast_forward_only: false,
                force_with_lease: false,
            },
            &cancellation,
            |_| {},
        )
        .expect("fetch");
    service
        .remote_sync(
            &repository,
            &RemoteRequest {
                kind: RemoteOperationKind::Pull,
                remote: None,
                fetch_tags: false,
                auto_stash: false,
                fast_forward_only: true,
                force_with_lease: false,
            },
            &cancellation,
            |_| {},
        )
        .expect("pull");
    assert_eq!(
        std::fs::read_to_string(clone_path.join("tracked.txt"))
            .expect("read pulled file")
            .replace("\r\n", "\n"),
        "from source\n"
    );

    std::fs::write(clone_path.join("clone.txt"), "from clone\n").expect("write clone file");
    run_git(&clone_path, &["add", "clone.txt"]);
    run_git(&clone_path, &["commit", "-m", "clone update"]);
    service
        .remote_sync(
            &repository,
            &RemoteRequest {
                kind: RemoteOperationKind::Push,
                remote: None,
                fetch_tags: false,
                auto_stash: false,
                fast_forward_only: false,
                force_with_lease: false,
            },
            &cancellation,
            |_| {},
        )
        .expect("push");
    source.git(["pull", "--ff-only"]);
    assert!(source.path().join("clone.txt").is_file());
}

#[test]
fn rejects_force_with_lease_for_non_push_operations() {
    let fixture = TestRepository::init();
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    let error = service
        .remote_sync(
            &repository,
            &RemoteRequest {
                kind: RemoteOperationKind::Fetch,
                remote: None,
                fetch_tags: false,
                auto_stash: false,
                fast_forward_only: false,
                force_with_lease: true,
            },
            &CancellationToken::default(),
            |_| {},
        )
        .expect_err("fetch must reject push-only option");
    assert!(error.to_string().contains("only valid for push"));
}

#[test]
fn cancels_a_running_remote_process_through_the_shared_token() {
    let fixture = TestRepository::init();
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let cancellation = CancellationToken::default();
    cancellation.cancel();

    let error = service
        .remote_sync(
            &repository,
            &RemoteRequest {
                kind: RemoteOperationKind::Fetch,
                remote: None,
                fetch_tags: false,
                auto_stash: false,
                fast_forward_only: false,
                force_with_lease: false,
            },
            &cancellation,
            |_| {},
        )
        .expect_err("cancelled operation");
    assert_eq!(error.to_string(), "Operation was cancelled");
}
