use std::path::PathBuf;

use app_core::{
    AppError, AppErrorDto, ConflictResolution, DiffTarget, HistoryFilter, PatchSelection,
};
use git_domain::RepositorySnapshot;
use tauri::{AppHandle, Manager, State, ipc::Channel};
use uuid::Uuid;

use crate::dto::{
    AppInfoDto, BranchRequestDto, CloneRequestDto, CommandResult, CommitRequestDto, DiffDto,
    HistoryPageDto, OperationEventDto, OperationRecordDto, OperationStartedDto, PatchSelectionDto,
    ReferenceDto, RemoteRequestDto, RepositorySidebarDto, RepositorySnapshotDto, SessionDto,
    SessionTabUpdateDto, StashRequestDto,
};
use crate::state::{ApplicationState, SessionTabUpdate};

#[tauri::command]
pub fn app_info() -> AppInfoDto {
    AppInfoDto::current()
}

#[tauri::command]
pub async fn remote_sync(
    repo_id: String,
    request: RemoteRequestDto,
    channel: Channel<OperationEventDto>,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<OperationStartedDto> {
    let request =
        app_core::RemoteRequest::try_from(request).map_err(|error| AppErrorDto::from(&error))?;
    let (operation_id, parsed_repo_id) = state
        .begin_remote_operation(&repo_id)
        .map_err(|error| AppErrorDto::from(&error))?;
    let operation_id_string = operation_id.to_string();
    let event_repo_id = repo_id.clone();
    let kind = request.kind.label().to_owned();
    state
        .start_operation_record(
            &operation_id_string,
            Some(&repo_id),
            &kind,
            &format!("{kind} queued"),
        )
        .await
        .map_err(|error| AppErrorDto::from(&error))?;
    let _ = channel.send(operation_event(
        &operation_id_string,
        Some(event_repo_id.clone()),
        kind.clone(),
        "queued",
    ));
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ApplicationState>();
        let _ = channel.send(operation_event(
            &operation_id_string,
            Some(event_repo_id.clone()),
            kind.clone(),
            "running",
        ));
        let result =
            state.run_remote_operation(operation_id, parsed_repo_id, &request, |progress| {
                let mut event = operation_event(
                    &operation_id_string,
                    Some(event_repo_id.clone()),
                    kind.clone(),
                    "running",
                );
                event.message = Some(progress.message);
                event.stream = Some(progress.stream);
                let _ = channel.send(event);
            });
        let mut event = operation_event(
            &operation_id_string,
            Some(event_repo_id),
            kind,
            match &result {
                Ok(_) => "succeeded",
                Err(app_core::AppError::Cancelled) => "cancelled",
                Err(_) => "failed",
            },
        );
        match result {
            Ok(snapshot) => event.snapshot = Some(snapshot.into()),
            Err(error) => event.error = Some(AppErrorDto::from(&error)),
        }
        let diagnostic = event
            .error
            .as_ref()
            .and_then(|error| error.details.as_deref());
        let summary = event
            .message
            .as_deref()
            .unwrap_or(if event.state == "succeeded" {
                "Operation completed"
            } else {
                "Operation did not complete"
            });
        let _ = tauri::async_runtime::block_on(state.finish_operation_record(
            &operation_id_string,
            event.state,
            summary,
            diagnostic,
        ));
        let _ = channel.send(event);
    });
    Ok(OperationStartedDto {
        schema_version: 1,
        operation_id: operation_id.to_string(),
    })
}

#[tauri::command]
pub async fn repository_clone(
    request: CloneRequestDto,
    channel: Channel<OperationEventDto>,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<OperationStartedDto> {
    let request: app_core::CloneRequest = request.into();
    let destination = request.destination.to_string_lossy().into_owned();
    let operation_id = state.begin_clone_operation();
    let operation_id_string = operation_id.to_string();
    state
        .start_operation_record(&operation_id_string, None, "clone", "Clone queued")
        .await
        .map_err(|error| AppErrorDto::from(&error))?;
    let _ = channel.send(operation_event(
        &operation_id_string,
        None,
        "clone".to_owned(),
        "queued",
    ));
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ApplicationState>();
        let _ = channel.send(operation_event(
            &operation_id_string,
            None,
            "clone".to_owned(),
            "running",
        ));
        let result = state.run_clone_operation(operation_id, &request, |progress| {
            let mut event =
                operation_event(&operation_id_string, None, "clone".to_owned(), "running");
            event.message = Some(progress.message);
            event.stream = Some(progress.stream);
            let _ = channel.send(event);
        });
        let mut event = operation_event(
            &operation_id_string,
            None,
            "clone".to_owned(),
            match &result {
                Ok(_) => "succeeded",
                Err(app_core::AppError::Cancelled) => "cancelled",
                Err(_) => "failed",
            },
        );
        match result {
            Ok(()) => event.destination = Some(destination),
            Err(error) => event.error = Some(AppErrorDto::from(&error)),
        }
        let diagnostic = event
            .error
            .as_ref()
            .and_then(|error| error.details.as_deref());
        let summary = if event.state == "succeeded" {
            "Clone completed"
        } else {
            "Clone did not complete"
        };
        let _ = tauri::async_runtime::block_on(state.finish_operation_record(
            &operation_id_string,
            event.state,
            summary,
            diagnostic,
        ));
        let _ = channel.send(event);
    });
    Ok(OperationStartedDto {
        schema_version: 1,
        operation_id: operation_id.to_string(),
    })
}

#[tauri::command]
pub fn operation_cancel(
    operation_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .cancel_operation(&operation_id)
        .map_err(|error| AppErrorDto::from(&error))
}

