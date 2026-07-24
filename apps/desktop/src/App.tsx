import { useEffect, useState } from "react";
import { getAppInfo, type AppInfoDto } from "./app-info";
import {
  chooseRepositoryDirectory,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  normalizeAppError,
  openRepository,
  type AppErrorDto,
  type FileChangeDto,
  type RepositorySnapshotDto,
} from "./repository";

type Page = "changes" | "history";
type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfoDto }
  | { status: "error"; message: string };
type RepositoryState =
  | { status: "idle" }
  | { status: "opening" }
  | { status: "ready"; snapshot: RepositorySnapshotDto; refreshing: boolean }
  | { status: "error"; error: AppErrorDto; snapshot?: RepositorySnapshotDto };

const navigation: ReadonlyArray<{ id: Page; label: string; shortcut: string }> = [
  { id: "changes", label: "Changes", shortcut: "⌘1" },
  { id: "history", label: "History", shortcut: "⌘2" },
];

export function App() {
  const [page, setPage] = useState<Page>("changes");
  const [appInfo, setAppInfo] = useState<AppInfoState>({ status: "loading" });
  const [repository, setRepository] = useState<RepositoryState>({ status: "idle" });
  const [selectedPath, setSelectedPath] = useState<string>();
  const activeSnapshot =
    repository.status === "ready" || repository.status === "error"
      ? repository.snapshot
      : undefined;
  const activeRepoId = activeSnapshot?.repository.id;

  useEffect(() => {
    let active = true;
    getAppInfo()
      .then((value) => active && setAppInfo({ status: "ready", value }))
      .catch((error: unknown) => {
        if (active) {
          const message = error instanceof Error ? error.message : String(error);
          setAppInfo({ status: "error", message });
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!activeRepoId) {
      return;
    }
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let unlisten: (() => void) | undefined;

    listenForRepositoryChanges((changedRepoId) => {
      if (changedRepoId !== activeRepoId) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        setRepository((current) =>
          current.status === "ready" ? { ...current, refreshing: true } : current,
        );
        getRepositorySnapshot(activeRepoId)
          .then((snapshot) => {
            if (!disposed) {
              setRepository({ status: "ready", snapshot, refreshing: false });
            }
          })
          .catch((error: unknown) => {
            if (!disposed) {
              setRepository((current) => ({
                status: "error",
                error: normalizeAppError(error),
                snapshot: current.status === "ready" ? current.snapshot : undefined,
              }));
            }
          });
      }, 250);
    }).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });

    return () => {
      disposed = true;
      clearTimeout(timer);
      unlisten?.();
    };
  }, [activeRepoId]);

  async function handleOpenRepository() {
    try {
      const path = await chooseRepositoryDirectory();
      if (!path) {
        return;
      }
      setRepository({ status: "opening" });
      setSelectedPath(undefined);
      const snapshot = await openRepository(path);
      setRepository({ status: "ready", snapshot, refreshing: false });
      setPage("changes");
    } catch (error: unknown) {
      setRepository({ status: "error", error: normalizeAppError(error) });
    }
  }

  const branchLabel = activeSnapshot
    ? activeSnapshot.head.kind === "branch"
      ? activeSnapshot.head.name
      : activeSnapshot.head.kind === "detached"
        ? `Detached ${activeSnapshot.head.oid?.slice(0, 8) ?? ""}`
        : "Unborn branch"
    : undefined;

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="brand">
          <span className="acorn-mark" aria-hidden="true"><span /></span>
          <span>GitAcorn</span>
          <span className="alpha-badge">ALPHA</span>
        </div>
        <div className="window-drag-region" />
      </header>

      <div className="tabbar" aria-label="Repository tabs">
        {activeSnapshot ? (
          <div className="repository-tab" aria-current="page">
            <span className="repository-dot" aria-hidden="true" />
            <strong>{activeSnapshot.repository.name}</strong>
            <span>{activeSnapshot.changes.length}</span>
          </div>
        ) : (
          <div className="tabbar-empty">No repositories open</div>
        )}
        <button
          className="open-button"
          type="button"
          disabled={repository.status === "opening"}
          onClick={handleOpenRepository}
        >
          <span aria-hidden="true">＋</span>
          {repository.status === "opening" ? " Opening…" : " Open repository"}
        </button>
      </div>

      <main className="workspace">
        <aside className="sidebar">
          <nav aria-label="Repository navigation">
            <p className="section-label">Workspace</p>
            {navigation.map((item) => (
              <button
                key={item.id}
                className={page === item.id ? "nav-item active" : "nav-item"}
                type="button"
                aria-current={page === item.id ? "page" : undefined}
                onClick={() => setPage(item.id)}
              >
                <span className={`nav-icon ${item.id}`} aria-hidden="true" />
                <span>{item.label}</span>
                <kbd>{item.shortcut}</kbd>
              </button>
            ))}
          </nav>

          <div className="sidebar-groups">
            <div className="sidebar-group"><span>›</span>Worktrees</div>
            <div className="sidebar-group">
              <span>›</span>Branches
              {branchLabel && <small>{branchLabel}</small>}
            </div>
            <div className="sidebar-group"><span>›</span>Tags</div>
            <div className="sidebar-group">
              <span>›</span>Stashes
              {activeSnapshot && <small>{activeSnapshot.stashCount}</small>}
            </div>
          </div>

          <div className="runtime-status" role="status">
            <span className={appInfo.status === "error" ? "status-dot error" : "status-dot"} />
            {appInfo.status === "loading" && "Connecting to core…"}
            {appInfo.status === "ready" && `${appInfo.value.runtime} · v${appInfo.value.version}`}
            {appInfo.status === "error" && "Core unavailable"}
          </div>
        </aside>

        <section className="content" aria-live="polite">
          <div className="contextbar">
            <div>
              <span className="eyebrow">
                {activeSnapshot?.repository.worktreePath ?? "Local workspace"}
              </span>
              <strong>
                {activeSnapshot
                  ? `${branchLabel} · ${page === "changes" ? "Changes" : "History"}`
                  : page === "changes"
                    ? "Changes"
                    : "History"}
              </strong>
            </div>
            <div className="remote-actions" aria-label="Remote actions">
              {repository.status === "ready" && repository.refreshing && (
                <span className="refreshing">Refreshing…</span>
              )}
              <button type="button" disabled>Fetch</button>
              <button type="button" disabled>Pull</button>
              <button type="button" disabled>Push</button>
            </div>
          </div>

          {appInfo.status === "error" && (
            <ErrorBanner title="Could not reach the GitAcorn core." message={appInfo.message} />
          )}
          {repository.status === "error" && (
            <ErrorBanner
              title="Could not open repository."
              message={repository.error.message}
              detail={repository.error.details}
              actionLabel="Choose another folder"
              onAction={handleOpenRepository}
            />
          )}

          {page === "changes" ? (
            activeSnapshot ? (
              <ChangesView
                snapshot={activeSnapshot}
                selectedPath={selectedPath}
                onSelect={setSelectedPath}
              />
            ) : (
              <ChangesEmpty onOpen={handleOpenRepository} opening={repository.status === "opening"} />
            )
          ) : (
            <HistoryEmpty hasRepository={Boolean(activeSnapshot)} />
          )}
        </section>
      </main>
    </div>
  );
}

