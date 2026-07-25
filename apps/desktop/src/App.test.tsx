import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { getAppInfo } from "./app-info";
import {
  applyPatchSelection,
  activateSessionTab,
  activateWorktree,
  checkoutBranch,
  chooseRepositoryDirectory,
  closeSessionTab,
  createBranch,
  createCommit,
  createStash,
  deleteBranch,
  discardPath,
  getDiff,
  getHistoryPage,
  getOperationHistory,
  getReferences,
  getRepositorySidebar,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  mergeBranch,
  openRepository,
  reorderSessionTabs,
  resolveConflict,
  restoreSession,
  stagePaths,
  unstagePaths,
  updateSessionTab,
  type RepositorySnapshotDto,
  type SessionDto,
} from "./repository";

vi.mock("./app-info", () => ({
  getAppInfo: vi.fn(),
}));

vi.mock("./repository", () => ({
  applyPatchSelection: vi.fn(),
  abortMerge: vi.fn(),
  chooseRepositoryDirectory: vi.fn(),
  openRepository: vi.fn(),
  restoreSession: vi.fn(),
  activateSessionTab: vi.fn(),
  activateWorktree: vi.fn(),
  applyStash: vi.fn(),
  checkoutBranch: vi.fn(),
  closeSessionTab: vi.fn(),
  createBranch: vi.fn(),
  createCommit: vi.fn(),
  createStash: vi.fn(),
  deleteBranch: vi.fn(),
  discardPath: vi.fn(),
  dropStash: vi.fn(),
  getDiff: vi.fn(),
  getHistoryPage: vi.fn(),
  getDiagnostics: vi.fn(),
  getOperationHistory: vi.fn(),
  getReferences: vi.fn(),
  getRepositorySidebar: vi.fn(),
  reorderSessionTabs: vi.fn(),
  resolveConflict: vi.fn(),
  updateSessionTab: vi.fn(),
  getRepositorySnapshot: vi.fn(),
  stagePaths: vi.fn(),
  unstagePaths: vi.fn(),
  listenForRepositoryChanges: vi.fn(),
  mergeBranch: vi.fn(),
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
const mockedCheckoutBranch = vi.mocked(checkoutBranch);
const mockedCloseTab = vi.mocked(closeSessionTab);
const mockedCreateBranch = vi.mocked(createBranch);
const mockedGetSidebar = vi.mocked(getRepositorySidebar);
const mockedReorderTabs = vi.mocked(reorderSessionTabs);
const mockedUpdateTab = vi.mocked(updateSessionTab);
const mockedGetSnapshot = vi.mocked(getRepositorySnapshot);
const mockedGetDiff = vi.mocked(getDiff);
const mockedGetHistory = vi.mocked(getHistoryPage);
const mockedGetReferences = vi.mocked(getReferences);
const mockedStagePaths = vi.mocked(stagePaths);
const mockedUnstagePaths = vi.mocked(unstagePaths);
const mockedApplyPatch = vi.mocked(applyPatchSelection);
const mockedDiscardPath = vi.mocked(discardPath);
const mockedCreateStash = vi.mocked(createStash);
const mockedCreateCommit = vi.mocked(createCommit);
const mockedDeleteBranch = vi.mocked(deleteBranch);
const mockedMergeBranch = vi.mocked(mergeBranch);
const mockedResolveConflict = vi.mocked(resolveConflict);
const mockedGetOperationHistory = vi.mocked(getOperationHistory);
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
      selectedDiff: "unstaged",
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
    mockedGetDiff.mockResolvedValue({
      schemaVersion: 1,
      binary: false,
      oldPath: "tracked.txt",
      newPath: "tracked.txt",
      hunks: [
        {
          index: 0,
          header: "@@ -1 +1 @@",
          oldStart: 1,
          oldCount: 1,
          newStart: 1,
          newCount: 1,
          lines: [
            {
              index: 0,
              kind: "deletion",
              oldLine: 1,
              content: "initial",
              selectable: true,
            },
            {
              index: 1,
              kind: "addition",
              newLine: 1,
              content: "modified",
              selectable: true,
            },
          ],
        },
      ],
    });
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: "abcdef123456",
          parents: [],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "Initial commit",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    mockedGetReferences.mockResolvedValue([
      {
        fullName: "refs/heads/main",
        shortName: "main",
        oid: "abcdef123456",
        kind: "localBranch",
        ahead: 0,
        behind: 0,
      },
    ]);
    mockedStagePaths.mockResolvedValue(snapshot);
    mockedUnstagePaths.mockResolvedValue(snapshot);
    mockedApplyPatch.mockResolvedValue(snapshot);
    mockedDiscardPath.mockResolvedValue(snapshot);
    mockedCreateCommit.mockResolvedValue({ ...snapshot, changes: [] });
    mockedCreateBranch.mockResolvedValue(snapshot);
    mockedCheckoutBranch.mockResolvedValue(snapshot);
    mockedDeleteBranch.mockResolvedValue(snapshot);
    mockedMergeBranch.mockResolvedValue(snapshot);
    mockedCreateStash.mockResolvedValue(snapshot);
    mockedResolveConflict.mockResolvedValue(snapshot);
    mockedGetOperationHistory.mockResolvedValue([]);
    mockedListenForChanges.mockResolvedValue(vi.fn());
  });

  it("renders the typed app info returned by the Rust core", async () => {
    render(<App />);

    expect(screen.getByText("Connecting to core…")).toBeInTheDocument();
    expect(await screen.findByText("Tauri 2 · v0.1.0")).toBeInTheDocument();
  });

  it("switches between Changes and History", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const graphCommit = (
      oid: string,
      subject: string,
      parents: string[],
    ) => ({
      oid,
      parents,
      authorName: "Ada",
      authorEmail: "ada@example.com",
      authoredAt: 1_700_000_000,
      subject,
      body: "",
      references: [],
      lane: 0,
      laneCount: 1,
    });
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        graphCommit("merge", "Merge topic", ["main", "topic"]),
        graphCommit("main", "Main change", ["root"]),
        graphCommit("topic", "Topic change", ["root"]),
        graphCommit("root", "Initial commit", []),
      ],
    });
    const { container } = render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    expect(await screen.findByRole("button", { name: /Initial commit/ })).toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: /Graph lane/ })).toHaveLength(4);
    expect(container.querySelectorAll(".graph-edge").length).toBeGreaterThan(4);
    expect(screen.getByRole("button", { name: /^History/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(mockedUpdateTab).toHaveBeenCalledWith(
      snapshot.repository.id,
      "history",
      undefined,
      "unstaged",
      280,
      undefined,
      undefined,
      undefined,
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

  it("keeps reference selection separate from explicit checkout", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], page: "history" }],
    });
    mockedGetReferences.mockResolvedValue([
      {
        fullName: "refs/heads/main",
        shortName: "main",
        oid: "abcdef123456",
        kind: "localBranch",
        ahead: 0,
        behind: 0,
      },
      {
        fullName: "refs/heads/topic",
        shortName: "topic",
        oid: "123456abcdef",
        kind: "localBranch",
        upstream: "origin/topic",
        ahead: 2,
        behind: 1,
      },
    ]);
    render(<App />);

    const picker = await screen.findByRole("combobox", {
      name: "Branch or tag reference",
    });
    await screen.findByRole("option", { name: "topic" });
    fireEvent.change(picker, { target: { value: "refs/heads/topic" } });

    expect(mockedCheckoutBranch).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Checkout" }));
    await waitFor(() =>
      expect(mockedCheckoutBranch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "topic",
      ),
    );
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
          selectedDiff: "unstaged",
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
          selectedDiff: "unstaged",
          panelWidth: 360,
          unavailable: false,
          snapshot: thirdSnapshot,
        },
      ],
    });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^second-repo0$/ }));
    expect(await screen.findByRole("button", { name: /Initial commit/ })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^third-repo1$/ }));
    expect(screen.getByText("release · Changes")).toBeInTheDocument();
    expect(screen.getByTitle("third-only.txt")).toHaveClass("selected");
    expect(screen.getByRole("slider", { name: "Changed files panel width" })).toHaveValue("360");

    fireEvent.click(screen.getByRole("button", { name: /^acorn-demo2$/ }));
    expect(screen.getByText("main · Changes")).toBeInTheDocument();
    expect(screen.getByTitle("tracked.txt")).toHaveClass("selected");
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
      "unstaged",
      340,
      undefined,
      undefined,
      undefined,
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
          selectedDiff: "unstaged",
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

  it("renders a diff and stages only selected lines", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /modified/ }));
    fireEvent.click(screen.getByRole("button", { name: "Stage selected lines" }));

    await waitFor(() =>
      expect(mockedApplyPatch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        snapshot.changes[0].pathBytes,
        "unstaged",
        [{ hunkIndex: 0, lineIndices: [1] }],
      ),
    );
  });

  it("loads a diff when an unselected changed file is clicked", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /tracked\.txt/ }));

    await waitFor(() =>
      expect(mockedGetDiff).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.changes[0].pathBytes,
        "unstaged",
      ),
    );
    expect(
      await screen.findByRole("button", { name: /modified/ }),
    ).toBeInTheDocument();
  });

  it("opens the staged side and unstages the whole file", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /staged file\.txt/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Unstage file" }));

    expect(mockedUnstagePaths).toHaveBeenCalledWith(
      snapshot.repository.id,
      snapshot.revision,
      [snapshot.changes[1].pathBytes],
    );
  });

  it("validates and submits the commit form", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const commit = await screen.findByRole("button", { name: "Commit to main" });
    expect(commit).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { name: "Commit summary" }), {
      target: { value: "Ship M3" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Commit description" }), {
      target: { value: "Partial staging is ready." },
    });
    fireEvent.click(commit);

    expect(mockedCreateCommit).toHaveBeenCalledWith(snapshot.repository.id, snapshot.revision, {
      summary: "Ship M3",
      description: "Partial staging is ready.",
      amend: false,
    });
  });

  it("windows a 10k file list instead of mounting every row", async () => {
    const changes = Array.from({ length: 10_000 }, (_, index) => ({
      ...snapshot.changes[0],
      path: `file-${index}.txt`,
      pathBytes: Array.from(new TextEncoder().encode(`file-${index}.txt`)),
    }));
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        {
          ...sessionWithSnapshot.tabs[0],
          snapshot: { ...snapshot, changes },
        },
      ],
    });
    render(<App />);

    expect(await screen.findByRole("button", { name: /file-0\.txt/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /file-9999\.txt/ })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /file-\d+\.txt/ }).length).toBeLessThan(40);
  });

  it("windows a large diff while preserving selectable line controls", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    mockedGetDiff.mockResolvedValue({
      schemaVersion: 1,
      binary: false,
      oldPath: "tracked.txt",
      newPath: "tracked.txt",
      hunks: [
        {
          index: 0,
          header: "@@ -0,0 +1,1000 @@",
          oldStart: 0,
          oldCount: 0,
          newStart: 1,
          newCount: 1000,
          lines: Array.from({ length: 1000 }, (_, index) => ({
            index,
            kind: "addition" as const,
            newLine: index + 1,
            content: `large-diff-line-${index}`,
            selectable: true,
          })),
        },
      ],
    });
    render(<App />);

    expect(
      await screen.findByRole("button", { name: /large-diff-line-0/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /large-diff-line-999/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: /large-diff-line-/ }).length,
    ).toBeLessThan(100);
  });

  it("creates a stash including untracked files from the sidebar", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    fireEvent.change(await screen.findByRole("textbox", { name: "Stash message" }), {
      target: { value: "before refactor" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Stash changes" }));

    await waitFor(() =>
      expect(mockedCreateStash).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "before refactor",
        true,
      ),
    );
  });

  it("offers explicit resolution actions for a conflicted file", async () => {
    const conflicted = {
      ...snapshot,
      changes: [{ ...snapshot.changes[0], conflict: true, indexStatus: "U" }],
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: conflicted }],
    });
    mockedResolveConflict.mockResolvedValue({
      ...conflicted,
      revision: 2,
      changes: [],
    });
    render(<App />);

    fireEvent.click((await screen.findAllByRole("button", { name: /tracked\.txt/ }))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Use theirs" }));

    await waitFor(() =>
      expect(mockedResolveConflict).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        snapshot.changes[0].pathBytes,
        "theirs",
      ),
    );
  });

  it("shows interrupted work in the operation center", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetOperationHistory.mockResolvedValue([
      {
        schemaVersion: 1,
        id: "operation-one",
        repoId: snapshot.repository.id,
        kind: "fetch",
        state: "interrupted",
        summary: "Interrupted when GitAcorn last exited",
        startedAt: "2026-07-25 01:00:00",
      },
    ]);
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /Operations/ }));

    expect(
      await screen.findByText("Interrupted when GitAcorn last exited"),
    ).toBeInTheDocument();
  });
});
