import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ForgeDashboard } from "./forge-dashboard";
import { getForgeDashboard } from "./repository";

vi.mock("./repository", () => ({
  getForgeDashboard: vi.fn(),
  normalizeAppError: (reason: unknown) => ({ message: reason instanceof Error ? reason.message : String(reason) }),
}));

const mockedDashboard = vi.mocked(getForgeDashboard);

const dashboard = {
  schemaVersion: 1 as const,
  coveredRepositories: 3,
  skippedRepositories: 2,
  failures: [{ accountId: "account", repositoryName: "team/offline", message: "Offline" }],
  items: [
    {
      id: "pr-failed",
      kind: "pullRequest" as const,
      provider: "github" as const,
      accountId: "account",
      accountLogin: "ada",
      repositoryId: "repo",
      repositoryName: "team/demo",
      number: 12,
      title: "Fix failing Windows checks",
      author: "ada",
      webUrl: "https://github.com/team/demo/pull/12",
      state: "open",
      personal: true,
      attention: "ciFailed" as const,
      reviewStatus: "approved" as const,
      ciStatus: "failure" as const,
      updatedAt: "2026-08-10T00:00:00Z",
    },
    {
      id: "issue-assigned",
      kind: "issue" as const,
      provider: "gitlab" as const,
      accountId: "account",
      accountLogin: "ada",
      repositoryId: "repo-two",
      repositoryName: "team/service",
      number: 7,
      title: "Investigate cache regression",
      author: "grace",
      webUrl: "https://gitlab.example/team/service/-/issues/7",
      state: "opened",
      personal: true,
      attention: "assignedIssue" as const,
      updatedAt: "2026-08-09T00:00:00Z",
    },
    {
      id: "team-pr",
      kind: "pullRequest" as const,
      provider: "bitbucket" as const,
      accountId: "account",
      accountLogin: "ada",
      repositoryId: "repo-three",
      repositoryName: "team/web",
      number: 4,
      title: "Refresh navigation",
      author: "linus",
      webUrl: "https://bitbucket.org/team/web/pull-requests/4",
      state: "OPEN",
      personal: false,
      reviewStatus: "approved" as const,
      ciStatus: "success" as const,
      updatedAt: "2026-08-08T00:00:00Z",
    },
  ],
};

describe("ForgeDashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockedDashboard.mockResolvedValue(dashboard);
  });

  it("aggregates pull requests, issues, CI attention, and partial refresh failures", async () => {
    render(<ForgeDashboard onClose={vi.fn()} />);

    expect(await screen.findByText(/Fix failing Windows checks/)).toBeVisible();
    expect(screen.getByText(/Investigate cache regression/)).toBeVisible();
    expect(screen.queryByText(/Refresh navigation/)).not.toBeInTheDocument();
    expect(screen.getByText(/recent repositories|최근 저장소/i)).toHaveTextContent("3");

    fireEvent.click(screen.getByRole("tab", { name: /^Team$|^팀$/i }));
    expect(screen.getByText(/Refresh navigation/)).toBeVisible();
    expect(screen.queryByText(/Fix failing Windows checks/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByText(/sources could not be refreshed|새로 고치지 못했습니다/i));
    expect(screen.getByText("team/offline")).toBeVisible();
    expect(screen.getByText("Offline")).toBeVisible();
  });

  it("persists notification read state and reports the unread count", async () => {
    const onUnreadChange = vi.fn();
    render(<ForgeDashboard onClose={vi.fn()} onUnreadChange={onUnreadChange} />);
    await screen.findByText(/Fix failing Windows checks/);

    await waitFor(() => expect(onUnreadChange).toHaveBeenLastCalledWith(2));
    fireEvent.click(screen.getByRole("button", { name: /Mark all read|모두 읽음/i }));
    await waitFor(() => expect(onUnreadChange).toHaveBeenLastCalledWith(0));
    expect(JSON.parse(localStorage.getItem("git-acorn.forge-dashboard.read.v1") ?? "[]")).toEqual(expect.arrayContaining(["pr-failed", "issue-assigned"]));
  });

  it("does not expose a non-HTTPS provider URL as a link", async () => {
    mockedDashboard.mockResolvedValue({
      ...dashboard,
      items: [{ ...dashboard.items[0], webUrl: "javascript:alert(1)" }],
    });
    render(<ForgeDashboard onClose={vi.fn()} />);

    await screen.findByText(/Fix failing Windows checks/);
    expect(screen.getByRole("button", { name: /^Open$|^열기$/i })).toBeDisabled();
    expect(screen.queryByRole("link", { name: /^Open$|^열기$/i })).not.toBeInTheDocument();
  });
});
