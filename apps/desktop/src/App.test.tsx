import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App, buildRemoteBranchTree } from "./App";
import { getAppInfo } from "./app-info";
import { getSystemFileIcons } from "./fileIcons";
import {
  closeAppWindow,
  minimizeAppWindow,
  toggleMaximizeAppWindow,
} from "./windowControls";
import {
  abortHistory,
  abortRebase,
  addSubmodule,
  applyPatchSelection,
  applyStash,
  addRemote,
  activateSessionTab,
  activateWorktree,
  checkoutBranch,
  chooseRepositoryDirectory,
  closeSessionTab,
  compareDiff,
  getBinaryPreview,
  getComparePatch,
  getExternalDiffTool,
  createBranch,
  createCommit,
  createStash,
  createTag,
  createWorktree,
  continueHistory,
  continueRebase,
  deleteBranch,
  deleteTag,
  deinitializeSubmodule,
  discardPath,
  dropStash,
  fastForwardBranch,
  getDiff,
  getFileBlame,
  getPathHistory,
  getCommitDiff,
  getCommitFiles,
  getHistoryPage,
  getGitIdentity,
  getOperationHistory,
  getReflog,
  getRemotes,
  getReferences,
  getRemoteTags,
  getRepositorySidebar,
  getRepositorySnapshot,
  initializeSubmodule,
  listenForRepositoryChanges,
  mergeBranch,
  mutateHistory,
  openRepository,
  previewInteractiveRebase,
  renameBranch,
  reorderSessionTabs,
  resetBranch,
  removeRemote,
  removeSubmodule,
  removeWorktree,
  redoOperation,
  resolveConflict,
  restoreSession,
  restoreReflogReference,
  skipRebase,
  stagePaths,
  startInteractiveRebase,
  startRemoteOperation,
  unstagePaths,
  undoOperation,
  lockWorktree,
  unlockWorktree,
  updateGlobalGitIdentity,
  updateRepositoryGitIdentity,
  updateSessionTab,
  updateRemote,
  type RepositorySnapshotDto,
  type RepositorySidebarDto,
  type SessionDto,
} from "./repository";

vi.mock("./app-info", () => ({
  getAppInfo: vi.fn(),
}));

vi.mock("./fileIcons", () => ({
  getSystemFileIcons: vi.fn(),
}));

vi.mock("./windowControls", () => ({
  closeAppWindow: vi.fn(),
  minimizeAppWindow: vi.fn(),
  toggleMaximizeAppWindow: vi.fn(),
}));

