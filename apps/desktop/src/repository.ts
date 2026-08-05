import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export type AppErrorDto = {
  schemaVersion: 1;
  code: string;
  message: string;
  details?: string;
  recoveryActions: string[];
};

export type RepositorySnapshotDto = {
  schemaVersion: 1;
  revision: number;
  repository: {
    id: string;
    name: string;
    worktreePath: string;
    gitDir: string;
  };
  head: {
    kind: "unborn" | "detached" | "branch";
    name?: string;
    oid?: string;
  };
  upstream?: string;
  ahead: number;
  behind: number;
  stashCount: number;
  changes: FileChangeDto[];
  operation?:
    | "rebase"
    | "rebaseEdit"
    | "autostashConflict"
    | "cherryPick"
    | "revert";
};

export type FileChangeDto = {
  path: string;
  pathBytes: number[];
  originalPath?: string;
  indexStatus: string;
  worktreeStatus: string;
  conflict: boolean;
  submodule: boolean;
};

export type DiffTarget = "unstaged" | "staged";

export type DiffDto = {
  schemaVersion: 1;
  binary: boolean;
  oldPath: string;
  newPath: string;
  hunks: Array<{
    index: number;
    header: string;
    oldStart: number;
    oldCount: number;
    newStart: number;
    newCount: number;
    lines: Array<{
      index: number;
      kind: "context" | "addition" | "deletion" | "noNewline";
      oldLine?: number;
      newLine?: number;
      content: string;
      selectable: boolean;
    }>;
  }>;
};

export type CompareFileDto = {
  oldPath: string;
  newPath: string;
  binary: boolean;
  hunks: DiffDto["hunks"];
};

export type CompareDto = {
  schemaVersion: 1;
  files: CompareFileDto[];
};

export type ComparePatchDto = {
  schemaVersion: number;
  patch: string;
  fileCount: number;
  binary: boolean;
};

export type PatchValidationDto = {
  schemaVersion: number;
  valid: boolean;
  message?: string | null;
};

export type ExternalDiffToolDto = {
  schemaVersion: number;
  configured?: string | null;
  mergeConfigured?: string | null;
};

export type ExternalDiffResultDto = {
  schemaVersion: number;
  tool: string;
  exitCode: number;
};

export type LfsFileStatusDto = {
  path: string;
  oid?: string | null;
  size?: number | null;
  downloaded: boolean;
};

export type LfsStatusDto = {
  schemaVersion: number;
  installed: boolean;
  tracked: LfsFileStatusDto[];
};

export type LfsLockDto = {
  id: string;
  path: string;
  owner: string;
  lockedAt?: string | null;
};

export type SignatureStatusDto = {
  schemaVersion: number;
  revision: string;
  kind: string;
  status: string;
  signer?: string | null;
  keyId?: string | null;
  fingerprint?: string | null;
};

export type SignatureSettingsDto = {
  schemaVersion: number;
  commitSign: boolean;
  tagSign: boolean;
  format?: string | null;
  signingKey?: string | null;
  sshAllowedSignersFile?: string | null;
};

export type BinaryPreviewDto = {
  schemaVersion: number;
  oldPath: string;
  newPath: string;
  mimeType?: string | null;
  oldSize?: number | null;
  newSize?: number | null;
  oldDataUrl?: string | null;
  newDataUrl?: string | null;
};

export type FileBlameDto = {
  schemaVersion: 1;
  path: number[];
  revision?: string;
  lines: Array<{
    line: number;
    commitOid: string;
    authorName: string;
    authorEmail: string;
    authoredAt: number;
    content: string;
  }>;
};

export type PathHistoryDto = {
  schemaVersion: 1;
  path: number[];
  isDirectory: boolean;
  entries: Array<{
    oid: string;
    parentOid?: string;
    authorName: string;
    authorEmail: string;
    authoredAt: number;
    subject: string;
    path: number[];
    previousPath?: number[];
    status: string;
  }>;
  nextCursor?: string;
};

