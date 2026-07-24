mod commands;
mod dto;
mod state;

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
            app.manage(state::ApplicationState::new(session));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::repository_open,
            commands::repository_snapshot,
            commands::repository_sidebar,
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
