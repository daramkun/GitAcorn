mod commands;
mod dto;
mod path_display;
mod state;
mod system_icons;
#[cfg(windows)]
mod windows_snap;

use persistence::SessionStore;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("git-acorn.sqlite3");
            let session = tauri::async_runtime::block_on(SessionStore::open(&database_path))?;
            let recovered =
                tauri::async_runtime::block_on(session.recover_interrupted_operations())?;
            if recovered > 0 {
                tracing::warn!(recovered, "recovered interrupted operations");
            }
            app.manage(state::ApplicationState::new(session));
            #[cfg(windows)]
            {
                let window = app
                    .get_webview_window("main")
                    .ok_or("main window is unavailable")?;
                windows_snap::install(&window)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::system_file_icons,
            commands::git_identity_get,
            commands::git_identity_update_global,
            commands::git_identity_update_repository,
            commands::remote_sync,
            commands::repository_clone,
            commands::operation_cancel,
            commands::repository_open,
            commands::repository_snapshot,
            commands::repository_sidebar,
            commands::worktree_create,
            commands::worktree_lock,
            commands::worktree_unlock,
            commands::worktree_remove,
            commands::history_page,
            commands::references_list,
            commands::reflog_list,
            commands::reflog_restore,
            commands::remote_tags_list,
            commands::remotes_list,
            commands::remote_add,
            commands::remote_update,
            commands::remote_remove,
            commands::submodule_add,
            commands::submodule_initialize,
            commands::submodule_deinitialize,
            commands::submodule_remove,
            commands::diff_get,
            commands::compare_get,
            commands::compare_patch_get,
            commands::compare_patch_validate,
            commands::compare_patch_apply,
            commands::compare_patch_save,
            commands::external_diff_tool_get,
            commands::external_diff_tool_update,
            commands::external_diff_run,
            commands::external_merge_run,
            commands::binary_preview_get,
            commands::lfs_sync,
            commands::lfs_status_get,
            commands::lfs_locks_get,
            commands::lfs_lock,
            commands::lfs_unlock,
            commands::signature_status_get,
            commands::signature_settings_get,
            commands::signature_settings_update,
            commands::blame_get,
            commands::path_history_get,
            commands::commit_files,
            commands::commit_diff_get,
            commands::stage_paths,
            commands::unstage_paths,
            commands::apply_patch_selection,
            commands::discard_path,
            commands::commit_create,
            commands::branch_create,
            commands::branch_checkout,
            commands::branch_delete,
            commands::branch_rename,
            commands::branch_rebase,
            commands::branch_reset,
            commands::history_mutate,
            commands::history_continue,
            commands::history_abort,
            commands::history_skip,
            commands::interactive_rebase_preview,
            commands::interactive_rebase_start,
            commands::rebase_continue,
            commands::rebase_skip,
            commands::rebase_abort,
            commands::branch_merge,
            commands::branch_fast_forward,
            commands::tag_create,
            commands::tag_delete,
            commands::stash_create,
            commands::stash_apply,
            commands::stash_drop,
            commands::conflict_resolve,
            commands::merge_abort,
            commands::operation_history,
            commands::operation_undo,
            commands::operation_redo,
            commands::diagnostics_copy,
            commands::worktree_activate,
            commands::session_restore,
            commands::session_tab_activate,
            commands::session_tab_close,
            commands::session_tabs_reorder,
            commands::session_tab_update
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitAcorn");
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("git_acorn=info,warn"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();

    tracing::info!("GitAcorn core initialized");
}
