use std::path::PathBuf;

use app_core::AppErrorDto;
use tauri::{AppHandle, State};

use crate::dto::{AppInfoDto, CommandResult, RepositorySnapshotDto};
use crate::state::ApplicationState;

#[tauri::command]
pub fn app_info() -> AppInfoDto {
    AppInfoDto::current()
}

#[tauri::command]
pub fn repository_open(
    path: String,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .open_repository(&PathBuf::from(path), &app)
        .map(RepositorySnapshotDto::from)
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
