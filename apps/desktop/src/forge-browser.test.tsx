import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ForgeBrowser } from "./forge-browser";
import {
  checkoutForgePullRequest,
  connectForgeAccount,
  createForgePullRequest,
  disconnectForgeAccount,
  getForgeAccounts,
  getForgePullRequests,
  getForgeRepositories,
  mergeForgePullRequest,
} from "./repository";

vi.mock("./repository", () => ({
  checkoutForgePullRequest: vi.fn(),
  connectForgeAccount: vi.fn(),
  createForgePullRequest: vi.fn(),
  disconnectForgeAccount: vi.fn(),
  getForgeAccounts: vi.fn(),
  getForgePullRequests: vi.fn(),
  getForgeRepositories: vi.fn(),
  mergeForgePullRequest: vi.fn(),
  normalizeAppError: (reason: unknown) => ({
    schemaVersion: 1,
    code: "test",
    message: reason instanceof Error ? reason.message : String(reason),
    recoveryActions: [],
  }),
}));

const account = {
  id: "github-account",
  provider: "github" as const,
  host: "github.com",
  login: "acorn",
  displayName: "Acorn User",
};

const repository = {
  id: "repository-one",
  name: "demo",
  fullName: "acorn/demo",
  cloneUrl: "https://github.com/acorn/demo.git",
  webUrl: "https://github.com/acorn/demo",
  private: true,
  archived: false,
  updatedAt: "2026-08-09T00:00:00Z",
};

const pullRequest = {
  id: "pr-one",
  number: 12,
  title: "Improve hosted repository browser",
  author: "acorn",
  sourceBranch: "feature/forge-pr",
  targetBranch: "main",
  sourceOid: "0123456789abcdef0123456789abcdef01234567",
  sourceCloneUrl: repository.cloneUrl,
  webUrl: `${repository.webUrl}/pull/12`,
  state: "open",
  draft: false,
  mergeability: "mergeable" as const,
  reviewStatus: "approved" as const,
  ciStatus: "success" as const,
};
const mockedGetAccounts = vi.mocked(getForgeAccounts);
const mockedConnect = vi.mocked(connectForgeAccount);
const mockedCreatePullRequest = vi.mocked(createForgePullRequest);
const mockedMergePullRequest = vi.mocked(mergeForgePullRequest);
const mockedCheckoutPullRequest = vi.mocked(checkoutForgePullRequest);
const mockedDisconnect = vi.mocked(disconnectForgeAccount);
const mockedGetRepositories = vi.mocked(getForgeRepositories);
const mockedGetPullRequests = vi.mocked(getForgePullRequests);