export type PatchSelection = {
  hunkIndex: number;
  lineIndices: number[];
};

export type SessionDto = {
  schemaVersion: 1;
  tabs: SessionTabDto[];
};

export type SessionTabDto = {
  repoId: string;
  worktreeId: string;
  worktreePath: string;
  openedFrom?: {
    repositoryName: string;
    worktreePath: string;
  };
  active: boolean;
  page: "changes" | "history" | "operations";
  selectedPath?: string;
  selectedDiff: DiffTarget;
  panelWidth: number;
  historyCursor?: string;
  selectedCommit?: string;
  historyFilter?: string;
  unavailable: boolean;
  loading?: boolean;
  snapshot?: RepositorySnapshotDto;
};

export type CommitDto = {
  oid: string;
  parents: string[];
  authorName: string;
  authorEmail: string;
  authoredAt: number;
  subject: string;
  body: string;
  references: string[];
  remoteOnly?: boolean;
  lane: number;
  laneCount: number;
};

export type CommitFileDto = {
  path: string;
  pathBytes: number[];
};

export type HistoryPageDto = {
  schemaVersion: 1;
  commits: CommitDto[];
  nextCursor?: string;
};

export type InteractiveRebaseAction =
  | "pick"
  | "reword"
  | "edit"
  | "squash"
  | "fixup"
  | "drop";

export type InteractiveRebasePreviewDto = {
  schemaVersion: 1;
  baseOid: string;
  headOid: string;
  branch: string;
  commits: Array<{
    oid: string;
    subject: string;
  }>;
};

export type InteractiveRebaseRequest = {
  baseOid: string;
  expectedHeadOid: string;
  items: Array<{
    oid: string;
    action: InteractiveRebaseAction;
    summary?: string;
    description?: string;
  }>;
  autoStash: boolean;
};

export type ReferenceDto = {
  fullName: string;
  shortName: string;
  oid: string;
  kind: "localBranch" | "remoteBranch" | "tag";
  upstream?: string;
  ahead: number;
  behind: number;
};

export type RemoteTagDto = {
  remote: string;
  name: string;
  oid: string;
};

export type ReflogEntryDto = {
  schemaVersion: 1;
  selector: string;
  oid: string;
  message: string;
  parents: string[];
  authorName: string;
  authorEmail: string;
  authoredAt: number;
  subject: string;
  body: string;
  reflogOnly: boolean;
};

export type GitRemoteDto = {
  name: string;
  url: string;
};

export type GitIdentityDto = {
  name?: string;
  email?: string;
};

export type RepositoryGitIdentityDto = {
  repoId: string;
  repositoryName: string;
  local: GitIdentityDto;
  effective: GitIdentityDto;
};

export type GitIdentitySettingsDto = {
  schemaVersion: 1;
  global: GitIdentityDto;
  repository?: RepositoryGitIdentityDto;
};

export type RemoteReferenceDeleteTarget = {
  remote: string;
  name: string;
};

export type RepositorySidebarDto = {
  schemaVersion: 1;
  worktrees: Array<{
    id: string;
    path: string;
    head?: string;
    branch?: string;
    isCurrent: boolean;
    isLocked: boolean;
    isPrunable?: boolean;
    isMissing?: boolean;
  }>;
  submodules?: Array<{
    path: string;
    absolutePath: string;
    initialized: boolean;
  }>;
  branches: { total: number; items: string[] };
  remoteBranches: { total: number; items: string[] };
  tags: { total: number; items: string[] };
  stashes: Array<{ reference: string; message: string }>;
};

export type OperationEventDto = {
  schemaVersion: 1;
  operationId: string;
  repoId?: string;
  kind: "clone" | "fetch" | "pull" | "push" | "lfs-fetch" | "lfs-pull" | "lfs-prune";
  state: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  message?: string;
  stream?: "stdout" | "stderr";
  snapshot?: RepositorySnapshotDto;
  destination?: string;
  error?: AppErrorDto;
};

