import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IdentityProfiles } from "./identity-profiles";
import {
  applyIdentityProfile,
  chooseSshPrivateKey,
  deleteIdentityProfile,
  getIdentityProfiles,
  saveIdentityProfile,
} from "./repository";

vi.mock("./repository", () => ({
  applyIdentityProfile: vi.fn(),
  chooseSshPrivateKey: vi.fn(),
  deleteIdentityProfile: vi.fn(),
  getIdentityProfiles: vi.fn(),
  saveIdentityProfile: vi.fn(),
  normalizeAppError: (reason: unknown) => ({
    schemaVersion: 1,
    code: "test",
    message: reason instanceof Error ? reason.message : String(reason),
    recoveryActions: [],
  }),
}));

const profile = {
  id: "work",
  label: "Work",
  name: "Work Author",
  email: "work@example.invalid",
  sshKeyPath: "C:/Users/test/.ssh/id_ed25519",
};
const identity = {
  repoId: "repository",
  repositoryName: "demo",
  local: { name: profile.name, email: profile.email },
  effective: { name: profile.name, email: profile.email },
};

const mockedApply = vi.mocked(applyIdentityProfile);
const mockedChooseKey = vi.mocked(chooseSshPrivateKey);
const mockedDelete = vi.mocked(deleteIdentityProfile);
const mockedGet = vi.mocked(getIdentityProfiles);
const mockedSave = vi.mocked(saveIdentityProfile);

describe("IdentityProfiles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGet.mockResolvedValue({ schemaVersion: 1, profiles: [profile] });
    mockedApply.mockResolvedValue(identity);
    mockedChooseKey.mockResolvedValue("C:/keys/personal");
    mockedDelete.mockResolvedValue();
    mockedSave.mockResolvedValue(profile);
  });

  it("saves reusable author and SSH key path metadata", async () => {
    render(<IdentityProfiles />);

    expect(await screen.findByLabelText(/Profile name|프로필 이름/i)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Browse|찾아보기/i }));
    expect(await screen.findByDisplayValue("C:/keys/personal")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Save profile|프로필 저장/i }));

    await waitFor(() =>
      expect(mockedSave).toHaveBeenCalledWith({
        id: profile.id,
        label: profile.label,
        name: profile.name,
        email: profile.email,
        sshKeyPath: "C:/keys/personal",
      }),
    );
  });

  it("shows exact local config impact before applying to a repository", async () => {
    const onApplied = vi.fn();
    render(<IdentityProfiles repoId="repository" onApplied={onApplied} />);

    await screen.findByLabelText(/Profile name|프로필 이름/i);
    fireEvent.click(screen.getByRole("button", { name: /Apply to repository|저장소에 적용/i }));
    const dialog = screen.getByRole("alertdialog", { name: /Apply identity profile|Identity 프로필 적용/i });
    expect(within(dialog).getByText(/git config --local user.name/)).toBeVisible();
    expect(within(dialog).getByText(/IdentitiesOnly=yes/)).toBeVisible();
    expect(mockedApply).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: /Apply profile|프로필 적용/i }));
    await waitFor(() => expect(mockedApply).toHaveBeenCalledWith("repository", profile.id));
    expect(onApplied).toHaveBeenCalledWith(identity);
  });

  it("requires confirmation and leaves repository config unchanged when deleting metadata", async () => {
    render(<IdentityProfiles repoId="repository" />);

    await screen.findByLabelText(/Profile name|프로필 이름/i);
    fireEvent.click(screen.getByRole("button", { name: /^Delete$|^삭제$/i }));
    const dialog = screen.getByRole("alertdialog", { name: /Delete identity profile|Identity 프로필 삭제/i });
    expect(dialog).toHaveTextContent(/Git configuration will not be changed|Git 설정은 바뀌지 않습니다/i);
    expect(mockedDelete).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: /Delete profile|프로필 삭제/i }));
    await waitFor(() => expect(mockedDelete).toHaveBeenCalledWith(profile.id));
  });
});