vi.mock("./repository", () => ({
  abortHistory: vi.fn(),
  abortRebase: vi.fn(),
  addSubmodule: vi.fn(),
  applyPatchSelection: vi.fn(),
  applyComparePatch: vi.fn(),
  addRemote: vi.fn(),
  abortMerge: vi.fn(),
  chooseRepositoryDirectory: vi.fn(),
  openRepository: vi.fn(),
  restoreSession: vi.fn(),
  restoreReflogReference: vi.fn(),
  activateSessionTab: vi.fn(),
  activateWorktree: vi.fn(),
  applyStash: vi.fn(),
  checkoutBranch: vi.fn(),
  closeSessionTab: vi.fn(),
  compareDiff: vi.fn(),
  getBinaryPreview: vi.fn(),
  getComparePatch: vi.fn(),
  getExternalDiffTool: vi.fn(),
  createBranch: vi.fn(),
  createCommit: vi.fn(),
  createStash: vi.fn(),
  createTag: vi.fn(),
  createWorktree: vi.fn(),
  continueHistory: vi.fn(),
  continueRebase: vi.fn(),
  deleteBranch: vi.fn(),
  deleteTag: vi.fn(),
  deinitializeSubmodule: vi.fn(),
  discardPath: vi.fn(),
  dropStash: vi.fn(),
  fastForwardBranch: vi.fn(),
  getDiff: vi.fn(),
  getFileBlame: vi.fn(),
  getPathHistory: vi.fn(),
  getCommitDiff: vi.fn(),
  getCommitFiles: vi.fn(),
  getHistoryPage: vi.fn(),
  getGitIdentity: vi.fn(),
  getDiagnostics: vi.fn(),
  getOperationHistory: vi.fn(),
  getReflog: vi.fn(),
  getRemotes: vi.fn(),
  getReferences: vi.fn(),
  getRemoteTags: vi.fn(),
  getRepositorySidebar: vi.fn(),
  initializeSubmodule: vi.fn(),
  reorderSessionTabs: vi.fn(),
  removeRemote: vi.fn(),
  removeSubmodule: vi.fn(),
  removeWorktree: vi.fn(),
  redoOperation: vi.fn(),
  resolveConflict: vi.fn(),
  updateGlobalGitIdentity: vi.fn(),
  updateExternalDiffTool: vi.fn(),
  updateRepositoryGitIdentity: vi.fn(),
  updateSessionTab: vi.fn(),
  updateRemote: vi.fn(),
  saveComparePatch: vi.fn(),
  runExternalDiff: vi.fn(),
  validateComparePatch: vi.fn(),
  getRepositorySnapshot: vi.fn(),
  stagePaths: vi.fn(),
  startRemoteOperation: vi.fn(),
  unstagePaths: vi.fn(),
  undoOperation: vi.fn(),
  lockWorktree: vi.fn(),
  unlockWorktree: vi.fn(),
  listenForRepositoryChanges: vi.fn(),
  mergeBranch: vi.fn(),
  mutateHistory: vi.fn(),
  skipHistory: vi.fn(),
  previewInteractiveRebase: vi.fn(),
  rebaseBranch: vi.fn(),
  renameBranch: vi.fn(),
  skipRebase: vi.fn(),
  resetBranch: vi.fn(),
  startInteractiveRebase: vi.fn(),
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
const mockedCloseAppWindow = vi.mocked(closeAppWindow);
const mockedMinimizeAppWindow = vi.mocked(minimizeAppWindow);
const mockedToggleMaximizeAppWindow = vi.mocked(toggleMaximizeAppWindow);
const mockedChooseRepository = vi.mocked(chooseRepositoryDirectory);
const mockedOpenRepository = vi.mocked(openRepository);
const mockedAddSubmodule = vi.mocked(addSubmodule);
const mockedPreviewInteractiveRebase = vi.mocked(previewInteractiveRebase);
const mockedRestoreSession = vi.mocked(restoreSession);
const mockedActivateTab = vi.mocked(activateSessionTab);
const mockedActivateWorktree = vi.mocked(activateWorktree);
const mockedCheckoutBranch = vi.mocked(checkoutBranch);
const mockedCloseTab = vi.mocked(closeSessionTab);
const mockedCompareDiff = vi.mocked(compareDiff);
const mockedGetExternalDiffTool = vi.mocked(getExternalDiffTool);
const mockedGetBinaryPreview = vi.mocked(getBinaryPreview);
const mockedGetComparePatch = vi.mocked(getComparePatch);
const mockedCreateBranch = vi.mocked(createBranch);
const mockedCreateTag = vi.mocked(createTag);
const mockedCreateWorktree = vi.mocked(createWorktree);
const mockedGetSidebar = vi.mocked(getRepositorySidebar);
const mockedReorderTabs = vi.mocked(reorderSessionTabs);
const mockedUpdateTab = vi.mocked(updateSessionTab);
const mockedGetSnapshot = vi.mocked(getRepositorySnapshot);
const mockedInitializeSubmodule = vi.mocked(initializeSubmodule);
const mockedDeinitializeSubmodule = vi.mocked(deinitializeSubmodule);
const mockedRemoveSubmodule = vi.mocked(removeSubmodule);
const mockedRemoveWorktree = vi.mocked(removeWorktree);
const mockedLockWorktree = vi.mocked(lockWorktree);
const mockedUnlockWorktree = vi.mocked(unlockWorktree);
const mockedGetDiff = vi.mocked(getDiff);
const mockedGetFileBlame = vi.mocked(getFileBlame);
const mockedGetPathHistory = vi.mocked(getPathHistory);
const mockedGetCommitDiff = vi.mocked(getCommitDiff);
const mockedGetCommitFiles = vi.mocked(getCommitFiles);
const mockedGetHistory = vi.mocked(getHistoryPage);
const mockedGetGitIdentity = vi.mocked(getGitIdentity);
const mockedGetReferences = vi.mocked(getReferences);
const mockedGetRemoteTags = vi.mocked(getRemoteTags);
const mockedGetRemotes = vi.mocked(getRemotes);
const mockedAddRemote = vi.mocked(addRemote);
const mockedUpdateRemote = vi.mocked(updateRemote);
const mockedRemoveRemote = vi.mocked(removeRemote);
const mockedStagePaths = vi.mocked(stagePaths);
const mockedStartInteractiveRebase = vi.mocked(startInteractiveRebase);
const mockedStartRemoteOperation = vi.mocked(startRemoteOperation);
const mockedAbortHistory = vi.mocked(abortHistory);
const mockedContinueHistory = vi.mocked(continueHistory);
const mockedMutateHistory = vi.mocked(mutateHistory);
const mockedAbortRebase = vi.mocked(abortRebase);
const mockedContinueRebase = vi.mocked(continueRebase);
const mockedSkipRebase = vi.mocked(skipRebase);
const mockedUnstagePaths = vi.mocked(unstagePaths);
const mockedApplyPatch = vi.mocked(applyPatchSelection);
const mockedDiscardPath = vi.mocked(discardPath);
const mockedCreateStash = vi.mocked(createStash);
const mockedApplyStash = vi.mocked(applyStash);
const mockedDropStash = vi.mocked(dropStash);
const mockedFastForwardBranch = vi.mocked(fastForwardBranch);
const mockedCreateCommit = vi.mocked(createCommit);
const mockedDeleteBranch = vi.mocked(deleteBranch);
const mockedRenameBranch = vi.mocked(renameBranch);
const mockedDeleteTag = vi.mocked(deleteTag);
const mockedMergeBranch = vi.mocked(mergeBranch);
const mockedResolveConflict = vi.mocked(resolveConflict);
const mockedUpdateGlobalGitIdentity = vi.mocked(updateGlobalGitIdentity);
const mockedUpdateRepositoryGitIdentity = vi.mocked(
  updateRepositoryGitIdentity,
);
const mockedGetOperationHistory = vi.mocked(getOperationHistory);
const mockedGetReflog = vi.mocked(getReflog);
const mockedRestoreReflogReference = vi.mocked(restoreReflogReference);
const mockedResetBranch = vi.mocked(resetBranch);
const mockedUndoOperation = vi.mocked(undoOperation);
const mockedRedoOperation = vi.mocked(redoOperation);
const mockedListenForChanges = vi.mocked(listenForRepositoryChanges);
const mockedGetSystemFileIcons = vi.mocked(getSystemFileIcons);

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
    vi.clearAllMocks();
    localStorage.clear();
    mockedCloseAppWindow.mockResolvedValue();
    mockedMinimizeAppWindow.mockResolvedValue();
    mockedToggleMaximizeAppWindow.mockResolvedValue();
    mockedGetAppInfo.mockResolvedValue({
      schemaVersion: 1,
      name: "GitAcorn",
      version: "0.1.0",
      runtime: "Tauri 2",
    });
    mockedGetSystemFileIcons.mockResolvedValue({});
    mockedChooseRepository.mockResolvedValue(null);
    mockedOpenRepository.mockResolvedValue(sessionWithSnapshot);
    mockedRestoreSession.mockResolvedValue({ schemaVersion: 1, tabs: [] });
    mockedActivateTab.mockResolvedValue();
    mockedActivateWorktree.mockResolvedValue(sessionWithSnapshot);
    mockedCompareDiff.mockResolvedValue({ schemaVersion: 1, files: [] });
    mockedGetExternalDiffTool.mockResolvedValue({ schemaVersion: 1, configured: null, mergeConfigured: null });
    mockedGetBinaryPreview.mockResolvedValue({ schemaVersion: 1, oldPath: "", newPath: "" });
    mockedGetComparePatch.mockResolvedValue({ schemaVersion: 1, patch: "", fileCount: 0, binary: false });
    mockedCreateWorktree.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedLockWorktree.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedUnlockWorktree.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedRemoveWorktree.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedCloseTab.mockResolvedValue({ schemaVersion: 1, tabs: [] });
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedReorderTabs.mockResolvedValue();
    mockedUpdateTab.mockResolvedValue();
    mockedGetGitIdentity.mockResolvedValue({
      schemaVersion: 1,
      global: {},
    });
    mockedUpdateGlobalGitIdentity.mockResolvedValue({});
    mockedUpdateRepositoryGitIdentity.mockResolvedValue({
      repoId: snapshot.repository.id,
      repositoryName: snapshot.repository.name,
      local: {},
      effective: {},
    });
    mockedGetSnapshot.mockResolvedValue(snapshot);
    mockedAddSubmodule.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedInitializeSubmodule.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedDeinitializeSubmodule.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedRemoveSubmodule.mockResolvedValue({ ...snapshot, revision: 2 });
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
    mockedGetRemoteTags.mockResolvedValue([]);
    mockedGetRemotes.mockResolvedValue([]);
    mockedAddRemote.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedUpdateRemote.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedRemoveRemote.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedStagePaths.mockResolvedValue(snapshot);
    mockedStartInteractiveRebase.mockResolvedValue(snapshot);
    mockedAbortHistory.mockResolvedValue(snapshot);
    mockedContinueHistory.mockResolvedValue(snapshot);
    mockedMutateHistory.mockResolvedValue({ ...snapshot, revision: 2, changes: [] });
    mockedAbortRebase.mockResolvedValue(snapshot);
    mockedContinueRebase.mockResolvedValue(snapshot);
    mockedSkipRebase.mockResolvedValue(snapshot);
    mockedStartRemoteOperation.mockResolvedValue({
      schemaVersion: 1,
      operationId: "remote-operation",
    });
    mockedGetCommitFiles.mockResolvedValue([
      {
        path: "tracked.txt",
        pathBytes: [116, 114, 97, 99, 107, 101, 100, 46, 116, 120, 116],
      },
    ]);
    mockedGetCommitDiff.mockResolvedValue({
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
              content: "before commit",
              selectable: true,
            },
            {
              index: 1,
              kind: "addition",
              newLine: 1,
              content: "after commit",
              selectable: true,
            },
          ],
        },
      ],
    });
    mockedGetFileBlame.mockResolvedValue({
      schemaVersion: 1,
      path: snapshot.changes[0].pathBytes,
      lines: [
        {
          line: 1,
          commitOid: "abcdef123456",
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          content: "modified",
        },
      ],
    });
    mockedGetPathHistory.mockResolvedValue({
      schemaVersion: 1,
      path: snapshot.changes[0].pathBytes,
      isDirectory: false,
      entries: [
        {
          oid: "abcdef123456",
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "Update tracked file",
          path: snapshot.changes[0].pathBytes,
          status: "M",
        },
      ],
    });
    mockedUnstagePaths.mockResolvedValue(snapshot);
    mockedApplyPatch.mockResolvedValue(snapshot);
    mockedDiscardPath.mockResolvedValue(snapshot);
    mockedCreateCommit.mockResolvedValue({ ...snapshot, changes: [] });
    mockedCreateBranch.mockResolvedValue(snapshot);
    mockedCreateTag.mockResolvedValue(snapshot);
    mockedCheckoutBranch.mockResolvedValue(snapshot);
    mockedDeleteBranch.mockResolvedValue(snapshot);
    mockedRenameBranch.mockResolvedValue(snapshot);
    mockedDeleteTag.mockResolvedValue(snapshot);
    mockedMergeBranch.mockResolvedValue(snapshot);
    mockedCreateStash.mockResolvedValue(snapshot);
    mockedApplyStash.mockResolvedValue(snapshot);
    mockedDropStash.mockResolvedValue(snapshot);
    mockedResolveConflict.mockResolvedValue(snapshot);
    mockedGetOperationHistory.mockResolvedValue([]);
    mockedGetReflog.mockResolvedValue([]);
    mockedRestoreReflogReference.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedResetBranch.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedListenForChanges.mockResolvedValue(vi.fn());
  });

  it("renders the typed app info returned by the Rust core", async () => {
    render(<App />);

    expect(await screen.findByText("Tauri 2 · v0.1.0")).toBeInTheDocument();
  });

  it("shows only the centered session loading state while restoring", () => {
    mockedRestoreSession.mockImplementation(() => new Promise(() => undefined));

    render(<App />);

    const loadingState = screen.getByRole("status");
    expect(loadingState).toHaveTextContent("Loading session");
    expect(loadingState).toHaveClass("session-loading-screen");
    expect(loadingState.querySelector(".session-loading-spinner")).toBeInTheDocument();
    expect(screen.queryByRole("banner")).not.toBeInTheDocument();
  });

  it("renders the active repository first and fills inactive tabs in the background", async () => {
    const inactiveSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      repository: {
        ...snapshot.repository,
        id: "2fbb5062-b9d5-4aa4-a5e7-75bd9cf4dc51",
        name: "second-repo",
        worktreePath: "C:\\Code\\second-repo",
      },
      changes: [],
    };
    let completeSnapshotLoad:
      | ((snapshot: RepositorySnapshotDto) => void)
      | undefined;
    mockedGetSnapshot.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          completeSnapshotLoad = resolve;
        }),
    );
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        sessionWithSnapshot.tabs[0],
        {
          repoId: inactiveSnapshot.repository.id,
          worktreeId: "worktree-two",
          worktreePath: inactiveSnapshot.repository.worktreePath,
          active: false,
          page: "changes",
          selectedDiff: "unstaged",
          panelWidth: 280,
          unavailable: false,
          loading: true,
        },
      ],
    });

    render(<App />);

    await screen.findByText("acorn-demo");
    expect(screen.getByRole("button", { name: /^second-repo…$/ })).toBeInTheDocument();
    expect(mockedGetSnapshot).toHaveBeenCalledWith(inactiveSnapshot.repository.id);

    await act(async () => completeSnapshotLoad?.(inactiveSnapshot));

    expect(await screen.findByRole("button", { name: /^second-repo0$/ })).toBeInTheDocument();
  });

  it("renders a draggable custom titlebar with native window actions", async () => {
    render(<App />);

    expect(await screen.findByRole("banner")).toHaveAttribute("data-tauri-drag-region");
    fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize or restore window" }));
    fireEvent.click(screen.getByRole("button", { name: "Close window" }));

    expect(mockedMinimizeAppWindow).toHaveBeenCalledOnce();
    expect(mockedToggleMaximizeAppWindow).toHaveBeenCalledOnce();
    expect(mockedCloseAppWindow).toHaveBeenCalledOnce();
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

  it("compares history refs and switches between unified and split views", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedCompareDiff.mockResolvedValue({
      schemaVersion: 1,
      files: [
        {
          oldPath: "tracked.txt",
          newPath: "tracked.txt",
          binary: false,
          hunks: [
            {
              index: 0,
              header: "@@ -1 +1 @@",
              oldStart: 1,
              oldCount: 1,
              newStart: 1,
              newCount: 1,
              lines: [
                { index: 0, kind: "deletion", oldLine: 1, content: "before", selectable: false },
                { index: 1, kind: "addition", newLine: 1, content: "after", selectable: false },
              ],
            },
          ],
        },
      ],
    });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Compare revisions" }));
    const compareDialog = screen.getByRole("dialog", { name: "Compare revisions" });
    expect(compareDialog).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Left revision"), { target: { value: "HEAD~1" } });
    fireEvent.change(screen.getByLabelText("Right revision"), { target: { value: "HEAD" } });
    fireEvent.click(within(compareDialog).getByRole("button", { name: "Compare" }));

    await waitFor(() => {
      expect(mockedCompareDiff).toHaveBeenCalledWith(snapshot.repository.id, "HEAD~1", "HEAD");
    });
    expect(await screen.findByRole("region", { name: "Comparison result" })).toBeInTheDocument();
    fireEvent.click(within(compareDialog).getByRole("button", { name: "Unified" }));
    expect(within(compareDialog).getByRole("button", { name: "Unified" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("opens and submits an interactive rebase plan from a commit context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const baseOid = "1".repeat(40);
    const firstOid = "2".repeat(40);
    const secondOid = "3".repeat(40);
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: secondOid,
          parents: [firstOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_002,
          subject: "Second change",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: firstOid,
          parents: [baseOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_001,
          subject: "First change",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: baseOid,
          parents: [],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "Base commit",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    mockedPreviewInteractiveRebase.mockResolvedValue({
      schemaVersion: 1,
      baseOid,
      headOid: snapshot.head.oid!,
      branch: "main",
      commits: [
        { oid: firstOid, subject: "First change" },
        { oid: secondOid, subject: "Second change" },
      ],
    });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    fireEvent.contextMenu(
      await screen.findByRole("button", { name: /Base commit/ }),
    );
    fireEvent.click(
      screen.getByRole("menuitem", {
        name: "Interactively rebase commits after this…",
      }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Interactive rebase",
    });
    const firstRow = within(dialog).getByRole("listitem", {
      name: "Drag to reorder First change",
    });
    const secondRow = within(dialog).getByRole("listitem", {
      name: "Drag to reorder Second change",
    });
    const dataTransfer = {
      dropEffect: "none",
      effectAllowed: "none",
      setData: vi.fn(),
      getData: vi.fn(),
    };
    fireEvent.dragStart(firstRow, { dataTransfer });
    fireEvent.dragOver(secondRow, { clientY: 10, dataTransfer });
    fireEvent.drop(secondRow, { clientY: 10, dataTransfer });
    const firstAction = within(dialog).getByRole("combobox", {
      name: "Action for First change",
    });
    expect(
      within(firstAction).getByRole("option", { name: "edit" }),
    ).toBeInTheDocument();
    fireEvent.change(firstAction, { target: { value: "reword" } });
    fireEvent.change(
      within(dialog).getByRole("textbox", {
        name: "New summary for First change",
      }),
      { target: { value: "Renamed first change" } },
    );
    fireEvent.change(
      within(dialog).getByRole("textbox", {
        name: "New description for First change",
      }),
      { target: { value: "Updated details" } },
    );
    fireEvent.click(within(dialog).getByRole("button", { name: "Start rebase" }));

    await waitFor(() =>
      expect(mockedStartInteractiveRebase).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        {
          baseOid,
          expectedHeadOid: snapshot.head.oid,
          items: [
            { oid: secondOid, action: "pick" },
            {
              oid: firstOid,
              action: "reword",
              summary: "Renamed first change",
              description: "Updated details",
            },
          ],
          autoStash: true,
        },
      ),
    );
  });

  it("previews and submits a multi-commit cherry-pick from the history graph", async () => {
    const firstOid = "1".repeat(40);
    const secondOid = "2".repeat(40);
    const headOid = "3".repeat(40);
    const cleanSnapshot = {
      ...snapshot,
      changes: [],
      head: { ...snapshot.head, oid: headOid },
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], page: "history", snapshot: cleanSnapshot }],
    });
    mockedGetSnapshot.mockResolvedValue(cleanSnapshot);
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: headOid,
          parents: [secondOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_002,
          subject: "Head commit",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: secondOid,
          parents: [firstOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_001,
          subject: "Second change",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: firstOid,
          parents: ["0".repeat(40)],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "First change",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    const firstButton = await screen.findByRole("button", { name: /First change/ });
    const secondButton = await screen.findByRole("button", { name: /Second change/ });
    fireEvent.click(firstButton);
    fireEvent.mouseDown(secondButton, { button: 0, ctrlKey: true });
    fireEvent.mouseUp(secondButton, { button: 0, ctrlKey: true });
    fireEvent.click(secondButton, { ctrlKey: true });
    fireEvent.contextMenu(secondButton);
    fireEvent.click(screen.getByRole("menuitem", { name: "Cherry-pick this commit…" }));

    const dialog = await screen.findByRole("dialog", { name: "Cherry-pick commit" });
    expect(within(dialog).getByText(/2 selected commit/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cherry-pick commit" }));
    await waitFor(() =>
      expect(mockedMutateHistory).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "cherry-pick",
        [secondOid, firstOid],
      ),
    );
    confirm.mockRestore();
  });

  it("clears a previous interactive rebase error when retrying", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const baseOid = "1".repeat(40);
    const commitOid = "2".repeat(40);
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: commitOid,
          parents: [baseOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_001,
          subject: "Change",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: baseOid,
          parents: [],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "Base commit",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    mockedPreviewInteractiveRebase
      .mockRejectedValueOnce({
        code: "unsupported_history",
        message: "Interactive rebase does not yet support merge commits",
      })
      .mockResolvedValueOnce({
        schemaVersion: 1,
        baseOid,
        headOid: snapshot.head.oid!,
        branch: "main",
        commits: [{ oid: commitOid, subject: "Change" }],
      });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    const baseCommit = await screen.findByRole("button", {
      name: /Base commit/,
    });

    fireEvent.contextMenu(baseCommit);
    fireEvent.click(
      screen.getByRole("menuitem", {
        name: "Interactively rebase commits after this…",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Interactive rebase does not yet support merge commits",
    );

    fireEvent.contextMenu(baseCommit);
    fireEvent.click(
      screen.getByRole("menuitem", {
        name: "Interactively rebase commits after this…",
      }),
    );

    expect(
      await screen.findByRole("dialog", { name: "Interactive rebase" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("dims commits that are only reachable from remote branches", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: "remote123456",
          parents: ["abcdef123456"],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_100,
          subject: "Remote-only commit",
          body: "",
          references: ["refs/remotes/origin/dev"],
          remoteOnly: true,
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    expect(
      await screen.findByRole("button", { name: /Remote-only commit/ }),
    ).toHaveClass("commit-row", "remote-only");
  });

  it("toggles the changed file list and each selected file diff", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSystemFileIcons.mockResolvedValue({
      "first.txt": "data:image/png;base64,Y29tbWl0LWZpbGUtaWNvbg==",
    });
    mockedGetCommitFiles.mockResolvedValue([
      { path: "first.txt", pathBytes: [102, 105, 114, 115, 116, 46, 116, 120, 116] },
      { path: "second.txt", pathBytes: [115, 101, 99, 111, 110, 100, 46, 116, 120, 116] },
    ]);
    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    const firstFile = await screen.findByRole("button", { name: "first.txt" });
    await waitFor(() => {
      expect(mockedGetSystemFileIcons).toHaveBeenCalledWith(
        snapshot.repository.worktreePath,
        ["first.txt", "second.txt"],
      );
      expect(firstFile.querySelector("img.change-file-icon.system")).toHaveAttribute(
        "src",
        "data:image/png;base64,Y29tbWl0LWZpbGUtaWNvbg==",
      );
    });
    expect(firstFile).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("after commit")).not.toBeInTheDocument();
    expect(screen.queryByText("Reference actions")).not.toBeInTheDocument();

    firstFile.focus();
    const fileFindShortcut = new KeyboardEvent("keydown", {
      key: "f",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    await act(async () => {
      window.dispatchEvent(fileFindShortcut);
    });

    expect(fileFindShortcut.defaultPrevented).toBe(true);
    const commitFileFilter = await screen.findByRole("searchbox", {
      name: "Filter commit changed files",
    });
    fireEvent.change(commitFileFilter, { target: { value: "first" } });
    expect(screen.getByRole("button", { name: "first.txt" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "second.txt" }),
    ).not.toBeInTheDocument();
    fireEvent.keyDown(commitFileFilter, { key: "Escape" });
    expect(
      screen.queryByRole("searchbox", {
        name: "Filter commit changed files",
      }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "second.txt" })).toBeInTheDocument();

    fireEvent.click(firstFile);
    expect(firstFile).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByText("after commit")).toBeInTheDocument();

    fireEvent.click(firstFile);
    expect(firstFile).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("after commit")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "second.txt" }));
    await waitFor(() =>
      expect(mockedGetCommitDiff).toHaveBeenLastCalledWith(
        snapshot.repository.id,
        "abcdef123456",
        [115, 101, 99, 111, 110, 100, 46, 116, 120, 116],
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: /Changed files/ }));
    expect(screen.queryByRole("button", { name: "second.txt" })).not.toBeInTheDocument();
  });

  it("opens history at the newest page instead of a persisted older cursor", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [
        {
          ...sessionWithSnapshot.tabs[0],
          page: "history",
          historyCursor: "offset:100",
        },
      ],
    });
    render(<App />);

    await screen.findByRole("button", { name: /Initial commit/ });

    expect(mockedGetHistory.mock.calls[0]).toEqual([
      snapshot.repository.id,
      undefined,
      undefined,
      undefined,
    ]);
  });

  it("shows a recoverable error state when the core cannot be reached", async () => {
    mockedGetAppInfo.mockRejectedValue(new Error("IPC unavailable"));
    mockedRestoreSession.mockRejectedValue(new Error("Session unavailable"));
    render(<App />);

    await waitFor(() => {
      expect(mockedRestoreSession).toHaveBeenCalled();
      expect(screen.getAllByRole("alert")).toHaveLength(1);
    });
    expect(screen.getByRole("alert")).toHaveTextContent("IPC unavailable");
    expect(screen.queryByText("Session unavailable")).not.toBeInTheDocument();
    expect(screen.getByText("Core unavailable")).toBeInTheDocument();
  });

  it("opens a real repository snapshot and separates staged changes", async () => {
    mockedChooseRepository.mockResolvedValue("C:\\Code\\acorn-demo");
    render(<App />);

    const [openRepository] = await screen.findAllByRole("button", {
      name: "Open a repository",
    });
    fireEvent.click(openRepository);

    expect(await screen.findByText("acorn-demo")).toBeInTheDocument();
    expect(screen.getByText("main · Changes")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /tracked\.txt/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /staged file\.txt/ })).toBeInTheDocument();
    expect(mockedOpenRepository).toHaveBeenCalledWith("C:\\Code\\acorn-demo");
  });

  it("lists submodules and opens an initialized submodule on double-click", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      submodules: [
        {
          path: "vendor/core",
          absolutePath: "C:\\Code\\acorn-demo\\vendor\\core",
          initialized: true,
        },
        {
          path: "vendor/pending",
          absolutePath: "C:\\Code\\acorn-demo\\vendor\\pending",
          initialized: false,
        },
      ],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const initialized = await screen.findByRole("button", {
      name: "vendor/core",
    });
    const uninitialized = screen.getByRole("button", {
      name: /vendor\/pending.*not initialized/,
    });
    expect(uninitialized).toBeEnabled();

    fireEvent.doubleClick(initialized);

    await waitFor(() =>
      expect(mockedOpenRepository).toHaveBeenCalledWith(
        "C:\\Code\\acorn-demo\\vendor\\core",
        {
          repositoryName: "acorn-demo",
          worktreePath: "C:\\Code\\acorn-demo",
        },
      ),
    );

    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const initializeButton = screen.getByRole("button", {
      name: "Initialize submodule vendor/pending",
    });
    fireEvent.click(initializeButton);
    expect(confirm).toHaveBeenCalledWith(
      "Initialize submodule vendor/pending?",
    );
    expect(mockedInitializeSubmodule).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    fireEvent.click(
      initializeButton,
    );
    await waitFor(() =>
      expect(mockedInitializeSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "vendor/pending",
      ),
    );

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Remove submodule vendor/core",
      }),
    );
    await waitFor(() =>
      expect(mockedRemoveSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        2,
        "vendor/core",
      ),
    );
    expect(confirm).toHaveBeenLastCalledWith(
      "Remove submodule vendor/core? Its worktree will be removed and the deletion staged.",
    );
    confirm.mockRestore();
  });

  it("asks before initializing and opening an uninitialized submodule", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      submodules: [
        {
          path: "vendor/pending",
          absolutePath: "C:\\Code\\acorn-demo\\vendor\\pending",
          initialized: false,
        },
      ],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);

    const pending = await screen.findByRole("button", {
      name: /vendor\/pending.*not initialized/,
    });
    fireEvent.doubleClick(pending);
    expect(confirm).toHaveBeenCalledWith(
      "Initialize submodule vendor/pending and open it as a repository?",
    );
    expect(mockedInitializeSubmodule).not.toHaveBeenCalled();
    expect(mockedOpenRepository).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    fireEvent.doubleClick(pending);

    await waitFor(() =>
      expect(mockedInitializeSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "vendor/pending",
      ),
    );
    await waitFor(() =>
      expect(mockedOpenRepository).toHaveBeenCalledWith(
        "C:\\Code\\acorn-demo\\vendor\\pending",
        {
          repositoryName: "acorn-demo",
          worktreePath: "C:\\Code\\acorn-demo",
        },
      ),
    );
    confirm.mockRestore();
  });

  it("closes an open submodule tab before removing its worktree", async () => {
    const submodulePath = "C:\\Code\\acorn-demo\\vendor\\core";
    const refreshedSnapshot = { ...snapshot, revision: snapshot.revision + 1 };
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        ...sessionWithSnapshot.tabs,
        {
          ...sessionWithSnapshot.tabs[0],
          repoId: "submodule-repository",
          worktreeId: "submodule-worktree",
          worktreePath: submodulePath,
          openedFrom: {
            repositoryName: snapshot.repository.name,
            worktreePath: snapshot.repository.worktreePath,
          },
          active: false,
          snapshot: {
            ...snapshot,
            repository: {
              ...snapshot.repository,
              id: "submodule-repository",
              name: "core",
              worktreePath: submodulePath,
            },
          },
        },
      ],
    });
    mockedCloseTab.mockResolvedValue(sessionWithSnapshot);
    mockedGetSnapshot.mockResolvedValue(refreshedSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      submodules: [
        {
          path: "vendor/core",
          absolutePath: submodulePath,
          initialized: true,
        },
      ],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<App />);

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Remove submodule vendor/core",
      }),
    );

    await waitFor(() =>
      expect(mockedCloseTab).toHaveBeenCalledWith("submodule-repository"),
    );
    await waitFor(() =>
      expect(mockedGetSnapshot).toHaveBeenCalledWith(snapshot.repository.id),
    );
    await waitFor(() =>
      expect(mockedRemoveSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        refreshedSnapshot.revision,
        "vendor/core",
      ),
    );
    expect(mockedCloseTab.mock.invocationCallOrder[0]).toBeLessThan(
      mockedGetSnapshot.mock.invocationCallOrder[0],
    );
    expect(mockedGetSnapshot.mock.invocationCallOrder[0]).toBeLessThan(
      mockedRemoveSubmodule.mock.invocationCallOrder[0],
    );
    confirm.mockRestore();
  });

  it("shows state-specific actions in the submodule context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      submodules: [
        {
          path: "vendor/core",
          absolutePath: "C:\\Code\\acorn-demo\\vendor\\core",
          initialized: true,
        },
        {
          path: "vendor/pending",
          absolutePath: "C:\\Code\\acorn-demo\\vendor\\pending",
          initialized: false,
        },
      ],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const pending = await screen.findByRole("button", {
      name: /vendor\/pending.*not initialized/,
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    fireEvent.contextMenu(pending, { clientX: 40, clientY: 50 });
    let menu = screen.getByRole("menu");
    expect(within(menu).getByRole("menuitem", { name: "Initialize" })).toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Open" })).not.toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Deinitialize" })).not.toBeInTheDocument();
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Initialize" }));
    expect(confirm).toHaveBeenCalledWith(
      "Initialize submodule vendor/pending?",
    );
    await waitFor(() =>
      expect(mockedInitializeSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "vendor/pending",
      ),
    );

    const initialized = await screen.findByRole("button", { name: "vendor/core" });
    fireEvent.contextMenu(initialized, { clientX: 60, clientY: 70 });
    menu = screen.getByRole("menu");
    expect(within(menu).getByRole("menuitem", { name: "Open" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Deinitialize" })).toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Initialize" })).not.toBeInTheDocument();
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open" }));
    await waitFor(() =>
      expect(mockedOpenRepository).toHaveBeenCalledWith(
        "C:\\Code\\acorn-demo\\vendor\\core",
        {
          repositoryName: "acorn-demo",
          worktreePath: "C:\\Code\\acorn-demo",
        },
      ),
    );

    fireEvent.contextMenu(
      await screen.findByRole("button", { name: "vendor/core" }),
      { clientX: 60, clientY: 70 },
    );
    fireEvent.click(screen.getByRole("menuitem", { name: "Deinitialize" }));
    expect(confirm).toHaveBeenCalledWith(
      "Deinitialize submodule vendor/core? Its worktree will be removed.",
    );
    await waitFor(() =>
      expect(mockedDeinitializeSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "vendor/core",
      ),
    );
    confirm.mockRestore();
  });

  it("shows the parent repository on a submodule repository tab", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [
        {
          ...sessionWithSnapshot.tabs[0],
          openedFrom: {
            repositoryName: "primary",
            worktreePath: "C:\\Code\\primary",
          },
        },
      ],
    });

    render(<App />);

    expect(await screen.findByText("Submodule of primary")).toBeInTheDocument();
    expect(
      screen.getByTitle("Submodule of primary (C:\\Code\\primary)"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Scroll repository tabs left" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Scroll repository tabs right" }),
    ).toBeInTheDocument();
  });

  it("adds a submodule from the sidebar dialog", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await screen.findByText("acorn-demo");
    const addButton = screen.getByRole("button", { name: "Add submodule" });
    expect(addButton).toHaveTextContent("+");
    expect(addButton.closest(".sidebar-group-row")).toHaveTextContent(
      "Submodules",
    );
    fireEvent.click(addButton);

    const dialog = screen.getByRole("dialog", { name: "Add submodule" });
    fireEvent.change(
      within(dialog).getByRole("textbox", { name: "Repository URL" }),
      { target: { value: "https://example.com/team/core.git" } },
    );
    fireEvent.change(
      within(dialog).getByRole("textbox", { name: "Path in repository" }),
      { target: { value: "vendor/core" } },
    );
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Add submodule" }),
    );

    await waitFor(() =>
      expect(mockedAddSubmodule).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        {
          url: "https://example.com/team/core.git",
          path: "vendor/core",
        },
      ),
    );
  });

  it("reorders repository tabs by dragging without move buttons", async () => {
    const secondSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      repository: {
        ...snapshot.repository,
        id: "second-repository",
        name: "second-demo",
        worktreePath: "C:\\Code\\second-demo",
      },
    };
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [
        { ...sessionWithSnapshot.tabs[0], active: false },
        {
          ...sessionWithSnapshot.tabs[0],
          repoId: secondSnapshot.repository.id,
          worktreeId: "worktree-two",
          worktreePath: secondSnapshot.repository.worktreePath,
          snapshot: secondSnapshot,
        },
      ],
    });
    const { container } = render(<App />);

    await screen.findByText("second-demo");
    expect(screen.queryByRole("button", { name: /Move .* left/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Move .* right/ })).not.toBeInTheDocument();

    const [firstTab, secondTab] = Array.from(
      container.querySelectorAll<HTMLElement>(".repository-tab"),
    );
    const firstTabMain = firstTab.querySelector<HTMLElement>(".tab-main");
    expect(firstTab).not.toHaveAttribute("draggable");
    expect(firstTabMain).not.toHaveAttribute("draggable");
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: vi.fn(() => secondTab),
    });
    vi.spyOn(secondTab, "getBoundingClientRect").mockReturnValue({
      ...secondTab.getBoundingClientRect(),
      left: 50,
      width: 100,
    });
    fireEvent.mouseDown(firstTabMain!, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(window, { buttons: 1, clientX: 125, clientY: 10 });
    fireEvent.mouseUp(window, { button: 0, clientX: 125, clientY: 10 });

    await waitFor(() =>
      expect(mockedReorderTabs).toHaveBeenCalledWith([
        secondSnapshot.repository.id,
        snapshot.repository.id,
      ]),
    );
    expect(
      Array.from(container.querySelectorAll(".repository-tab strong")).map(
        (element) => element.textContent,
      ),
    ).toEqual(["second-demo", "acorn-demo"]);
    Reflect.deleteProperty(document, "elementFromPoint");
  });

  it("shows history search only after Ctrl+F in the graph and clears it when closed", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], page: "history" }],
    });
    render(<App />);

    const commit = await screen.findByRole("button", {
      name: /Initial commit/,
    });
    expect(
      screen.queryByRole("searchbox", { name: "Search commit messages" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("combobox", { name: "Branch or tag reference" }),
    ).not.toBeInTheDocument();

    commit.focus();
    const historyFindShortcut = new KeyboardEvent("keydown", {
      key: "f",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(historyFindShortcut);
    expect(historyFindShortcut.defaultPrevented).toBe(true);

    const search = await screen.findByRole("searchbox", {
      name: "Search commit messages",
    });
    fireEvent.change(search, { target: { value: "topic" } });
    fireEvent.submit(search.closest("form")!);

    await waitFor(() =>
      expect(mockedGetHistory).toHaveBeenLastCalledWith(
        snapshot.repository.id,
        undefined,
        undefined,
        "topic",
      ),
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Close history search" }),
    );

    expect(
      screen.queryByRole("searchbox", { name: "Search commit messages" }),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(mockedGetHistory).toHaveBeenLastCalledWith(
        snapshot.repository.id,
        undefined,
        undefined,
        undefined,
      ),
    );
  });

  it("filters sidebar references after Ctrl+F and closes with Escape", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "feature/topic"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const localBranches = await screen.findByRole("button", {
      name: /Local Branches/,
    });
    expect(
      screen.queryByRole("searchbox", { name: "Filter sidebar" }),
    ).not.toBeInTheDocument();

    localBranches.focus();
    const sidebarFindShortcut = new KeyboardEvent("keydown", {
      key: "f",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(sidebarFindShortcut);
    expect(sidebarFindShortcut.defaultPrevented).toBe(true);

    const filter = await screen.findByRole("searchbox", {
      name: "Filter sidebar",
    });
    fireEvent.change(filter, { target: { value: "topic" } });

    expect(
      screen.queryByRole("button", { name: "Branch main" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Branch feature/topic" }),
    ).toBeInTheDocument();

    fireEvent.keyDown(filter, { key: "Escape" });

    expect(
      screen.queryByRole("searchbox", { name: "Filter sidebar" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Branch main" }),
    ).toBeInTheDocument();
  });

  it("routes Ctrl+F to changed-file filtering and diff-content search", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const { container } = render(<App />);

    const trackedFile = await screen.findByRole("button", {
      name: /tracked\.txt/,
    });
    trackedFile.focus();
    const fileFindShortcut = new KeyboardEvent("keydown", {
      key: "f",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    await act(async () => {
      window.dispatchEvent(fileFindShortcut);
    });

    expect(fileFindShortcut.defaultPrevented).toBe(true);
    const fileFilter = await screen.findByRole("searchbox", {
      name: "Filter changed files",
    });
    fireEvent.change(fileFilter, { target: { value: "staged file" } });
    expect(
      screen.queryByRole("button", { name: /tracked\.txt/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /staged file\.txt/ }),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Close file filter" }),
    );
    const restoredTrackedFile = screen.getByRole("button", {
      name: /tracked\.txt/,
    });
    fireEvent.click(restoredTrackedFile);

    await waitFor(() =>
      expect(container.querySelector(".diff-line")).toBeInTheDocument(),
    );
    const diffLine = container.querySelector<HTMLElement>(".diff-line")!;
    restoredTrackedFile.focus();
    fireEvent.pointerDown(diffLine);
    expect(document.activeElement).toBe(restoredTrackedFile);
    const diffFindShortcut = new KeyboardEvent("keydown", {
      key: "f",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(diffFindShortcut);

    expect(diffFindShortcut.defaultPrevented).toBe(true);
    const contentSearch = await screen.findByRole("searchbox", {
      name: "Search file changes",
    });
    fireEvent.change(contentSearch, { target: { value: "initial" } });

    expect(
      container.querySelectorAll(".diff-line.search-match"),
    ).toHaveLength(1);
    expect(
      container.querySelector(".diff-line.active-search-match"),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Close content search" }),
    );
    expect(
      screen.queryByRole("searchbox", { name: "Search file changes" }),
    ).not.toBeInTheDocument();
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

    fireEvent.click(screen.getByRole("button", { name: /^acorn-demo2$/ }));
    expect(screen.getByText("main · Changes")).toBeInTheDocument();
    expect(screen.getByTitle("tracked.txt")).toHaveClass("selected");
    expect(mockedActivateTab).toHaveBeenCalledWith(secondSnapshot.repository.id);
    expect(mockedActivateTab).toHaveBeenCalledWith(thirdSnapshot.repository.id);
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
      remoteBranches: { total: 0, items: [] },
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

  it("creates and manages worktrees from the sidebar actions", async () => {
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
      remoteBranches: { total: 1, items: ["origin/main"] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    const sidebar: RepositorySidebarDto = {
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
      remoteBranches: { total: 1, items: ["origin/main"] },
      tags: { total: 0, items: [] },
      stashes: [],
    };
    mockedCreateWorktree.mockResolvedValue(sidebar);
    mockedLockWorktree.mockResolvedValue(sidebar);
    mockedUnlockWorktree.mockResolvedValue(sidebar);
    mockedRemoveWorktree.mockResolvedValue(sidebar);
    render(<App />);

    const createWorktreeButton = await screen.findByRole("button", { name: "Create worktree" });
    fireEvent.click(createWorktreeButton);
    const worktreeDialog = screen.getByRole("dialog", { name: "Create worktree" });
    expect(worktreeDialog).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Worktree path"), {
      target: { value: "C:\\Code\\acorn-feature-new" },
    });
    fireEvent.change(screen.getByLabelText("New branch (optional)"), {
      target: { value: "feature/new" },
    });
    fireEvent.change(screen.getByLabelText("Start point or remote branch"), {
      target: { value: "origin/main" },
    });
    fireEvent.click(within(worktreeDialog).getByRole("button", { name: "Create worktree" }));
    await waitFor(() => {
      expect(mockedCreateWorktree).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        {
          path: "C:\\Code\\acorn-feature-new",
          branch: "feature/new",
          startPoint: "origin/main",
        },
      );
    });

    const feature = screen.getByRole("button", { name: "feature" });
    fireEvent.contextMenu(feature);
    expect(screen.getByRole("menu", { name: "Worktree actions" })).toBeInTheDocument();
    expect(
      screen.getByRole("menuitem", { name: "Remove worktree and delete branch" }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "Lock" }));
    await waitFor(() => {
      expect(mockedLockWorktree).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "worktree-feature",
      );
    });
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

    const [openRepository] = await screen.findAllByRole("button", {
      name: "Open a repository",
    });
    fireEvent.click(openRepository);

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

  it("does not reload the selected diff when unrelated app state changes", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    render(<App />);

    expect(
      await screen.findByRole("button", { name: /modified/ }),
    ).toBeInTheDocument();
    mockedGetDiff.mockClear();

    const resizer = screen.getByRole("separator", { name: "Sidebar width" });
    fireEvent.mouseDown(resizer, { button: 0, clientX: 200 });
    fireEvent.mouseMove(window, { buttons: 1, clientX: 240 });
    fireEvent.mouseUp(window, { button: 0, clientX: 240 });

    await act(async () => undefined);
    expect(mockedGetDiff).not.toHaveBeenCalled();
  });

  it("persists the file panel width once after dragging", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const resizer = await screen.findByRole("separator", {
      name: "File panel width",
    });
    mockedUpdateTab.mockClear();

    fireEvent.mouseDown(resizer, { button: 0, clientX: 280 });
    fireEvent.mouseMove(window, { buttons: 1, clientX: 300 });
    fireEvent.mouseMove(window, { buttons: 1, clientX: 320 });
    expect(mockedUpdateTab).not.toHaveBeenCalled();

    fireEvent.mouseUp(window, { button: 0, clientX: 320 });

    expect(mockedUpdateTab).toHaveBeenCalledTimes(1);
    expect(mockedUpdateTab).toHaveBeenCalledWith(
      snapshot.repository.id,
      "changes",
      undefined,
      "unstaged",
      320,
      undefined,
      undefined,
      undefined,
    );
  });

  it("opens the staged side and unstages the whole file", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const stagedRow = await screen.findByRole("button", { name: /staged file\.txt/ });
    fireEvent.contextMenu(stagedRow, { clientX: 40, clientY: 50 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Unstage file" }));

    expect(mockedUnstagePaths).toHaveBeenCalledWith(
      snapshot.repository.id,
      snapshot.revision,
      [snapshot.changes[1].pathBytes],
    );
  });

  it("selects a continuous range of changed lines by dragging", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    render(<App />);

    const firstLine = await screen.findByRole("button", { name: /initial/ });
    const lastLine = screen.getByRole("button", { name: /modified/ });
    fireEvent.mouseDown(firstLine, { button: 0, buttons: 1 });
    fireEvent.mouseEnter(lastLine, { buttons: 1 });
    fireEvent.mouseUp(window, { button: 0 });

    expect(firstLine).toHaveAttribute("aria-pressed", "true");
    expect(lastLine).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Stage selected lines" }));

    await waitFor(() =>
      expect(mockedApplyPatch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        snapshot.changes[0].pathBytes,
        "unstaged",
        [{ hunkIndex: 0, lineIndices: [0, 1] }],
      ),
    );
  });

  it("blocks mutations as soon as a repository refresh is scheduled", async () => {
    let notifyRepositoryChange: ((repoId: string) => void) | undefined;
    let completeRefresh: ((snapshot: RepositorySnapshotDto) => void) | undefined;
    mockedListenForChanges.mockImplementation(async (callback) => {
      notifyRepositoryChange = callback;
      return () => undefined;
    });
    mockedGetSnapshot.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          completeRefresh = resolve;
        }),
    );
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    render(<App />);

    const trackedRow = await screen.findByRole("button", { name: /tracked\.txt/ });
    fireEvent.contextMenu(trackedRow, { clientX: 40, clientY: 50 });
    const stageFile = screen.getByRole("menuitem", { name: "Stage file" });
    expect(stageFile).toBeEnabled();
    await waitFor(() => expect(notifyRepositoryChange).toBeDefined());

    act(() => notifyRepositoryChange?.(snapshot.repository.id));

    expect(stageFile).toBeDisabled();
    expect(screen.getByText("Refreshing…")).toBeInTheDocument();

    await act(async () => completeRefresh?.(snapshot));
  });

  it("does not reload remotes after an ordinary repository refresh", async () => {
    let notifyRepositoryChange: ((repoId: string) => void) | undefined;
    mockedListenForChanges.mockImplementation(async (callback) => {
      notifyRepositoryChange = callback;
      return () => undefined;
    });
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await waitFor(() => expect(mockedGetRemotes).toHaveBeenCalled());
    await waitFor(() => expect(notifyRepositoryChange).toBeDefined());
    mockedGetRemotes.mockClear();
    mockedGetSnapshot.mockClear();

    act(() => notifyRepositoryChange?.(snapshot.repository.id));
    await waitFor(() =>
      expect(screen.queryByText("Refreshing…")).not.toBeInTheDocument(),
    );

    expect(mockedGetSnapshot).toHaveBeenCalledTimes(1);
    expect(mockedGetRemotes).not.toHaveBeenCalled();
  });

  it("reloads commit history when window focus detects an external commit", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], page: "history" }],
    });
    render(<App />);

    await screen.findByRole("button", { name: /Initial commit/ });
    mockedGetSnapshot.mockClear();
    mockedGetHistory.mockClear();
    mockedGetSnapshot.mockResolvedValue({
      ...snapshot,
      revision: 2,
      head: { ...snapshot.head, oid: "fedcba654321" },
    });
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: "fedcba654321",
          parents: ["abcdef123456"],
          authorName: "Grace",
          authorEmail: "grace@example.com",
          authoredAt: 1_700_000_100,
          subject: "External commit",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
      ],
    });

    fireEvent.focus(window);

    await waitFor(() =>
      expect(mockedGetSnapshot).toHaveBeenCalledWith(snapshot.repository.id),
    );
    expect(
      await screen.findByRole("button", { name: /External commit/ }),
    ).toBeInTheDocument();
  });

  it("configures fetch, pull, and push from remote operation dialogs", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetRemotes.mockResolvedValue([
      { name: "origin", url: "https://example.com/origin.git" },
      { name: "upstream", url: "https://example.com/upstream.git" },
    ]);
    render(<App />);

    await screen.findByRole("button", { name: "Fetch" });
    await waitFor(() => expect(mockedGetRemotes).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: "Fetch" }));
    let dialog = screen.getByRole("dialog", { name: "Fetch" });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Remote" }), {
      target: { value: "upstream" },
    });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Fetch tags" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Fetch" }));

    expect(mockedStartRemoteOperation).toHaveBeenLastCalledWith(
      snapshot.repository.id,
      "fetch",
      expect.any(Function),
      {
        remote: "upstream",
        fetchTags: true,
        autoStash: false,
        fastForwardOnly: false,
        forceWithLease: false,
      },
    );

    fireEvent.click(screen.getByRole("button", { name: "Pull" }));
    dialog = screen.getByRole("dialog", { name: "Pull" });
    fireEvent.click(
      within(dialog).getByRole("checkbox", {
        name: "Automatically stash and reapply local changes",
      }),
    );
    expect(
      within(dialog).getByRole("checkbox", { name: "Use fast-forward only" }),
    ).toBeChecked();
    fireEvent.click(within(dialog).getByRole("button", { name: "Pull" }));

    expect(mockedStartRemoteOperation).toHaveBeenLastCalledWith(
      snapshot.repository.id,
      "pull",
      expect.any(Function),
      {
        remote: "origin",
        fetchTags: false,
        autoStash: true,
        fastForwardOnly: true,
        forceWithLease: false,
      },
    );

    fireEvent.click(screen.getByRole("button", { name: "Push" }));
    dialog = screen.getByRole("dialog", { name: "Push" });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Force Push" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Push" }));

    expect(mockedStartRemoteOperation).toHaveBeenLastCalledWith(
      snapshot.repository.id,
      "push",
      expect.any(Function),
      {
        remote: "origin",
        fetchTags: false,
        autoStash: false,
        fastForwardOnly: false,
        forceWithLease: true,
      },
    );
    expect(
      screen.queryByRole("button", { name: "Push with lease" }),
    ).not.toBeInTheDocument();
  });

  it("recovers a stale line mutation with the latest snapshot", async () => {
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], selectedPath: "tracked.txt" }],
    });
    mockedApplyPatch.mockRejectedValueOnce({
      code: "staleRevision",
      message: "The request used repository revision 2, but the current revision is 3",
    });
    mockedGetSnapshot.mockResolvedValueOnce({ ...snapshot, revision: 3 });
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /modified/ }));
    fireEvent.click(screen.getByRole("button", { name: "Stage selected lines" }));

    await waitFor(() =>
      expect(mockedGetSnapshot).toHaveBeenCalledWith(snapshot.repository.id),
    );
    expect(
      screen.queryByText(
        "The request used repository revision 2, but the current revision is 3",
      ),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stage selected lines" })).toBeDisabled();
  });

  it("selects multiple changed files with mouse ranges and keyboard ranges", async () => {
    const multiSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      changes: ["one.txt", "two.txt", "three.txt"].map((path, index) => ({
        ...snapshot.changes[0],
        path,
        pathBytes: [index + 1],
      })),
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: multiSnapshot }],
    });
    render(<App />);

    const first = await screen.findByRole("button", { name: /one\.txt/ });
    const second = screen.getByRole("button", { name: /two\.txt/ });
    const third = screen.getByRole("button", { name: /three\.txt/ });

    fireEvent.mouseDown(first, { button: 0, buttons: 1 });
    fireEvent.mouseEnter(third, { buttons: 1 });
    fireEvent.mouseUp(window);

    expect(first).toHaveAttribute("aria-pressed", "true");
    expect(second).toHaveAttribute("aria-pressed", "true");
    expect(third).toHaveAttribute("aria-pressed", "true");

    fireEvent.mouseDown(second, { button: 0, buttons: 1 });
    fireEvent.mouseEnter(third, { buttons: 1 });
    fireEvent.mouseUp(window);
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(second).toHaveAttribute("aria-pressed", "true");
    expect(third).toHaveAttribute("aria-pressed", "true");

    fireEvent.keyDown(second, { key: "ArrowUp", shiftKey: true });
    expect(first).toHaveAttribute("aria-pressed", "true");
    expect(second).toHaveAttribute("aria-pressed", "true");

    fireEvent.mouseDown(second, { button: 0 });
    fireEvent.click(second);
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(second).toHaveAttribute("aria-pressed", "true");

    fireEvent.mouseDown(second, { button: 0, ctrlKey: true });
    fireEvent.click(second, { ctrlKey: true });
    expect(second).toHaveAttribute("aria-pressed", "false");

    fireEvent.mouseDown(first, { button: 0, ctrlKey: true });
    fireEvent.click(first, { ctrlKey: true });
    expect(first).toHaveAttribute("aria-pressed", "true");
    fireEvent.mouseDown(first.closest(".change-list")!);
    expect(first).toHaveAttribute("aria-pressed", "false");
  });

  it("selects the rows crossed by a drag that starts in blank list space", async () => {
    const multiSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      changes: ["one.txt", "two.txt", "three.txt"].map((path, index) => ({
        ...snapshot.changes[0],
        path,
        pathBytes: [index + 1],
      })),
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: multiSnapshot }],
    });
    render(<App />);

    const first = await screen.findByRole("button", { name: /one\.txt/ });
    const second = screen.getByRole("button", { name: /two\.txt/ });
    const third = screen.getByRole("button", { name: /three\.txt/ });
    const list = first.closest(".change-list")!;
    const space = first.closest(".virtual-list-space")!;
    vi.spyOn(space, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      right: 300,
      bottom: 102,
      left: 0,
      width: 300,
      height: 102,
      toJSON: () => ({}),
    });

    fireEvent.mouseDown(list, {
      button: 0,
      buttons: 1,
      clientX: 250,
      clientY: 130,
    });
    fireEvent.mouseMove(window, {
      buttons: 1,
      clientX: 250,
      clientY: 40,
    });

    expect(list.querySelector(".selection-band")).toBeInTheDocument();
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(second).toHaveAttribute("aria-pressed", "true");
    expect(third).toHaveAttribute("aria-pressed", "true");

    fireEvent.mouseUp(window);
    expect(list.querySelector(".selection-band")).not.toBeInTheDocument();
  });

  it("stages all selected unstaged files when they are dropped on Staged", async () => {
    const multiSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      changes: ["one.txt", "two.txt"].map((path, index) => ({
        ...snapshot.changes[0],
        path,
        pathBytes: [index + 1],
      })),
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: multiSnapshot }],
    });
    render(<App />);

    const first = await screen.findByRole("button", { name: /one\.txt/ });
    const second = screen.getByRole("button", { name: /two\.txt/ });
    fireEvent.mouseDown(first, { button: 0, ctrlKey: true });
    fireEvent.mouseDown(second, { button: 0, ctrlKey: true });

    const data = new Map<string, string>();
    const dataTransfer = {
      effectAllowed: "all",
      dropEffect: "none",
      setData: (type: string, value: string) => data.set(type, value),
      getData: (type: string) => data.get(type) ?? "",
    };
    fireEvent.dragStart(first, { dataTransfer });
    const stagedSection = screen.getByRole("heading", { name: "Staged" }).closest(".change-section");
    expect(stagedSection).not.toBeNull();
    fireEvent.dragEnter(stagedSection!, { dataTransfer });
    fireEvent.dragOver(stagedSection!, { dataTransfer });
    fireEvent.drop(stagedSection!, { dataTransfer });

    await waitFor(() =>
      expect(mockedStagePaths).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        [[1], [2]],
      ),
    );
  });

  it("unstages all selected staged files when they are dropped on Unstaged", async () => {
    const stagedSnapshot: RepositorySnapshotDto = {
      ...snapshot,
      changes: ["one.txt", "two.txt"].map((path, index) => ({
        ...snapshot.changes[1],
        path,
        pathBytes: [index + 1],
        indexStatus: "M",
      })),
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: stagedSnapshot }],
    });
    render(<App />);

    const first = await screen.findByRole("button", { name: /one\.txt/ });
    const second = screen.getByRole("button", { name: /two\.txt/ });
    fireEvent.mouseDown(first, { button: 0, ctrlKey: true });
    fireEvent.click(first, { ctrlKey: true });
    fireEvent.mouseDown(second, { button: 0, ctrlKey: true });
    fireEvent.click(second, { ctrlKey: true });

    const unstagedSection = screen
      .getByRole("heading", { name: "Unstaged" })
      .closest(".change-section")!;
    const originalElementFromPoint = document.elementFromPoint;
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: () => unstagedSection,
    });
    try {
      fireEvent.mouseDown(first, {
        button: 0,
        buttons: 1,
        clientX: 10,
        clientY: 100,
      });
      fireEvent.mouseMove(window, {
        buttons: 1,
        clientX: 10,
        clientY: 20,
      });
      fireEvent.mouseUp(window, { clientX: 10, clientY: 20 });
    } finally {
      if (originalElementFromPoint) {
        Object.defineProperty(document, "elementFromPoint", {
          configurable: true,
          value: originalElementFromPoint,
        });
      } else {
        Reflect.deleteProperty(document, "elementFromPoint");
      }
    }

    await waitFor(() =>
      expect(mockedUnstagePaths).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        [[1], [2]],
      ),
    );
  });

  it("multi-selects branches and tags without checking out branches", async () => {
    mockedCheckoutBranch.mockClear();
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 3, items: ["main", "topic", "release"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 2, items: ["v1.0.0", "v2.0.0"] },
      stashes: [],
    });
    render(<App />);

    const main = await screen.findByRole("button", { name: "Branch main" });
    const release = await screen.findByRole("button", { name: "Branch release" });
    fireEvent.mouseDown(main, { button: 0, buttons: 1 });
    fireEvent.mouseEnter(release, { buttons: 1 });
    fireEvent.mouseUp(window);
    expect(main).toHaveAttribute("aria-pressed", "true");
    expect(release).toHaveAttribute("aria-pressed", "true");

    const v1 = screen.getByRole("button", { name: /v1\.0\.0/ });
    const v2 = screen.getByRole("button", { name: /v2\.0\.0/ });
    fireEvent.mouseDown(v1, { button: 0 });
    fireEvent.keyDown(v1, { key: "ArrowDown", shiftKey: true });
    expect(v1).toHaveAttribute("aria-pressed", "true");
    expect(v2).toHaveAttribute("aria-pressed", "true");
    expect(mockedCheckoutBranch).not.toHaveBeenCalled();
  });

  it("opens a checkout dialog on local branch double-click and auto-stashes changes", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "topic"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const topic = await screen.findByRole("button", { name: "Branch topic" });
    fireEvent.doubleClick(topic);
    expect(
      screen.getByRole("dialog", { name: "Checkout branch" }),
    ).toBeInTheDocument();
    const autoStash = screen.getByRole("checkbox", {
      name: "Automatically stash and reapply local changes",
    });
    expect(autoStash).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Checkout" }));

    expect(mockedCheckoutBranch).toHaveBeenCalledWith(
      snapshot.repository.id,
      snapshot.revision,
      "topic",
      false,
      false,
      true,
    );
  });

  it("opens a checkout dialog for remote branches and can skip auto-stash", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 1, items: ["origin/dev"] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const remote = await screen.findByRole("button", {
      name: "Branch origin/dev",
    });
    fireEvent.doubleClick(remote);
    expect(
      screen.getByText(
        "Check out remote branch origin/dev as a tracking branch?",
      ),
    ).toBeInTheDocument();
    const autoStash = screen.getByRole("checkbox", {
      name: "Automatically stash and reapply local changes",
    });
    fireEvent.click(autoStash);
    expect(autoStash).not.toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Checkout" }));

    expect(mockedCheckoutBranch).toHaveBeenCalledWith(
      snapshot.repository.id,
      snapshot.revision,
      "origin/dev",
      true,
      false,
      false,
    );
  });

  it("opens a detached checkout dialog on local tag double-click", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
    });
    render(<App />);

    const tag = await screen.findByRole("button", { name: /v1\.0\.0/ });
    fireEvent.doubleClick(tag);
    expect(
      screen.getByText(
        "Check out tag v1.0.0 in detached HEAD mode?",
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Checkout" }));

    expect(mockedCheckoutBranch).toHaveBeenCalledWith(
      snapshot.repository.id,
      snapshot.revision,
      "v1.0.0",
      false,
      true,
      true,
    );
  });

  it("shows branch actions and renames an upstream branch from its context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "topic"] },
      remoteBranches: { total: 1, items: ["origin/topic"] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
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
        ahead: 0,
        behind: 0,
      },
    ]);
    render(<App />);

    const topic = await screen.findByRole("button", { name: "Branch topic" });
    fireEvent.contextMenu(topic, { clientX: 100, clientY: 140 });
    expect(
      screen.getAllByRole("menuitem").map((item) => item.textContent),
    ).toEqual([
      "New branch from here",
      "Rename branch",
      "Delete branch",
      "Rebase current branch onto this branch",
      "Fast-forward to topic",
      "Create tag here",
    ]);
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename branch" }));
    fireEvent.change(screen.getByLabelText("Branch name"), {
      target: { value: "topic-renamed" },
    });
    fireEvent.click(
      screen.getByLabelText("Also rename upstream branch origin/topic"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Rename branch" }));
    await waitFor(() =>
      expect(mockedRenameBranch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "topic",
        "topic-renamed",
        true,
      ),
    );
  });

  it("waits for a repository refresh before creating a tag", async () => {
    let notifyRepositoryChange: ((repoId: string) => void) | undefined;
    let completeRefresh:
      | ((snapshot: RepositorySnapshotDto) => void)
      | undefined;
    mockedListenForChanges.mockImplementation(async (callback) => {
      notifyRepositoryChange = callback;
      return () => undefined;
    });
    mockedGetSnapshot.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          completeRefresh = resolve;
        }),
    );
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "topic"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const topic = await screen.findByRole("button", { name: "Branch topic" });
    fireEvent.contextMenu(topic, { clientX: 100, clientY: 140 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Create tag here" }));
    fireEvent.change(screen.getByLabelText("Tag name"), {
      target: { value: "v2.0.0" },
    });

    act(() => notifyRepositoryChange?.(snapshot.repository.id));
    expect(screen.getByRole("button", { name: "Create tag" })).toBeDisabled();

    const refreshedSnapshot = { ...snapshot, revision: 3 };
    await act(async () => completeRefresh?.(refreshedSnapshot));
    const createButton = screen.getByRole("button", { name: "Create tag" });
    expect(createButton).toBeEnabled();
    fireEvent.click(createButton);

    await waitFor(() =>
      expect(mockedCreateTag).toHaveBeenCalledWith(
        snapshot.repository.id,
        3,
        "v2.0.0",
        "topic",
      ),
    );
  });

  it("fast-forwards to the matching origin branch from the branch context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 1, items: ["origin/main"] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const main = await screen.findByRole("button", { name: "Branch main" });
    fireEvent.contextMenu(main, { clientX: 100, clientY: 140 });
    fireEvent.click(
      screen.getByRole("menuitem", { name: "Fast-forward to main" }),
    );

    await waitFor(() =>
      expect(mockedFastForwardBranch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "main",
      ),
    );
  });

  it("deletes a local tag from its right-click context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
    });
    render(<App />);

    const tag = await screen.findByRole("button", { name: /v1\.0\.0/ });
    fireEvent.contextMenu(tag, { clientX: 100, clientY: 140 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete tag" }));
    expect(mockedDeleteTag).not.toHaveBeenCalled();
    fireEvent.click(
      await screen.findByRole("button", { name: "Delete locally" }),
    );
    await waitFor(() =>
      expect(mockedDeleteTag).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "v1.0.0",
        [],
      ),
    );
  });

  it("can delete a matching remote tag together with its local tag", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
    });
    mockedGetRemoteTags.mockResolvedValue([
      { remote: "origin", name: "v1.0.0", oid: "abcdef123456" },
    ]);
    render(<App />);

    const tag = await screen.findByRole("button", { name: /v1\.0\.0/ });
    fireEvent.contextMenu(tag, { clientX: 100, clientY: 140 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete tag" }));
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Delete locally and remotely",
      }),
    );

    await waitFor(() =>
      expect(mockedDeleteTag).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "v1.0.0",
        [{ remote: "origin", name: "v1.0.0" }],
      ),
    );
  });

  it("can delete a matching remote branch together with its local branch", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "topic"] },
      remoteBranches: { total: 1, items: ["origin/topic"] },
      tags: { total: 0, items: [] },
      stashes: [],
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
        ahead: 0,
        behind: 0,
      },
    ]);
    mockedGetRemotes.mockResolvedValue([
      { name: "origin", url: "https://example.com/acorn.git" },
    ]);
    render(<App />);

    const branch = await screen.findByRole("button", { name: "Branch topic" });
    fireEvent.contextMenu(branch, { clientX: 100, clientY: 140 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete branch" }));
    expect(
      screen.getByText("This reference also exists on origin/topic."),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Delete locally and remotely" }),
    );

    await waitFor(() =>
      expect(mockedDeleteBranch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "topic",
        [{ remote: "origin", name: "topic" }],
      ),
    );
  });

  it("adds, edits, and removes remotes from right-click context menus", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetRemotes.mockResolvedValue([
      { name: "origin", url: "https://example.com/old.git" },
    ]);
    render(<App />);

    const origin = await screen.findByRole("button", { name: "Remote origin" });
    fireEvent.contextMenu(origin, { clientX: 120, clientY: 160 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Edit remote" }));
    fireEvent.change(screen.getByLabelText("Remote name"), {
      target: { value: "upstream" },
    });
    fireEvent.change(screen.getByLabelText("Remote URL"), {
      target: { value: "https://example.com/new.git" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save remote" }));
    await waitFor(() =>
      expect(mockedUpdateRemote).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "origin",
        { name: "upstream", url: "https://example.com/new.git" },
      ),
    );
    await waitFor(() =>
      expect(screen.queryByRole("heading", { name: "Edit remote" })).not.toBeInTheDocument(),
    );

    const remoteHeader = origin
      .closest(".sidebar-read-group")
      ?.querySelector<HTMLElement>(".sidebar-group");
    expect(remoteHeader).not.toBeNull();
    fireEvent.contextMenu(remoteHeader!, { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Add remote" }));
    fireEvent.change(screen.getByLabelText("Remote name"), {
      target: { value: "mirror" },
    });
    fireEvent.change(screen.getByLabelText("Remote URL"), {
      target: { value: "ssh://git@example.com/mirror.git" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add remote" }));
    await waitFor(() =>
      expect(mockedAddRemote).toHaveBeenCalledWith(
        snapshot.repository.id,
        2,
        { name: "mirror", url: "ssh://git@example.com/mirror.git" },
      ),
    );

    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    fireEvent.contextMenu(
      screen.getByRole("button", { name: "Remote origin" }),
      { clientX: 120, clientY: 160 },
    );
    fireEvent.click(screen.getByRole("menuitem", { name: "Remove remote" }));
    await waitFor(() =>
      expect(mockedRemoveRemote).toHaveBeenCalledWith(
        snapshot.repository.id,
        2,
        "origin",
      ),
    );
    confirm.mockRestore();
  });

  it("shows remote details from the right-click context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 2, items: ["origin/main", "origin/topic"] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    mockedGetRemotes.mockResolvedValue([
      { name: "origin", url: "https://example.com/acorn.git" },
    ]);
    mockedGetRemoteTags.mockResolvedValue([
      { remote: "origin", name: "v1.0.0", oid: "999999999999" },
    ]);
    render(<App />);

    const origin = await screen.findByRole("button", { name: "Remote origin" });
    fireEvent.contextMenu(origin, { clientX: 120, clientY: 160 });
    fireEvent.click(
      screen.getByRole("menuitem", { name: "Refresh remote tags" }),
    );
    await screen.findByRole("button", { name: "Tag v1.0.0" });

    fireEvent.contextMenu(origin, { clientX: 120, clientY: 160 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Remote details" }));

    const dialog = screen.getByRole("dialog", { name: "Remote details" });
    expect(within(dialog).getByText("origin")).toBeInTheDocument();
    expect(
      within(dialog).getByText("https://example.com/acorn.git"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("main")).toBeInTheDocument();
    expect(within(dialog).getByText("topic")).toBeInTheDocument();
    expect(within(dialog).getByText("v1.0.0")).toBeInTheDocument();

    fireEvent.click(
      within(dialog).getByRole("button", { name: "Close remote details" }),
    );
    expect(
      screen.queryByRole("dialog", { name: "Remote details" }),
    ).not.toBeInTheDocument();
  });

  it("groups remote branches and remote tags under Remote", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 1, items: ["main"] },
      remoteBranches: { total: 1, items: ["origin/dev"] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
    });
    mockedGetRemoteTags.mockResolvedValue([
      { remote: "origin", name: "v9.0.0", oid: "999999999999" },
    ]);
    render(<App />);

    const remoteBranch = await screen.findByRole("button", {
      name: "Branch origin/dev",
    });
    const remoteGroup = remoteBranch.closest(".sidebar-read-group");
    const remoteNode = remoteBranch.closest(".remote-reference-node");
    const remoteChildren = remoteBranch.closest(
      ".remote-reference-children",
    );
    expect(remoteGroup).not.toBeNull();
    expect(remoteNode).not.toBeNull();
    expect(remoteChildren).toHaveAttribute("role", "group");
    expect(remoteChildren).toContainElement(remoteBranch);
    expect(remoteGroup?.querySelector(".sidebar-group")).toHaveTextContent("Remote");
    expect(screen.queryByText(/Remote Branches/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Remote Tags/)).not.toBeInTheDocument();

    fireEvent.contextMenu(
      screen.getByRole("button", { name: "Remote origin" }),
      { clientX: 120, clientY: 160 },
    );
    fireEvent.click(
      screen.getByRole("menuitem", { name: "Refresh remote tags" }),
    );
    const remoteTag = await screen.findByRole("button", { name: "Tag v9.0.0" });
    expect(remoteNode).toContainElement(remoteTag);
    expect(mockedGetRemoteTags).toHaveBeenCalledWith(
      snapshot.repository.id,
      "origin",
    );

    const localTag = screen.getByText("v1.0.0");
    const localTagsGroup = localTag.closest(".sidebar-read-group");
    expect(localTagsGroup?.querySelector(".sidebar-group")).toHaveTextContent("Tags");
    expect(localTagsGroup).toContainElement(localTag);
    expect(localTagsGroup).not.toContainElement(remoteTag);
  });

  it("uses the branch icon for current and non-current local branches", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "topic"] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [],
    });
    render(<App />);

    const main = await screen.findByRole("button", { name: "Branch main" });
    const topic = screen.getByRole("button", { name: "Branch topic" });
    expect(main.querySelector(".branch-icon")).toHaveTextContent("⎇");
    expect(topic.querySelector(".branch-icon")).toHaveTextContent("⎇");
    expect(main.querySelector(".branch-icon")).not.toHaveTextContent("●");
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

  it("moves focus between commit fields without editing their contents", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await screen.findByRole("button", { name: "Commit to main" });
    const summary = screen.getByRole("textbox", {
      name: "Commit summary",
    }) as HTMLTextAreaElement;
    const description = screen.getByRole("textbox", {
      name: "Commit description",
    }) as HTMLTextAreaElement;

    fireEvent.change(summary, { target: { value: "Ship M3" } });
    summary.focus();
    fireEvent.keyDown(summary, { key: "Enter" });
    expect(summary).toHaveValue("Ship M3");
    expect(description).toHaveFocus();

    fireEvent.change(description, { target: { value: "Details" } });
    description.setSelectionRange(0, 0);
    fireEvent.keyDown(description, { key: "Backspace" });
    expect(description).toHaveValue("Details");
    expect(summary).toHaveFocus();
    expect(summary).toHaveProperty("selectionStart", "Ship M3".length);
    expect(summary).toHaveProperty("selectionEnd", "Ship M3".length);
  });

  it("places the commit form below the diff and preserves it while collapsed", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await screen.findByRole("button", { name: "Commit to main" });
    const toggle = screen.getByRole("button", {
      name: "Commit form",
    });
    const commitPanel = toggle.closest(".commit-panel");
    expect(commitPanel?.parentElement).toHaveClass("diff-panel");
    expect(toggle).toHaveAttribute("aria-expanded", "true");

    fireEvent.change(screen.getByRole("textbox", { name: "Commit summary" }), {
      target: { value: "Keep this draft" },
    });
    fireEvent.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.queryByRole("textbox", { name: "Commit summary" }),
    ).not.toBeInTheDocument();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("textbox", { name: "Commit summary" })).toHaveValue(
      "Keep this draft",
    );
  });

  it("resizes the vertical commit form within its height limits", async () => {
    localStorage.removeItem("gitacorn:commit-panel-height");
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    await screen.findByRole("button", { name: "Commit to main" });
    const resizer = screen.getByRole("separator", {
      name: "Commit panel height",
    });
    const commitPanel = resizer.closest(".commit-panel");
    expect(commitPanel).toHaveStyle({ height: "300px" });
    expect(resizer).toHaveAttribute("aria-valuemin", "240");
    expect(resizer).toHaveAttribute("aria-valuemax", "480");

    fireEvent.keyDown(resizer, { key: "Home" });
    expect(commitPanel).toHaveStyle({ height: "240px" });
    fireEvent.keyDown(resizer, { key: "ArrowDown" });
    expect(commitPanel).toHaveStyle({ height: "240px" });

    fireEvent.keyDown(resizer, { key: "End" });
    expect(commitPanel).toHaveStyle({ height: "480px" });
    fireEvent.keyDown(resizer, { key: "ArrowUp" });
    expect(commitPanel).toHaveStyle({ height: "480px" });

    fireEvent.mouseDown(resizer, { clientY: 300 });
    fireEvent.mouseMove(window, { clientY: 350 });
    fireEvent.mouseUp(window);
    expect(commitPanel).toHaveStyle({ height: "430px" });
    expect(localStorage.getItem("gitacorn:commit-panel-height")).toBe("430");
    expect(screen.getByRole("textbox", { name: "Commit summary" })).toHaveClass(
      "commit-summary",
    );
    expect(
      screen.getByRole("textbox", { name: "Commit description" }),
    ).toHaveClass("commit-description");
    localStorage.removeItem("gitacorn:commit-panel-height");
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

  it("creates a stash for files selected in Changes while the sidebar stays list-only", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const navigation = await screen.findByRole("navigation", {
      name: "Repository navigation",
    });
    expect(
      within(navigation).queryByRole("textbox", { name: "Stash message" }),
    ).not.toBeInTheDocument();

    const trackedRow = await screen.findByRole("button", { name: /tracked\.txt/ });
    fireEvent.contextMenu(trackedRow, { clientX: 40, clientY: 50 });

    const fileActions = screen.getByRole("menu", { name: "File actions" });
    expect(within(fileActions).getByRole("menuitem", { name: "Stage file" })).toBeInTheDocument();
    expect(within(fileActions).getByRole("menuitem", { name: "Discard…" })).toBeInTheDocument();
    fireEvent.click(within(fileActions).getByRole("menuitem", { name: "Stash…" }));

    const dialog = screen.getByRole("dialog", { name: "Create stash" });
    expect(within(dialog).queryByRole("checkbox")).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Stash name" }), {
      target: { value: "before refactor" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create stash" }));

    await waitFor(() =>
      expect(mockedCreateStash).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "before refactor",
        [snapshot.changes[0].pathBytes],
      ),
    );
  });

  it("inspects blame and file history from the changed-file context menu", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const trackedRow = await screen.findByRole("button", { name: /tracked\.txt/ });
    fireEvent.contextMenu(trackedRow, { clientX: 40, clientY: 50 });
    const actions = screen.getByRole("menu", { name: "File actions" });
    fireEvent.click(within(actions).getByRole("menuitem", { name: "Blame file" }));

    const blameDialog = await screen.findByRole("dialog", { name: "Blame file" });
    expect(within(blameDialog).getByText("modified")).toBeInTheDocument();
    fireEvent.click(within(blameDialog).getByRole("button", { name: "Close path inspector" }));

    fireEvent.contextMenu(trackedRow, { clientX: 40, clientY: 50 });
    fireEvent.click(
      within(screen.getByRole("menu", { name: "File actions" })).getByRole(
        "menuitem",
        { name: "File history" },
      ),
    );
    const historyDialog = await screen.findByRole("dialog", { name: "File history" });
    expect(within(historyDialog).getByText("Update tracked file")).toBeInTheDocument();
    fireEvent.change(within(historyDialog).getByRole("textbox", { name: "Filter path history" }), {
      target: { value: "missing" },
    });
    expect(within(historyDialog).queryByText("Update tracked file")).not.toBeInTheDocument();

    fireEvent.change(within(historyDialog).getByRole("textbox", { name: "Filter path history" }), {
      target: { value: "" },
    });
    fireEvent.click(within(historyDialog).getByRole("button", { name: /abcdef12/ }));
    expect(mockedUpdateTab).toHaveBeenCalledWith(
      snapshot.repository.id,
      "history",
      "tracked.txt",
      "unstaged",
      280,
      undefined,
      "abcdef123456",
      undefined,
    );
  });

  it("keeps the staged and unstaged file selections mutually exclusive", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const unstagedRow = await screen.findByRole("button", { name: /tracked\.txt/ });
    const stagedRow = screen.getByRole("button", { name: /staged file\.txt/ });

    fireEvent.click(unstagedRow);
    expect(unstagedRow).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(stagedRow);
    expect(unstagedRow).toHaveAttribute("aria-pressed", "false");
    expect(stagedRow).toHaveAttribute("aria-pressed", "true");
    fireEvent.contextMenu(stagedRow, { clientX: 40, clientY: 50 });
    const stagedActions = screen.getByRole("menu", { name: "File actions" });
    expect(
      within(stagedActions).getByRole("menuitem", { name: "Unstage file" }),
    ).toBeInTheDocument();
    expect(
      within(stagedActions).queryByRole("menuitem", { name: "Discard…" }),
    ).not.toBeInTheDocument();
    fireEvent.click(document.body);

    fireEvent.click(unstagedRow);
    expect(stagedRow).toHaveAttribute("aria-pressed", "false");
    expect(unstagedRow).toHaveAttribute("aria-pressed", "true");
  });

  it("switches changed files between list and collapsible tree views with file icons", async () => {
    const nestedChange = {
      path: "src/components/card.tsx",
      pathBytes: [],
      indexStatus: ".",
      worktreeStatus: "M",
      conflict: false,
      submodule: false,
    };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [
        {
          ...sessionWithSnapshot.tabs[0],
          snapshot: {
            ...snapshot,
            changes: [...snapshot.changes, nestedChange],
          },
        },
      ],
    });
    mockedGetSystemFileIcons.mockResolvedValue({
      [nestedChange.path]: "data:image/png;base64,c3lzdGVtLWljb24=",
    });
    render(<App />);

    const unstagedHeading = await screen.findByRole("heading", { name: "Unstaged" });
    const unstagedSection = unstagedHeading.closest<HTMLElement>(".change-section");
    expect(unstagedSection).not.toBeNull();
    const section = within(unstagedSection!);
    const listRow = section.getByRole("button", { name: nestedChange.path });
    await waitFor(() =>
      expect(listRow.querySelector("img.change-file-icon.system")).toHaveAttribute(
        "src",
        "data:image/png;base64,c3lzdGVtLWljb24=",
      ),
    );

    fireEvent.click(section.getByRole("button", { name: "Show as tree" }));
    expect(section.getByRole("button", { name: "src" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    const componentsFolder = section.getByRole("button", { name: "components" });
    expect(section.getByRole("button", { name: nestedChange.path })).toHaveTextContent(
      "card.tsx",
    );

    fireEvent.click(componentsFolder);
    expect(section.queryByRole("button", { name: nestedChange.path })).not.toBeInTheDocument();
    fireEvent.click(section.getByRole("button", { name: "components" }));
    expect(section.getByRole("button", { name: nestedChange.path })).toBeInTheDocument();
    expect(localStorage.getItem("gitacorn:change-view:unstaged")).toBe("tree");
  });

  it("opens stash actions from a branch-like row and can apply then drop", async () => {
    const stash = {
      reference: "stash@{0}",
      message: "before refactor",
    };
    const stashSnapshot = { ...snapshot, stashCount: 1 };
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], snapshot: stashSnapshot }],
    });
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 0, items: [] },
      remoteBranches: { total: 0, items: [] },
      tags: { total: 0, items: [] },
      stashes: [stash],
    });
    mockedApplyStash.mockResolvedValue({ ...stashSnapshot, revision: 2 });
    mockedDropStash.mockResolvedValue({
      ...stashSnapshot,
      revision: 3,
      stashCount: 0,
    });
    render(<App />);

    const stashRow = await screen.findByRole("button", {
      name: "Stash stash@{0}: before refactor",
    });
    expect(stashRow).toHaveClass("tree-leaf-row", "branch-item-row");

    fireEvent.contextMenu(stashRow, { clientX: 40, clientY: 50 });
    expect(screen.getByRole("menuitem", { name: "Drop" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "Apply" }));

    expect(
      screen.getByRole("dialog", { name: "Apply stash" }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: "Drop this stash after applying",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));

    await waitFor(() =>
      expect(mockedApplyStash).toHaveBeenCalledWith(
        snapshot.repository.id,
        stashSnapshot.revision,
        stash.reference,
      ),
    );
    await waitFor(() =>
      expect(mockedDropStash).toHaveBeenCalledWith(
        snapshot.repository.id,
        2,
        stash.reference,
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

    expect(screen.queryByRole("button", { name: "Operations" })).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Show operation history" }));

    expect(
      await screen.findByText("Interrupted when GitAcorn last exited"),
    ).toBeInTheDocument();
  });

  it("undoes and redoes a recoverable commit from the operation center", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const ready = {
      schemaVersion: 1,
      id: "commit-operation",
      repoId: snapshot.repository.id,
      kind: "commit",
      state: "succeeded" as const,
      summary: "Created commit",
      startedAt: "2026-08-02 01:00:00",
      recoveryAction: "commit" as const,
      recoveryState: "ready" as const,
    };
    mockedGetOperationHistory
      .mockResolvedValueOnce([ready])
      .mockResolvedValueOnce([{ ...ready, recoveryState: "undone" }])
      .mockResolvedValueOnce([ready]);
    mockedUndoOperation.mockResolvedValue({ ...snapshot, revision: 2 });
    mockedRedoOperation.mockResolvedValue({ ...snapshot, revision: 3 });
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "Show operation history" }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo commit" }));
    await waitFor(() =>
      expect(mockedUndoOperation).toHaveBeenCalledWith(
        "commit-operation",
        snapshot.repository.id,
        snapshot.revision,
      ),
    );

    fireEvent.click(await screen.findByRole("button", { name: "Redo commit" }));
    await waitFor(() =>
      expect(mockedRedoOperation).toHaveBeenCalledWith(
        "commit-operation",
        snapshot.repository.id,
        2,
      ),
    );
  });

  it("labels checkout and deleted-branch recovery actions explicitly", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetOperationHistory.mockResolvedValue([
      {
        schemaVersion: 1,
        id: "checkout-operation",
        repoId: snapshot.repository.id,
        kind: "checkout",
        state: "succeeded",
        summary: "Checked out reference",
        startedAt: "2026-08-02 02:00:00",
        recoveryAction: "checkout",
        recoveryState: "ready",
      },
      {
        schemaVersion: 1,
        id: "delete-operation",
        repoId: snapshot.repository.id,
        kind: "branch-delete",
        state: "succeeded",
        summary: "Deleted branch",
        startedAt: "2026-08-02 02:01:00",
        recoveryAction: "branch-delete",
        recoveryState: "ready",
      },
      {
        schemaVersion: 1,
        id: "rebase-operation",
        repoId: snapshot.repository.id,
        kind: "rebase",
        state: "succeeded",
        summary: "Rebased branch",
        startedAt: "2026-08-02 02:02:00",
        recoveryAction: "rebase",
        recoveryState: "ready",
      },
    ]);
    mockedUndoOperation.mockResolvedValue({ ...snapshot, revision: 2 });
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "Show operation history" }));
    expect(await screen.findByRole("button", { name: "Undo checkout" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Undo rebase" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Restore deleted branch" }));

    await waitFor(() =>
      expect(mockedUndoOperation).toHaveBeenCalledWith(
        "delete-operation",
        snapshot.repository.id,
        snapshot.revision,
      ),
    );
  });

  it("restores a selected reflog entry as a new branch", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetReflog.mockResolvedValue([
      {
        schemaVersion: 1,
        selector: "HEAD@{1}",
        oid: "1234567890abcdef1234567890abcdef12345678",
        message: "reset: moving to HEAD~1",
        parents: [],
        authorName: "Ada",
        authorEmail: "ada@example.com",
        authoredAt: 1_699_999_900,
        subject: "Recovered commit",
        body: "",
        reflogOnly: true,
      },
    ]);
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "Show operation history" }));
    expect(await screen.findByText("reset: moving to HEAD~1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Restore as branch…" }));
    const name = screen.getByLabelText("Branch name");
    fireEvent.change(name, { target: { value: "recovered-work" } });
    fireEvent.click(screen.getByRole("button", { name: "Restore reference" }));

    await waitFor(() =>
      expect(mockedRestoreReflogReference).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        "1234567890abcdef1234567890abcdef12345678",
        "recovered-work",
        false,
      ),
    );
  });

  it("resizes the sidebar and persists width in localStorage", async () => {
    localStorage.clear();
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const sidebarResizer = await screen.findByRole("separator", { name: "Sidebar width" });
    expect(sidebarResizer).toBeInTheDocument();

    fireEvent.mouseDown(sidebarResizer, { clientX: 200 });
    fireEvent.mouseMove(window, { clientX: 250 });
    fireEvent.mouseUp(window);

    expect(localStorage.getItem("gitacorn:sidebar-width")).toBe("256");

    fireEvent.keyDown(sidebarResizer, { key: "ArrowLeft" });
    expect(localStorage.getItem("gitacorn:sidebar-width")).toBe("246");
  });

  it("resizes Stage and Unstage split height and persists ratio in localStorage", async () => {
    localStorage.clear();
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const stageResizer = await screen.findByRole("separator", {
      name: "Stage and Unstage split height",
    });
    expect(stageResizer).toBeInTheDocument();

    const filePanel = stageResizer.closest(".file-panel");
    expect(filePanel).not.toBeNull();
    if (filePanel) {
      vi.spyOn(filePanel, "getBoundingClientRect").mockReturnValue({
        top: 0,
        bottom: 500,
        left: 0,
        right: 300,
        width: 300,
        height: 500,
        x: 0,
        y: 0,
        toJSON: () => {},
      });
    }

    fireEvent.mouseDown(stageResizer, { clientY: 200 });
    fireEvent.mouseMove(window, { clientY: 300 });
    fireEvent.mouseUp(window);

    expect(localStorage.getItem("gitacorn:stage-split-ratio")).not.toBeNull();

    fireEvent.keyDown(stageResizer, { key: "ArrowDown" });
    expect(localStorage.getItem("gitacorn:stage-split-ratio")).not.toBeNull();
  });

  it("resizes the file panel width via mouse dragging", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    render(<App />);

    const filePanelResizer = await screen.findByRole("separator", {
      name: "File panel width",
    });
    expect(filePanelResizer).toBeInTheDocument();

    fireEvent.mouseDown(filePanelResizer, { clientX: 280 });
    fireEvent.mouseMove(window, { clientX: 340 });
    fireEvent.mouseUp(window);

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

  it("navigates to History tab and selects head commit when a branch or tag in the sidebar is clicked", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetSidebar.mockResolvedValue({
      schemaVersion: 1,
      worktrees: [],
      branches: { total: 2, items: ["main", "feature/login"] },
      remoteBranches: { total: 1, items: ["origin/dev"] },
      tags: { total: 1, items: ["v1.0.0"] },
      stashes: [],
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
        fullName: "refs/heads/feature/login",
        shortName: "feature/login",
        oid: "111122223333",
        kind: "localBranch",
        ahead: 1,
        behind: 0,
      },
      {
        fullName: "refs/remotes/origin/dev",
        shortName: "origin/dev",
        oid: "444455556666",
        kind: "remoteBranch",
        ahead: 0,
        behind: 0,
      },
      {
        fullName: "refs/tags/v1.0.0",
        shortName: "v1.0.0",
        oid: "777788889999",
        kind: "tag",
        ahead: 0,
        behind: 0,
      },
    ]);

    render(<App />);

    const featureBranch = await screen.findByText("login");
    fireEvent.click(featureBranch);

    await waitFor(() =>
      expect(mockedUpdateTab).toHaveBeenCalledWith(
        snapshot.repository.id,
        "history",
        undefined,
        "unstaged",
        280,
        undefined,
        "111122223333",
        undefined,
      ),
    );

    const tagItem = await screen.findByText("v1.0.0");
    fireEvent.click(tagItem);

    await waitFor(() =>
      expect(mockedUpdateTab).toHaveBeenLastCalledWith(
        snapshot.repository.id,
        "history",
        undefined,
        "unstaged",
        280,
        undefined,
        "777788889999",
        undefined,
      ),
    );
  });

  it("builds a hierarchical tree from remote branch short names", () => {
    const tree = buildRemoteBranchTree([
      "origin/main",
      "origin/feature/login",
      "origin/feature/signup",
      "upstream/main",
    ]);

    expect(tree).toHaveLength(2);
    expect(tree[0].name).toBe("origin");
    expect(tree[0].count).toBe(3);
    expect(tree[0].children).toHaveLength(2);

    const featureNode = tree[0].children.find((child) => child.name === "feature");
    expect(featureNode).toBeDefined();
    expect(featureNode?.count).toBe(2);
    expect(featureNode?.children).toHaveLength(2);
    expect(featureNode?.children[0].name).toBe("login");

    expect(tree[1].name).toBe("upstream");
    expect(tree[1].count).toBe(1);
  });

  it("opens settings modal and allows changing theme between system, light, and dark", async () => {
    mockedRestoreSession.mockResolvedValueOnce({
      schemaVersion: 1,
      tabs: [],
    });

    render(<App />);

    const settingsBtn = await screen.findByRole("button", {
      name: /^Settings$|^설정$/i,
    });
    fireEvent.click(settingsBtn);

    expect(screen.getByRole("dialog")).toBeInTheDocument();

    const lightOption = screen.getByRole("button", { name: /light|라이트/i });
    fireEvent.click(lightOption);

    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("gitacorn_theme")).toBe("light");

    const darkOption = screen.getByRole("button", { name: /dark|다크/i });
    fireEvent.click(darkOption);

    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem("gitacorn_theme")).toBe("dark");

    const systemOption = screen.getByRole("button", { name: /system|시스템/i });
    fireEvent.click(systemOption);

    expect(localStorage.getItem("gitacorn_theme")).toBe("system");

    const gravatarOption = screen.getByRole("checkbox", {
      name: /Show Gravatar images|Gravatar 이미지 표시/i,
    });
    expect(gravatarOption).not.toBeChecked();
    expect(localStorage.getItem("gitacorn_show_gravatars")).toBe("false");

    fireEvent.click(gravatarOption);

    expect(gravatarOption).toBeChecked();
    expect(localStorage.getItem("gitacorn_show_gravatars")).toBe("true");

    const compactGraphOption = screen.getByRole("checkbox", {
      name: /Compact commit graph|커밋 그래프 작게 보기/i,
    });
    expect(compactGraphOption).not.toBeChecked();
    expect(localStorage.getItem("gitacorn_compact_commit_graph")).toBe("false");

    fireEvent.click(compactGraphOption);

    expect(compactGraphOption).toBeChecked();
    expect(localStorage.getItem("gitacorn_compact_commit_graph")).toBe("true");

    const closeBtn = screen.getByRole("button", { name: /close settings|설정 닫기/i });
    fireEvent.click(closeBtn);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("uses a one-line compact layout for the commit graph when enabled", async () => {
    localStorage.setItem("gitacorn_compact_commit_graph", "true");
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);

    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    const commitRow = await screen.findByRole("button", {
      name: /Initial commit/,
    });
    expect(commitRow.closest(".commit-list")).toHaveClass("compact");
    expect(commitRow.querySelector(".commit-graph")).toHaveAttribute(
      "viewBox",
      "0 0 44 32",
    );
  });

  it("copies the full commit SHA from the commit context menu", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);

    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    const commitRow = await screen.findByRole("button", {
      name: /Initial commit/,
    });

    fireEvent.contextMenu(commitRow);
    fireEvent.click(
      screen.getByRole("menuitem", { name: /Copy commit SHA|커밋 SHA 복사/i }),
    );

    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith("abcdef123456"),
    );
    expect(screen.queryByRole("menu", { name: /Commit actions|커밋 작업/i }))
      .not.toBeInTheDocument();
  });

  it("previews a reset mode and submits a recoverable branch reset", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    const targetOid = "base123456";
    const headOid = snapshot.head.oid ?? "abcdef123456";
    mockedGetHistory.mockResolvedValue({
      schemaVersion: 1,
      commits: [
        {
          oid: headOid,
          parents: [targetOid],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_001,
          subject: "Head commit",
          body: "",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
        {
          oid: targetOid,
          parents: [],
          authorName: "Ada",
          authorEmail: "ada@example.com",
          authoredAt: 1_700_000_000,
          subject: "Base commit",
          body: "",
          references: [],
          lane: 0,
          laneCount: 1,
        },
      ],
    });

    render(<App />);

    await screen.findByText("acorn-demo");
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));
    const commitRow = await screen.findByRole("button", {
      name: /Base commit/,
    });

    fireEvent.contextMenu(commitRow);
    fireEvent.click(
      screen.getByRole("menuitem", {
        name: /Reset current branch to this|현재 브랜치를 이 커밋으로 리셋/i,
      }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: /Reset current branch|현재 브랜치 리셋/i,
    });
    fireEvent.change(
      within(dialog).getByRole("combobox", {
        name: /Reset mode|리셋 모드/i,
      }),
      { target: { value: "hard" } },
    );
    expect(
      within(dialog).getByText(/Hard reset discards uncommitted changes/i),
    ).toBeInTheDocument();

    fireEvent.click(
      within(dialog).getByRole("button", {
        name: /Reset branch|브랜치 리셋/i,
      }),
    );

    await waitFor(() =>
      expect(mockedResetBranch).toHaveBeenCalledWith(
        snapshot.repository.id,
        snapshot.revision,
        targetOid,
        "hard",
      ),
    );
    expect(confirm).toHaveBeenCalled();
    confirm.mockRestore();
  });

  it("updates global and repository Git identity from separate settings menus", async () => {
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetGitIdentity.mockResolvedValue({
      schemaVersion: 1,
      global: {
        name: "Ada Lovelace",
        email: "ada@example.com",
      },
      repository: {
        repoId: snapshot.repository.id,
        repositoryName: snapshot.repository.name,
        local: {
          name: "Grace Hopper",
        },
        effective: {
          name: "Grace Hopper",
          email: "ada@example.com",
        },
      },
    });

    render(<App />);
    await screen.findByText(snapshot.repository.name);
    fireEvent.click(
      screen.getByRole("button", { name: /^Settings$|^설정$/i }),
    );

    const globalForm = await screen.findByRole("form", {
      name: /Global Git identity|글로벌 Git 작성자 정보/i,
    });
    const globalFields = within(globalForm).getAllByRole("textbox");
    expect(globalFields[0]).toHaveValue("Ada Lovelace");
    expect(globalFields[1]).toHaveValue("ada@example.com");
    fireEvent.change(globalFields[0], { target: { value: "Ada Byron" } });
    fireEvent.click(
      within(globalForm).getByRole("button", {
        name: /Save global identity|글로벌 작성자 저장/i,
      }),
    );

    await waitFor(() =>
      expect(mockedUpdateGlobalGitIdentity).toHaveBeenCalledWith({
        name: "Ada Byron",
        email: "ada@example.com",
      }),
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: /Close settings|설정 닫기/i,
      }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: /Repository settings|저장소 설정/i,
      }),
    );

    const repositoryForm = await screen.findByRole("form", {
      name: /Repository Git identity|저장소 Git 작성자 정보/i,
    });
    const repositoryFields = within(repositoryForm).getAllByRole("textbox");
    expect(repositoryFields[0]).toBeEnabled();
    expect(repositoryFields[1]).toBeDisabled();
    fireEvent.click(
      within(repositoryForm).getByRole("checkbox", {
        name: /Override email|이 저장소에서 이메일 재정의/i,
      }),
    );
    fireEvent.change(repositoryFields[1], {
      target: { value: "grace@example.com" },
    });
    fireEvent.click(
      within(repositoryForm).getByRole("button", {
        name: /Save repository identity|저장소 작성자 저장/i,
      }),
    );

    await waitFor(() =>
      expect(mockedUpdateRepositoryGitIdentity).toHaveBeenCalledWith(
        snapshot.repository.id,
        {
          name: "Grace Hopper",
          email: "grace@example.com",
        },
      ),
    );
  });

  it("disables the repository settings menu when no repository is open", async () => {
    mockedRestoreSession.mockResolvedValue({
      schemaVersion: 1,
      tabs: [],
    });

    render(<App />);
    const repositorySettings = await screen.findByRole("button", {
      name: /Repository settings|저장소 설정/i,
    });
    expect(repositorySettings).toBeDisabled();
    fireEvent.click(
      screen.getByRole("button", { name: /^Settings$|^설정$/i }),
    );
    expect(
      screen.queryByRole("form", {
        name: /Repository Git identity|저장소 Git 작성자 정보/i,
      }),
    ).not.toBeInTheDocument();
  });

  it("shows Gravatar images in commit history only when enabled", async () => {
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
          body: "Co-authored-by: Grace Hopper <grace@example.com>",
          references: ["HEAD -> refs/heads/main"],
          lane: 0,
          laneCount: 1,
        },
      ],
    });
    mockedRestoreSession.mockResolvedValue({
      ...sessionWithSnapshot,
      tabs: [{ ...sessionWithSnapshot.tabs[0], page: "history" }],
    });

    const disabledView = render(<App />);
    await screen.findByRole("button", { name: /Initial commit/ });
    expect(screen.queryByTitle(/Ada.*Gravatar/i)).not.toBeInTheDocument();
    disabledView.unmount();

    localStorage.setItem("gitacorn_show_gravatars", "true");
    render(<App />);

    const avatar = await screen.findByTitle(/Ada.*Gravatar/i);
    expect(avatar).toHaveAttribute(
      "src",
      expect.stringMatching(
        /^https:\/\/www\.gravatar\.com\/avatar\/[0-9a-f]{64}\?s=40&d=identicon$/,
      ),
    );
    expect(await screen.findByTitle(/Grace Hopper.*Gravatar/i)).toBeInTheDocument();
  });

  it("shows reflog commits in history when enabled for the repository", async () => {
    const reflogOid = "1234567890abcdef1234567890abcdef12345678";
    mockedRestoreSession.mockResolvedValue(sessionWithSnapshot);
    mockedGetReflog.mockResolvedValue([
      {
        schemaVersion: 1,
        selector: "HEAD@{1}",
        oid: reflogOid,
        message: "reset: moving to HEAD~1",
        parents: [],
        authorName: "Grace",
        authorEmail: "grace@example.com",
        authoredAt: 1_699_999_900,
        subject: "Recovered reflog commit",
        body: "Lost work",
        reflogOnly: true,
      },
      {
        schemaVersion: 1,
        selector: "HEAD@{0}",
        oid: "abcdef123456",
        message: "commit: Initial commit",
        parents: [],
        authorName: "Ada",
        authorEmail: "ada@example.com",
        authoredAt: 1_700_000_000,
        subject: "Initial commit",
        body: "",
        reflogOnly: false,
      },
    ]);

    render(<App />);
    await screen.findByText(snapshot.repository.name);
    fireEvent.click(
      screen.getByRole("button", {
        name: /Repository settings|저장소 설정/i,
      }),
    );
    const toggle = await screen.findByRole("checkbox", {
      name: /Show reflog in history|기록에서 Reflog 보기/i,
    });
    expect(toggle).not.toBeChecked();
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(
        JSON.parse(
          localStorage.getItem("gitacorn_show_reflog_by_repository") ?? "{}",
        ),
      ).toEqual({ [snapshot.repository.id]: true }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: /Close repository settings|저장소 설정 닫기/i,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    const reflogCommit = await screen.findByRole("button", {
      name: /Recovered reflog commit/,
    });
    expect(reflogCommit).toHaveTextContent("Reflog · HEAD@{1}");
    expect(reflogCommit).toHaveClass("commit-row", "reflog-only");
    expect(
      screen.getByRole("button", { name: /Initial commit/ }),
    ).not.toHaveClass("reflog-only");
    expect(
      screen.getByRole("button", { name: /Initial commit/ }),
    ).not.toHaveTextContent("Reflog");
    expect(mockedGetReflog).toHaveBeenCalledWith(snapshot.repository.id);
  });
});