export type OperationStartedDto = {
  schemaVersion: 1;
  operationId: string;
};

export type OperationRecordDto = {
  schemaVersion: number;
  id: string;
  repoId?: string;
  kind: string;
  state: "running" | "succeeded" | "failed" | "cancelled" | "interrupted";
  summary: string;
  diagnostic?: string;
  startedAt: string;
  finishedAt?: string;
    recoveryAction?:
      | "commit"
      | "checkout"
      | "branch-delete"
      | "rebase"
      | "interactive-rebase"
      | "reset-soft"
      | "reset-mixed"
      | "reset-hard"
      | "cherry-pick"
      | "revert";
  recoveryState?: "ready" | "undone";
};

export type RemoteOperationOptions = {
  remote?: string;
  fetchTags?: boolean;
  autoStash?: boolean;
  fastForwardOnly?: boolean;
  forceWithLease?: boolean;
};

export async function chooseRepositoryDirectory(): Promise<string | null> {
  const path = await open({
    title: "Open Git repository",
    directory: true,
    multiple: false,
  });
  return path;
}

export function openRepository(
  path: string,
  openedFrom?: { repositoryName: string; worktreePath: string },
): Promise<SessionDto> {
  return invoke<SessionDto>("repository_open", { path, openedFrom });
}

export async function chooseCloneParentDirectory(): Promise<string | null> {
  return open({
    title: "Choose clone destination",
    directory: true,
    multiple: false,
  });
}

export function startRemoteOperation(
  repoId: string,
  kind: "fetch" | "pull" | "push",
  onEvent: (event: OperationEventDto) => void,
  options: RemoteOperationOptions = {},
): Promise<OperationStartedDto> {
  const channel = new Channel<OperationEventDto>();
  channel.onmessage = onEvent;
  return invoke<OperationStartedDto>("remote_sync", {
    repoId,
    request: { kind, ...options },
    channel,
  });
}

export function startClone(
  remoteUrl: string,
  destination: string,
  onEvent: (event: OperationEventDto) => void,
): Promise<OperationStartedDto> {
  const channel = new Channel<OperationEventDto>();
  channel.onmessage = onEvent;
  return invoke<OperationStartedDto>("repository_clone", {
    request: { remoteUrl, destination },
    channel,
  });
}

export function cancelOperation(operationId: string): Promise<void> {
  return invoke("operation_cancel", { operationId });
}

export function restoreSession(): Promise<SessionDto> {
  return invoke<SessionDto>("session_restore");
}

export function activateSessionTab(repoId: string): Promise<void> {
  return invoke("session_tab_activate", { repoId });
}

export function closeSessionTab(repoId: string): Promise<SessionDto> {
  return invoke<SessionDto>("session_tab_close", { repoId });
}

export function reorderSessionTabs(repoIds: string[]): Promise<void> {
  return invoke("session_tabs_reorder", { repoIds });
}

export function updateSessionTab(
  repoId: string,
  page: "changes" | "history" | "operations",
  selectedPath: string | undefined,
  selectedDiff: DiffTarget,
  panelWidth: number,
  historyCursor?: string,
  selectedCommit?: string,
  historyFilter?: string,
): Promise<void> {
  return invoke("session_tab_update", {
    repoId,
    update: {
      page,
      selectedPath,
      selectedDiff,
      panelWidth,
      historyCursor,
      selectedCommit,
      historyFilter,
    },
  });
}

export function getHistoryPage(
  repoId: string,
  cursor?: string,
  reference?: string,
  query?: string,
  author?: string,
  limit = 100,
): Promise<HistoryPageDto> {
  return invoke<HistoryPageDto>("history_page", {
    repoId,
    cursor,
    reference,
    query,
    author,
    limit,
  });
}

export function getReferences(repoId: string): Promise<ReferenceDto[]> {
  return invoke<ReferenceDto[]>("references_list", { repoId });
}

