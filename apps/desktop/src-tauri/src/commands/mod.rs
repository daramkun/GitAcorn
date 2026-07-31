use std::path::PathBuf;

use app_core::{
    AppError, AppErrorDto, ConflictResolution, DiffTarget, HistoryFilter, PatchSelection,
};
use git_domain::RepositorySnapshot;
use tauri::{AppHandle, Manager, State, ipc::Channel};
use uuid::Uuid;

use crate::dto::{
    AppInfoDto, BranchRequestDto, CloneRequestDto, CommandResult, CommitFileDto, CommitRequestDto,
    DiffDto, GitIdentityDto, GitIdentitySettingsDto, GitIdentityUpdateDto, GitRemoteDto,
    HistoryPageDto, InteractiveRebasePreviewDto, InteractiveRebaseRequestDto, OperationEventDto,
    OperationRecordDto, OperationStartedDto, PatchSelectionDto, ReferenceDto,
    RemoteMutationRequestDto, RemoteReferenceDeleteDto, RemoteRequestDto, RemoteTagDto,
    RepositoryGitIdentityDto, RepositoryOpenSourceDto, RepositorySidebarDto, RepositorySnapshotDto,
    SessionDto, SessionTabUpdateDto, StashRequestDto, SubmoduleAddRequestDto,
};
use crate::state::{ApplicationState, RepositoryOpenSource, SessionTabUpdate};

#[tauri::command]
pub fn app_info() -> AppInfoDto {
    AppInfoDto::current()
}

#[tauri::command]
pub fn git_identity_get(
    repo_id: Option<String>,
    state: State<'_, ApplicationState>,
) -> CommandResult<GitIdentitySettingsDto> {
    let repository = repo_id
        .as_deref()
        .map(|repo_id| state.repository_identity(repo_id))
        .transpose()
        .map_err(|error| AppErrorDto::from(&error))?;
    let global = match &repository {
        Some(repository) => repository.settings.global.clone(),
        None => state
            .global_identity()
            .map_err(|error| AppErrorDto::from(&error))?,
    };
    Ok(GitIdentitySettingsDto {
        schema_version: 1,
        global: global.into(),
        repository: repository.map(RepositoryGitIdentityDto::from),
    })
}

