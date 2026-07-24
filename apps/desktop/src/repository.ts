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
  panelWidth: number;
  unavailable: boolean;
  snapshot?: RepositorySnapshotDto;
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
  panelWidth: number,
): Promise<void> {
  return invoke("session_tab_update", { repoId, page, selectedPath, panelWidth });
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