export function getReflog(repoId: string): Promise<ReflogEntryDto[]> {
  return invoke<ReflogEntryDto[]>("reflog_list", { repoId });
}

export function restoreReflogReference(
  repoId: string,
  revision: number,
  oid: string,
  name: string,
  isTag: boolean,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("reflog_restore", {
    repoId,
    revision,
    oid,
    name,
    isTag,
  });
}

export function getCommitFiles(
  repoId: string,
  revision: string,
): Promise<CommitFileDto[]> {
  return invoke<CommitFileDto[]>("commit_files", { repoId, revision });
}

export function getCommitDiff(
  repoId: string,
  revision: string,
  pathBytes: number[],
): Promise<DiffDto> {
  return invoke<DiffDto>("commit_diff_get", { repoId, revision, pathBytes });
}

export function getRemoteTags(
  repoId: string,
  remote?: string,
): Promise<RemoteTagDto[]> {
  return invoke<RemoteTagDto[]>("remote_tags_list", { repoId, remote });
}

export function getRemotes(repoId: string): Promise<GitRemoteDto[]> {
  return invoke<GitRemoteDto[]>("remotes_list", { repoId });
}

export function getGitIdentity(repoId?: string): Promise<GitIdentitySettingsDto> {
  return invoke<GitIdentitySettingsDto>("git_identity_get", { repoId });
}

export function updateGlobalGitIdentity(
  request: GitIdentityDto,
): Promise<GitIdentityDto> {
  return invoke<GitIdentityDto>("git_identity_update_global", { request });
}

export function updateRepositoryGitIdentity(
  repoId: string,
  request: GitIdentityDto,
): Promise<RepositoryGitIdentityDto> {
  return invoke<RepositoryGitIdentityDto>("git_identity_update_repository", {
    repoId,
    request,
  });
}

export function addRemote(
  repoId: string,
  revision: number,
  request: GitRemoteDto,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("remote_add", { repoId, revision, request });
}

export function updateRemote(
  repoId: string,
  revision: number,
  existingName: string,
  request: GitRemoteDto,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("remote_update", {
    repoId,
    revision,
    existingName,
    request,
  });
}

export function removeRemote(
  repoId: string,
  revision: number,
  name: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("remote_remove", { repoId, revision, name });
}

export function addSubmodule(
  repoId: string,
  revision: number,
  request: { url: string; path: string },
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("submodule_add", {
    repoId,
    revision,
    request,
  });
}

export function initializeSubmodule(
  repoId: string,
  revision: number,
  path: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("submodule_initialize", {
    repoId,
    revision,
    path,
  });
}

export function deinitializeSubmodule(
  repoId: string,
  revision: number,
  path: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("submodule_deinitialize", {
    repoId,
    revision,
    path,
  });
}

export function removeSubmodule(
  repoId: string,
  revision: number,
  path: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("submodule_remove", {
    repoId,
    revision,
    path,
  });
}

export function createBranch(
  repoId: string,
  revision: number,
  request: { name: string; startPoint?: string },
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_create", { repoId, revision, request });
}

export function checkoutBranch(
  repoId: string,
  revision: number,
  name: string,
  isRemote = false,
  isTag = false,
  autoStash = false,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_checkout", {
    repoId,
    revision,
    name,
    isRemote,
    isTag,
    autoStash,
  });
}

export function deleteBranch(
  repoId: string,
  revision: number,
  name: string,
  remoteReferences: RemoteReferenceDeleteTarget[] = [],
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_delete", {
    repoId,
    revision,
    name,
    remoteReferences,
  });
}

export function renameBranch(
  repoId: string,
  revision: number,
  oldName: string,
  newName: string,
  renameRemote: boolean,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_rename", {
    repoId,
    revision,
    oldName,
    newName,
    renameRemote,
  });
}

export function rebaseBranch(
  repoId: string,
  revision: number,
  reference: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_rebase", {
    repoId,
    revision,
    reference,
  });
}