function ErrorBanner({
  title,
  message,
  detail,
  actionLabel,
  onAction,
}: {
  title: string;
  message: string;
  detail?: string;
  actionLabel?: string;
  onAction?: () => void;
}) {
  return (
    <div className="error-banner" role="alert">
      <div>
        <strong>{title}</strong>
        <span>{message}</span>
        {detail && <small>{detail}</small>}
      </div>
      {actionLabel && <button type="button" onClick={onAction}>{actionLabel}</button>}
    </div>
  );
}

function ChangesView({
  snapshot,
  selectedPath,
  onSelect,
}: {
  snapshot: RepositorySnapshotDto;
  selectedPath?: string;
  onSelect: (path: string) => void;
}) {
  const unstaged = snapshot.changes.filter(
    (change) => change.worktreeStatus !== "." || change.conflict,
  );
  const staged = snapshot.changes.filter(
    (change) => change.indexStatus !== "." && change.indexStatus !== "?",
  );
  const selected = snapshot.changes.find((change) => change.path === selectedPath);

  return (
    <div className="changes-layout">
      <section className="file-panel" aria-label="Changed files">
        <ChangeSection title="Unstaged" changes={unstaged} selectedPath={selectedPath} onSelect={onSelect} />
        <ChangeSection title="Staged" changes={staged} selectedPath={selectedPath} onSelect={onSelect} />
      </section>
      <section className="selected-file-panel">
        {selected ? (
          <>
            <p className="eyebrow">Selected change</p>
            <h1>{selected.path}</h1>
            {selected.originalPath && <p>Renamed from {selected.originalPath}</p>}
            <div className="change-metadata">
              <span>Index <strong>{statusLabel(selected.indexStatus)}</strong></span>
              <span>Working tree <strong>{statusLabel(selected.worktreeStatus)}</strong></span>
              {selected.conflict && <span className="conflict-label">Conflict</span>}
            </div>
            <p className="diff-placeholder">Diff rendering arrives in M3.</p>
          </>
        ) : (
          <div className="empty-selection">
            <span className="file-glyph" aria-hidden="true" />
            <h1>{snapshot.changes.length === 0 ? "Working tree clean" : "Select a changed file"}</h1>
            <p>
              {snapshot.changes.length === 0
                ? "There are no staged or unstaged changes."
                : "Choose a file to inspect its Git status."}
            </p>
          </div>
        )}
      </section>
      <aside className="commit-panel" aria-label="Commit form preview">
        <div className="panel-heading"><h2>Commit</h2><span>{staged.length}</span></div>
        <textarea aria-label="Commit summary" placeholder="Summary" disabled />
        <textarea aria-label="Commit description" placeholder="Description (optional)" disabled />
        <button type="button" disabled>Commit to {snapshot.head.name ?? "HEAD"}</button>
      </aside>
    </div>
  );
}

