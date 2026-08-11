import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GitFlowSettings, buildGitFlowCommands } from "./git-flow-settings";
import {
  getGitFlowSettings,
  type GitFlowSettingsDto,
} from "./repository";

vi.mock("./repository", () => ({
  getGitFlowSettings: vi.fn(),
}));

const mockedGetGitFlowSettings = vi.mocked(getGitFlowSettings);

const settings: GitFlowSettingsDto = {
  schemaVersion: 1,
  mainBranch: "main",
  developBranch: "develop",
  featurePrefix: "feature/",
  releasePrefix: "release/",
  hotfixPrefix: "hotfix/",
  supportPrefix: "support/",
  versionTagPrefix: "v",
  mainExists: true,
  developExists: false,
  configured: false,
};

describe("GitFlowSettings", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    mockedGetGitFlowSettings.mockResolvedValue(settings);
  });

  it("builds repository-local config and develop creation commands", () => {
    expect(buildGitFlowCommands(settings, true, false)).toEqual([
      'git config --local --replace-all gitflow.branch.master "main"',
      'git config --local --replace-all gitflow.branch.develop "develop"',
      'git config --local --replace-all gitflow.prefix.feature "feature/"',
      'git config --local --replace-all gitflow.prefix.release "release/"',
      'git config --local --replace-all gitflow.prefix.hotfix "hotfix/"',
      'git config --local --replace-all gitflow.prefix.support "support/"',
      'git config --local --replace-all gitflow.prefix.versiontag "v"',
      'git branch "develop" "main"',
    ]);
  });

  it("shows the exact initialization plan before configuring", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const onConfigure = vi.fn().mockResolvedValue(undefined);
    render(
      <GitFlowSettings
        repoId="repository"
        revision={4}
        onConfigure={onConfigure}
      />,
    );

    await screen.findByText("Preset not configured");
    fireEvent.click(screen.getByRole("button", { name: "Initialize Git-flow" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(confirm.mock.calls[0][0]).toContain(
      'git config --local --replace-all gitflow.prefix.feature "feature/"',
    );
    expect(confirm.mock.calls[0][0]).toContain('git branch "develop" "main"');
    await waitFor(() =>
      expect(onConfigure).toHaveBeenCalledWith(
        expect.objectContaining({
          mainBranch: "main",
          developBranch: "develop",
          featurePrefix: "feature/",
        }),
        true,
      ),
    );
  });
});
