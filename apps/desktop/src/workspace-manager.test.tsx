import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceManager } from "./workspace-manager";
import {
  chooseRepositoryDirectory,
  deleteWorkspace,
  getWorkspaces,
  runWorkspaceBatch,
  saveWorkspace,
} from "./repository";

vi.mock("./repository", () => ({
  chooseRepositoryDirectory: vi.fn(),
  deleteWorkspace: vi.fn(),
  getWorkspaces: vi.fn(),
  runWorkspaceBatch: vi.fn(),
  saveWorkspace: vi.fn(),
  normalizeAppError: (reason: unknown) => ({
    schemaVersion: 1,
    code: "test",
    message: reason instanceof Error ? reason.message : String(reason),
    recoveryActions: [],
  }),
}));

const workspace = {
  id: "workspace-one",
  name: "Client application",
  repositories: [{ path: "C:/repos/frontend", cloneUrl: "https://example.invalid/frontend.git" }],
};

const mockedChooseRepositoryDirectory = vi.mocked(chooseRepositoryDirectory);
const mockedDeleteWorkspace = vi.mocked(deleteWorkspace);
const mockedGetWorkspaces = vi.mocked(getWorkspaces);
const mockedRunWorkspaceBatch = vi.mocked(runWorkspaceBatch);
const mockedSaveWorkspace = vi.mocked(saveWorkspace);

describe("WorkspaceManager", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedChooseRepositoryDirectory.mockResolvedValue(null);
    mockedDeleteWorkspace.mockResolvedValue();
    mockedGetWorkspaces.mockResolvedValue({ schemaVersion: 1, workspaces: [workspace] });
    mockedSaveWorkspace.mockResolvedValue(workspace);
    mockedRunWorkspaceBatch.mockResolvedValue({
      schemaVersion: 1,
      operation: "pull",
      results: [
        { path: workspace.repositories[0].path, state: "succeeded", message: "pull completed" },
        { path: "C:/repos/backend", state: "failed", message: "not a repository" },
      ],
    });
  });

  it("adds open repositories and persists the named group", async () => {
    render(
      <WorkspaceManager
        openRepositoryPaths={["C:/repos/frontend", "C:/repos/backend"]}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByDisplayValue("Client application")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Add open repositories|열린 저장소 추가/i }));
    expect(screen.getByDisplayValue("C:/repos/backend")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Save workspace|워크스페이스 저장/i }));

    await waitFor(() =>
      expect(mockedSaveWorkspace).toHaveBeenCalledWith({
        id: workspace.id,
        name: workspace.name,
        repositories: [
          workspace.repositories[0],
          { path: "C:/repos/backend", cloneUrl: "" },
        ],
      }),
    );
  });

  it("requires confirmation before pull and shows per-repository results", async () => {
    render(<WorkspaceManager openRepositoryPaths={[]} onClose={vi.fn()} />);

    await screen.findByDisplayValue("Client application");
    fireEvent.click(screen.getByRole("button", { name: /Pull all|모두 Pull/i }));
    expect(screen.getByRole("alertdialog", { name: /Confirm workspace operation|워크스페이스 작업 확인/i })).toBeVisible();
    expect(mockedRunWorkspaceBatch).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Continue|계속/i }));
    await waitFor(() => expect(mockedRunWorkspaceBatch).toHaveBeenCalledWith(workspace.id, "pull"));
    expect(await screen.findByText("pull completed")).toBeVisible();
    expect(screen.getByText("not a repository")).toBeVisible();
  });

  it("requires confirmation before deleting workspace metadata", async () => {
    render(<WorkspaceManager openRepositoryPaths={[]} onClose={vi.fn()} />);

    await screen.findByDisplayValue("Client application");
    fireEvent.click(screen.getByRole("button", { name: /^Delete$|^삭제$/i }));
    expect(screen.getByRole("alertdialog", { name: /Delete workspace|워크스페이스 삭제/i })).toBeVisible();
    expect(mockedDeleteWorkspace).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Delete workspace|워크스페이스 삭제/i }));
    await waitFor(() => expect(mockedDeleteWorkspace).toHaveBeenCalledWith(workspace.id));
  });
});
