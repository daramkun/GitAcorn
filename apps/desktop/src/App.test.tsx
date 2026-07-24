import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { getAppInfo } from "./app-info";
import {
  chooseRepositoryDirectory,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  openRepository,
  type RepositorySnapshotDto,
} from "./repository";

vi.mock("./app-info", () => ({
  getAppInfo: vi.fn(),
}));

vi.mock("./repository", () => ({
  chooseRepositoryDirectory: vi.fn(),
  openRepository: vi.fn(),
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

describe("App", () => {
  beforeEach(() => {
    mockedGetAppInfo.mockResolvedValue({
      schemaVersion: 1,
      name: "GitAcorn",
      version: "0.1.0",
      runtime: "Tauri 2",
    });
    mockedChooseRepository.mockResolvedValue(null);
    mockedOpenRepository.mockResolvedValue(snapshot);
    mockedGetSnapshot.mockResolvedValue(snapshot);
    mockedListenForChanges.mockResolvedValue(vi.fn());
  });

  it("renders the typed app info returned by the Rust core", async () => {
    render(<App />);

    expect(screen.getByText("Connecting to core…")).toBeInTheDocument();
    expect(await screen.findByText("Tauri 2 · v0.1.0")).toBeInTheDocument();
  });

  it("switches between Changes and History", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    expect(screen.getByRole("heading", { name: "History will appear here." })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^History/ })).toHaveAttribute(
      "aria-current",
      "page",
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
