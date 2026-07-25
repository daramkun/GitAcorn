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
  applyPatchSelection,
  abortMerge,
  activateSessionTab,
  activateWorktree,
  applyStash,
  cancelOperation,
  checkoutBranch,
  chooseCloneParentDirectory,
  chooseRepositoryDirectory,
  closeSessionTab,
  createBranch,
  createCommit,
  createStash,
  deleteBranch,
  discardPath,
  dropStash,
  getDiff,
  getHistoryPage,
  getDiagnostics,
  getOperationHistory,
  getReferences,
  getRepositorySidebar,
  getRepositorySnapshot,
  listenForRepositoryChanges,
  mergeBranch,
  normalizeAppError,
  openRepository,
  reorderSessionTabs,
  resolveConflict,
  restoreSession,
  startClone,
  startRemoteOperation,
  stagePaths,
  unstagePaths,
  updateSessionTab,
  type AppErrorDto,
  type CommitDto,
  type DiffDto,
  type DiffTarget,
  type FileChangeDto,
  type OperationEventDto,
  type OperationRecordDto,
  type RepositorySnapshotDto,
  type RepositorySidebarDto,
  type ReferenceDto,
  type SessionTabDto,
} from "./repository";
import { updateRepositoryOperation } from "./remote-operations";

type Page = "changes" | "history" | "operations";
type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfoDto }
  | { status: "error"; message: string };

const navigation: ReadonlyArray<{ id: Page; label: string; shortcut: string }> = [
  { id: "changes", label: "Changes", shortcut: "⌘1" },
  { id: "history", label: "History", shortcut: "⌘2" },
  { id: "operations", label: "Operations", shortcut: "⌘3" },
];

