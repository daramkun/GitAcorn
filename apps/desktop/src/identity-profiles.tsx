import { useEffect, useState } from "react";
import {
  applyIdentityProfile,
  chooseSshPrivateKey,
  deleteIdentityProfile,
  getIdentityProfiles,
  normalizeAppError,
  saveIdentityProfile,
  type IdentityProfileDto,
  type RepositoryGitIdentityDto,
} from "./repository";
import { t } from "./i18n";

type Props = {
  repoId?: string;
  onApplied?: (identity: RepositoryGitIdentityDto) => void;
};

export function IdentityProfiles({ repoId, onApplied }: Props) {
  const [profiles, setProfiles] = useState<IdentityProfileDto[]>([]);
  const [selectedId, setSelectedId] = useState<string>();
  const [appliedId, setAppliedId] = useState<string>();
  const [label, setLabel] = useState("");
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [sshKeyPath, setSshKeyPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmApply, setConfirmApply] = useState(false);

  useEffect(() => {
    let active = true;
    getIdentityProfiles(repoId)
      .then((response) => {
        if (!active) return;
        setProfiles(response.profiles);
        setAppliedId(response.selectedProfileId);
        const selected =
          response.profiles.find((profile) => profile.id === response.selectedProfileId) ??
          response.profiles[0];
        if (selected) selectProfile(selected);
        else startNew();
      })
      .catch((reason: unknown) => active && setMessage(normalizeAppError(reason).message));
    return () => {
      active = false;
    };
  }, [repoId]);

  function selectProfile(profile: IdentityProfileDto) {
    setSelectedId(profile.id);
    setLabel(profile.label);
    setName(profile.name);
    setEmail(profile.email);
    setSshKeyPath(profile.sshKeyPath ?? "");
    setMessage("");
    setConfirmDelete(false);
    setConfirmApply(false);
  }

  function startNew() {
    setSelectedId(undefined);
    setLabel("");
    setName("");
    setEmail("");
    setSshKeyPath("");
    setMessage("");
    setConfirmDelete(false);
    setConfirmApply(false);
  }

  async function save() {
    try {
      setBusy(true);
      setMessage("");
      const saved = await saveIdentityProfile({
        id: selectedId,
        label: label.trim(),
        name: name.trim(),
        email: email.trim(),
        sshKeyPath: sshKeyPath.trim() || undefined,
      });
      setProfiles((current) =>
        [...current.filter((profile) => profile.id !== saved.id), saved].sort((a, b) =>
          a.label.localeCompare(b.label),
        ),
      );
      selectProfile(saved);
      setMessage(t("Identity profile saved."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!selectedId) return;
    try {
      setBusy(true);
      await deleteIdentityProfile(selectedId);
      const remaining = profiles.filter((profile) => profile.id !== selectedId);
      setProfiles(remaining);
      if (remaining[0]) selectProfile(remaining[0]);
      else startNew();
      setMessage(t("Identity profile deleted."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!repoId || !selectedId) return;
    try {
      setBusy(true);
      setMessage("");
      const identity = await applyIdentityProfile(repoId, selectedId);
      setAppliedId(selectedId);
      setMessage(t("Identity profile applied to this repository."));
      onApplied?.(identity);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
      setConfirmApply(false);
    }
  }

  async function browseKey() {
    try {
      const path = await chooseSshPrivateKey();
      if (path) setSshKeyPath(path);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    }
  }

  const selected = profiles.find((profile) => profile.id === selectedId);

  return (
    <div className="identity-profile-manager">
      <div className="identity-profile-toolbar">
        <label>
          <span>{t("Saved profile")}</span>
          <select
            className="control-input"
            value={selectedId ?? ""}
            onChange={(event) => {
              const profile = profiles.find((item) => item.id === event.currentTarget.value);
              if (profile) selectProfile(profile);
              else startNew();
            }}
          >
            <option value="">{t("New identity profile")}</option>
            {profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.label}{profile.id === appliedId ? ` · ${t("Applied")}` : ""}
              </option>
            ))}
          </select>
        </label>
        <button className="control-button control-button--secondary" type="button" onClick={startNew}>
          ＋ {t("New profile")}
        </button>
      </div>
      <div className="identity-profile-grid">
        <label>
          <span>{t("Profile name")}</span>
          <input className="control-input" value={label} onChange={(event) => setLabel(event.currentTarget.value)} placeholder={t("e.g. Work account")} />
        </label>
        <label>
          <span>{t("Git user name")}</span>
          <input className="control-input" value={name} onChange={(event) => setName(event.currentTarget.value)} autoComplete="name" />
        </label>
        <label>
          <span>{t("Git email")}</span>
          <input className="control-input" type="email" value={email} onChange={(event) => setEmail(event.currentTarget.value)} autoComplete="email" />
        </label>
        <label className="identity-profile-key">
          <span>{t("SSH private key (optional)")}</span>
          <div>
            <input className="control-input" value={sshKeyPath} onChange={(event) => setSshKeyPath(event.currentTarget.value)} placeholder={t("Absolute path to a private key")} />
            <button className="control-button control-button--secondary" type="button" onClick={() => void browseKey()}>{t("Browse…")}</button>
          </div>
          <small>{t("Only the key path is stored. GitAcorn never reads or copies private key contents.")}</small>
        </label>
      </div>
      {message && <p className="identity-feedback" role="status">{message}</p>}
      {confirmDelete && selected && (
        <div className="identity-profile-confirm" role="alertdialog" aria-label={t("Delete identity profile")}>
          <p>{t("Delete identity profile {name}? Repository Git configuration will not be changed.", { name: selected.label })}</p>
          <button className="control-button control-button--secondary" type="button" onClick={() => setConfirmDelete(false)}>{t("Cancel")}</button>
          <button className="control-button control-button--danger" type="button" onClick={() => void remove()}>{t("Delete profile")}</button>
        </div>
      )}
      {confirmApply && selected && (
        <div className="identity-profile-confirm identity-profile-confirm--apply" role="alertdialog" aria-label={t("Apply identity profile")}>
          <div>
            <strong>{t("Apply {name} to this repository?", { name: selected.label })}</strong>
            <code>git config --local user.name &quot;{selected.name}&quot;</code>
            <code>git config --local user.email &quot;{selected.email}&quot;</code>
            <code>{selected.sshKeyPath
              ? `git config --local core.sshCommand 'ssh -i "…" -o IdentitiesOnly=yes'`
              : "git config --local --unset-all core.sshCommand"}</code>
            <small>{t("Recovery: edit or disable the repository overrides below.")}</small>
          </div>
          <button className="control-button control-button--secondary" type="button" onClick={() => setConfirmApply(false)}>{t("Cancel")}</button>
          <button className="control-button control-button--primary" type="button" onClick={() => void apply()}>{t("Apply profile")}</button>
        </div>
      )}
      <div className="identity-profile-actions">
        <div>
          {selectedId && (
            <button className="control-button control-button--danger" type="button" disabled={busy} onClick={() => setConfirmDelete(true)}>{t("Delete")}</button>
          )}
          {repoId && selectedId && (
            <button className="control-button control-button--secondary" type="button" disabled={busy} onClick={() => setConfirmApply(true)}>
              {selectedId === appliedId ? t("Reapply profile") : t("Apply to repository")}
            </button>
          )}
        </div>
        <button
          className="control-button control-button--primary"
          type="button"
          disabled={busy || !label.trim() || !name.trim() || !email.trim()}
          onClick={() => void save()}
        >
          {busy ? t("Working…") : t("Save profile")}
        </button>
      </div>
    </div>
  );
}