export function resetBranch(
  repoId: string,
  revision: number,
  targetOid: string,
  mode: "soft" | "mixed" | "hard",
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_reset", {
    repoId,
    revision,
    targetOid,
    mode,
  });
}

export type HistoryMutation = "cherry-pick" | "revert";

export function mutateHistory(
  repoId: string,
  revision: number,
  operation: HistoryMutation,
  oids: string[],
  mainline?: number,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("history_mutate", {
    repoId,
    revision,
    operation,
    oids,
    mainline,
  });
}

export function continueHistory(
  repoId: string,
  revision: number,
  operation: HistoryMutation,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("history_continue", {
    repoId,
    revision,
    operation,
  });
}

export function abortHistory(
  repoId: string,
  revision: number,
  operation: HistoryMutation,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("history_abort", {
    repoId,
    revision,
    operation,
  });
}

export function skipHistory(
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("history_skip", {
    repoId,
    revision,
  });
}

export function previewInteractiveRebase(
  repoId: string,
  revision: number,
  baseOid: string,
): Promise<InteractiveRebasePreviewDto> {
  return invoke<InteractiveRebasePreviewDto>("interactive_rebase_preview", {
    repoId,
    revision,
    baseOid,
  });
}

export function startInteractiveRebase(
  repoId: string,
  revision: number,
  request: InteractiveRebaseRequest,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("interactive_rebase_start", {
    repoId,
    revision,
    request,
  });
}

export function continueRebase(
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("rebase_continue", { repoId, revision });
}

export function skipRebase(
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("rebase_skip", { repoId, revision });
}

export function abortRebase(
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("rebase_abort", { repoId, revision });
}

export function createTag(
  repoId: string,
  revision: number,
  name: string,
  target: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("tag_create", {
    repoId,
    revision,
    name,
    target,
  });
}

export function deleteTag(
  repoId: string,
  revision: number,
  name: string,
  remoteReferences: RemoteReferenceDeleteTarget[] = [],
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("tag_delete", {
    repoId,
    revision,
    name,
    remoteReferences,
  });
}

export function mergeBranch(
  repoId: string,
  revision: number,
  reference: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_merge", { repoId, revision, reference });
}

export function fastForwardBranch(
  repoId: string,
  revision: number,
  branch: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_fast_forward", {
    repoId,
    revision,
    branch,
  });
}

export function createStash(
  repoId: string,
  revision: number,
  message: string,
  paths: number[][] = [],
): Promise<RepositorySnapshotDto> {
  return invoke("stash_create", {
    repoId,
    revision,
    request: { message, includeUntracked: true, paths },
  });
}

export function applyStash(
  repoId: string,
  revision: number,
  reference: string,
): Promise<RepositorySnapshotDto> {
  return invoke("stash_apply", { repoId, revision, reference });
}

export function dropStash(
  repoId: string,
  revision: number,
  reference: string,
): Promise<RepositorySnapshotDto> {
  return invoke("stash_drop", { repoId, revision, reference });
}

export function resolveConflict(
  repoId: string,
  revision: number,
  pathBytes: number[],
  resolution: "ours" | "theirs" | "markResolved",
): Promise<RepositorySnapshotDto> {
  return invoke("conflict_resolve", {
    repoId,
    revision,
    pathBytes,
    resolution,
  });
}

export function abortMerge(
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke("merge_abort", { repoId, revision });
}

export function getOperationHistory(): Promise<OperationRecordDto[]> {
  return invoke("operation_history");
}

export function undoOperation(
  operationId: string,
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke("operation_undo", { operationId, repoId, revision });
}

export function redoOperation(
  operationId: string,
  repoId: string,
  revision: number,
): Promise<RepositorySnapshotDto> {
  return invoke("operation_redo", { operationId, repoId, revision });
}

export function getDiagnostics(): Promise<string> {
  return invoke("diagnostics_copy");
}

export function getRepositorySnapshot(repoId: string): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("repository_snapshot", { repoId });
}