export function App() {
  const [appInfo, setAppInfo] = useState<AppInfoState>({ status: "loading" });
  const [tabs, setTabs] = useState<SessionTabDto[]>([]);
  const [sessionLoading, setSessionLoading] = useState(true);
  const [opening, setOpening] = useState(false);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [sidebars, setSidebars] = useState<Record<string, RepositorySidebarDto>>({});
  const [error, setError] = useState<AppErrorDto>();
  const [remoteOperations, setRemoteOperations] = useState<
    Record<string, OperationEventDto>
  >({});
  const [cloneUrl, setCloneUrl] = useState("");
  const [showClone, setShowClone] = useState(false);
  const [cloneOperation, setCloneOperation] = useState<OperationEventDto>();
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

  function handleRemote(kind: "fetch" | "pull" | "push", forceWithLease = false) {
    if (!activeTab) return;
    const repoId = activeTab.repoId;
    setError(undefined);
    startRemoteOperation(
      repoId,
      kind,
      (event) => {
        if (event.repoId !== repoId) return;
        setRemoteOperations((current) =>
          updateRepositoryOperation(current, repoId, event),
        );
        if (event.snapshot) {
          setTabs((current) =>
            current.map((tab) =>
              tab.repoId === repoId ? { ...tab, snapshot: event.snapshot } : tab,
            ),
          );
        }
        if (event.error) setError(event.error);
      },
      forceWithLease,
    ).catch((reason: unknown) => setError(normalizeAppError(reason)));
  }

  async function handleClone() {
    const remoteUrl = cloneUrl.trim();
    if (!remoteUrl) return;
    try {
      const parent = await chooseCloneParentDirectory();
      if (!parent) return;
      const separator = parent.includes("\\") && !parent.includes("/") ? "\\" : "/";
      const destination = `${parent}${parent.endsWith("\\") || parent.endsWith("/") ? "" : separator}${cloneRepositoryName(remoteUrl)}`;
      setError(undefined);
      await startClone(remoteUrl, destination, (event) => {
        setCloneOperation(event);
        if (event.error) setError(event.error);
        if (event.state === "succeeded" && event.destination) {
          openRepository(event.destination)
            .then((session) => {
              setTabs(session.tabs);
              setShowClone(false);
              setCloneUrl("");
            })
            .catch((reason: unknown) => setError(normalizeAppError(reason)));
        }
      });
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
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

  function updateActiveTab(
    patch: Partial<
      Pick<
        SessionTabDto,
        | "page"
        | "selectedPath"
        | "selectedDiff"
        | "historyCursor"
        | "selectedCommit"
        | "historyFilter"
      >
    >,
  ) {
    if (!activeTab) return;
    const next = { ...activeTab, ...patch };
    setTabs((current) => current.map((tab) => (tab.repoId === next.repoId ? next : tab)));
    updateSessionTab(
      next.repoId,
      next.page,
      next.selectedPath,
      next.selectedDiff,
      next.panelWidth,
      next.historyCursor,
      next.selectedCommit,
      next.historyFilter,
    ).catch(
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

  async function handleWorkspaceMutation(
    action: () => Promise<RepositorySnapshotDto>,
  ) {
    if (!activeTab) return;
    try {
      setError(undefined);
      const snapshot = await action();
      setTabs((current) =>
        current.map((tab) =>
          tab.repoId === snapshot.repository.id ? { ...tab, snapshot } : tab,
        ),
      );
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
        <button className="open-button" type="button" onClick={() => setShowClone((value) => !value)}>
          Clone
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
            <StashControls
              snapshot={activeSnapshot}
              stashes={activeSidebar?.stashes ?? []}
              onCreate={(message, includeUntracked) =>
                activeSnapshot &&
                handleWorkspaceMutation(() =>
                  createStash(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    message,
                    includeUntracked,
                  ),
                )
              }
              onApply={(reference) =>
                activeSnapshot &&
                handleWorkspaceMutation(() =>
                  applyStash(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    reference,
                  ),
                )
              }
              onDrop={(reference) =>
                activeSnapshot &&
                handleWorkspaceMutation(() =>
                  dropStash(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    reference,
                  ),
                )
              }
            />
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
              <strong>{activeSnapshot ? `${branchLabel} · ${navigation.find((item) => item.id === page)?.label}` : navigation.find((item) => item.id === page)?.label}</strong>
            </div>
            <div className="remote-actions" aria-label="Remote actions">
              {activeTab && refreshing.has(activeTab.repoId) && <span className="refreshing">Refreshing…</span>}
              {activeTab && remoteOperations[activeTab.repoId] &&
                ["queued", "running"].includes(remoteOperations[activeTab.repoId].state) ? (
                  <>
                    <span className="refreshing" role="status">
                      {remoteOperations[activeTab.repoId].kind} · {remoteOperations[activeTab.repoId].message ?? remoteOperations[activeTab.repoId].state}
                    </span>
                    <button type="button" onClick={() => cancelOperation(remoteOperations[activeTab.repoId].operationId).catch((reason: unknown) => setError(normalizeAppError(reason)))}>Cancel</button>
                  </>
                ) : (
                  <>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("fetch")}>Fetch</button>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("pull")}>Pull{activeSnapshot?.behind ? ` ${activeSnapshot.behind}` : ""}</button>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("push")}>Push{activeSnapshot?.ahead ? ` ${activeSnapshot.ahead}` : ""}</button>
                    <button type="button" disabled={!activeTab} title="Reject if the remote changed since the last fetch" onClick={() => handleRemote("push", true)}>Push with lease</button>
                  </>
                )}
            </div>
          </div>
          {showClone && (
            <form className="clone-bar" onSubmit={(event) => { event.preventDefault(); void handleClone(); }}>
              <label htmlFor="clone-url">Repository URL</label>
              <input id="clone-url" value={cloneUrl} onChange={(event) => setCloneUrl(event.target.value)} placeholder="https://host/owner/repository.git or git@host:owner/repository.git" />
              {cloneOperation && ["queued", "running"].includes(cloneOperation.state) ? (
                <>
                  <span role="status">{cloneOperation.message ?? "Cloning…"}</span>
                  <button type="button" onClick={() => cancelOperation(cloneOperation.operationId)}>Cancel</button>
                </>
              ) : (
                <button type="submit" disabled={!cloneUrl.trim()}>Choose destination and clone</button>
              )}
            </form>
          )}
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
                selectedTarget={activeTab.selectedDiff}
                onPanelWidth={(panelWidth) => {
                  const next = { ...activeTab, panelWidth };
                  setTabs((current) =>
                    current.map((tab) => (tab.repoId === next.repoId ? next : tab)),
                  );
                  updateSessionTab(
                    next.repoId,
                    next.page,
                    next.selectedPath,
                    next.selectedDiff,
                    panelWidth,
                    next.historyCursor,
                    next.selectedCommit,
                    next.historyFilter,
                  ).catch((reason: unknown) => setError(normalizeAppError(reason)));
                }}
                onSelect={(selectedPath, selectedDiff) =>
                  updateActiveTab({ selectedPath, selectedDiff })
                }
                onSnapshot={(snapshot) =>
                  setTabs((current) =>
                    current.map((tab) =>
                      tab.repoId === snapshot.repository.id
                        ? { ...tab, snapshot, unavailable: false }
                        : tab,
                    ),
                  )
                }
                onError={(reason) => setError(normalizeAppError(reason))}
              />
            ) : (
              <ChangesEmpty onOpen={handleOpenRepository} opening={opening || sessionLoading} />
            )
          ) : page === "history" ? (
            activeSnapshot && activeTab ? (
              <HistoryView
                key={activeTab.repoId}
                tab={activeTab}
                snapshot={activeSnapshot}
                onPersist={(patch) => updateActiveTab(patch)}
                onSnapshot={(next) =>
                  setTabs((current) =>
                    current.map((tab) =>
                      tab.repoId === next.repository.id ? { ...tab, snapshot: next } : tab,
                    ),
                  )
                }
                onSidebarInvalidated={() =>
                  setSidebars((current) => {
                    const next = { ...current };
                    delete next[activeTab.repoId];
                    return next;
                  })
                }
                onError={(reason) => setError(normalizeAppError(reason))}
              />
            ) : (
              <HistoryEmpty />
            )
          ) : (
            <OperationsView onError={(reason) => setError(normalizeAppError(reason))} />
          )}
        </section>
      </main>
    </div>
  );
}

function repositoryName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? "Repository";
}