#[tauri::command]
pub fn git_identity_update_global(
    request: GitIdentityUpdateDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<GitIdentityDto> {
    state
        .update_global_identity(request.name.as_deref(), request.email.as_deref())
        .map(GitIdentityDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn git_identity_update_repository(
    repo_id: String,
    request: GitIdentityUpdateDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositoryGitIdentityDto> {
    state
        .update_repository_identity(&repo_id, request.name.as_deref(), request.email.as_deref())
        .map(RepositoryGitIdentityDto::from)
        .map_err(|error| AppErrorDto::from(&error))
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
    opened_from: Option<RepositoryOpenSourceDto>,
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    let opened_from = opened_from.map(|source| RepositoryOpenSource {
        repository_name: source.repository_name,
        worktree_path: source.worktree_path,
    });
    state
        .open_repository(&PathBuf::from(path), opened_from, &app)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub async fn session_restore(
    app: AppHandle,
    state: State<'_, ApplicationState>,
) -> CommandResult<SessionDto> {
    let session = state
        .restore_session(&app)
        .await
        .map(SessionDto::from)
        .map_err(|error| AppErrorDto::from(&error))?;
    let background_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = background_app.state::<ApplicationState>();
        let _ = state.backfill_submodule_sources().await;
    });
    Ok(session)
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
pub fn remote_tags_list(
    repo_id: String,
    remote: Option<String>,
    state: State<'_, ApplicationState>,
) -> CommandResult<Vec<RemoteTagDto>> {
    state
        .repository_remote_tags(&repo_id, remote.as_deref())
        .map(|tags| tags.into_iter().map(RemoteTagDto::from).collect())
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn remotes_list(
    repo_id: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<Vec<GitRemoteDto>> {
    state
        .repository_remotes(&repo_id)
        .map(|remotes| remotes.into_iter().map(GitRemoteDto::from).collect())
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn remote_add(
    repo_id: String,
    revision: u64,
    request: RemoteMutationRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .add_remote(&repo_id, revision, &request.name, &request.url)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn remote_update(
    repo_id: String,
    revision: u64,
    existing_name: String,
    request: RemoteMutationRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .update_remote(
            &repo_id,
            revision,
            &existing_name,
            &request.name,
            &request.url,
        )
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn remote_remove(
    repo_id: String,
    revision: u64,
    name: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .remove_remote(&repo_id, revision, &name)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn submodule_add(
    repo_id: String,
    revision: u64,
    request: SubmoduleAddRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .add_submodule(&repo_id, revision, &request.url, &request.path)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn submodule_initialize(
    repo_id: String,
    revision: u64,
    path: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .initialize_submodule(&repo_id, revision, &path)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn submodule_deinitialize(
    repo_id: String,
    revision: u64,
    path: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .deinitialize_submodule(&repo_id, revision, &path)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn submodule_remove(
    repo_id: String,
    revision: u64,
    path: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .remove_submodule(&repo_id, revision, &path)
        .map(RepositorySnapshotDto::from)
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
    is_remote: bool,
    is_tag: bool,
    auto_stash: bool,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .checkout_branch(&repo_id, revision, &name, is_remote, is_tag, auto_stash)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_delete(
    repo_id: String,
    revision: u64,
    name: String,
    remote_references: Vec<RemoteReferenceDeleteDto>,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let remote_references = remote_references
        .into_iter()
        .map(|reference| (reference.remote, reference.name))
        .collect::<Vec<_>>();
    state
        .delete_branch(&repo_id, revision, &name, &remote_references)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_rename(
    repo_id: String,
    revision: u64,
    old_name: String,
    new_name: String,
    rename_remote: bool,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .rename_branch(&repo_id, revision, &old_name, &new_name, rename_remote)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn branch_rebase(
    repo_id: String,
    revision: u64,
    reference: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .rebase_onto(&repo_id, revision, &reference)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn interactive_rebase_preview(
    repo_id: String,
    revision: u64,
    base_oid: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<InteractiveRebasePreviewDto> {
    state
        .interactive_rebase_preview(&repo_id, revision, &base_oid)
        .map(InteractiveRebasePreviewDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn interactive_rebase_start(
    repo_id: String,
    revision: u64,
    request: InteractiveRebaseRequestDto,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let request = request
        .try_into()
        .map_err(|error: AppError| AppErrorDto::from(&error))?;
    let executable = std::env::current_exe().map_err(|error| {
        AppErrorDto::from(&AppError::InvalidRequest(format!(
            "Could not locate the GitAcorn executable: {error}"
        )))
    })?;
    state
        .start_interactive_rebase(&repo_id, revision, &request, &executable)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn rebase_continue(
    repo_id: String,
    revision: u64,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .continue_rebase(&repo_id, revision)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn rebase_skip(
    repo_id: String,
    revision: u64,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .skip_rebase(&repo_id, revision)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn rebase_abort(
    repo_id: String,
    revision: u64,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .abort_rebase(&repo_id, revision)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn tag_create(
    repo_id: String,
    revision: u64,
    name: String,
    target: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .create_tag(&repo_id, revision, &name, &target)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn tag_delete(
    repo_id: String,
    revision: u64,
    name: String,
    remote_references: Vec<RemoteReferenceDeleteDto>,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    let remote_references = remote_references
        .into_iter()
        .map(|reference| (reference.remote, reference.name))
        .collect::<Vec<_>>();
    state
        .delete_tag(&repo_id, revision, &name, &remote_references)
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
pub fn branch_fast_forward(
    repo_id: String,
    revision: u64,
    branch: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<RepositorySnapshotDto> {
    state
        .fast_forward_branch(&repo_id, revision, &branch)
        .map(RepositorySnapshotDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn diff_get(
    repo_id: String,
    path_bytes: Vec<u8>,
    target: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<DiffDto> {
    state
        .repository_diff(&repo_id, &path_bytes, parse_diff_target(&target)?)
        .map(DiffDto::from)
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn commit_files(
    repo_id: String,
    revision: String,
    state: State<'_, ApplicationState>,
) -> CommandResult<Vec<CommitFileDto>> {
    state
        .repository_commit_files(&repo_id, &revision)
        .map(|files| files.into_iter().map(CommitFileDto::from).collect())
        .map_err(|error| AppErrorDto::from(&error))
}

#[tauri::command]
pub fn commit_diff_get(
    repo_id: String,
    revision: String,
    path_bytes: Vec<u8>,
    state: State<'_, ApplicationState>,
) -> CommandResult<DiffDto> {
    state
        .repository_commit_diff(&repo_id, &revision, &path_bytes)
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