fn operation_event(
    operation_id: &str,
    repo_id: Option<String>,
    kind: String,
    state: &'static str,
) -> OperationEventDto {
    OperationEventDto {
        schema_version: 1,
        operation_id: operation_id.to_owned(),
        repo_id,
        kind,
        state,
        message: None,
        stream: None,
        snapshot: None,
        destination: None,
        error: None,
    }
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
    update: SessionTabUpdateDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<()> {
    state
        .update_tab(
            &repo_id,
            SessionTabUpdate {
                page: update.page,
                selected_path: update.selected_path,
                selected_diff: update.selected_diff,
                panel_width: update.panel_width,
                history_cursor: update.history_cursor,
                selected_commit: update.selected_commit,
                history_filter: update.history_filter,
            },
        )
        .await
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn history_page(
    repo_id: String,
    cursor: Option<String>,
    reference: Option<String>,
    query: Option<String>,
    author: Option<String>,
    limit: usize,
    state: State<'_, ApplicationState>,
) -> CommandResult<HistoryPageDto> {
    state
        .repository_history(
            &repo_id,
            &HistoryFilter {
                cursor,
                reference,
                query,
                author,
                limit,
            },
        )
        .map(HistoryPageDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn references_list(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<Vec<ReferenceDto>> {
    state
        .repository_references(&repo_id)
        .map(|references| references.into_iter().map(ReferenceDto::from).collect())
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_create(
    repo_id: String,
    revision: u64,
    request: BranchRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .create_branch(&repo_id, revision, &request.into())
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_checkout(
    repo_id: String,
    revision: u64,
    name: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .checkout_branch(&repo_id, revision, &name)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_delete(
    repo_id: String,
    revision: u64,
    name: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .delete_branch(&repo_id, revision, &name)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_merge(
    repo_id: String,
    revision: u64,
    reference: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .merge_reference(&repo_id, revision, &reference)
        .map(RepositorySnapshotDto::from)
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

#[tauri::command]
pub async fn stash_create(
    repo_id: String,
    revision: u64,
    request: StashRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let request = request.into();
    recorded_mutation(&state, &repo_id, "stash-create", "Created stash", || {
        state.create_stash(&repo_id, revision, &request)
    })
    .await
}

#[tauri::command]
pub async fn stash_apply(
    repo_id: String,
    revision: u64,
    reference: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    recorded_mutation(&state, &repo_id, "stash-apply", "Applied stash", || {
        state.apply_stash(&repo_id, revision, &reference)
    })
    .await
}

#[tauri::command]
pub async fn stash_drop(
    repo_id: String,
    revision: u64,
    reference: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    recorded_mutation(&state, &repo_id, "stash-drop", "Dropped stash", || {
        state.drop_stash(&repo_id, revision, &reference)
    })
    .await
}

#[tauri::command]
pub async fn conflict_resolve(
    repo_id: String,
    revision: u64,
    path_bytes: Vec<u8>,
    resolution: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let resolution = match resolution.as_str() {
        "ours" => ConflictResolution::Ours,
        "theirs" => ConflictResolution::Theirs,
        "markResolved" => ConflictResolution::MarkResolved,
        _ => {
            return Err(AppErrorDto::from(&AppError::InvalidRequest(
                "Conflict resolution must be ours, theirs, or markResolved".to_owned(),
            )));
        }
    };
    recorded_mutation(
        &state,
        &repo_id,
        "conflict-resolve",
        "Resolved conflict",
        || state.resolve_conflict(&repo_id, revision, &path_bytes, resolution),
    )
    .await
}

#[tauri::command]
pub async fn merge_abort(
    repo_id: String,
    revision: u64,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    recorded_mutation(&state, &repo_id, "merge-abort", "Aborted merge", || {
        state.abort_merge(&repo_id, revision)
    })
    .await
}

#[tauri::command]
pub async fn operation_history(
    state: State<'_, ApplicationState>,
) -> CommandResult<Vec<OperationRecordDto>> {
    state
        .operation_history()
        .await
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn diagnostics_copy(state: State<'_, ApplicationState>) -> CommandResult<String> {
    let operations = state
        .operation_history()
        .await
        .map_err(|error| AppErrorDto::from(&error))?;
    let mut output = format!(
        "GitAcorn {}\nOS: {}\nArchitecture: {}\n\nRecent operations:\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for operation in operations.iter().take(20) {
        output.push_str(&format!(
            "{} | {} | {} | {}\n",
            operation.started_at, operation.kind, operation.state, operation.summary
        ));
        if let Some(diagnostic) = &operation.diagnostic {
            output.push_str(&format!("  diagnostic: {diagnostic}\n"));
        }
    }
    Ok(output)
}

async fn recorded_mutation(
    state: &ApplicationState,
    repo_id: &str,
    kind: &str,
    success_summary: &str,
    action: impl FnOnce() -> Result<RepositorySnapshot, AppError>,
) -> CommandResult<RepositorySnapshotDto> {
    let operation_id = Uuid::new_v4().to_string();
    state
        .start_operation_record(&operation_id, Some(repo_id), kind, "Operation started")
        .await
        .map_err(|error| AppErrorDto::from(&error))?;
    let result = action();
    let (operation_state, summary, diagnostic) = match &result {
        Ok(_) => ("succeeded", success_summary.to_owned(), None),
        Err(error) => {
            let dto = AppErrorDto::from(error);
            ("failed", dto.message, dto.details)
        }
    };
    state
        .finish_operation_record(
            &operation_id,
            operation_state,
            &summary,
            diagnostic.as_deref(),
        )
        .await
        .map_err(|error| AppErrorDto::from(&error))?;
    result
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
