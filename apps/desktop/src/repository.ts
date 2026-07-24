import { invoke } from "@tauri-apps/api/core";
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
  active: boolean;
  page: "changes" | "history";
  selectedPath?: string;
  selectedDiff: DiffTarget;
  panelWidth: number;
  historyCursor?: string;
  selectedCommit?: string;
  historyFilter?: string;
  unavailable: boolean;
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
  lane: number;
  laneCount: number;
};

export type HistoryPageDto = {
  schemaVersion: 1;
  commits: CommitDto[];
  nextCursor?: string;
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

export type RepositorySidebarDto = {
  schemaVersion: 1;
  worktrees: Array<{
    id: string;
    path: string;
    head?: string;
    branch?: string;
    isCurrent: boolean;
    isLocked: boolean;
  }>;
  branches: { total: number; items: string[] };
  tags: { total: number; items: string[] };
  stashes: Array<{ reference: string; message: string }>;
};

export async function chooseRepositoryDirectory(): Promise<string | null> {
  const path = await open({
    title: "Open Git repository",
    directory: true,
    multiple: false,
  });
  return path;
}

export function openRepository(path: string): Promise<SessionDto> {
  return invoke<SessionDto>("repository_open", { path });
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
  page: "changes" | "history",
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
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_checkout", { repoId, revision, name });
}

export function deleteBranch(
  repoId: string,
  revision: number,
  name: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_delete", { repoId, revision, name });
}

export function mergeBranch(
  repoId: string,
  revision: number,
  reference: string,
): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("branch_merge", { repoId, revision, reference });
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

export function getDiff(
  repoId: string,
  revision: number,
  pathBytes: number[],
  target: DiffTarget,
): Promise<DiffDto> {
  return invoke<DiffDto>("diff_get", { repoId, revision, pathBytes, target });
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
