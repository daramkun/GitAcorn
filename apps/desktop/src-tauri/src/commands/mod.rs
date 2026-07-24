use std::path::PathBuf;

use app_core::{AppError, AppErrorDto, DiffTarget, PatchSelection};
use tauri::{AppHandle, State};

use crate::dto::{
    AppInfoDto, CommandResult, CommitRequestDto, DiffDto, PatchSelectionDto, RepositorySidebarDto,
    RepositorySnapshotDto, SessionDto,
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
    selected_diff: String,
    panel_width: f64,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .update_tab(&repo_id, page, selected_path, selected_diff, panel_width)
        .await
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn diff_get(
    repo_id: String,
    revision: u64,
    path_bytes: Vec<u8>,
    target: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<DiffDto> {
    state
        .repository_diff(&repo_id, revision, &path_bytes, parse_diff_target(&target)?)
        .map(DiffDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn stage_paths(
    repo_id: String,
    revision: u64,
    paths: Vec<Vec<u8>>,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .stage_paths(&repo_id, revision, &paths)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn unstage_paths(
    repo_id: String,
    revision: u64,
    paths: Vec<Vec<u8>>,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .unstage_paths(&repo_id, revision, &paths)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn apply_patch_selection(
    repo_id: String,
    revision: u64,
    path_bytes: Vec<u8>,
    target: String,
    selections: Vec<PatchSelectionDto>,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let selections: Vec<PatchSelection> = selections.into_iter().map(Into::into).collect();
    state
        .apply_selection(
            &repo_id,
            revision,
            &path_bytes,
            parse_diff_target(&target)?,
            &selections,
        )
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn discard_path(
    repo_id: String,
    revision: u64,
    path_bytes: Vec<u8>,
    untracked: bool,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .discard_path(&repo_id, revision, &path_bytes, untracked)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn commit_create(
    repo_id: String,
    revision: u64,
    request: CommitRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .commit(&repo_id, revision, &request.into())
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

fn parse_diff_target(value: &str) -> Result<DiffTarget, AppErrorDto> {
    match value {
        "unstaged" => Ok(DiffTarget::Unstaged),
        "staged" => Ok(DiffTarget::Staged),
        _ => Err(AppErrorDto::from(&AppError::InvalidRequest(
            "Diff target must be staged or unstaged".to_owned(),
        ))),
    }
}
