import { useEffect, useMemo, useState } from "react";
import {
  chooseRepositoryDirectory,
  deleteWorkspace,
  getWorkspaces,
  normalizeAppError,
  runWorkspaceBatch,
  saveWorkspace,
  type WorkspaceBatchOperation,
  type WorkspaceBatchResultDto,
  type WorkspaceDto,
  type WorkspaceRepositoryDto,
} from "./repository";
import { t } from "./i18n";

type Props = {
  openRepositoryPaths: string[];
  onClose: () => void;
};

const emptyRepository = (): WorkspaceRepositoryDto => ({ path: "", cloneUrl: "" });

export function WorkspaceManager({ openRepositoryPaths, onClose }: Props) {
  const [workspaces, setWorkspaces] = useState<WorkspaceDto[]>([]);
  const [selectedId, setSelectedId] = useState<string>();
  const [name, setName] = useState("");
  const [repositories, setRepositories] = useState<WorkspaceRepositoryDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [results, setResults] = useState<WorkspaceBatchResultDto[]>([]);
  const [confirmOperation, setConfirmOperation] = useState<WorkspaceBatchOperation>();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const selected = useMemo(
    () => workspaces.find((workspace) => workspace.id === selectedId),
    [selectedId, workspaces],
  );

  useEffect(() => {
    getWorkspaces()
      .then(({ workspaces: items }) => {
        setWorkspaces(items);
        if (items[0]) selectWorkspace(items[0]);
        else startNew();
      })
      .catch((reason: unknown) => setMessage(normalizeAppError(reason).message))
      .finally(() => setLoading(false));
  }, []);

  function selectWorkspace(workspace: WorkspaceDto) {
    setSelectedId(workspace.id);
    setName(workspace.name);
    setRepositories(workspace.repositories.map((repository) => ({ ...repository })));
    setResults([]);
    setMessage("");
    setConfirmOperation(undefined);
    setConfirmDelete(false);
  }

  function startNew() {
    setSelectedId(undefined);
    setName("");
    setRepositories([]);
    setResults([]);
    setMessage("");
    setConfirmOperation(undefined);
    setConfirmDelete(false);
  }

  function updateRepository(index: number, patch: Partial<WorkspaceRepositoryDto>) {
    setRepositories((current) =>
      current.map((repository, currentIndex) =>
        currentIndex === index ? { ...repository, ...patch } : repository,
      ),
    );
  }

  function addOpenRepositories() {
    setRepositories((current) => {
      const seen = new Set(current.map((repository) => repository.path.toLocaleLowerCase()));
      return [
        ...current,
        ...openRepositoryPaths
          .filter((path) => !seen.has(path.toLocaleLowerCase()))
          .map((path) => ({ path, cloneUrl: "" })),
      ];
    });
  }

  async function browseRepository(index: number) {
    try {
      const path = await chooseRepositoryDirectory();
      if (path) updateRepository(index, { path });
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    }
  }

  async function persistWorkspace() {
    if (!name.trim()) return;
    try {
      setBusy(true);
      setMessage("");
      const saved = await saveWorkspace({
        id: selectedId,
        name: name.trim(),
        repositories,
      });
      setWorkspaces((current) =>
        [...current.filter((workspace) => workspace.id !== saved.id), saved].sort((a, b) =>
          a.name.localeCompare(b.name),
        ),
      );
      selectWorkspace(saved);
      setMessage(t("Workspace saved."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function removeWorkspace() {
    if (!selected) return;
    try {
      setBusy(true);
      await deleteWorkspace(selected.id);
      const remaining = workspaces.filter((workspace) => workspace.id !== selected.id);
      setWorkspaces(remaining);
      if (remaining[0]) selectWorkspace(remaining[0]);
      else startNew();
      setMessage(t("Workspace deleted."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
    }
  }

  async function executeBatch(operation: WorkspaceBatchOperation) {
    if (!selectedId) return;
    try {
      setBusy(true);
      setResults([]);
      setMessage(t("Running workspace {operation}…", { operation }));
      const response = await runWorkspaceBatch(selectedId, operation);
      setResults(response.results);
      const succeeded = response.results.filter((result) => result.state === "succeeded").length;
      const failed = response.results.filter((result) => result.state === "failed").length;
      setMessage(
        t("Workspace operation finished: {succeeded} succeeded, {failed} failed.", {
          succeeded,
          failed,
        }),
      );
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setBusy(false);
      setConfirmOperation(undefined);
    }
  }

  return (
    <div className="modal-overlay" role="presentation" onClick={onClose}>
      <section
        className="settings-modal workspace-manager-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-manager-title"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="settings-modal-header">
          <div>
            <h2 id="workspace-manager-title">{t("Repository workspaces")}</h2>
            <p>{t("Save repository groups and run clone, fetch, or fast-forward pull in one place.")}</p>
          </div>
          <button type="button" aria-label={t("Close")} onClick={onClose}>×</button>
        </header>
        <div className="workspace-manager-layout">
          <aside className="workspace-list-pane" aria-label={t("Workspaces")}>
            <div className="forge-pane-heading">
              <span>{t("Workspaces")}</span>
              <button type="button" aria-label={t("New workspace")} onClick={startNew}>＋</button>
            </div>
            {loading ? (
              <p className="forge-empty" role="status">{t("Loading workspaces…")}</p>
            ) : workspaces.length === 0 ? (
              <p className="forge-empty">{t("No workspaces saved.")}</p>
            ) : (
              <div className="workspace-list">
                {workspaces.map((workspace) => (
                  <button
                    key={workspace.id}
                    type="button"
                    className={workspace.id === selectedId ? "workspace-list-item active" : "workspace-list-item"}
                    onClick={() => selectWorkspace(workspace)}
                  >
                    <strong>{workspace.name}</strong>
                    <small>{t("{count} repositories", { count: workspace.repositories.length })}</small>
                  </button>
                ))}
              </div>
            )}
            <button className="control-button control-button--secondary" type="button" onClick={startNew}>
              ＋ {t("New workspace")}
            </button>
          </aside>
          <div className="workspace-editor">
            <label className="field">
              <span>{t("Workspace name")}</span>
              <input
                className="control-input"
                value={name}
                onChange={(event) => setName(event.currentTarget.value)}
                placeholder={t("e.g. Client application")}
                autoFocus={!selectedId}
              />
            </label>
            <div className="workspace-repository-heading">
              <div>
                <strong>{t("Repositories")}</strong>
                <small>{t("A clone URL is only required for paths that do not exist yet.")}</small>
              </div>
              <div>
                <button className="control-button control-button--secondary" type="button" onClick={addOpenRepositories} disabled={openRepositoryPaths.length === 0}>
                  {t("Add open repositories")}
                </button>
                <button className="control-button control-button--secondary" type="button" onClick={() => setRepositories((current) => [...current, emptyRepository()])}>
                  ＋ {t("Add repository")}
                </button>
              </div>
            </div>
            <div className="workspace-repositories">
              {repositories.length === 0 ? (
                <p className="workspace-empty">{t("Add an open repository or enter a local path.")}</p>
              ) : repositories.map((repository, index) => (
                <div className="workspace-repository-row" key={index}>
                  <label className="field">
                    <span>{t("Local path")}</span>
                    <div className="workspace-path-control">
                      <input
                        className="control-input"
                        value={repository.path}
                        onChange={(event) => updateRepository(index, { path: event.currentTarget.value })}
                      />
                      <button className="control-button control-button--secondary" type="button" onClick={() => void browseRepository(index)}>
                        {t("Browse…")}
                      </button>
                    </div>
                  </label>
                  <label className="field">
                    <span>{t("Clone URL (optional)")}</span>
                    <input
                      className="control-input"
                      value={repository.cloneUrl ?? ""}
                      onChange={(event) => updateRepository(index, { cloneUrl: event.currentTarget.value })}
                      placeholder="https://…"
                    />
                  </label>
                  <button
                    className="workspace-remove-repository"
                    type="button"
                    aria-label={t("Remove repository")}
                    onClick={() => setRepositories((current) => current.filter((_, currentIndex) => currentIndex !== index))}
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
            {message && <p className="workspace-message" role="status">{message}</p>}
            {results.length > 0 && (
              <div className="workspace-results" aria-label={t("Workspace operation results")}>
                {results.map((result) => (
                  <div className={`workspace-result workspace-result--${result.state}`} key={result.path}>
                    <span aria-hidden="true">{result.state === "succeeded" ? "✓" : result.state === "skipped" ? "–" : "!"}</span>
                    <div><strong>{result.path}</strong><small>{result.message}</small></div>
                  </div>
                ))}
              </div>
            )}
            {confirmDelete && selected && (
              <div className="workspace-confirm" role="alertdialog" aria-label={t("Delete workspace")}>
                <p>{t("Delete workspace {name}? Repository files will not be removed.", { name: selected.name })}</p>
                <button className="control-button control-button--secondary" type="button" onClick={() => setConfirmDelete(false)}>{t("Cancel")}</button>
                <button className="control-button control-button--danger" type="button" onClick={() => void removeWorkspace()}>{t("Delete workspace")}</button>
              </div>
            )}
            {confirmOperation && (
              <div className="workspace-confirm" role="alertdialog" aria-label={t("Confirm workspace operation")}>
                <p>{confirmOperation === "pull"
                  ? t("Pull every repository using fast-forward only? Local working trees will not be auto-stashed.")
                  : t("Clone every missing repository that has a clone URL?")}</p>
                <button className="control-button control-button--secondary" type="button" onClick={() => setConfirmOperation(undefined)}>{t("Cancel")}</button>
                <button className="control-button control-button--primary" type="button" onClick={() => void executeBatch(confirmOperation)}>{t("Continue")}</button>
              </div>
            )}
            <footer className="workspace-actions">
              <div>
                <button className="control-button control-button--secondary" type="button" disabled={!selectedId || busy} onClick={() => setConfirmOperation("clone")}>{t("Clone missing")}</button>
                <button className="control-button control-button--secondary" type="button" disabled={!selectedId || busy} onClick={() => void executeBatch("fetch")}>{t("Fetch all")}</button>
                <button className="control-button control-button--secondary" type="button" disabled={!selectedId || busy} onClick={() => setConfirmOperation("pull")}>{t("Pull all")}</button>
              </div>
              <div>
                {selectedId && (
                  <button className="control-button control-button--danger" type="button" disabled={busy} onClick={() => setConfirmDelete(true)}>{t("Delete")}</button>
                )}
                <button className="control-button control-button--primary" type="button" disabled={busy || !name.trim()} onClick={() => void persistWorkspace()}>
                  {busy ? t("Working…") : t("Save workspace")}
                </button>
              </div>
            </footer>
          </div>
        </div>
      </section>
    </div>
  );
}
