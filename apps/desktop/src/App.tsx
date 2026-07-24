import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { getAppInfo, type AppInfoDto } from "./app-info";
import {
  activateSessionTab,
  activateWorktree,
  chooseRepositoryDirectory,
  closeSessionTab,
  getRepositorySidebar,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  normalizeAppError,
  openRepository,
  reorderSessionTabs,
  restoreSession,
  updateSessionTab,
  type AppErrorDto,
  type FileChangeDto,
  type RepositorySnapshotDto,
  type RepositorySidebarDto,
  type SessionTabDto,
} from "./repository";

type Page = "changes" | "history";
type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfoDto }
  | { status: "error"; message: string };

const navigation: ReadonlyArray<{ id: Page; label: string; shortcut: string }> = [
  { id: "changes", label: "Changes", shortcut: "⌘1" },
  { id: "history", label: "History", shortcut: "⌘2" },
];

export function App() {
  const [appInfo, setAppInfo] = useState<AppInfoState>({ status: "loading" });
  const [tabs, setTabs] = useState<SessionTabDto[]>([]);
  const [sessionLoading, setSessionLoading] = useState(true);
  const [opening, setOpening] = useState(false);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [sidebars, setSidebars] = useState<Record<string, RepositorySidebarDto>>({});
  const [error, setError] = useState<AppErrorDto>();
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const activeTab = tabs.find((tab) => tab.active) ?? tabs[0];
  const activeSnapshot = activeTab?.snapshot;
  const page = activeTab?.page ?? "changes";
  const activeSidebar = activeTab ? sidebars[activeTab.repoId] : undefined;

  useEffect(() => {
    let active = true;
    getAppInfo()
      .then((value) => active && setAppInfo({ status: "ready", value }))
      .catch((reason: unknown) => {
        if (active) {
          setAppInfo({
            status: "error",
            message: reason instanceof Error ? reason.message : String(reason),
          });
        }
      });
    restoreSession()
      .then((session) => active && setTabs(session.tabs))
      .catch((reason: unknown) => active && setError(normalizeAppError(reason)))
      .finally(() => active && setSessionLoading(false));
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!activeTab?.snapshot || sidebars[activeTab.repoId]) return;
    getRepositorySidebar(activeTab.repoId)
      .then((sidebar) =>
        setSidebars((current) => ({ ...current, [activeTab.repoId]: sidebar })),
      )
      .catch((reason: unknown) => setError(normalizeAppError(reason)));
  }, [activeTab, sidebars]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listenForRepositoryChanges((repoId) => {
      clearTimeout(timers.current.get(repoId));
      timers.current.set(
        repoId,
        setTimeout(() => {
          setRefreshing((current) => new Set(current).add(repoId));
          getRepositorySnapshot(repoId)
            .then((snapshot) => {
              if (!disposed) {
                setTabs((current) =>
                  current.map((tab) =>
                    tab.repoId === repoId ? { ...tab, snapshot, unavailable: false } : tab,
                  ),
                );
              }
            })
            .catch((reason: unknown) => !disposed && setError(normalizeAppError(reason)))
            .finally(() => {
              if (!disposed) {
                setRefreshing((current) => {
                  const next = new Set(current);
                  next.delete(repoId);
                  return next;
                });
              }
            });
        }, 250),
      );
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      for (const timer of timers.current.values()) clearTimeout(timer);
      unlisten?.();
    };
  }, []);

  async function handleOpenRepository() {
    try {
      const path = await chooseRepositoryDirectory();
      if (!path) return;
      setOpening(true);
      setError(undefined);
      const session = await openRepository(path);
      setTabs(session.tabs);
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    } finally {
      setOpening(false);
      setSessionLoading(false);
    }
  }

  function activateTab(repoId: string) {
    setTabs((current) =>
      current.map((tab) => ({ ...tab, active: tab.repoId === repoId })),
    );
    activateSessionTab(repoId).catch((reason: unknown) => setError(normalizeAppError(reason)));
  }

  async function closeTab(repoId: string) {
    try {
      const session = await closeSessionTab(repoId);
      setTabs(session.tabs);
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  function moveTab(repoId: string, offset: number) {
    const from = tabs.findIndex((tab) => tab.repoId === repoId);
    const to = from + offset;
    if (from < 0 || to < 0 || to >= tabs.length) return;
    const next = [...tabs];
    [next[from], next[to]] = [next[to], next[from]];
    setTabs(next);
    reorderSessionTabs(next.map((tab) => tab.repoId)).catch((reason: unknown) =>
      setError(normalizeAppError(reason)),
    );
  }

  function updateActiveTab(patch: Partial<Pick<SessionTabDto, "page" | "selectedPath">>) {
    if (!activeTab) return;
    const next = { ...activeTab, ...patch };
    setTabs((current) => current.map((tab) => (tab.repoId === next.repoId ? next : tab)));
    updateSessionTab(next.repoId, next.page, next.selectedPath, next.panelWidth).catch(
      (reason: unknown) => setError(normalizeAppError(reason)),
    );
  }

  async function handleWorktreeActivate(worktreeId: string) {
    if (!activeTab || activeTab.worktreeId === worktreeId) return;
    try {
      setError(undefined);
      const session = await activateWorktree(activeTab.repoId, worktreeId);
      setTabs(session.tabs);
      setSidebars((current) => {
        const next = { ...current };
        delete next[activeTab.repoId];
        return next;
      });
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
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
          <span>GitAcorn</span><span className="alpha-badge">ALPHA</span>
        </div>
        <div className="window-drag-region" />
      </header>

      <div className="tabbar" aria-label="Repository tabs">
        <div className="repository-tabs">
          {tabs.length === 0 && (
            <div className="tabbar-empty">
              {sessionLoading ? "Restoring session…" : "No repositories open"}
            </div>
          )}
          {tabs.map((tab, index) => (
            <div
              className={`repository-tab ${tab.active ? "active" : ""} ${tab.unavailable ? "unavailable" : ""}`}
              key={tab.repoId}
            >
              <button
                className="tab-main"
                type="button"
                aria-current={tab.active ? "page" : undefined}
                onClick={() => activateTab(tab.repoId)}
              >
                <span className="repository-dot" aria-hidden="true" />
                <strong>{tab.snapshot?.repository.name ?? repositoryName(tab.worktreePath)}</strong>
                <span>{tab.unavailable ? "!" : (tab.snapshot?.changes.length ?? 0)}</span>
              </button>
              <div className="tab-controls">
                <button type="button" aria-label={`Move ${repositoryName(tab.worktreePath)} left`} disabled={index === 0} onClick={() => moveTab(tab.repoId, -1)}>‹</button>
                <button type="button" aria-label={`Move ${repositoryName(tab.worktreePath)} right`} disabled={index === tabs.length - 1} onClick={() => moveTab(tab.repoId, 1)}>›</button>
                <button type="button" aria-label={`Close ${repositoryName(tab.worktreePath)}`} onClick={() => closeTab(tab.repoId)}>×</button>
              </div>
            </div>
          ))}
        </div>
        <button className="open-button" type="button" disabled={opening} onClick={handleOpenRepository}>
          <span aria-hidden="true">＋</span>{opening ? " Opening…" : " Open a repository"}
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
                disabled={!activeTab}
                aria-current={page === item.id ? "page" : undefined}
                onClick={() => updateActiveTab({ page: item.id })}
              >
                <span className={`nav-icon ${item.id}`} aria-hidden="true" />
                <span>{item.label}</span><kbd>{item.shortcut}</kbd>
              </button>
            ))}
          </nav>
          <div className="sidebar-groups">
            <SidebarGroup label="Worktrees" count={activeSidebar?.worktrees.length}>
              {activeSidebar?.worktrees.map((worktree) => (
                <button
                  type="button"
                  key={worktree.id}
                  title={worktree.path}
                  aria-current={worktree.id === activeTab?.worktreeId ? "true" : undefined}
                  onClick={() => handleWorktreeActivate(worktree.id)}
                >
                  {worktree.isCurrent ? "● " : ""}{worktree.branch ?? "Detached"}
                  {worktree.isLocked ? " · locked" : ""}
                </button>
              ))}
            </SidebarGroup>
            <SidebarGroup label="Branches" count={activeSidebar?.branches.total}>
              {activeSidebar?.branches.items.map((branch) => (
                <span key={branch}>{branch === branchLabel ? "● " : ""}{branch}</span>
              ))}
            </SidebarGroup>
            <SidebarGroup label="Tags" count={activeSidebar?.tags.total}>
              {activeSidebar?.tags.items.map((tag) => <span key={tag}>{tag}</span>)}
            </SidebarGroup>
            <SidebarGroup label="Stashes" count={activeSnapshot?.stashCount}>
              {activeSidebar?.stashes.map((stash) => (
                <span key={stash.reference} title={stash.message}>{stash.reference}</span>
              ))}
            </SidebarGroup>
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
              <span className="eyebrow">{activeTab?.worktreePath ?? "Local workspace"}</span>
              <strong>{activeSnapshot ? `${branchLabel} · ${page === "changes" ? "Changes" : "History"}` : page === "changes" ? "Changes" : "History"}</strong>
            </div>
            <div className="remote-actions" aria-label="Remote actions">
              {activeTab && refreshing.has(activeTab.repoId) && <span className="refreshing">Refreshing…</span>}
              <button type="button" disabled>Fetch</button>
              <button type="button" disabled>Pull{activeSnapshot?.behind ? ` ${activeSnapshot.behind}` : ""}</button>
              <button type="button" disabled>Push{activeSnapshot?.ahead ? ` ${activeSnapshot.ahead}` : ""}</button>
            </div>
          </div>
          {appInfo.status === "error" && <ErrorBanner title="Could not reach the GitAcorn core." message={appInfo.message} />}
          {error && <ErrorBanner title="Repository session needs attention." message={error.message} detail={error.details} actionLabel={error.code === "repositoryNotFound" ? "Choose another folder" : undefined} onAction={handleOpenRepository} />}
          {activeTab?.unavailable ? (
            <UnavailableRepository tab={activeTab} onLocate={handleOpenRepository} />
          ) : page === "changes" ? (
            activeSnapshot ? (
              <ChangesView
                snapshot={activeSnapshot}
                selectedPath={activeTab.selectedPath}
                panelWidth={activeTab.panelWidth}
                onPanelWidth={(panelWidth) => {
                  const next = { ...activeTab, panelWidth };
                  setTabs((current) =>
                    current.map((tab) => (tab.repoId === next.repoId ? next : tab)),
                  );
                  updateSessionTab(
                    next.repoId,
                    next.page,
                    next.selectedPath,
                    panelWidth,
                  ).catch((reason: unknown) => setError(normalizeAppError(reason)));
                }}
                onSelect={(selectedPath) => updateActiveTab({ selectedPath })}
              />
            ) : (
              <ChangesEmpty onOpen={handleOpenRepository} opening={opening || sessionLoading} />
            )
          ) : (
            <HistoryEmpty hasRepository={Boolean(activeSnapshot)} />
          )}
        </section>
      </main>
    </div>
  );
}

