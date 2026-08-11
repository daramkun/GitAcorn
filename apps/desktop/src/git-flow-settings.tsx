import { useEffect, useState } from "react";
import {
  getGitFlowSettings,
  normalizeAppError,
  type GitFlowSettingsDto,
  type GitFlowSettingsRequest,
} from "./repository";
import { t } from "./i18n";

type Props = {
  repoId: string;
  revision: number;
  onConfigure: (
    settings: GitFlowSettingsRequest,
    initializeDevelop: boolean,
  ) => Promise<void>;
};

function asRequest(settings: GitFlowSettingsDto): GitFlowSettingsRequest {
  return {
    mainBranch: settings.mainBranch,
    developBranch: settings.developBranch,
    featurePrefix: settings.featurePrefix,
    releasePrefix: settings.releasePrefix,
    hotfixPrefix: settings.hotfixPrefix,
    supportPrefix: settings.supportPrefix,
    versionTagPrefix: settings.versionTagPrefix,
  };
}

function quote(value: string) {
  return '"' + value.replaceAll("\\", "\\\\").replaceAll('"', '\\"') + '"';
}

export function buildGitFlowCommands(
  settings: GitFlowSettingsRequest,
  initializeDevelop: boolean,
  developExists: boolean,
) {
  const pairs: Array<[string, string]> = [
    ["gitflow.branch.master", settings.mainBranch],
    ["gitflow.branch.develop", settings.developBranch],
    ["gitflow.prefix.feature", settings.featurePrefix],
    ["gitflow.prefix.release", settings.releasePrefix],
    ["gitflow.prefix.hotfix", settings.hotfixPrefix],
    ["gitflow.prefix.support", settings.supportPrefix],
    ["gitflow.prefix.versiontag", settings.versionTagPrefix],
  ];
  const commands = pairs.map(([key, value]) =>
    ["git config --local --replace-all", key, quote(value.trim())].join(" "),
  );
  if (initializeDevelop && !developExists) {
    commands.push(
      ["git branch", quote(settings.developBranch.trim()), quote(settings.mainBranch.trim())].join(" "),
    );
  }
  return commands;
}

export function GitFlowSettings({ repoId, revision, onConfigure }: Props) {
  const [loaded, setLoaded] = useState<GitFlowSettingsDto>();
  const [draft, setDraft] = useState<GitFlowSettingsRequest>();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();

  const load = () =>
    getGitFlowSettings(repoId)
      .then((settings) => {
        setLoaded(settings);
        setDraft(asRequest(settings));
      })
      .catch((reason: unknown) => setMessage(normalizeAppError(reason).message));

  useEffect(() => {
    void load();
  }, [repoId, revision]);

  const update = (patch: Partial<GitFlowSettingsRequest>) =>
    setDraft((current) => current && { ...current, ...patch });

  const configure = async (initializeDevelop: boolean) => {
    if (!draft || !loaded) return;
    const commands = buildGitFlowCommands(draft, initializeDevelop, loaded.developExists);
    if (
      !window.confirm(
        [
          initializeDevelop
            ? t("Initialize Git-flow with these repository-local commands?")
            : t("Save this branch naming preset with these repository-local commands?"),
          "",
          ...commands,
          "",
          t("This does not check out another branch or change working tree files."),
          t("Recovery: edit or remove the gitflow.* keys in repository Git config."),
        ].join("\n"),
      )
    ) return;

    setBusy(true);
    setMessage(undefined);
    try {
      await onConfigure(draft, initializeDevelop);
      await load();
      setMessage(
        initializeDevelop
          ? t("Git-flow is configured for this repository.")
          : t("Branch naming preset saved."),
      );
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  };

  if (!draft || !loaded) {
    return <p className="identity-state">{message ?? t("Loading Git-flow settings…")}</p>;
  }

  const fields: Array<[keyof GitFlowSettingsRequest, string]> = [
    ["mainBranch", t("Main branch")],
    ["developBranch", t("Develop branch")],
    ["featurePrefix", t("Feature prefix")],
    ["releasePrefix", t("Release prefix")],
    ["hotfixPrefix", t("Hotfix prefix")],
    ["supportPrefix", t("Support prefix")],
    ["versionTagPrefix", t("Version tag prefix")],
  ];

  return (
    <div className="git-flow-settings">
      <div className="git-flow-status">
        <span className={loaded.configured ? "git-flow-state git-flow-state--configured" : "git-flow-state"}>
          {loaded.configured ? t("Preset configured") : t("Preset not configured")}
        </span>
        <small>
          {loaded.mainExists ? t("Main branch exists") : t("Main branch is missing")}
          {" · "}
          {loaded.developExists ? t("Develop branch exists") : t("Develop branch will be created")}
        </small>
      </div>
      <div className="git-flow-grid">
        {fields.map(([key, label]) => (
          <label key={key}>
            <span>{label}</span>
            <input
              className="control-input"
              value={draft[key]}
              disabled={busy}
              onChange={(event) => update({ [key]: event.target.value })}
            />
          </label>
        ))}
      </div>
      <p className="settings-note">
        {t("Feature and release branches start from develop; hotfix and support branches start from main.")}
      </p>
      <div className="remote-form-actions">
        <button className="control-button" type="button" disabled={busy} onClick={() => void configure(false)}>
          {busy ? t("Saving…") : t("Save naming preset")}
        </button>
        <button className="control-button control-button--primary" type="button" disabled={busy} onClick={() => void configure(true)}>
          {busy ? t("Saving…") : t("Initialize Git-flow")}
        </button>
      </div>
      {message && <p className="identity-feedback" role="status">{message}</p>}
    </div>
  );
}
