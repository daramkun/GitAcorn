use app_core::{
    BranchRequest, CommitRequest, DiffTarget, HistoryFilter, HistoryOperation,
    InteractiveRebaseAction, InteractiveRebaseItem, InteractiveRebaseRequest, PatchSelection,
    ReferenceKind, RepositoryService, WorktreeCreateRequest,
};
use git_cli::GitExecutor;
use git_domain::{DiffLineKind, HeadState, RepositoryOperation};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use test_support::TestRepository;

fn sequence_editor_helper(directory: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path = directory.join("sequence-editor.cmd");
        fs::write(&path, "@echo off\r\ncopy /Y \"%~2\" \"%~3\" >nul\r\n")
            .expect("write sequence editor helper");
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.join("sequence-editor.sh");
        fs::write(&path, "#!/bin/sh\ncp \"$2\" \"$3\"\n").expect("write sequence editor helper");
        let mut permissions = fs::metadata(&path)
            .expect("read sequence editor metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("make sequence editor executable");
        path
    }
}

fn commit_independent_file(fixture: &TestRepository, path: &str, subject: &str) -> String {
    fixture.write(path, &format!("{subject}\n"));
    fixture.git(["add", path]);
    fixture.git(["commit", "-m", subject]);
    String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("commit oid")
        .trim()
        .to_owned()
}

