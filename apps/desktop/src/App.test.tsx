import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { getAppInfo } from "./app-info";
import {
  activateSessionTab,
  activateWorktree,
  chooseRepositoryDirectory,
  closeSessionTab,
  getRepositorySidebar,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  openRepository,
  reorderSessionTabs,
  restoreSession,
  updateSessionTab,
  type RepositorySnapshotDto,
  type SessionDto,
} from "./repository";

vi.mock("./app-info", () => ({
  getAppInfo: vi.fn(),
}));

vi.mock("./repository", () => ({
  chooseRepositoryDirectory: vi.fn(),
  openRepository: vi.fn(),
  restoreSession: vi.fn(),
  activateSessionTab: vi.fn(),
  activateWorktree: vi.fn(),
  closeSessionTab: vi.fn(),
  getRepositorySidebar: vi.fn(),
  reorderSessionTabs: vi.fn(),
  updateSessionTab: vi.fn(),
  getRepositorySnapshot: vi.fn(),
  listenForRepositoryChanges: vi.fn(),
  normalizeAppError: (error: unknown) => {
    const value = error as { code?: string; message?: string; details?: string };
    return {
      schemaVersion: 1,
      code: value.code ?? "unknown",
      message: value.message ?? String(error),
      details: value.details,
      recoveryActions: ["retry"],
    };
  },
}));

const mockedGetAppInfo = vi.mocked(getAppInfo);
const mockedChooseRepository = vi.mocked(chooseRepositoryDirectory);
const mockedOpenRepository = vi.mocked(openRepository);
const mockedRestoreSession = vi.mocked(restoreSession);
const mockedActivateTab = vi.mocked(activateSessionTab);
const mockedActivateWorktree = vi.mocked(activateWorktree);
const mockedCloseTab = vi.mocked(closeSessionTab);
const mockedGetSidebar = vi.mocked(getRepositorySidebar);
const mockedReorderTabs = vi.mocked(reorderSessionTabs);
const mockedUpdateTab = vi.mocked(updateSessionTab);
const mockedGetSnapshot = vi.mocked(getRepositorySnapshot);
const mockedListenForChanges = vi.mocked(listenForRepositoryChanges);

const snapshot: RepositorySnapshotDto = {
  schemaVersion: 1,
  revision: 1,
  repository: {
    id: "1fbb5062-b9d5-4aa4-a5e7-75bd9cf4dc50",
    name: "acorn-demo",
    worktreePath: "C:\\Code\\acorn-demo",
    gitDir: "C:\\Code\\acorn-demo\\.git",
  },
  head: { kind: "branch", name: "main", oid: "abcdef123456" },
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  stashCount: 0,
  changes: [
    {
      path: "tracked.txt",
      pathBytes: [116, 114, 97, 99, 107, 101, 100, 46, 116, 120, 116],
      indexStatus: ".",
      worktreeStatus: "M",
      conflict: false,
      submodule: false,
    },
    {
      path: "staged file.txt",
      pathBytes: [],
      indexStatus: "A",
      worktreeStatus: ".",
      conflict: false,
      submodule: false,
    },
  ],
};

const sessionWithSnapshot: SessionDto = {
  schemaVersion: 1,
  tabs: [
    {
      repoId: snapshot.repository.id,
      worktreeId: "worktree-one",
      worktreePath: snapshot.repository.worktreePath,
      active: true,
      page: "changes",
      panelWidth: 280,
      unavailable: false,
      snapshot,
    },
  ],
};

