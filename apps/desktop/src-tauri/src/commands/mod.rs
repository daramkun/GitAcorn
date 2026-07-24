use std::path::PathBuf;

use app_core::AppErrorDto;
use tauri::{AppHandle, State};

use crate::dto::{
    AppInfoDto, CommandResult, RepositorySidebarDto, RepositorySnapshotDto, SessionDto,
};
use crate::state::ApplicationState;

#[tauri::command]
pub fn app_info() -> AppInfoDto {
    AppInfoDto::current()
}

#[tauri::command]
pub async fn repository_open(
    path: String,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    state
        .open_repository(&PathBuf::from(path), &app)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_restore(
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    state
        .restore_session(&app)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn repository_snapshot(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .repository_snapshot(&repo_id)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn repository_sidebar(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySidebarDto> {
    state
        .repository_sidebar(&repo_id)
        .map(RepositorySidebarDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn worktree_activate(
    repo_id: String,
    worktree_id: String,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    state
        .activate_worktree(&repo_id, &worktree_id, &app)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_tab_activate(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .activate_tab(&repo_id)
        .await
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_tab_close(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    state
        .close_tab(&repo_id)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_tabs_reorder(
    repo_ids: Vec<String>,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .reorder_tabs(&repo_ids)
        .await
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_tab_update(
    repo_id: String,
    page: String,
    selected_path: Option<String>,
    panel_width: f64,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .update_tab(&repo_id, page, selected_path, panel_width)
        .await
        .map_err(|error| AppErrorDto::from(&error))
}