#[test]
fn reads_blame_and_tracks_renames_in_file_and_directory_history() {
    let fixture = TestRepository::init();
    fs::create_dir_all(fixture.path().join("src")).expect("create source directory");
    fixture.write("src/original.txt", "first\nsecond\n");
    fixture.git(["add", "src/original.txt"]);
    fixture.git(["commit", "-m", "add source file"]);
    fixture.write("src/original.txt", "first\nupdated\n");
    fixture.git(["add", "src/original.txt"]);
    fixture.git(["commit", "-m", "update source file"]);
    fixture.git(["mv", "src/original.txt", "src/renamed.txt"]);
    fixture.git(["commit", "-m", "rename source file"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let blame = service
        .blame(&repository, b"src/renamed.txt", None)
        .expect("blame current file");
    assert_eq!(blame.path, b"src/renamed.txt");
    assert_eq!(blame.lines.len(), 2);
    assert_eq!(blame.lines[1].content, "updated");

    let history = service
        .path_history(&repository, b"src/renamed.txt", false, None, 20)
        .expect("file history");
    assert!(!history.entries.is_empty());
    assert!(
        history
            .entries
            .iter()
            .any(|entry| entry.previous_path.as_deref() == Some(b"src/original.txt"))
    );

    let directory_history = service
        .path_history(&repository, b"src", true, Some("source"), 20)
        .expect("directory history");
    assert!(directory_history.is_directory);
    assert!(!directory_history.entries.is_empty());
}

#[test]
fn soft_head_recovery_undoes_and_redoes_a_commit_without_losing_the_index() {
    let fixture = TestRepository::init();
    let before = commit_independent_file(&fixture, "baseline.txt", "baseline");
    fixture.write("change.txt", "recoverable\n");
    fixture.git(["add", "change.txt"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .commit(
            &repository,
            &CommitRequest {
                summary: "recoverable commit".to_owned(),
                description: String::new(),
                amend: false,
            },
        )
        .expect("commit");
    let after = service
        .current_head_oid(&repository)
        .expect("read committed head")
        .expect("committed head");

    service
        .move_head_soft(&repository, &after, &before)
        .expect("undo commit");
    assert_eq!(
        service.current_head_oid(&repository).expect("undo head"),
        Some(before.clone())
    );
    assert!(
        String::from_utf8(fixture.git_output(["diff", "--cached", "--name-only"]))
            .expect("cached paths")
            .contains("change.txt")
    );

    service
        .move_head_soft(&repository, &before, &after)
        .expect("redo commit");
    assert_eq!(
        service.current_head_oid(&repository).expect("redo head"),
        Some(after)
    );
}

#[test]
fn reset_modes_round_trip_with_the_matching_recovery_strategy() {
    for mode in ["soft", "mixed", "hard"] {
        let fixture = TestRepository::init();
        let before = commit_independent_file(&fixture, "baseline.txt", "baseline");
        let after = commit_independent_file(&fixture, "change.txt", "change");
        let service = RepositoryService::default();
        let repository = service.discover(fixture.path()).expect("discover");

        service
            .reset_head(&repository, &before, mode)
            .expect("reset branch");
        assert_eq!(
            service.current_head_oid(&repository).expect("reset head"),
            Some(before.clone())
        );

        match mode {
            "soft" => service
                .move_head_soft(&repository, &before, &after)
                .expect("redo soft reset"),
            "mixed" => service
                .move_head_mixed(&repository, &before, &after)
                .expect("redo mixed reset"),
            "hard" => service
                .move_head_hard(&repository, &before, &after)
                .expect("redo hard reset"),
            _ => unreachable!(),
        }
        assert_eq!(
            service
                .current_head_oid(&repository)
                .expect("recovery head"),
            Some(after)
        );
    }
}

#[test]
fn cherry_pick_and_revert_round_trip_clean_commits() {
    let fixture = TestRepository::init();
    let base = commit_independent_file(&fixture, "base.txt", "base");
    fixture.git(["switch", "-c", "topic"]);
    let source = commit_independent_file(&fixture, "topic.txt", "topic change");
    fixture.git(["switch", "main"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .cherry_pick(&repository, std::slice::from_ref(&source))
        .expect("cherry-pick");
    let cherry_head = service
        .current_head_oid(&repository)
        .expect("cherry-picked head")
        .expect("born head");
    assert_ne!(cherry_head, base);
    assert_eq!(
        fs::read_to_string(fixture.path().join("topic.txt"))
            .expect("topic file")
            .replace("\r\n", "\n"),
        "topic change\n"
    );

    service
        .revert(&repository, std::slice::from_ref(&source))
        .expect("revert");
    let reverted_head = service
        .current_head_oid(&repository)
        .expect("reverted head")
        .expect("born head");
    assert_ne!(reverted_head, cherry_head);
    assert!(!fixture.path().join("topic.txt").exists());
}

#[test]
fn cherry_pick_conflict_exposes_operation_and_supports_continue_and_abort() {
    let fixture = TestRepository::init();
    fixture.write("shared.txt", "base\n");
    fixture.git(["add", "shared.txt"]);
    fixture.git(["commit", "-m", "shared base"]);
    fixture.git(["switch", "-c", "topic"]);
    fixture.write("shared.txt", "topic\n");
    fixture.git(["add", "shared.txt"]);
    fixture.git(["commit", "-m", "topic change"]);
    let source = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("topic oid")
        .trim()
        .to_owned();
    fixture.git(["switch", "main"]);
    fixture.write("shared.txt", "main\n");
    fixture.git(["add", "shared.txt"]);
    fixture.git(["commit", "-m", "main change"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .cherry_pick(&repository, std::slice::from_ref(&source))
        .expect("conflicting cherry-pick remains recoverable");
    let conflicted = service.snapshot(&repository, 2).expect("conflict snapshot");
    assert_eq!(conflicted.operation, Some(RepositoryOperation::CherryPick));
    assert!(
        conflicted
            .status
            .changes
            .iter()
            .any(|change| change.is_conflict)
    );

    fixture.write("shared.txt", "resolved\n");
    fixture.git(["add", "shared.txt"]);
    service
        .continue_history_operation(&repository, HistoryOperation::CherryPick)
        .expect("continue cherry-pick");
    assert_eq!(
        service
            .snapshot(&repository, 3)
            .expect("continued snapshot")
            .operation,
        None
    );

    fixture.git(["reset", "--hard", "HEAD~2"]);
    fixture.write("shared.txt", "main\n");
    fixture.git(["add", "shared.txt"]);
    fixture.git(["commit", "-m", "main change again"]);
    service
        .cherry_pick(&repository, std::slice::from_ref(&source))
        .expect("second conflicting cherry-pick");
    service
        .abort_history_operation(&repository, HistoryOperation::CherryPick)
        .expect("abort cherry-pick");
    assert_eq!(
        service
            .snapshot(&repository, 4)
            .expect("aborted snapshot")
            .operation,
        None
    );

    service
        .cherry_pick(&repository, std::slice::from_ref(&source))
        .expect("third conflicting cherry-pick");
    service
        .skip_history_operation(&repository)
        .expect("skip cherry-pick");
    assert_eq!(
        service
            .snapshot(&repository, 5)
            .expect("skipped snapshot")
            .operation,
        None
    );
}

#[test]
fn interactive_rebase_recovery_round_trips_a_clean_reorder() {
    let fixture = TestRepository::init();
    let base_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("base oid")
        .trim()
        .to_owned();
    let first_oid = commit_independent_file(&fixture, "first.txt", "First");
    let second_oid = commit_independent_file(&fixture, "second.txt", "Second");
    let helper_directory = tempfile::tempdir().expect("sequence editor directory");
    let editor = sequence_editor_helper(helper_directory.path());
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .start_interactive_rebase(
            &repository,
            &InteractiveRebaseRequest {
                base_oid,
                expected_head_oid: second_oid.clone(),
                items: vec![
                    InteractiveRebaseItem {
                        oid: second_oid.clone(),
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                    InteractiveRebaseItem {
                        oid: first_oid,
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                ],
                auto_stash: false,
            },
            &editor,
        )
        .expect("start interactive rebase");
    let after = service
        .current_head_oid(&repository)
        .expect("rebased head")
        .expect("born head");
    assert_ne!(after, second_oid);

    service
        .move_head_hard(&repository, &after, &second_oid)
        .expect("undo interactive rebase");
    assert_eq!(
        service.current_head_oid(&repository).expect("undo head"),
        Some(second_oid.clone())
    );
    service
        .move_head_hard(&repository, &second_oid, &after)
        .expect("redo interactive rebase");
    assert_eq!(
        service.current_head_oid(&repository).expect("redo head"),
        Some(after)
    );
}

#[test]
fn checkout_recovery_round_trips_branches_and_rejects_dirty_worktrees() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    let main_oid = commit_independent_file(&fixture, "main.txt", "main work");
    let topic_oid = String::from_utf8(fixture.git_output(["rev-parse", "topic"]))
        .expect("topic oid")
        .trim()
        .to_owned();
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .checkout_branch(&repository, "topic", false, false, false)
        .expect("checkout topic");
    service
        .checkout_for_recovery(&repository, &topic_oid, &main_oid, Some("main"))
        .expect("undo checkout");
    assert_eq!(
        service.current_head_ref(&repository).expect("head ref"),
        Some("main".to_owned())
    );

    service
        .checkout_for_recovery(&repository, &main_oid, &topic_oid, Some("topic"))
        .expect("redo checkout");
    assert_eq!(
        service.current_head_ref(&repository).expect("head ref"),
        Some("topic".to_owned())
    );

    fixture.write("tracked.txt", "dirty after checkout\n");
    let error = service
        .checkout_for_recovery(&repository, &topic_oid, &main_oid, Some("main"))
        .expect_err("dirty recovery must fail");
    assert!(matches!(error, app_core::AppError::InvalidRequest(_)));
}

#[test]
fn deleted_branch_recovery_round_trips_and_detects_external_ref_changes() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let head_oid = service
        .current_head_oid(&repository)
        .expect("head oid")
        .expect("born head");
    let topic_oid = service
        .local_branch_oid(&repository, "topic")
        .expect("topic oid")
        .expect("topic branch");

    service
        .delete_branch(&repository, "topic")
        .expect("delete branch");
    service
        .restore_deleted_branch(&repository, &head_oid, "topic", &topic_oid)
        .expect("restore branch");
    assert_eq!(
        service
            .local_branch_oid(&repository, "topic")
            .expect("restored oid"),
        Some(topic_oid.clone())
    );
    service
        .delete_restored_branch(&repository, &head_oid, "topic", &topic_oid)
        .expect("delete branch again");
    assert_eq!(
        service
            .local_branch_oid(&repository, "topic")
            .expect("deleted ref"),
        None
    );

    fixture.git(["branch", "topic"]);
    let error = service
        .restore_deleted_branch(&repository, &head_oid, "topic", &topic_oid)
        .expect_err("recreated branch must block recovery");
    assert!(matches!(error, app_core::AppError::InvalidRequest(_)));
}

#[test]
fn reads_reflog_and_restores_selected_commits_as_new_refs() {
    let fixture = TestRepository::init();
    let recovered_oid = commit_independent_file(&fixture, "lost.txt", "temporarily lost");
    fixture.git(["reset", "--hard", "HEAD~1"]);
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let current_head = service
        .current_head_oid(&repository)
        .expect("current head")
        .expect("born head");

    let reflog = service.reflog(&repository, 50).expect("read reflog");
    let recovered_entry = reflog
        .iter()
        .find(|entry| entry.oid == recovered_oid)
        .expect("lost commit remains in reflog");
    assert_eq!(recovered_entry.subject, "temporarily lost");
    assert!(!recovered_entry.author_name.is_empty());
    assert!(!recovered_entry.author_email.is_empty());
    assert!(recovered_entry.authored_at > 0);
    assert_eq!(recovered_entry.parents.len(), 1);
    assert!(recovered_entry.reflog_only);

    service
        .restore_reflog_reference(&repository, &recovered_oid, "recovered-work", false)
        .expect("restore branch");
    service
        .restore_reflog_reference(&repository, &recovered_oid, "recovered-v1", true)
        .expect("restore tag");
    let references = service
        .references(&repository)
        .expect("restored references");
    assert!(references.iter().any(|reference| {
        reference.kind == ReferenceKind::LocalBranch
            && reference.short_name == "recovered-work"
            && reference.oid == recovered_oid
    }));
    let restored_reflog = service
        .reflog(&repository, 50)
        .expect("read restored reflog");
    assert!(
        restored_reflog
            .iter()
            .find(|entry| entry.oid == recovered_oid)
            .is_some_and(|entry| !entry.reflog_only)
    );
    assert!(references.iter().any(|reference| {
        reference.kind == ReferenceKind::Tag
            && reference.short_name == "recovered-v1"
            && reference.oid == recovered_oid
    }));
    assert_eq!(
        service
            .current_head_oid(&repository)
            .expect("unchanged head"),
        Some(current_head)
    );
}

#[test]
fn hard_head_recovery_undoes_and_redoes_a_clean_rebase() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    commit_independent_file(&fixture, "main.txt", "main change");
    fixture.git(["switch", "topic"]);
    let before = commit_independent_file(&fixture, "topic.txt", "topic change");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .rebase_onto(&repository, "main")
        .expect("clean rebase");
    let after = service
        .current_head_oid(&repository)
        .expect("rebased head")
        .expect("born head");
    assert_ne!(before, after);

    service
        .move_head_hard(&repository, &after, &before)
        .expect("undo rebase");
    assert_eq!(
        service.current_head_oid(&repository).expect("undo head"),
        Some(before.clone())
    );
    service
        .move_head_hard(&repository, &before, &after)
        .expect("redo rebase");
    assert_eq!(
        service.current_head_oid(&repository).expect("redo head"),
        Some(after.clone())
    );

    fixture.write("tracked.txt", "dirty after rebase\n");
    let error = service
        .move_head_hard(&repository, &after, &before)
        .expect_err("dirty recovery must fail");
    assert!(matches!(error, app_core::AppError::InvalidRequest(_)));
}

#[test]
fn reads_and_updates_global_and_repository_git_identity() {
    let fixture = TestRepository::init();
    let global_directory = tempfile::tempdir().expect("global Git config directory");
    let global_config = global_directory.path().join("gitconfig");
    let service = RepositoryService::new(
        GitExecutor::default().with_environment("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
    );
    let repository = service.discover(fixture.path()).expect("discover");

    assert_eq!(
        service
            .global_identity()
            .expect("empty global identity")
            .name,
        None
    );
    service
        .update_global_identity(Some(" Ada Lovelace "), Some("ada@example.com"))
        .expect("update global identity");

    let configured = service
        .repository_identity(&repository)
        .expect("repository identity");
    assert_eq!(configured.global.name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(configured.global.email.as_deref(), Some("ada@example.com"));
    assert_eq!(configured.local.name.as_deref(), Some("GitAcorn Test"));
    assert_eq!(
        configured.local.email.as_deref(),
        Some("test@gitacorn.local")
    );

    let partially_overridden = service
        .update_repository_identity(&repository, Some("Grace Hopper"), None)
        .expect("update repository identity");
    assert_eq!(
        partially_overridden.local.name.as_deref(),
        Some("Grace Hopper")
    );
    assert_eq!(partially_overridden.local.email, None);
    assert_eq!(
        partially_overridden.effective.name.as_deref(),
        Some("Grace Hopper")
    );
    assert_eq!(
        partially_overridden.effective.email.as_deref(),
        Some("ada@example.com")
    );

    let inherited = service
        .update_repository_identity(&repository, None, None)
        .expect("remove repository identity");
    assert_eq!(inherited.local.name, None);
    assert_eq!(inherited.local.email, None);
    assert_eq!(inherited.effective.name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        inherited.effective.email.as_deref(),
        Some("ada@example.com")
    );
}

#[test]
fn rejects_multiline_git_identity_values() {
    let global_directory = tempfile::tempdir().expect("global Git config directory");
    let service = RepositoryService::new(GitExecutor::default().with_environment(
        "GIT_CONFIG_GLOBAL",
        global_directory.path().join("gitconfig").as_os_str(),
    ));

    let error = service
        .update_global_identity(Some("Ada\nLovelace"), None)
        .expect_err("multiline identity must fail");
    assert!(matches!(error, app_core::AppError::InvalidRequest(_)));
}

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
fn adds_initializes_and_removes_a_submodule() {
    let fixture = TestRepository::init();
    let child = TestRepository::init();
    let service = RepositoryService::new(GitExecutor::default());
    let repository = service.discover(fixture.path()).expect("discover");
    let child_url = child.path().to_string_lossy().into_owned();

    service
        .add_submodule(&repository, &child_url, "vendor/child")
        .expect("add submodule");
    let added = service.sidebar(&repository).expect("read added submodule");
    assert_eq!(added.submodules.len(), 1);
    assert_eq!(added.submodules[0].path, "vendor/child");
    assert!(added.submodules[0].initialized);

    fixture.git(["submodule", "deinit", "-f", "--", "vendor/child"]);
    let deinitialized = service
        .sidebar(&repository)
        .expect("read deinitialized submodule");
    assert!(!deinitialized.submodules[0].initialized);

    service
        .initialize_submodule(&repository, "vendor/child")
        .expect("initialize submodule");
    assert!(
        service
            .sidebar(&repository)
            .expect("read initialized submodule")
            .submodules[0]
            .initialized
    );

    fs::write(
        fixture.path().join("vendor/child/tracked.txt"),
        "dirty submodule\n",
    )
    .expect("dirty submodule worktree");
    assert!(
        service
            .deinitialize_submodule(&repository, "vendor/child")
            .is_err(),
        "dirty submodule must not be deinitialized"
    );
    assert!(
        service
            .remove_submodule(&repository, "vendor/child")
            .is_err(),
        "dirty submodule must not be removed"
    );
    fixture.git(["-C", "vendor/child", "restore", "tracked.txt"]);

    service
        .deinitialize_submodule(&repository, "vendor/child")
        .expect("deinitialize submodule");
    assert!(
        !service
            .sidebar(&repository)
            .expect("read deinitialized submodule")
            .submodules[0]
            .initialized
    );
    service
        .initialize_submodule(&repository, "vendor/child")
        .expect("reinitialize submodule");

    fs::write(
        fixture.path().join("vendor/child/tracked.txt"),
        "clean local submodule commit\n",
    )
    .expect("change submodule for local commit");
    fixture.git(["-C", "vendor/child", "add", "tracked.txt"]);
    fixture.git([
        "-C",
        "vendor/child",
        "-c",
        "user.name=GitAcorn Test",
        "-c",
        "user.email=test@gitacorn.local",
        "commit",
        "-m",
        "local submodule commit",
    ]);

    service
        .remove_submodule(&repository, "vendor/child")
        .expect("remove clean submodule with a changed gitlink");
    assert!(
        service
            .sidebar(&repository)
            .expect("read removed submodule")
            .submodules
            .is_empty()
    );
    assert!(!fixture.path().join("vendor/child").exists());
}

#[test]
fn updates_clean_submodules_after_head_changes_and_preserves_dirty_ones() {
    let fixture = TestRepository::init();
    let child = TestRepository::init();
    let service = RepositoryService::new(GitExecutor::default());
    let repository = service.discover(fixture.path()).expect("discover");
    let child_url = child.path().to_string_lossy().into_owned();
    let first_child_oid = String::from_utf8(child.git_output(["rev-parse", "HEAD"]))
        .expect("first child oid")
        .trim()
        .to_owned();

    service
        .add_submodule(&repository, &child_url, "vendor/child")
        .expect("add submodule");
    fixture.git(["commit", "-m", "add child submodule"]);
    fixture.git(["branch", "old-submodule"]);

    child.write("tracked.txt", "child v2\n");
    child.git(["add", "tracked.txt"]);
    child.git(["commit", "-m", "child v2"]);
    let second_child_oid = String::from_utf8(child.git_output(["rev-parse", "HEAD"]))
        .expect("second child oid")
        .trim()
        .to_owned();
    fixture.git(["-C", "vendor/child", "fetch", "origin"]);
    fixture.git(["-C", "vendor/child", "checkout", &second_child_oid]);
    fixture.git(["add", "vendor/child"]);
    fixture.git(["commit", "-m", "update child submodule"]);

    service
        .checkout_branch(&repository, "old-submodule", false, false, false)
        .expect("checkout old submodule pointer");
    assert_eq!(
        String::from_utf8(fixture.git_output(["-C", "vendor/child", "rev-parse", "HEAD"]))
            .expect("updated child oid")
            .trim(),
        first_child_oid
    );

    service
        .checkout_branch(&repository, "main", false, false, false)
        .expect("checkout new submodule pointer");
    assert_eq!(
        String::from_utf8(fixture.git_output(["-C", "vendor/child", "rev-parse", "HEAD"]))
            .expect("restored child oid")
            .trim(),
        second_child_oid
    );

    fs::write(
        fixture.path().join("vendor/child/tracked.txt"),
        "dirty child\n",
    )
    .expect("dirty child worktree");
    service
        .checkout_branch(&repository, "old-submodule", false, false, false)
        .expect("checkout while child is dirty");
    assert_eq!(
        String::from_utf8(fixture.git_output(["-C", "vendor/child", "rev-parse", "HEAD"]))
            .expect("preserved dirty child oid")
            .trim(),
        second_child_oid
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("vendor/child/tracked.txt"))
            .expect("preserved dirty child file"),
        "dirty child\n"
    );
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
fn history_keeps_every_remote_tip_visible_and_marks_remote_only_commits() {
    let fixture = TestRepository::init();
    let main_oid = String::from_utf8(fixture.git_output(["rev-parse", "main"])).expect("main oid");
    let mut remote_oids = Vec::new();
    for branch in ["dev", "release", "experiment"] {
        fixture.git(["switch", "-c", branch]);
        fixture.write("tracked.txt", &format!("{branch}\n"));
        fixture.git(["add", "tracked.txt"]);
        fixture.git(["commit", "-m", &format!("{branch} commit")]);
        let oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
            .expect("remote branch oid")
            .trim()
            .to_owned();
        fixture.git(["update-ref", &format!("refs/remotes/origin/{branch}"), &oid]);
        fixture.git(["switch", "main"]);
        fixture.git(["branch", "-D", branch]);
        remote_oids.push(oid);
    }

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let first = service
        .history(
            &repository,
            &HistoryFilter {
                limit: 1,
                ..HistoryFilter::default()
            },
        )
        .expect("first history page");

    for oid in &remote_oids {
        let commit = first
            .commits
            .iter()
            .find(|commit| &commit.oid == oid)
            .expect("remote tip is pinned to the first page");
        assert!(commit.remote_only);
        assert!(
            commit
                .references
                .iter()
                .any(|reference| reference.starts_with("refs/remotes/origin/"))
        );
    }

    let complete = service
        .history(
            &repository,
            &HistoryFilter {
                limit: 200,
                ..HistoryFilter::default()
            },
        )
        .expect("complete history");
    assert!(
        !complete
            .commits
            .iter()
            .find(|commit| commit.oid == main_oid.trim())
            .expect("main commit")
            .remote_only
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
fn interactive_rebase_reorders_rewords_edits_and_restores_autostash() {
    let fixture = TestRepository::init();
    let base_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("base oid")
        .trim()
        .to_owned();
    let first_oid = commit_independent_file(&fixture, "first.txt", "First");
    let second_oid = commit_independent_file(&fixture, "second.txt", "Second");
    let third_oid = commit_independent_file(&fixture, "third.txt", "Third");
    fixture.write("tracked.txt", "local work\n");

    let helper_directory = tempfile::tempdir().expect("sequence editor directory");
    let editor = sequence_editor_helper(helper_directory.path());
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let preview = service
        .interactive_rebase_preview(&repository, &base_oid)
        .expect("preview interactive rebase");
    assert_eq!(preview.commits.len(), 3);

    service
        .start_interactive_rebase(
            &repository,
            &InteractiveRebaseRequest {
                base_oid,
                expected_head_oid: third_oid.clone(),
                items: vec![
                    InteractiveRebaseItem {
                        oid: third_oid,
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                    InteractiveRebaseItem {
                        oid: first_oid,
                        action: InteractiveRebaseAction::Reword,
                        summary: Some("Renamed first".to_owned()),
                        description: Some("Reworded by GitAcorn".to_owned()),
                    },
                    InteractiveRebaseItem {
                        oid: second_oid,
                        action: InteractiveRebaseAction::Edit,
                        summary: None,
                        description: None,
                    },
                ],
                auto_stash: true,
            },
            &editor,
        )
        .expect("start interactive rebase");

    let paused = service.snapshot(&repository, 2).expect("paused snapshot");
    assert_eq!(paused.operation, Some(RepositoryOperation::RebaseEdit));
    service
        .commit(
            &repository,
            &CommitRequest {
                summary: "Edited second".to_owned(),
                description: "Amended during edit".to_owned(),
                amend: true,
            },
        )
        .expect("amend edit commit");
    service
        .continue_rebase(&repository)
        .expect("continue edited rebase");

    let completed = service
        .snapshot(&repository, 3)
        .expect("completed snapshot");
    assert_eq!(completed.operation, None);
    assert!(
        completed
            .status
            .changes
            .iter()
            .any(|change| change.path == b"tracked.txt"),
        "autostash did not restore local work"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("tracked.txt"))
            .expect("restored tracked file")
            .replace("\r\n", "\n"),
        "local work\n"
    );
    let subjects =
        String::from_utf8(fixture.git_output(["log", "--format=%s", "-4"])).expect("log subjects");
    assert_eq!(
        subjects.lines().collect::<Vec<_>>(),
        ["Edited second", "Renamed first", "Third", "initial"]
    );
}

#[test]
fn aborting_an_editing_interactive_rebase_restores_the_original_head() {
    let fixture = TestRepository::init();
    let base_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("base oid")
        .trim()
        .to_owned();
    let first_oid = commit_independent_file(&fixture, "first.txt", "First");
    let second_oid = commit_independent_file(&fixture, "second.txt", "Second");
    let original_head = second_oid.clone();
    let helper_directory = tempfile::tempdir().expect("sequence editor directory");
    let editor = sequence_editor_helper(helper_directory.path());
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .start_interactive_rebase(
            &repository,
            &InteractiveRebaseRequest {
                base_oid,
                expected_head_oid: original_head.clone(),
                items: vec![
                    InteractiveRebaseItem {
                        oid: first_oid,
                        action: InteractiveRebaseAction::Edit,
                        summary: None,
                        description: None,
                    },
                    InteractiveRebaseItem {
                        oid: second_oid,
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                ],
                auto_stash: false,
            },
            &editor,
        )
        .expect("pause interactive rebase");
    assert_eq!(
        service.snapshot(&repository, 2).expect("paused").operation,
        Some(RepositoryOperation::RebaseEdit)
    );

    service.abort_rebase(&repository).expect("abort rebase");
    assert_eq!(
        String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
            .expect("restored head")
            .trim(),
        original_head
    );
    assert_eq!(
        service.snapshot(&repository, 3).expect("aborted").operation,
        None
    );
}

#[test]
fn conflicting_interactive_rebase_can_be_resolved_and_continued() {
    let fixture = TestRepository::init();
    let base_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("base oid")
        .trim()
        .to_owned();
    fixture.write("tracked.txt", "first version\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "First version"]);
    let first_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("first oid")
        .trim()
        .to_owned();
    fixture.write("tracked.txt", "second version\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "Second version"]);
    let second_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("second oid")
        .trim()
        .to_owned();

    let helper_directory = tempfile::tempdir().expect("sequence editor directory");
    let editor = sequence_editor_helper(helper_directory.path());
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .start_interactive_rebase(
            &repository,
            &InteractiveRebaseRequest {
                base_oid,
                expected_head_oid: second_oid.clone(),
                items: vec![
                    InteractiveRebaseItem {
                        oid: second_oid,
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                    InteractiveRebaseItem {
                        oid: first_oid,
                        action: InteractiveRebaseAction::Pick,
                        summary: None,
                        description: None,
                    },
                ],
                auto_stash: false,
            },
            &editor,
        )
        .expect("start conflicting rebase");

    let mut completed = false;
    for attempt in 0..3 {
        let snapshot = service
            .snapshot(&repository, attempt + 2)
            .expect("conflict snapshot");
        if snapshot.operation.is_none() {
            completed = true;
            break;
        }
        assert_eq!(snapshot.operation, Some(RepositoryOperation::Rebase));
        assert!(
            snapshot
                .status
                .changes
                .iter()
                .any(|change| change.is_conflict)
        );
        fixture.write("tracked.txt", &format!("resolved {attempt}\n"));
        fixture.git(["add", "tracked.txt"]);
        service
            .continue_rebase(&repository)
            .expect("continue resolved rebase");
    }
    assert!(
        completed
            || service
                .snapshot(&repository, 6)
                .expect("final snapshot")
                .operation
                .is_none(),
        "rebase remained paused after resolving conflicts"
    );
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
fn deletes_branches_and_tags_from_a_selected_remote() {
    let fixture = TestRepository::init();
    fixture.git(["branch", "topic"]);
    fixture.git(["tag", "v1.0.0"]);
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
    fixture.git(["push", "origin", "topic"]);
    fixture.git(["push", "origin", "refs/tags/v1.0.0"]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .delete_remote_branch(&repository, "origin", "topic")
        .expect("delete remote branch");
    service
        .delete_remote_tag(&repository, "origin", "v1.0.0")
        .expect("delete remote tag");

    let remote_refs =
        String::from_utf8(fixture.git_output(["ls-remote", "origin"])).expect("UTF-8 remote refs");
    assert!(!remote_refs.contains("refs/heads/topic"));
    assert!(!remote_refs.contains("refs/tags/v1.0.0"));
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
fn fast_forward_branch_merges_the_matching_origin_tracking_reference() {
    let fixture = TestRepository::init();
    fixture.git(["switch", "-c", "remote-main"]);
    fixture.write("tracked.txt", "remote main\n");
    fixture.git(["add", "tracked.txt"]);
    fixture.git(["commit", "-m", "remote main change"]);
    let remote_oid =
        String::from_utf8(fixture.git_output(["rev-parse", "HEAD"])).expect("remote oid");
    fixture.git(["switch", "main"]);
    fixture.git(["update-ref", "refs/remotes/origin/main", remote_oid.trim()]);

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    service
        .fast_forward_branch(&repository, "main")
        .expect("fast-forward");

    let head_oid = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"])).expect("head oid");
    assert_eq!(head_oid.trim(), remote_oid.trim());
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
fn manages_worktree_create_lock_unlock_and_force_remove_lifecycle() {
    let fixture = TestRepository::init();
    let linked_root = tempfile::tempdir().expect("create linked worktree parent");
    let linked_path = linked_root.path().join("feature-worktree");
    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");

    service
        .create_worktree(
            &repository,
            &WorktreeCreateRequest {
                path: linked_path.clone(),
                branch: Some("feature/lifecycle".to_owned()),
                start_point: Some("HEAD".to_owned()),
            },
        )
        .expect("create worktree");
    let sidebar = service.sidebar(&repository).expect("read created worktree");
    let created = sidebar
        .worktrees
        .iter()
        .find(|worktree| worktree.branch.as_deref() == Some("feature/lifecycle"))
        .expect("created worktree entry");
    assert_eq!(created.branch.as_deref(), Some("feature/lifecycle"));

    service
        .lock_worktree(&repository, &linked_path, Some("qa hold"))
        .expect("lock worktree");
    assert!(
        service
            .sidebar(&repository)
            .expect("read locked worktree")
            .worktrees
            .iter()
            .any(|worktree| worktree.is_locked)
    );
    service
        .unlock_worktree(&repository, &linked_path)
        .expect("unlock worktree");

    fs::write(linked_path.join("dirty.txt"), "uncommitted\n").expect("dirty worktree");
    assert!(
        service
            .remove_worktree(&repository, &linked_path, false)
            .is_err()
    );
    service
        .remove_worktree(&repository, &linked_path, true)
        .expect("force remove worktree");
    assert!(!linked_path.exists());
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
fn reads_files_and_diff_for_a_selected_commit() {
    let fixture = TestRepository::init();
    fixture.write("tracked.txt", "changed in commit\n");
    fixture.write("added.txt", "new file\n");
    fixture.git(["add", "tracked.txt", "added.txt"]);
    fixture.git(["commit", "-m", "selected commit"]);
    let revision = String::from_utf8(fixture.git_output(["rev-parse", "HEAD"]))
        .expect("UTF-8 revision")
        .trim()
        .to_owned();

    let service = RepositoryService::default();
    let repository = service.discover(fixture.path()).expect("discover");
    let files = service
        .commit_files(&repository, &revision)
        .expect("commit files");

    assert_eq!(
        files
            .iter()
            .map(|file| String::from_utf8_lossy(&file.path).into_owned())
            .collect::<Vec<_>>(),
        ["added.txt", "tracked.txt"]
    );

    let diff = service
        .commit_diff(&repository, &revision, b"tracked.txt")
        .expect("commit diff");
    let lines = &diff.files[0].hunks[0].lines;
    assert!(
        lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Deletion && line.content == "initial")
    );
    assert!(
        lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Addition && line.content == "changed in commit")
    );
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