export function getRepositorySidebar(repoId: string): Promise<RepositorySidebarDto> {
  return invoke<RepositorySidebarDto>("repository_sidebar", { repoId });
}

export function activateWorktree(repoId: string, worktreeId: string): Promise<SessionDto> {
  return invoke<SessionDto>("worktree_activate", { repoId, worktreeId });
}

export function createWorktree(
  repoId: string,
  revision: number,
  request: { path: string; branch?: string; startPoint?: string },
): Promise<RepositorySidebarDto> {
  return invoke<RepositorySidebarDto>("worktree_create", {
    repoId,
    revision,
    request,
  });
}

export function lockWorktree(
  repoId: string,
  revision: number,
  worktreeId: string,
  reason?: string,
): Promise<RepositorySidebarDto> {
  return invoke<RepositorySidebarDto>("worktree_lock", {
    repoId,
    revision,
    worktreeId,
    reason,
  });
}

export function unlockWorktree(
  repoId: string,
  revision: number,
  worktreeId: string,
): Promise<RepositorySidebarDto> {
  return invoke<RepositorySidebarDto>("worktree_unlock", {
    repoId,
    revision,
    worktreeId,
  });
}

export function removeWorktree(
  repoId: string,
  revision: number,
  worktreeId: string,
  force: boolean,
): Promise<RepositorySidebarDto> {
  return invoke<RepositorySidebarDto>("worktree_remove", {
    repoId,
    revision,
    worktreeId,
    force,
  });
}

export function getDiff(
  repoId: string,
  pathBytes: number[],
  target: DiffTarget,
): Promise<DiffDto> {
  return invoke<DiffDto>("diff_get", { repoId, pathBytes, target });
}

export function compareDiff(
  repoId: string,
  left: string,
  right: string,
): Promise<CompareDto> {
  return invoke<CompareDto>("compare_get", { repoId, request: { left, right } });
}

export function getComparePatch(
  repoId: string,
  left: string,
  right: string,
): Promise<ComparePatchDto> {
  return invoke<ComparePatchDto>("compare_patch_get", { repoId, request: { left, right } });
}

export function validateComparePatch(repoId: string, patch: string): Promise<PatchValidationDto> {
  return invoke<PatchValidationDto>("compare_patch_validate", { repoId, patch });
}

export function applyComparePatch(
  repoId: string,
  revision: number,
  patch: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("compare_patch_apply", { repoId, revision, patch });
}

export function saveComparePatch(repoId: string, path: string, patch: string): Promise<void> {
  return invoke<void>("compare_patch_save", { repoId, path, patch });
}

export function getExternalDiffTool(repoId: string): Promise<ExternalDiffToolDto> {
  return invoke<ExternalDiffToolDto>("external_diff_tool_get", { repoId });
}

export function updateExternalDiffTool(
  repoId: string,
  tool?: string,
  mergeTool?: string,
): Promise<ExternalDiffToolDto> {
  return invoke<ExternalDiffToolDto>("external_diff_tool_update", {
    repoId,
    request: { tool: tool?.trim() || null, mergeTool: mergeTool?.trim() || null },
  });
}

export function runExternalDiff(
  repoId: string,
  left: string,
  right: string,
): Promise<ExternalDiffResultDto> {
  return invoke<ExternalDiffResultDto>("external_diff_run", {
    repoId,
    request: { left, right },
  });
}

export function runExternalMerge(repoId: string): Promise<ExternalDiffResultDto> {
  return invoke<ExternalDiffResultDto>("external_merge_run", { repoId });
}

export function getBinaryPreview(
  repoId: string,
  left: string,
  right: string,
  oldPath: string,
  newPath: string,
): Promise<BinaryPreviewDto> {
  return invoke<BinaryPreviewDto>("binary_preview_get", {
    repoId,
    request: { left, right, oldPath, newPath },
  });
}

