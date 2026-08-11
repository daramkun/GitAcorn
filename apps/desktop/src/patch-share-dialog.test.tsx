import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PatchShareDialog } from "./patch-share-dialog";
import {
  applyComparePatch,
  deleteSharedPatch,
  fetchSharedPatch,
  getComparePatch,
  publishSharedPatch,
  validateComparePatch,
} from "./repository";

vi.mock("./repository", () => ({
  applyComparePatch: vi.fn(),
  deleteSharedPatch: vi.fn(),
  fetchSharedPatch: vi.fn(),
  getComparePatch: vi.fn(),
  publishSharedPatch: vi.fn(),
  validateComparePatch: vi.fn(),
  normalizeAppError: (reason: unknown) => ({ message: reason instanceof Error ? reason.message : String(reason) }),
}));

const mockedApply = vi.mocked(applyComparePatch);
const mockedDelete = vi.mocked(deleteSharedPatch);
const mockedFetch = vi.mocked(fetchSharedPatch);
const mockedGetPatch = vi.mocked(getComparePatch);
const mockedPublish = vi.mocked(publishSharedPatch);
const mockedValidate = vi.mocked(validateComparePatch);
const patch = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";

describe("PatchShareDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    localStorage.setItem("git-acorn.patch-share.endpoint.v1", "https://patch.example/");
    mockedGetPatch.mockResolvedValue({ schemaVersion: 1, patch, fileCount: 1, binary: false });
    mockedPublish.mockResolvedValue({ schemaVersion: 1, patchId: "shared-1", sha256: "a".repeat(64), webUrl: "https://patch.example/p/shared-1" });
    mockedDelete.mockResolvedValue();
    mockedValidate.mockResolvedValue({ schemaVersion: 1, valid: true });
  });

  it("previews the exact publish endpoint and waits for confirmation before writing remotely", async () => {
    render(<PatchShareDialog repoId="repo" repositoryName="team/demo" revision={7} defaultBaseRevision="main" onClose={vi.fn()} onSnapshot={vi.fn()} />);

    fireEvent.change(screen.getByLabelText(/^Title$|^제목$/i), { target: { value: "Fix cache" } });
    fireEvent.change(screen.getByLabelText(/Bearer token/i), { target: { value: "secret-token" } });
    fireEvent.click(screen.getByRole("button", { name: /Generate preview|미리보기 생성/i }));
    expect(await screen.findByText(/diff --git a\/a.txt/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Share patch|패치 공유/i }));

    const confirmation = screen.getByRole("alertdialog", { name: /Publish this patch|이 패치를 게시/i });
    expect(confirmation).toHaveTextContent("POST https://patch.example/v1/patches");
    expect(confirmation).toHaveTextContent(/No Git command runs|Git 명령은 실행하지 않습니다/i);
    expect(mockedPublish).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Publish patch|패치 게시/i }));
    await waitFor(() => expect(mockedPublish).toHaveBeenCalledWith({
      endpoint: "https://patch.example/",
      token: "secret-token",
      title: "Fix cache",
      description: "",
      repository: "team/demo",
      baseRevision: "main",
      patch,
    }));
    expect(localStorage.getItem("git-acorn.patch-share.endpoint.v1")).toBe("https://patch.example/");
    expect(localStorage.getItem("secret-token")).toBeNull();
  });

  it("verifies an imported patch and confirms the exact Git apply command before mutation", async () => {
    const imported = { schemaVersion: 1 as const, patchId: "shared-2", title: "Fix cache", description: "", repository: "team/demo", baseRevision: "main", patch, sha256: "b".repeat(64) };
    const snapshot = { schemaVersion: 1, revision: 8, repository: { id: "repo" } } as never;
    mockedFetch.mockResolvedValue(imported);
    mockedApply.mockResolvedValue(snapshot);
    const onSnapshot = vi.fn();
    render(<PatchShareDialog repoId="repo" repositoryName="team/demo" revision={7} defaultBaseRevision="main" onClose={vi.fn()} onSnapshot={onSnapshot} />);

    fireEvent.click(screen.getByRole("tab", { name: /Import|가져오기/i }));
    fireEvent.change(screen.getByLabelText(/Patch ID|패치 ID/i), { target: { value: "shared-2" } });
    fireEvent.click(screen.getByRole("button", { name: /Load and validate|불러와 검증/i }));
    expect(await screen.findByText(/Applies cleanly|깨끗하게 적용 가능/i)).toBeVisible();
    expect(mockedFetch).toHaveBeenCalledWith({ endpoint: "https://patch.example/", token: undefined }, "shared-2");
    expect(mockedValidate).toHaveBeenCalledWith("repo", patch);

    fireEvent.click(screen.getByRole("button", { name: /Apply shared patch|공유 패치 적용/i }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("git apply --index --recount --whitespace=error-all --");
    expect(mockedApply).not.toHaveBeenCalled();
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: /Apply shared patch|공유 패치 적용/i,
      }),
    );
    await waitFor(() => expect(mockedApply).toHaveBeenCalledWith("repo", 7, patch));
    expect(onSnapshot).toHaveBeenCalledWith(snapshot);
  });

  it("clears the previous action status when switching tabs", async () => {
    render(<PatchShareDialog repoId="repo" repositoryName="team/demo" revision={7} defaultBaseRevision="main" onClose={vi.fn()} onSnapshot={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /Generate preview|미리보기 생성/i }));
    expect(await screen.findByText(/Shared patch preview generated|공유 패치 미리보기를 생성/i)).toBeVisible();
    fireEvent.click(screen.getByRole("tab", { name: /Import|가져오기/i }));

    expect(screen.queryByText(/Shared patch preview generated|공유 패치 미리보기를 생성/i)).not.toBeInTheDocument();
  });

  it("does not enable publishing for endpoints containing credentials", async () => {
    render(<PatchShareDialog repoId="repo" repositoryName="team/demo" revision={7} defaultBaseRevision="main" onClose={vi.fn()} onSnapshot={vi.fn()} />);

    fireEvent.change(screen.getByLabelText(/^Title$|^제목$/i), { target: { value: "Fix cache" } });
    fireEvent.change(screen.getByLabelText(/Service endpoint|서비스 엔드포인트/i), { target: { value: "https://token@patch.example/" } });
    fireEvent.click(screen.getByRole("button", { name: /Generate preview|미리보기 생성/i }));
    await screen.findByText(/diff --git a\/a.txt/);

    expect(screen.getByRole("button", { name: /Share patch|패치 공유/i })).toBeDisabled();
  });
});