function cloneRepositoryName(remoteUrl: string) {
  const withoutQuery = remoteUrl.split(/[?#]/, 1)[0].replace(/[\\/]+$/, "");
  const name = withoutQuery.split(/[\\/:]/).filter(Boolean).at(-1) ?? "repository";
  return name.replace(/\.git$/i, "") || "repository";
}

function SidebarGroup({ label, count, children }: { label: string; count?: number; children?: ReactNode }) {
  return <div className="sidebar-read-group"><div className="sidebar-group"><span>›</span>{label}{count !== undefined && <small>{Math.min(count, 5)} of {count}</small>}</div>{children && <div className="sidebar-items">{children}</div>}</div>;
}

function StashControls({
  snapshot,
  stashes,
  onCreate,
  onApply,
  onDrop,
}: {
  snapshot?: RepositorySnapshotDto;
  stashes: RepositorySidebarDto["stashes"];
  onCreate: (message: string, includeUntracked: boolean) => void;
  onApply: (reference: string) => void;
  onDrop: (reference: string) => void;
}) {
  const [message, setMessage] = useState("");
  const [includeUntracked, setIncludeUntracked] = useState(true);
  return (
    <div className="sidebar-read-group stash-controls">
      <div className="sidebar-group">
        <span>›</span>Stashes
        {snapshot && <small>{Math.min(snapshot.stashCount, 5)} of {snapshot.stashCount}</small>}
      </div>
      {snapshot && (
        <form
          aria-label="Create stash"
          onSubmit={(event) => {
            event.preventDefault();
            onCreate(message, includeUntracked);
            setMessage("");
          }}
        >
          <input
            aria-label="Stash message"
            placeholder="Stash message"
            value={message}
            onChange={(event) => setMessage(event.currentTarget.value)}
          />
          <label>
            <input
              type="checkbox"
              checked={includeUntracked}
              onChange={(event) => setIncludeUntracked(event.currentTarget.checked)}
            />
            Include untracked
          </label>
          <button type="submit" disabled={snapshot.changes.length === 0}>Stash changes</button>
        </form>
      )}
      <div className="sidebar-items">
        {stashes.map((stash) => (
          <div className="stash-item" key={stash.reference} title={stash.message}>
            <span>{stash.reference} · {stash.message}</span>
            <div>
              <button type="button" onClick={() => onApply(stash.reference)}>Apply</button>
              <button
                type="button"
                className="danger-button"
                onClick={() => {
                  if (window.confirm(`Drop ${stash.reference}? The stash entry cannot be recovered.`)) {
                    onDrop(stash.reference);
                  }
                }}
              >
                Drop
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function OperationsView({ onError }: { onError: (error: unknown) => void }) {
  const [operations, setOperations] = useState<OperationRecordDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [copyState, setCopyState] = useState("");

  function refresh() {
    setLoading(true);
    getOperationHistory()
      .then(setOperations)
      .catch(onError)
      .finally(() => setLoading(false));
  }

  useEffect(refresh, [onError]);

  async function copyDiagnostics() {
    try {
      const diagnostics = await getDiagnostics();
      await navigator.clipboard.writeText(diagnostics);
      setCopyState("Diagnostics copied");
    } catch (reason: unknown) {
      onError(reason);
    }
  }

  return (
    <section className="operations-view" aria-labelledby="operations-title">
      <div className="operations-heading">
        <div>
          <span className="eyebrow">Recovery and diagnostics</span>
          <h1 id="operations-title">Operation center</h1>
        </div>
        <div>
          <button type="button" onClick={refresh}>Refresh</button>
          <button type="button" onClick={() => void copyDiagnostics()}>Copy diagnostics</button>
        </div>
      </div>
      {copyState && <p role="status">{copyState}</p>}
      {loading ? (
        <div className="history-state" role="status">Loading operations…</div>
      ) : operations.length === 0 ? (
        <div className="history-state">No operations have been recorded.</div>
      ) : (
        <ol className="operation-list">
          {operations.map((operation) => (
            <li key={operation.id}>
              <span className={`operation-state ${operation.state}`}>{operation.state}</span>
              <div>
                <strong>{operation.kind}</strong>
                <span>{operation.summary}</span>
                {operation.diagnostic && <code>{operation.diagnostic}</code>}
              </div>
              <time>{operation.startedAt}</time>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function ErrorBanner({ title, message, detail, actionLabel, onAction }: { title: string; message: string; detail?: string; actionLabel?: string; onAction?: () => void }) {
  return <div className="error-banner" role="alert"><div><strong>{title}</strong><span>{message}</span>{detail && <small>{detail}</small>}</div>{actionLabel && <button type="button" onClick={onAction}>{actionLabel}</button>}</div>;
}

function UnavailableRepository({ tab, onLocate }: { tab: SessionTabDto; onLocate: () => void }) {
  return <div className="welcome-panel"><p className="eyebrow">Repository unavailable</p><h1>{repositoryName(tab.worktreePath)} moved or was deleted.</h1><p>{tab.worktreePath}</p><button type="button" onClick={onLocate}>Locate repository</button></div>;
}

function ChangesView({
  snapshot,
  selectedPath,
  selectedTarget,
  panelWidth,
  onPanelWidth,
  onSelect,
  onSnapshot,
  onError,
}: {
  snapshot: RepositorySnapshotDto;
  selectedPath?: string;
  selectedTarget: DiffTarget;
  panelWidth: number;
  onPanelWidth: (width: number) => void;
  onSelect: (path: string, target: DiffTarget) => void;
  onSnapshot: (snapshot: RepositorySnapshotDto) => void;
  onError: (error: unknown) => void;
}) {
  const unstaged = useMemo(
    () =>
      snapshot.changes.filter(
        (change) => change.worktreeStatus !== "." || change.conflict,
      ),
    [snapshot],
  );
  const staged = useMemo(
    () =>
      snapshot.changes.filter(
        (change) => change.indexStatus !== "." && change.indexStatus !== "?",
      ),
    [snapshot],
  );
  const selected = snapshot.changes.find((change) => change.path === selectedPath);
  const [diff, setDiff] = useState<DiffDto>();
  const [diffLoading, setDiffLoading] = useState(false);
  const [selectedLines, setSelectedLines] = useState<Set<string>>(new Set());
  const [operation, setOperation] = useState<string>();
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [amend, setAmend] = useState(false);

  useEffect(() => {
    let active = true;
    setSelectedLines(new Set());
    if (!selected || selected.conflict) {
      setDiff(undefined);
      return () => {
        active = false;
      };
    }
    setDiffLoading(true);
    getDiff(
      snapshot.repository.id,
      snapshot.revision,
      selected.pathBytes,
      selectedTarget,
    )
      .then((value) => active && setDiff(value))
      .catch((reason: unknown) => active && onError(reason))
      .finally(() => active && setDiffLoading(false));
    return () => {
      active = false;
    };
  }, [
    onError,
    selected,
    selectedTarget,
    snapshot.repository.id,
    snapshot.revision,
  ]);

  async function mutate(
    label: string,
    action: () => Promise<RepositorySnapshotDto>,
  ) {
    try {
      setOperation(label);
      const next = await action();
      setSelectedLines(new Set());
      onSnapshot(next);
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setOperation(undefined);
    }
  }

  function applyLines() {
    if (!selected || selectedLines.size === 0) return;
    const byHunk = new Map<number, number[]>();
    for (const key of selectedLines) {
      const [hunk, line] = key.split(":").map(Number);
      byHunk.set(hunk, [...(byHunk.get(hunk) ?? []), line]);
    }
    void mutate(selectedTarget === "staged" ? "Unstaging lines…" : "Staging lines…", () =>
      applyPatchSelection(
        snapshot.repository.id,
        snapshot.revision,
        selected.pathBytes,
        selectedTarget,
        [...byHunk].map(([hunkIndex, lineIndices]) => ({
          hunkIndex,
          lineIndices,
        })),
      ),
    );
  }

  function discardSelected() {
    if (!selected || selectedTarget !== "unstaged") return;
    if (
      !window.confirm(
        `Discard the displayed working-tree changes in ${selected.path}? This cannot be undone by GitAcorn.`,
      )
    ) {
      return;
    }
    void mutate("Discarding…", () =>
      discardPath(
        snapshot.repository.id,
        snapshot.revision,
        selected.pathBytes,
        selected.worktreeStatus === "?",
      ),
    );
  }

  function submitCommit() {
    if (!summary.trim()) return;
    void mutate(amend ? "Amending…" : "Committing…", () =>
      createCommit(snapshot.repository.id, snapshot.revision, {
        summary,
        description,
        amend,
      }),
    );
  }

  return (
    <div
      className="changes-layout"
      style={{ "--file-panel-width": `${panelWidth}px` } as CSSProperties}
    >
      <section className="file-panel" aria-label="Changed files">
        <label className="panel-width-control">
          <span>File panel width</span>
          <input
            aria-label="Changed files panel width"
            type="range"
            min="190"
            max="420"
            value={panelWidth}
            onChange={(event) => onPanelWidth(Number(event.currentTarget.value))}
          />
        </label>
        <ChangeSection
          title="Unstaged"
          target="unstaged"
          changes={unstaged}
          selectedPath={selectedPath}
          selectedTarget={selectedTarget}
          onSelect={onSelect}
        />
        <ChangeSection
          title="Staged"
          target="staged"
          changes={staged}
          selectedPath={selectedPath}
          selectedTarget={selectedTarget}
          onSelect={onSelect}
        />
      </section>
      <section className="selected-file-panel diff-panel">
        {selected ? (
          <>
            <div className="diff-toolbar">
              <div>
                <span className="eyebrow">
                  {selectedTarget === "staged" ? "Staged diff" : "Unstaged diff"}
                </span>
                <strong>{selected.path}</strong>
              </div>
              {selected.conflict ? (
                <div className="conflict-actions" aria-label="Conflict resolution">
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate("Using our version…", () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "ours",
                        ),
                      )
                    }
                  >
                    Use ours
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate("Using their version…", () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "theirs",
                        ),
                      )
                    }
                  >
                    Use theirs
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate("Marking resolved…", () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "markResolved",
                        ),
                      )
                    }
                  >
                    Mark current content resolved
                  </button>
                  <button
                    type="button"
                    className="danger-button"
                    disabled={Boolean(operation)}
                    onClick={() => {
                      if (window.confirm("Abort this merge and restore the pre-merge working tree?")) {
                        void mutate("Aborting merge…", () =>
                          abortMerge(snapshot.repository.id, snapshot.revision),
                        );
                      }
                    }}
                  >
                    Abort merge…
                  </button>
                </div>
              ) : (
              <div>
                <button
                  type="button"
                  disabled={Boolean(operation)}
                  onClick={() =>
                    void mutate(
                      selectedTarget === "staged" ? "Unstaging file…" : "Staging file…",
                      () =>
                        selectedTarget === "staged"
                          ? unstagePaths(snapshot.repository.id, snapshot.revision, [
                              selected.pathBytes,
                            ])
                          : stagePaths(snapshot.repository.id, snapshot.revision, [
                              selected.pathBytes,
                            ]),
                    )
                  }
                >
                  {selectedTarget === "staged" ? "Unstage file" : "Stage file"}
                </button>
                <button
                  type="button"
                  disabled={selectedLines.size === 0 || Boolean(operation)}
                  onClick={applyLines}
                >
                  {selectedTarget === "staged" ? "Unstage" : "Stage"} selected lines
                </button>
                {selectedTarget === "unstaged" && (
                  <button
                    className="danger-button"
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={discardSelected}
                  >
                    Discard…
                  </button>
                )}
              </div>
              )}
            </div>
            {operation && <div className="operation-status" role="status">{operation}</div>}
            {selected.conflict ? (
              <div className="conflict-panel" role="region" aria-label="Conflict resolution guidance">
                <h2>Resolve merge conflict</h2>
                <p>
                  Choose one side, or edit the file in your editor and mark the current
                  content resolved. Aborting restores the state from before the merge.
                </p>
              </div>
            ) : diffLoading ? (
              <div className="diff-state" role="status">Loading diff…</div>
            ) : diff?.binary ? (
              <div className="diff-state">Binary file. Use the whole-file action.</div>
            ) : diff && diff.hunks.length > 0 ? (
              <DiffRenderer
                diff={diff}
                selectedLines={selectedLines}
                onToggleLine={(key) =>
                  setSelectedLines((current) => {
                    const next = new Set(current);
                    if (next.has(key)) next.delete(key);
                    else next.add(key);
                    return next;
                  })
                }
                onApplyHunk={(hunkIndex) =>
                  void mutate(
                    selectedTarget === "staged" ? "Unstaging hunk…" : "Staging hunk…",
                    () =>
                      applyPatchSelection(
                        snapshot.repository.id,
                        snapshot.revision,
                        selected.pathBytes,
                        selectedTarget,
                        [{ hunkIndex, lineIndices: [] }],
                      ),
                  )
                }
                actionLabel={selectedTarget === "staged" ? "Unstage hunk" : "Stage hunk"}
              />
            ) : (
              <div className="diff-state">No text diff is available for this side.</div>
            )}
          </>
        ) : (
          <div className="empty-selection">
            <span className="file-glyph" aria-hidden="true" />
            <h1>
              {snapshot.changes.length === 0
                ? "Working tree clean"
                : "Select a changed file"}
            </h1>
            <p>
              {snapshot.changes.length === 0
                ? "There are no staged or unstaged changes."
                : "Choose a file to inspect and stage its diff."}
            </p>
          </div>
        )}
      </section>
      <aside className="commit-panel" aria-label="Commit form">
        <div className="panel-heading"><h2>Commit</h2><span>{staged.length}</span></div>
        <textarea
          aria-label="Commit summary"
          placeholder="Summary"
          value={summary}
          onChange={(event) => setSummary(event.currentTarget.value)}
        />
        <textarea
          aria-label="Commit description"
          placeholder="Description (optional)"
          value={description}
          onChange={(event) => setDescription(event.currentTarget.value)}
        />
        <label className="amend-control">
          <input
            type="checkbox"
            checked={amend}
            onChange={(event) => setAmend(event.currentTarget.checked)}
          />
          Amend previous commit
        </label>
        <button
          className="primary-action"
          type="button"
          disabled={!summary.trim() || (!amend && staged.length === 0) || Boolean(operation)}
          onClick={submitCommit}
        >
          {amend ? "Amend" : "Commit"} to {snapshot.head.name ?? "HEAD"}
        </button>
      </aside>
    </div>
  );
}

function ChangeSection({
  title,
  target,
  changes,
  selectedPath,
  selectedTarget,
  onSelect,
}: {
  title: string;
  target: DiffTarget;
  changes: FileChangeDto[];
  selectedPath?: string;
  selectedTarget?: DiffTarget;
  onSelect: (path: string, target: DiffTarget) => void;
}) {
  return (
    <div className="change-section">
      <div className="panel-heading"><h2>{title}</h2><span>{changes.length}</span></div>
      {changes.length === 0 ? (
        <div className="panel-empty">No {title.toLowerCase()} changes.</div>
      ) : (
        <VirtualChangeList
          changes={changes}
          target={target}
          selectedPath={selectedPath}
          selectedTarget={selectedTarget}
          onSelect={onSelect}
        />
      )}
    </div>
  );
}

function VirtualChangeList({
  changes,
  target,
  selectedPath,
  selectedTarget,
  onSelect,
}: {
  changes: FileChangeDto[];
  target: DiffTarget;
  selectedPath?: string;
  selectedTarget?: DiffTarget;
  onSelect: (path: string, target: DiffTarget) => void;
}) {
  const rowHeight = 34;
  const [scrollTop, setScrollTop] = useState(0);
  const visibleCount = 20;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - 4);
  const end = Math.min(changes.length, start + visibleCount + 8);
  return (
    <div className="change-list" onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}>
      <div className="virtual-list-space" style={{ height: changes.length * rowHeight }}>
        <div style={{ transform: `translateY(${start * rowHeight}px)` }}>
          {changes.slice(start, end).map((change) => {
            const partial =
              change.indexStatus !== "." &&
              change.indexStatus !== "?" &&
              (change.worktreeStatus !== "." || change.conflict);
            return (
              <button
                className={
                  selectedPath === change.path && selectedTarget === target
                    ? "change-row selected"
                    : "change-row"
                }
                type="button"
                key={`${target}-${change.path}-${change.indexStatus}-${change.worktreeStatus}`}
                onClick={() => onSelect(change.path, target)}
                title={change.path}
              >
                <span className={`status-badge ${change.conflict ? "conflict" : ""}`}>
                  {change.conflict
                    ? "!"
                    : target === "staged"
                      ? change.indexStatus
                      : change.worktreeStatus}
                </span>
                <span className="change-path">{change.path}</span>
                {partial && <small>partial</small>}
                {!partial && change.submodule && <small>submodule</small>}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function DiffRenderer({
  diff,
  selectedLines,
  onToggleLine,
  onApplyHunk,
  actionLabel,
}: {
  diff: DiffDto;
  selectedLines: Set<string>;
  onToggleLine: (key: string) => void;
  onApplyHunk: (hunkIndex: number) => void;
  actionLabel: string;
}) {
  return (
    <div className="diff-scroll" aria-label="File diff">
      {diff.hunks.map((hunk) => (
        <div className="diff-hunk" key={hunk.index}>
          <div className="diff-hunk-header">
            <code>{hunk.header}</code>
            <button type="button" onClick={() => onApplyHunk(hunk.index)}>
              {actionLabel}
            </button>
          </div>
          <VirtualDiffLines
            hunkIndex={hunk.index}
            lines={hunk.lines}
            selectedLines={selectedLines}
            onToggleLine={onToggleLine}
          />
        </div>
      ))}
    </div>
  );
}

function VirtualDiffLines({
  hunkIndex,
  lines,
  selectedLines,
  onToggleLine,
}: {
  hunkIndex: number;
  lines: DiffDto["hunks"][number]["lines"];
  selectedLines: Set<string>;
  onToggleLine: (key: string) => void;
}) {
  const rowHeight = 23;
  const [scrollTop, setScrollTop] = useState(0);
  const virtual = lines.length > 300;
  const start = virtual ? Math.max(0, Math.floor(scrollTop / rowHeight) - 8) : 0;
  const end = virtual ? Math.min(lines.length, start + 80) : lines.length;
  const content = lines.slice(start, end).map((line) => {
    const key = `${hunkIndex}:${line.index}`;
    return (
      <button
        type="button"
        key={key}
        className={`diff-line ${line.kind} ${selectedLines.has(key) ? "selected" : ""}`}
        disabled={!line.selectable}
        aria-pressed={line.selectable ? selectedLines.has(key) : undefined}
        onClick={() => line.selectable && onToggleLine(key)}
      >
        <span>{line.oldLine ?? ""}</span>
        <span>{line.newLine ?? ""}</span>
        <code>{line.kind === "addition" ? "+" : line.kind === "deletion" ? "-" : " "}{line.content}</code>
      </button>
    );
  });
  if (!virtual) return <div className="diff-lines">{content}</div>;
  return (
    <div className="diff-lines virtual-diff" onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}>
      <div className="virtual-diff-space" style={{ height: lines.length * rowHeight }}>
        <div
          className="virtual-diff-window"
          style={{ transform: `translateY(${start * rowHeight}px)` }}
        >
          {content}
        </div>
      </div>
    </div>
  );
}

function ChangesEmpty({ onOpen, opening }: { onOpen: () => void; opening: boolean }) {
  return <div className="changes-layout"><section className="file-panel" aria-label="Changed files"><ChangeSection title="Unstaged" target="unstaged" changes={[]} onSelect={() => undefined} /><ChangeSection title="Staged" target="staged" changes={[]} onSelect={() => undefined} /></section><section className="welcome-panel"><div className="welcome-art" aria-hidden="true"><span className="branch-line" /><span className="branch-node node-one" /><span className="branch-node node-two" /><span className="branch-node node-three" /></div><p className="eyebrow">A calmer Git workflow</p><h1>Your repositories, clearly in view.</h1><p>Open a local repository to inspect its real staged, unstaged, and untracked changes.</p><button type="button" onClick={onOpen} disabled={opening}>{opening ? "Opening…" : "Open a repository"}</button><small>Git 2.40.0 or newer is required.</small></section><aside className="commit-panel" aria-label="Commit form preview"><div className="panel-heading"><h2>Commit</h2></div><textarea aria-label="Commit summary" placeholder="Summary" disabled /><textarea aria-label="Commit description" placeholder="Description (optional)" disabled /><button type="button" disabled>Commit</button></aside></div>;
}

function HistoryView({
  tab,
  snapshot,
  onPersist,
  onSnapshot,
  onSidebarInvalidated,
  onError,
}: {
  tab: SessionTabDto;
  snapshot: RepositorySnapshotDto;
  onPersist: (
    patch: Partial<
      Pick<SessionTabDto, "historyCursor" | "selectedCommit" | "historyFilter">
    >,
  ) => void;
  onSnapshot: (snapshot: RepositorySnapshotDto) => void;
  onSidebarInvalidated: () => void;
  onError: (error: unknown) => void;
}) {
  const savedFilter = parseHistoryFilter(tab.historyFilter);
  const [commits, setCommits] = useState<CommitDto[]>([]);
  const [references, setReferences] = useState<ReferenceDto[]>([]);
  const [reference, setReference] = useState(savedFilter.reference);
  const [query, setQuery] = useState(savedFilter.query);
  const [draftQuery, setDraftQuery] = useState(savedFilter.query);
  const [nextCursor, setNextCursor] = useState<string>();
  const [selectedOid, setSelectedOid] = useState(tab.selectedCommit);
  const [loading, setLoading] = useState(true);
  const [operation, setOperation] = useState<string>();
  const [branchName, setBranchName] = useState("");
  const selected = commits.find((commit) => commit.oid === selectedOid) ?? commits[0];

  useEffect(() => {
    let active = true;
    Promise.all([
      getHistoryPage(
        snapshot.repository.id,
        tab.historyCursor,
        reference || undefined,
        query || undefined,
      ),
      getReferences(snapshot.repository.id),
    ])
      .then(([page, refs]) => {
        if (!active) return;
        setCommits(page.commits);
        setNextCursor(page.nextCursor);
        setReferences(refs);
        const oid =
          page.commits.find((commit) => commit.oid === selectedOid)?.oid ??
          page.commits[0]?.oid;
        setSelectedOid(oid);
        if (oid && oid !== tab.selectedCommit) onPersist({ selectedCommit: oid });
      })
      .catch(onError)
      .finally(() => active && setLoading(false));
    return () => {
      active = false;
    };
  }, [snapshot.repository.id, reference, query]);

  async function loadMore() {
    if (!nextCursor) return;
    setLoading(true);
    try {
      const cursor = nextCursor;
      const page = await getHistoryPage(
        snapshot.repository.id,
        cursor,
        reference || undefined,
        query || undefined,
      );
      setCommits((current) => [...current, ...page.commits]);
      setNextCursor(page.nextCursor);
      onPersist({ historyCursor: cursor });
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setLoading(false);
    }
  }

  function persistFilter(nextReference: string, nextQuery: string) {
    onPersist({
      historyCursor: undefined,
      historyFilter: JSON.stringify({ reference: nextReference, query: nextQuery }),
    });
  }

  async function mutate(label: string, action: () => Promise<RepositorySnapshotDto>) {
    setOperation(label);
    try {
      const next = await action();
      onSnapshot(next);
      onSidebarInvalidated();
      setReferences(await getReferences(snapshot.repository.id));
      const page = await getHistoryPage(
        snapshot.repository.id,
        undefined,
        reference || undefined,
        query || undefined,
      );
      setCommits(page.commits);
      setNextCursor(page.nextCursor);
      onPersist({ historyCursor: undefined });
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setOperation(undefined);
    }
  }

  const selectedReference = references.find(
    (item) => item.fullName === reference || item.shortName === reference,
  );
  const currentBranch = snapshot.head.kind === "branch" ? snapshot.head.name : undefined;
  const hasConflicts = snapshot.changes.some((change) => change.conflict);

  return (
    <div className="history-view">
      <section className="history-list-panel" aria-label="Commit history">
        <form
          className="history-filterbar"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery(draftQuery.trim());
            persistFilter(reference, draftQuery.trim());
          }}
        >
          <select
            aria-label="Branch or tag reference"
            value={reference}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setReference(value);
              persistFilter(value, query);
            }}
          >
            <option value="">All branches and tags</option>
            {references.map((item) => (
              <option key={item.fullName} value={item.fullName}>
                {item.kind === "tag" ? "tag: " : ""}{item.shortName}
              </option>
            ))}
          </select>
          <input
            aria-label="Search commit messages"
            value={draftQuery}
            onChange={(event) => setDraftQuery(event.currentTarget.value)}
            placeholder="Search subject or body"
          />
          <button type="submit">Search</button>
        </form>
        {loading && commits.length === 0 ? (
          <div className="history-state" role="status">Loading history…</div>
        ) : commits.length === 0 ? (
          <div className="history-state">No commits match this filter.</div>
        ) : (
          <div className="commit-list">
            {commits.map((commit) => (
              <button
                type="button"
                key={commit.oid}
                className={selected?.oid === commit.oid ? "commit-row selected" : "commit-row"}
                aria-current={selected?.oid === commit.oid ? "true" : undefined}
                onClick={() => {
                  setSelectedOid(commit.oid);
                  onPersist({ selectedCommit: commit.oid });
                }}
              >
                <span
                  className={`graph-node lane-${commit.lane % 6}`}
                  style={{ "--lane": commit.lane, "--lanes": commit.laneCount } as CSSProperties}
                  aria-label={`Graph lane ${commit.lane + 1} of ${commit.laneCount}`}
                />
                <span className="commit-copy">
                  <strong>{commit.subject}</strong>
                  <span>{commit.authorName} · {relativeTime(commit.authoredAt)}</span>
                </span>
                <span className="commit-refs">
                  {commit.references.slice(0, 2).map((item) => <small key={item}>{shortRef(item)}</small>)}
                </span>
                <code>{commit.oid.slice(0, 8)}</code>
              </button>
            ))}
          </div>
        )}
        {nextCursor && (
          <button className="load-more" type="button" disabled={loading} onClick={loadMore}>
            {loading ? "Loading…" : "Load older commits"}
          </button>
        )}
      </section>

      <aside className="commit-detail" aria-label="Commit details">
        {hasConflicts && (
          <div className="merge-conflict" role="alert">
            Merge stopped with conflicts. Resolve the highlighted files in Changes.
          </div>
        )}
        {selected ? (
          <>
            <span className="eyebrow">{selected.oid}</span>
            <h1>{selected.subject}</h1>
            <p>{selected.authorName} &lt;{selected.authorEmail}&gt;</p>
            <time dateTime={new Date(selected.authoredAt * 1000).toISOString()}>
              {new Date(selected.authoredAt * 1000).toLocaleString()}
            </time>
            {selected.body && <pre>{selected.body}</pre>}
            <div className="detail-refs">
              {selected.references.map((item) => <span key={item}>{shortRef(item)}</span>)}
            </div>
          </>
        ) : (
          <div className="history-state">Select a commit to inspect it.</div>
        )}

        <div className="reference-actions">
          <h2>Reference actions</h2>
          {selectedReference ? (
            <>
              <strong>{selectedReference.shortName}</strong>
              {selectedReference.upstream && (
                <span>
                  {selectedReference.upstream} · ↑{selectedReference.ahead} ↓{selectedReference.behind}
                </span>
              )}
              {selectedReference.kind === "localBranch" && (
                <div>
                  <button
                    type="button"
                    disabled={Boolean(operation) || selectedReference.shortName === currentBranch}
                    onClick={() =>
                      void mutate("Checking out…", () =>
                        checkoutBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    Checkout
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation) || selectedReference.shortName === currentBranch}
                    onClick={() =>
                      window.confirm(`Delete merged branch ${selectedReference.shortName}?`) &&
                      void mutate("Deleting branch…", () =>
                        deleteBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    Delete
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation) || selectedReference.shortName === currentBranch}
                    onClick={() =>
                      window.confirm(`Merge ${selectedReference.shortName} into ${currentBranch ?? "HEAD"}?`) &&
                      void mutate("Merging…", () =>
                        mergeBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    Merge
                  </button>
                </div>
              )}
            </>
          ) : (
            <span>Choose a branch or tag to enable explicit actions.</span>
          )}
          <form
            onSubmit={(event) => {
              event.preventDefault();
              const name = branchName.trim();
              if (!name) return;
              void mutate("Creating branch…", () =>
                createBranch(snapshot.repository.id, snapshot.revision, {
                  name,
                  startPoint: selected?.oid,
                }),
              ).then(() => setBranchName(""));
            }}
          >
            <input
              aria-label="New branch name"
              value={branchName}
              onChange={(event) => setBranchName(event.currentTarget.value)}
              placeholder="new/branch-name"
            />
            <button type="submit" disabled={!branchName.trim() || Boolean(operation)}>
              Create at selected commit
            </button>
          </form>
          {operation && <span role="status">{operation}</span>}
        </div>
      </aside>
    </div>
  );
}

function parseHistoryFilter(value?: string): { reference: string; query: string } {
  if (!value) return { reference: "", query: "" };
  try {
    const parsed = JSON.parse(value) as { reference?: string; query?: string };
    return { reference: parsed.reference ?? "", query: parsed.query ?? "" };
  } catch {
    return { reference: "", query: value };
  }
}

function shortRef(value: string) {
  return value
    .replace("HEAD -> ", "")
    .replace("refs/heads/", "")
    .replace("refs/remotes/", "")
    .replace("refs/tags/", "");
}

function relativeTime(timestamp: number) {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 2_592_000) return `${Math.floor(seconds / 86_400)}d ago`;
  return new Date(timestamp * 1000).toLocaleDateString();
}

function HistoryEmpty() {
  return <div className="history-empty"><div className="history-lines" aria-hidden="true"><i /><i /><i /></div><p className="eyebrow">Commit graph</p><h1>History will appear here.</h1><p>Open a repository to explore commits, branches, tags, and authors.</p></div>;
}