describe("ForgeBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetAccounts.mockResolvedValue({ schemaVersion: 1, accounts: [] });
    mockedConnect.mockResolvedValue(account);
    mockedDisconnect.mockResolvedValue();
    mockedGetRepositories.mockResolvedValue({ schemaVersion: 1, repositories: [repository] });
    mockedGetPullRequests.mockResolvedValue({ schemaVersion: 1, pullRequests: [pullRequest] });
    mockedCreatePullRequest.mockResolvedValue(pullRequest);
    mockedMergePullRequest.mockResolvedValue();
  });

  it("shows provider-specific fields and connects without persisting the token in UI state", async () => {
    render(<ForgeBrowser onClose={vi.fn()} onClone={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: /Connect hosting account|호스팅 계정 연결/i })).toBeVisible();
    fireEvent.change(screen.getByLabelText(/Provider|서비스/i), { target: { value: "bitbucket" } });
    expect(screen.getByLabelText(/Workspace|작업 공간/i)).toBeVisible();

    fireEvent.change(screen.getByLabelText(/Authentication username|인증 사용자명/i), { target: { value: "acorn" } });
    fireEvent.change(screen.getByLabelText(/Access token|액세스 토큰/i), { target: { value: "secret-token" } });
    fireEvent.change(screen.getByLabelText(/Workspace|작업 공간/i), { target: { value: "acorn-space" } });
    fireEvent.click(screen.getByRole("button", { name: /Connect account|계정 연결/i }));

    await waitFor(() => expect(mockedConnect).toHaveBeenCalledWith({
      provider: "bitbucket",
      host: "bitbucket.org",
      authUsername: "acorn",
      token: "secret-token",
      scope: "acorn-space",
    }));
    await waitFor(() => expect(screen.queryByDisplayValue("secret-token")).not.toBeInTheDocument());
  });

  it("filters repositories and hands the HTTPS clone URL to the existing clone flow", async () => {
    mockedGetAccounts.mockResolvedValue({ schemaVersion: 1, accounts: [account] });
    const onClone = vi.fn();
    render(<ForgeBrowser onClose={vi.fn()} onClone={onClone} />);

    expect(await screen.findByText("acorn/demo")).toBeVisible();
    fireEvent.change(screen.getByRole("textbox", { name: /Search hosted repositories|호스팅 저장소 검색/i }), { target: { value: "missing" } });
    expect(screen.getByText(/No repositories match|검색과 일치하는 저장소가 없습니다/i)).toBeVisible();
    fireEvent.change(screen.getByRole("textbox", { name: /Search hosted repositories|호스팅 저장소 검색/i }), { target: { value: "demo" } });
    fireEvent.click(screen.getByRole("button", { name: /^Clone$|^복제$/i }));
    expect(onClone).toHaveBeenCalledWith(repository.cloneUrl);
  });

  it("requires confirmation before deleting account metadata and credentials", async () => {
    mockedGetAccounts.mockResolvedValue({ schemaVersion: 1, accounts: [account] });
    render(<ForgeBrowser onClose={vi.fn()} onClone={vi.fn()} />);

    await screen.findByText("acorn/demo");
    fireEvent.click(screen.getByRole("button", { name: /^Disconnect$|^연결 해제$/i }));
    expect(screen.getByRole("alertdialog")).toBeVisible();
    expect(mockedDisconnect).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Disconnect account|계정 연결 해제/i }));
    await waitFor(() => expect(mockedDisconnect).toHaveBeenCalledWith(account.id));
  });

  it("shows review and CI state and confirms before merging", async () => {
    mockedGetAccounts.mockResolvedValue({ schemaVersion: 1, accounts: [account] });
    render(<ForgeBrowser onClose={vi.fn()} onClone={vi.fn()} />);

    await screen.findByText("acorn/demo");
    fireEvent.click(screen.getByRole("button", { name: /Pull requests|풀 리퀘스트/i }));
    expect(await screen.findByText(/Improve hosted repository browser/)).toBeVisible();
    expect(screen.getByText(/Review: Approved|리뷰.*승인/i)).toBeVisible();
    expect(screen.getByText(/CI: Passed|CI.*통과/i)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /^Merge$|^병합$/i }));
    expect(screen.getByRole("alertdialog", { name: /Merge pull request #12|풀 리퀘스트 #12.*병합/i })).toBeVisible();
    expect(mockedMergePullRequest).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Merge pull request|풀 리퀘스트 병합/i }));
    await waitFor(() => expect(mockedMergePullRequest).toHaveBeenCalledWith(account.id, repository.id, 12, {
      expectedSourceOid: pullRequest.sourceOid,
      squash: false,
      deleteSourceBranch: false,
    }));
  });

  it("checks out a pull request into the active local repository", async () => {
    mockedGetAccounts.mockResolvedValue({ schemaVersion: 1, accounts: [account] });
    const snapshot = { repository: { id: "local" }, revision: 8 } as never;
    mockedCheckoutPullRequest.mockResolvedValue(snapshot);
    const onSnapshot = vi.fn();
    render(<ForgeBrowser onClose={vi.fn()} onClone={vi.fn()} activeRepoId="local" activeRevision={7} onSnapshot={onSnapshot} />);

    await screen.findByText("acorn/demo");
    fireEvent.click(screen.getByRole("button", { name: /Pull requests|풀 리퀘스트/i }));
    await screen.findByText(/Improve hosted repository browser/);
    fireEvent.click(screen.getByRole("button", { name: /Checkout|체크아웃/i }));
    await waitFor(() => expect(mockedCheckoutPullRequest).toHaveBeenCalledWith(account.id, repository.id, 12, "local", 7, pullRequest.sourceOid));
    expect(onSnapshot).toHaveBeenCalledWith(snapshot);
  });});