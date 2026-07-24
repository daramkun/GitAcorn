mod commands;
mod dto;
mod state;

use tracing_subscriber::EnvFilter;

pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::ApplicationState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::repository_open,
            commands::repository_snapshot
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
