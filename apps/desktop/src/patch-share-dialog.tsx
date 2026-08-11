import { useState, type FormEvent } from "react";
import {
  applyComparePatch,
  deleteSharedPatch,
  fetchSharedPatch,
  getComparePatch,
  normalizeAppError,
  publishSharedPatch,
  validateComparePatch,
  type ComparePatchDto,
  type PatchShareReceiptDto,
  type RepositorySnapshotDto,
  type SharedPatchDto,
} from "./repository";
import { t } from "./i18n";

type Props = {
  repoId: string;
  repositoryName: string;
  revision: number;
  defaultBaseRevision: string;
  onClose: () => void;
  onSnapshot: (snapshot: RepositorySnapshotDto) => void;
};

type PendingAction = "publish" | "delete" | "apply";
const ENDPOINT_KEY = "git-acorn.patch-share.endpoint.v1";

export function PatchShareDialog({ repoId, repositoryName, revision, defaultBaseRevision, onClose, onSnapshot }: Props) {
  const [tab, setTab] = useState<"publish" | "import">("publish");
  const [endpoint, setEndpoint] = useState(() => localStorage.getItem(ENDPOINT_KEY) ?? "https://");
  const [token, setToken] = useState("");
  const [left, setLeft] = useState(defaultBaseRevision || "HEAD");
  const [right, setRight] = useState("WORKTREE");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [generated, setGenerated] = useState<ComparePatchDto>();
  const [receipt, setReceipt] = useState<PatchShareReceiptDto>();
  const [patchId, setPatchId] = useState("");
  const [imported, setImported] = useState<SharedPatchDto>();
  const [validation, setValidation] = useState<"valid" | "invalid">();
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<PendingAction>();

  const connection = { endpoint: endpoint.trim(), token: token || undefined };

  function switchTab(next: "publish" | "import") {
    setTab(next);
    setMessage("");
    setPending(undefined);
  }

  async function generate(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    setReceipt(undefined);
    try {
      const patch = await getComparePatch(repoId, left.trim(), right.trim());
      setGenerated(patch);
      setMessage(t("Shared patch preview generated."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function publish() {
    if (!generated) return;
    setBusy(true);
    setMessage("");
    try {
      localStorage.setItem(ENDPOINT_KEY, endpoint.trim());
      const next = await publishSharedPatch({
        ...connection,
        title: title.trim(),
        description: description.trim(),
        repository: repositoryName,
        baseRevision: left.trim(),
        patch: generated.patch,
      });
      setReceipt(next);
      setMessage(t("Patch shared."));
      setPending(undefined);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function removeSharedPatch() {
    if (!receipt) return;
    setBusy(true);
    setMessage("");
    try {
      await deleteSharedPatch(connection, receipt.patchId);
      setReceipt(undefined);
      setMessage(t("Shared patch deleted."));
      setPending(undefined);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function loadSharedPatch(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    setImported(undefined);
    setValidation(undefined);
    try {
      localStorage.setItem(ENDPOINT_KEY, endpoint.trim());
      const patch = await fetchSharedPatch(connection, patchId.trim());
      setImported(patch);
      const result = await validateComparePatch(repoId, patch.patch);
      setValidation(result.valid ? "valid" : "invalid");
      setMessage(result.valid ? t("Shared patch is valid for this repository.") : result.message ?? t("Shared patch does not apply cleanly."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function applyImportedPatch() {
    if (!imported || validation !== "valid") return;
    setBusy(true);
    setMessage("");
    try {
      const snapshot = await applyComparePatch(repoId, revision, imported.patch);
      onSnapshot(snapshot);
      setMessage(t("Shared patch applied to the index and working tree."));
      setPending(undefined);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  const preview = tab === "publish" ? generated?.patch : imported?.patch;
  const previewTitle = tab === "publish" ? title : imported?.title;

  return (
    <div className="modal-overlay" role="presentation" onClick={onClose}>
      <section className="settings-modal patch-share-modal" role="dialog" aria-modal="true" aria-labelledby="patch-share-title" onClick={(event) => event.stopPropagation()}>
        <header className="settings-modal-header">
          <div><span className="eyebrow">{t("Collaboration")}</span><h2 id="patch-share-title">{t("Shared patches")}</h2><p>{t("Publish and import integrity-checked patches through a compatible self-hosted service.")}</p></div>
          <button className="settings-close-btn" type="button" aria-label={t("Close shared patches")} onClick={onClose}>×</button>
        </header>
        <div className="patch-share-body">
          <div className="patch-share-connection">
            <label><span>{t("Service endpoint")}</span><input className="control-input" type="url" value={endpoint} onChange={(event) => setEndpoint(event.target.value)} placeholder="https://patch.example/" autoCapitalize="none" spellCheck={false} /></label>
            <label><span>{t("Bearer token (optional)")}</span><input className="control-input" type="password" value={token} onChange={(event) => setToken(event.target.value)} autoComplete="off" /></label>
            <p>{t("The endpoint is saved locally. The token stays only in this open dialog.")}</p>
          </div>
          <div className="patch-share-tabs" role="tablist" aria-label={t("Shared patch action")}>
            <button role="tab" aria-selected={tab === "publish"} className={tab === "publish" ? "active" : ""} type="button" onClick={() => switchTab("publish")}>{t("Publish")}</button>
            <button role="tab" aria-selected={tab === "import"} className={tab === "import" ? "active" : ""} type="button" onClick={() => switchTab("import")}>{t("Import")}</button>
          </div>
          {tab === "publish" ? (
            <form className="patch-share-form" onSubmit={generate}>
              <div className="patch-share-grid">
                <label><span>{t("Title")}</span><input className="control-input" value={title} onChange={(event) => setTitle(event.target.value)} maxLength={200} /></label>
                <label><span>{t("Base revision")}</span><input className="control-input" value={left} onChange={(event) => setLeft(event.target.value)} autoCapitalize="none" spellCheck={false} /></label>
                <label><span>{t("Compare revision")}</span><input className="control-input" value={right} onChange={(event) => setRight(event.target.value)} autoCapitalize="none" spellCheck={false} /></label>
                <label className="patch-share-wide"><span>{t("Description")}</span><textarea className="control-input" rows={2} value={description} onChange={(event) => setDescription(event.target.value)} maxLength={4000} /></label>
              </div>
              <div className="patch-share-actions">
                <button className="control-button control-button--secondary" type="submit" disabled={busy || !left.trim() || !right.trim()}>{busy ? t("Generating…") : t("Generate preview")}</button>
                <button className="control-button control-button--primary" type="button" disabled={busy || !generated || !title.trim() || !validEndpointInput(endpoint)} onClick={() => setPending("publish")}>{t("Share patch")}</button>
              </div>
              {receipt && <div className="patch-share-receipt" role="status"><div><strong>{t("Shared as {id}", { id: receipt.patchId })}</strong><code>{receipt.sha256}</code>{receipt.expiresAt && <span>{t("Expires {date}", { date: receipt.expiresAt })}</span>}</div><button className="control-button control-button--danger" type="button" onClick={() => setPending("delete")}>{t("Delete shared patch")}</button></div>}
            </form>
          ) : (
            <form className="patch-share-form" onSubmit={loadSharedPatch}>
              <div className="patch-share-grid patch-share-grid--import"><label><span>{t("Patch ID")}</span><input className="control-input" value={patchId} onChange={(event) => setPatchId(event.target.value)} autoCapitalize="none" spellCheck={false} /></label><button className="control-button control-button--secondary" type="submit" disabled={busy || !patchId.trim() || !validEndpointInput(endpoint)}>{busy ? t("Loading…") : t("Load and validate")}</button></div>
              {imported && <div className="patch-share-import-meta"><div><strong>{imported.title}</strong><span>{imported.repository} · {imported.baseRevision} · {imported.patchId}</span></div><span className={`forge-status-badge forge-status-badge--${validation === "valid" ? "success" : "failure"}`}>{validation === "valid" ? t("Applies cleanly") : t("Does not apply")}</span><button className="control-button control-button--danger" type="button" disabled={busy || validation !== "valid"} onClick={() => setPending("apply")}>{t("Apply shared patch")}</button></div>}
            </form>
          )}
          {message && <p className="patch-share-message" role="status">{message}</p>}
          {preview && <div className="compare-patch-preview patch-share-preview" role="region" aria-label={t("Shared patch preview")}><div className="compare-patch-header"><strong>{previewTitle || t("Shared patch preview")}</strong><span>{formatBytes(new TextEncoder().encode(preview).length)}</span></div><pre>{preview.slice(0, 12000)}</pre></div>}
        </div>
        {pending && <Confirmation action={pending} endpoint={endpoint.trim()} patchId={receipt?.patchId} patchBytes={generated ? new TextEncoder().encode(generated.patch).length : 0} busy={busy} onCancel={() => setPending(undefined)} onConfirm={() => void (pending === "publish" ? publish() : pending === "delete" ? removeSharedPatch() : applyImportedPatch())} />}
      </section>
    </div>
  );
}

function Confirmation({ action, endpoint, patchId, patchBytes, busy, onCancel, onConfirm }: { action: PendingAction; endpoint: string; patchId?: string; patchBytes: number; busy: boolean; onCancel: () => void; onConfirm: () => void }) {
  const publish = action === "publish";
  const remove = action === "delete";
  return <div className="patch-share-confirm" role="alertdialog" aria-modal="true" aria-labelledby="patch-share-confirm-title"><div className="patch-share-confirm-card"><div><h3 id="patch-share-confirm-title">{publish ? t("Publish this patch?") : remove ? t("Delete this shared patch?") : t("Apply this shared patch?")}</h3>{publish ? <><p>{t("This sends repository metadata and {size} of patch content to:", { size: formatBytes(patchBytes) })}</p><code>POST {endpoint.replace(/\/$/, "")}/v1/patches</code><p>{t("No Git command runs. Recovery: delete the published patch from the same service.")}</p></> : remove ? <><code>DELETE {endpoint.replace(/\/$/, "")}/v1/patches/{patchId}</code><p>{t("This removes the remote shared copy and does not change the local repository.")}</p></> : <><p>{t("This writes the verified patch to the index and working tree with:")}</p><code>git apply --index --recount --whitespace=error-all --</code><p>{t("Recovery: immediately reverse the same patch with git apply -R --index.")}</p></>}</div><div className="patch-share-confirm-actions"><button className="control-button control-button--secondary" type="button" disabled={busy} onClick={onCancel}>{t("Cancel")}</button><button className={`control-button ${publish ? "control-button--primary" : "control-button--danger"}`} type="button" disabled={busy} onClick={onConfirm}>{busy ? t("Working…") : publish ? t("Publish patch") : remove ? t("Delete shared patch") : t("Apply shared patch")}</button></div></div></div>;
}

function validEndpointInput(value: string) {
  try {
    const url = new URL(value);
    const validTransport = url.protocol === "https:" || (url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname));
    return validTransport && !url.username && !url.password && !url.search && !url.hash;
  } catch {
    return false;
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}