function repositoryName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? "Repository";
}

function SidebarGroup({ label, count, children }: { label: string; count?: number; children?: ReactNode }) {
  return <div className="sidebar-read-group"><div className="sidebar-group"><span>›</span>{label}{count !== undefined && <small>{Math.min(count, 5)} of {count}</small>}</div>{children && <div className="sidebar-items">{children}</div>}</div>;
}

function ErrorBanner({ title, message, detail, actionLabel, onAction }: { title: string; message: string; detail?: string; actionLabel?: string; onAction?: () => void }) {
  return <div className="error-banner" role="alert"><div><strong>{title}</strong><span>{message}</span>{detail && <small>{detail}</small>}</div>{actionLabel && <button type="button" onClick={onAction}>{actionLabel}</button>}</div>;
}

function UnavailableRepository({ tab, onLocate }: { tab: SessionTabDto; onLocate: () => void }) {
  return <div className="welcome-panel"><p className="eyebrow">Repository unavailable</p><h1>{repositoryName(tab.worktreePath)} moved or was deleted.</h1><p>{tab.worktreePath}</p><button type="button" onClick={onLocate}>Locate repository</button></div>;
}

function ChangesView({ snapshot, selectedPath, panelWidth, onPanelWidth, onSelect }: { snapshot: RepositorySnapshotDto; selectedPath?: string; panelWidth: number; onPanelWidth: (width: number) => void; onSelect: (path: string) => void }) {
  const unstaged = useMemo(() => snapshot.changes.filter((change) => change.worktreeStatus !== "." || change.conflict), [snapshot]);
  const staged = useMemo(() => snapshot.changes.filter((change) => change.indexStatus !== "." && change.indexStatus !== "?"), [snapshot]);
  const selected = snapshot.changes.find((change) => change.path === selectedPath);
  return <div className="changes-layout" style={{ "--file-panel-width": `${panelWidth}px` } as CSSProperties}>
    <section className="file-panel" aria-label="Changed files">
      <label className="panel-width-control">
        <span>File panel width</span>
        <input aria-label="Changed files panel width" type="range" min="190" max="420" value={panelWidth} onChange={(event) => onPanelWidth(Number(event.currentTarget.value))} />
      </label>
      <ChangeSection title="Unstaged" changes={unstaged} selectedPath={selectedPath} onSelect={onSelect} />
      <ChangeSection title="Staged" changes={staged} selectedPath={selectedPath} onSelect={onSelect} />
    </section>
    <section className="selected-file-panel">{selected ? <><p className="eyebrow">Selected change</p><h1>{selected.path}</h1>{selected.originalPath && <p>Renamed from {selected.originalPath}</p>}<div className="change-metadata"><span>Index <strong>{statusLabel(selected.indexStatus)}</strong></span><span>Working tree <strong>{statusLabel(selected.worktreeStatus)}</strong></span>{selected.conflict && <span className="conflict-label">Conflict</span>}</div><p className="diff-placeholder">Diff rendering arrives in M3.</p></> : <div className="empty-selection"><span className="file-glyph" aria-hidden="true" /><h1>{snapshot.changes.length === 0 ? "Working tree clean" : "Select a changed file"}</h1><p>{snapshot.changes.length === 0 ? "There are no staged or unstaged changes." : "Choose a file to inspect its Git status."}</p></div>}</section>
    <aside className="commit-panel" aria-label="Commit form preview"><div className="panel-heading"><h2>Commit</h2><span>{staged.length}</span></div><textarea aria-label="Commit summary" placeholder="Summary" disabled /><textarea aria-label="Commit description" placeholder="Description (optional)" disabled /><button type="button" disabled>Commit to {snapshot.head.name ?? "HEAD"}</button></aside>
  </div>;
}