function ChangeSection({
  title,
  changes,
  selectedPath,
  onSelect,
}: {
  title: string;
  changes: FileChangeDto[];
  selectedPath?: string;
  onSelect: (path: string) => void;
}) {
  return (
    <div className="change-section">
      <div className="panel-heading">
        <h2>{title}</h2>
        <span>{changes.length}</span>
      </div>
      {changes.length === 0 ? (
        <div className="panel-empty">No {title.toLowerCase()} changes.</div>
      ) : (
        <div className="change-list">
          {changes.map((change) => (
            <button
              className={selectedPath === change.path ? "change-row selected" : "change-row"}
              type="button"
              key={`${title}-${change.path}-${change.indexStatus}-${change.worktreeStatus}`}
              onClick={() => onSelect(change.path)}
              title={change.path}
            >
              <span className={`status-badge ${change.conflict ? "conflict" : ""}`}>
                {change.conflict
                  ? "!"
                  : title === "Staged"
                    ? change.indexStatus
                    : change.worktreeStatus}
              </span>
              <span className="change-path">{change.path}</span>
              {change.submodule && <small>submodule</small>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function statusLabel(code: string) {
  return (
    {
      ".": "Unchanged",
      "?": "Untracked",
      M: "Modified",
      A: "Added",
      D: "Deleted",
      R: "Renamed",
      C: "Copied",
      T: "Type changed",
      U: "Unmerged",
    }[code] ?? code
  );
}

function ChangesEmpty({ onOpen, opening }: { onOpen: () => void; opening: boolean }) {
  return (
    <div className="changes-layout">
      <section className="file-panel" aria-label="Changed files">
        <ChangeSection title="Unstaged" changes={[]} onSelect={() => undefined} />
        <ChangeSection title="Staged" changes={[]} onSelect={() => undefined} />
      </section>
      <section className="welcome-panel">
        <div className="welcome-art" aria-hidden="true">
          <span className="branch-line" />
          <span className="branch-node node-one" />
          <span className="branch-node node-two" />
          <span className="branch-node node-three" />
        </div>
        <p className="eyebrow">A calmer Git workflow</p>
        <h1>Your repositories, clearly in view.</h1>
        <p>Open a local repository to inspect its real staged, unstaged, and untracked changes.</p>
        <button type="button" onClick={onOpen} disabled={opening}>
          {opening ? "Opening…" : "Open a repository"}
        </button>
        <small>Git 2.40.0 or newer is required.</small>
      </section>
      <aside className="commit-panel" aria-label="Commit form preview">
        <div className="panel-heading"><h2>Commit</h2></div>
        <textarea aria-label="Commit summary" placeholder="Summary" disabled />
        <textarea aria-label="Commit description" placeholder="Description (optional)" disabled />
        <button type="button" disabled>Commit</button>
      </aside>
    </div>
  );
}

function HistoryEmpty({ hasRepository }: { hasRepository: boolean }) {
  return (
    <div className="history-empty">
      <div className="history-lines" aria-hidden="true"><i /><i /><i /></div>
      <p className="eyebrow">Commit graph</p>
      <h1>History will appear here.</h1>
      <p>
        {hasRepository
          ? "Commit history is planned for M4."
          : "Open a repository to explore commits, branches, tags, and authors."}
      </p>
    </div>
  );
}
