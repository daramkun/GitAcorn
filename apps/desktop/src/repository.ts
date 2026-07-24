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

export async function chooseRepositoryDirectory(): Promise<string | null> {
  const path = await open({
    title: "Open Git repository",
    directory: true,
    multiple: false,
  });
  return path;
}

export function openRepository(path: string): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("repository_open", { path });
}

export function getRepositorySnapshot(repoId: string): Promise<RepositorySnapshotDto> {
  return invoke<RepositorySnapshotDto>("repository_snapshot", { repoId });
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