function ChangeSection({ title, changes, selectedPath, onSelect }: { title: string; changes: FileChangeDto[]; selectedPath?: string; onSelect: (path: string) => void }) {
  return <div className="change-section"><div className="panel-heading"><h2>{title}</h2><span>{changes.length}</span></div>{changes.length === 0 ? <div className="panel-empty">No {title.toLowerCase()} changes.</div> : <div className="change-list">{changes.map((change) => <button className={selectedPath === change.path ? "change-row selected" : "change-row"} type="button" key={`${title}-${change.path}-${change.indexStatus}-${change.worktreeStatus}`} onClick={() => onSelect(change.path)} title={change.path}><span className={`status-badge ${change.conflict ? "conflict" : ""}`}>{change.conflict ? "!" : title === "Staged" ? change.indexStatus : change.worktreeStatus}</span><span className="change-path">{change.path}</span>{change.submodule && <small>submodule</small>}</button>)}</div>}</div>;
}

function statusLabel(code: string) {
  return ({ ".": "Unchanged", "?": "Untracked", M: "Modified", A: "Added", D: "Deleted", R: "Renamed", C: "Copied", T: "Type changed", U: "Unmerged" }[code] ?? code);
}

function ChangesEmpty({ onOpen, opening }: { onOpen: () => void; opening: boolean }) {
  return <div className="changes-layout"><section className="file-panel" aria-label="Changed files"><ChangeSection title="Unstaged" changes={[]} onSelect={() => undefined} /><ChangeSection title="Staged" changes={[]} onSelect={() => undefined} /></section><section className="welcome-panel"><div className="welcome-art" aria-hidden="true"><span className="branch-line" /><span className="branch-node node-one" /><span className="branch-node node-two" /><span className="branch-node node-three" /></div><p className="eyebrow">A calmer Git workflow</p><h1>Your repositories, clearly in view.</h1><p>Open a local repository to inspect its real staged, unstaged, and untracked changes.</p><button type="button" onClick={onOpen} disabled={opening}>{opening ? "Opening…" : "Open a repository"}</button><small>Git 2.40.0 or newer is required.</small></section><aside className="commit-panel" aria-label="Commit form preview"><div className="panel-heading"><h2>Commit</h2></div><textarea aria-label="Commit summary" placeholder="Summary" disabled /><textarea aria-label="Commit description" placeholder="Description (optional)" disabled /><button type="button" disabled>Commit</button></aside></div>;
}

function HistoryEmpty({ hasRepository }: { hasRepository: boolean }) {
  return <div className="history-empty"><div className="history-lines" aria-hidden="true"><i /><i /><i /></div><p className="eyebrow">Commit graph</p><h1>History will appear here.</h1><p>{hasRepository ? "Commit history is planned for M4." : "Open a repository to explore commits, branches, tags, and authors."}</p></div>;
}