describe("App", () => {
  beforeEach(() => {
    mockedGetAppInfo.mockResolvedValue({
      schemaVersion: 1,
      name: "GitAcorn",
      version: "0.1.0",
      runtime: "Tauri 2",
    });
    mockedChooseRepository.mockResolvedValue(null);
    mockedOpenRepository.mockResolvedValue(sessionWithSnapshot);
    mockedRestoreSession.mockResolvedValue({ schemaVersion: 1, tabs: [] });
    mockedActivateTab.mockResolvedValue();
    mockedActivateWorktree.mockResolvedValue(sessionWithSnapshot);
    mockedCloseTab.mockResolvedValue({ schemaVersion: 1, tabs: [] });
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedReorderTabs.mockResolvedValue();
    mockedUpdateTab.mockResolvedValue();
    mockedGetSnapshot.mockResolvedValue(snapshot);
    mockedListenForChanges.mockResolvedValue(vi.fn());
  });

  it("renders the typed app info returned by the Rust core", async () => {
    render(<App />);

    expect(screen.getByText("Connecting to core…")).toBeInTheDocument();
    expect(await screen.findByText("Tauri 2 · v0.1.0")).toBeInTheDocument();
  });

  it("switches between Changes and History", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    expect(screen.getByRole("heading", { name: "History will appear here." })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^History/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(mockedUpdateTab).toHaveBeenCalledWith(
      snapshot.repository.id,
      "history",
      undefined,
      280,
    );
  });

  it("shows a recoverable error state when the core cannot be reached", async () => {
    mockedGetAppInfo.mockRejectedValue(new Error("IPC unavailable"));
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("IPC unavailable");
    expect(screen.getByText("Core unavailable")).toBeInTheDocument();
  });

  it("opens a real repository snapshot and separates staged changes", async () => {
    mockedChooseRepository.mockResolvedValue("C:\\Code\\acorn-demo");
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Open a repository" }));

    expect(await screen.findByText("acorn-demo")).toBeInTheDocument();
    expect(screen.getByText("main · Changes")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /tracked\.txt/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /staged file\.txt/ })).toBeInTheDocument();
    expect(mockedOpenRepository).toHaveBeenCalledWith("C:\\Code\\acorn-demo");
  });

  it("rapidly switches three repositories without mixing page, branch, or selection", async () => {
    const secondSnapshot = {
      ...snapshot,
      repository: {
        ...snapshot.repository,
        id: "2fbb5062-b9d5-4aa4-a5e7-75bd9cf4dc51",
        name: "second-repo",
        worktreePath: "C:\\Code\\second-repo",
      },
      changes: [],
    };
    const thirdSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      repository: {
        ...snapshot.repository,
        id: "3fbb5062-b9d5-4aa4-a5e7-75bd9cf4dc52",
        name: "third-repo",
        worktreePath: "C:\\Code\\third-repo",
      },
      head: { kind: "branch", name: "release", oid: "333333" },
      changes: [{ ...snapshot.changes[0], path: "third-only.txt" }],
    };
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        { ...sessionWithSnapshot.tabs[0], active: true, selectedPath: "tracked.txt" },
        {
          repoId: secondSnapshot.repository.id,
          worktreeId: "worktree-two",
          worktreePath: secondSnapshot.repository.worktreePath,
          active: false,
          page: "history",
          panelWidth: 280,
          unavailable: false,
          snapshot: secondSnapshot,
        },
        {
          repoId: thirdSnapshot.repository.id,
          worktreeId: "worktree-three",
          worktreePath: thirdSnapshot.repository.worktreePath,
          active: false,
          page: "changes",
          selectedPath: "third-only.txt",
          panelWidth: 360,
          unavailable: false,
          snapshot: thirdSnapshot,
        },
      ],
    });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^second-repo0$/ }));
    expect(screen.getByRole("heading", { name: "History will appear here." })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^third-repo1$/ }));
    expect(screen.getByText("release · Changes")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "third-only.txt" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Changed files panel width" })).toHaveValue("360");

    fireEvent.click(screen.getByRole("button", { name: /^acorn-demo2$/ }));
    expect(screen.getByText("main · Changes")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "tracked.txt" })).toBeInTheDocument();
    expect(mockedActivateTab).toHaveBeenCalledWith(secondSnapshot.repository.id);
    expect(mockedActivateTab).toHaveBeenCalledWith(thirdSnapshot.repository.id);
  });

  it("persists panel width independently for the active repository", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const slider = await screen.findByRole("slider", {
      name: "Changed files panel width",
    });
    fireEvent.change(slider, { target: { value: "340" } });

    expect(slider).toHaveValue("340");
    expect(mockedUpdateTab).toHaveBeenCalledWith(
      snapshot.repository.id,
      "changes",
      undefined,
      340,
    );
  });

  it("activates a worktree by stable id and replaces only that repository snapshot", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [
        {
          id: "worktree-one",
          path: "C:\\Code\\acorn-demo",
          branch: "main",
          isCurrent: true,
          isLocked: false,
        },
        {
          id: "worktree-feature",
          path: "C:\\Code\\acorn-feature",
          branch: "feature",
          isCurrent: false,
          isLocked: false,
        },
      ],
      branches: { total: 2, items: ["main", "feature"] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedActivateWorktree.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        {
          ...sessionWithSnapshot.tabs[0],
          worktreeId: "worktree-feature",
          worktreePath: "C:\\Code\\acorn-feature",
          snapshot: {
            ...snapshot,
            repository: {
              ...snapshot.repository,
              worktreePath: "C:\\Code\\acorn-feature",
            },
            head: { kind: "branch", name: "feature", oid: "feedface" },
          },
        },
      ],
    });
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "feature" }));

    expect(await screen.findByText("feature · Changes")).toBeInTheDocument();
    expect(mockedActivateWorktree).toHaveBeenCalledWith(
      snapshot.repository.id,
      "worktree-feature",
    );
  });

  it("shows a recoverable placeholder for a missing restored repository", async () => {
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        {
          repoId: "missing",
          worktreeId: "missing-worktree",
          worktreePath: "C:\\Moved\\missing-repo",
          active: true,
          page: "changes",
          panelWidth: 280,
          unavailable: true,
        },
      ],
    });
    render(<App />);

    expect(await screen.findByRole("heading", { name: /moved or was deleted/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Locate repository" })).toBeInTheDocument();
  });

  it("shows repository discovery errors with a recovery action", async () => {
    mockedChooseRepository.mockResolvedValue("C:\\NotARepo");
    mockedOpenRepository.mockRejectedValue({
      code: "repositoryNotFound",
      message: "The selected folder is not inside a Git working tree",
    });
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Open a repository" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("not inside a Git working tree");
    expect(screen.getByRole("button", { name: "Choose another folder" })).toBeInTheDocument();
  });
});