export function startLfsSync(
  repoId: string,
  kind: "fetch" | "pull" | "prune",
  onEvent: (event: OperationEventDto) => void,
  remote?: string,
): Promise<OperationStartedDto> {
  const channel = new Channel<OperationEventDto>();
  channel.onmessage = onEvent;
  return invoke<OperationStartedDto>("lfs_sync", {
    repoId,
    request: { kind, remote: remote?.trim() || null },
    channel,
  });
}

export function getLfsStatus(repoId: string): Promise<LfsStatusDto> {
  return invoke<LfsStatusDto>("lfs_status_get", { repoId });
}

export function getLfsLocks(repoId: string): Promise<LfsLockDto[]> {
  return invoke<LfsLockDto[]>("lfs_locks_get", { repoId });
}

export function lockLfsPath(repoId: string, path: string): Promise<LfsLockDto[]> {
  return invoke<LfsLockDto[]>("lfs_lock", { repoId, path });
}

export function unlockLfsPath(
  repoId: string,
  path?: string,
  lockId?: string,
): Promise<LfsLockDto[]> {
  return invoke<LfsLockDto[]>("lfs_unlock", {
    repoId,
    request: { path: path?.trim() || null, lockId: lockId?.trim() || null },
  });
}

export function getSignatureStatus(
  repoId: string,
  revision: string,
  kind: "commit" | "tag",
): Promise<SignatureStatusDto> {
  return invoke<SignatureStatusDto>("signature_status_get", { repoId, revision, kind });
}

export function getSignatureSettings(repoId: string): Promise<SignatureSettingsDto> {
  return invoke<SignatureSettingsDto>("signature_settings_get", { repoId });
}

export function updateSignatureSettings(
  repoId: string,
  settings: Omit<SignatureSettingsDto, "schemaVersion">,
): Promise<SignatureSettingsDto> {
  return invoke<SignatureSettingsDto>("signature_settings_update", {
    repoId,
    request: settings,
  });
}

export function getFileBlame(
  repoId: string,
  pathBytes: number[],
  revision?: string,
): Promise<FileBlameDto> {
  return invoke<FileBlameDto>("blame_get", { repoId, pathBytes, revision });
}

export function getPathHistory(
  repoId: string,
  pathBytes: number[],
  isDirectory: boolean,
  query?: string,
  limit = 100,
): Promise<PathHistoryDto> {
  return invoke<PathHistoryDto>("path_history_get", {
    repoId,
    pathBytes,
    isDirectory,
    query,
    limit,
  });
}

export function stagePaths(
  repoId: string,
  revision: number,
  paths: number[][],
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("stage_paths", { repoId, revision, paths });
}

export function unstagePaths(
  repoId: string,
  revision: number,
  paths: number[][],
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("unstage_paths", { repoId, revision, paths });
}

export function applyPatchSelection(
  repoId: string,
  revision: number,
  pathBytes: number[],
  target: DiffTarget,
  selections: PatchSelection[],
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("apply_patch_selection", {
    repoId,
    revision,
    pathBytes,
    target,
    selections,
  });
}

export function discardPath(
  repoId: string,
  revision: number,
  pathBytes: number[],
  untracked: boolean,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("discard_path", {
    repoId,
    revision,
    pathBytes,
    untracked,
  });
}

export function createCommit(
  repoId: string,
  revision: number,
  request: { summary: string; description: string; amend: boolean },
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("commit_create", { repoId, revision, request });
}

export function listenForRepositoryChanges(
  callback: (repoId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("repository-changed", (event) => callback(event.payload));
}

export function normalizeAppError(error: unknown): AppErrorDto {
  if (typeof error === "object" && error !== null && "message" in error) {
    const value = error as Partial<AppErrorDto>;
    return {
      schemaVersion: 1,
      code: value.code ?? "unknown",
      message: String(value.message),
      details: value.details,
      recoveryActions: value.recoveryActions ?? ["retry"],
    };
  }
  return {
    schemaVersion: 1,
    code: "unknown",
    message: error instanceof Error ? error.message : String(error),
    recoveryActions: ["retry"],
  };
}
