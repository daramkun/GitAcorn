mod commands;
mod dto;
mod state;
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
            commands::remote_sync,
            commands::repository_clone,
            commands::operation_cancel,
            commands::repository_open,
            commands::repository_snapshot,
            commands::repository_sidebar,
            commands::history_page,
            commands::references_list,
            commands::remote_tags_list,
            commands::remotes_list,
            commands::remote_add,
            commands::remote_update,
            commands::remote_remove,
            commands::diff_get,
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
            commands::branch_merge,
            commands::tag_create,
            commands::tag_delete,
            commands::stash_create,
            commands::stash_apply,
            commands::stash_drop,
            commands::conflict_resolve,
            commands::merge_abort,
            commands::operation_history,
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
