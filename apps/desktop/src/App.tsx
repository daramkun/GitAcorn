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
  layoutCommitGraph,
  type CommitGraphRow,
  type GraphSegment,
} from "./commitGraph";
import {
  closeAppWindow,
  minimizeAppWindow,
  toggleMaximizeAppWindow,
} from "./windowControls";
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
import { localeTag, t } from "./i18n";

type Page = "changes" | "history" | "operations";
type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfoDto }
  | { status: "error"; message: string };

const navigation: ReadonlyArray<{ id: Page; label: string; shortcut: string }> = [
  { id: "changes", label: t("Changes"), shortcut: "⌘1" },
  { id: "history", label: t("History"), shortcut: "⌘2" },
  { id: "operations", label: t("Operations"), shortcut: "⌘3" },
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
    document.documentElement.lang = localeTag();
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
        ? `${t("Detached")} ${activeSnapshot.head.oid?.slice(0, 8) ?? ""}`
        : t("Unborn branch")
    : undefined;

  return (
    <div className="app-shell">
      <header className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="acorn-mark" aria-hidden="true"><span /></span>
          <span>GitAcorn</span><span className="alpha-badge">ALPHA</span>
        </div>
        <div className="window-drag-region" data-tauri-drag-region />
        <div className="window-controls">
          <button
            className="window-control"
            type="button"
            aria-label={t("Minimize window")}
            onClick={() => runWindowCommand(minimizeAppWindow)}
          >
            <span className="window-control-icon minimize" aria-hidden="true" />
          </button>
          <button
            className="window-control"
            type="button"
            aria-label={t("Maximize or restore window")}
            onClick={() => runWindowCommand(toggleMaximizeAppWindow)}
          >
            <span className="window-control-icon maximize" aria-hidden="true" />
          </button>
          <button
            className="window-control close"
            type="button"
            aria-label={t("Close window")}
            onClick={() => runWindowCommand(closeAppWindow)}
          >
            <span className="window-control-icon close" aria-hidden="true" />
          </button>
        </div>
      </header>

      <div className="tabbar" aria-label={t("Repository tabs")}>
        <div className="repository-tabs">
          {tabs.length === 0 && (
            <div className="tabbar-empty">
              {sessionLoading ? t("Restoring session…") : t("No repositories open")}
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
                <button type="button" aria-label={t("Move {name} left", { name: repositoryName(tab.worktreePath) })} disabled={index === 0} onClick={() => moveTab(tab.repoId, -1)}>‹</button>
                <button type="button" aria-label={t("Move {name} right", { name: repositoryName(tab.worktreePath) })} disabled={index === tabs.length - 1} onClick={() => moveTab(tab.repoId, 1)}>›</button>
                <button type="button" aria-label={t("Close {name}", { name: repositoryName(tab.worktreePath) })} onClick={() => closeTab(tab.repoId)}>×</button>
              </div>
            </div>
          ))}
        </div>
        <button className="open-button" type="button" disabled={opening} onClick={handleOpenRepository}>
          <span aria-hidden="true">＋</span>{" "}{opening ? t("Opening…") : t("Open a repository")}
        </button>
        <button className="open-button" type="button" onClick={() => setShowClone((value) => !value)}>
          {t("Clone")}
        </button>
      </div>

      <main className="workspace">
        <aside className="sidebar">
          <nav aria-label={t("Repository navigation")}>
            <p className="section-label">{t("Workspace")}</p>
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
            <SidebarGroup label={t("Worktrees")} count={activeSidebar?.worktrees.length}>
              {activeSidebar?.worktrees.map((worktree) => (
                <button
                  type="button"
                  key={worktree.id}
                  title={worktree.path}
                  aria-current={worktree.id === activeTab?.worktreeId ? "true" : undefined}
                  onClick={() => handleWorktreeActivate(worktree.id)}
                >
                  {worktree.isCurrent ? "● " : ""}{worktree.branch ?? t("Detached")}
                  {worktree.isLocked ? ` · ${t("locked")}` : ""}
                </button>
              ))}
            </SidebarGroup>
            <SidebarGroup label={t("Branches")} count={activeSidebar?.branches.total}>
              {activeSidebar?.branches.items.map((branch) => (
                <span key={branch}>{branch === branchLabel ? "● " : ""}{branch}</span>
              ))}
            </SidebarGroup>
            <SidebarGroup label={t("Tags")} count={activeSidebar?.tags.total}>
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
            {appInfo.status === "loading" && t("Connecting to core…")}
            {appInfo.status === "ready" && `${appInfo.value.runtime} · v${appInfo.value.version}`}
            {appInfo.status === "error" && t("Core unavailable")}
          </div>
        </aside>

        <section className="content" aria-live="polite">
          <div className="contextbar">
            <div>
              <span className="eyebrow">{activeTab?.worktreePath ?? t("Local workspace")}</span>
              <strong>{activeSnapshot ? `${branchLabel} · ${navigation.find((item) => item.id === page)?.label}` : navigation.find((item) => item.id === page)?.label}</strong>
            </div>
            <div className="remote-actions" aria-label={t("Remote actions")}>
              {activeTab && refreshing.has(activeTab.repoId) && <span className="refreshing">{t("Refreshing…")}</span>}
              {activeTab && remoteOperations[activeTab.repoId] &&
                ["queued", "running"].includes(remoteOperations[activeTab.repoId].state) ? (
                  <>
                    <span className="refreshing" role="status">
                      {operationTerm(remoteOperations[activeTab.repoId].kind)} · {remoteOperations[activeTab.repoId].message ?? operationTerm(remoteOperations[activeTab.repoId].state)}
                    </span>
                    <button type="button" onClick={() => cancelOperation(remoteOperations[activeTab.repoId].operationId).catch((reason: unknown) => setError(normalizeAppError(reason)))}>{t("Cancel")}</button>
                  </>
                ) : (
                  <>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("fetch")}>{t("Fetch")}</button>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("pull")}>{t("Pull")}{activeSnapshot?.behind ? ` ${activeSnapshot.behind}` : ""}</button>
                    <button type="button" disabled={!activeTab} onClick={() => handleRemote("push")}>{t("Push")}{activeSnapshot?.ahead ? ` ${activeSnapshot.ahead}` : ""}</button>
                    <button type="button" disabled={!activeTab} title={t("Reject if the remote changed since the last fetch")} onClick={() => handleRemote("push", true)}>{t("Push with lease")}</button>
                  </>
                )}
            </div>
          </div>
          {showClone && (
            <form className="clone-bar" onSubmit={(event) => { event.preventDefault(); void handleClone(); }}>
              <label htmlFor="clone-url">{t("Repository URL")}</label>
              <input id="clone-url" value={cloneUrl} onChange={(event) => setCloneUrl(event.target.value)} placeholder="https://host/owner/repository.git or git@host:owner/repository.git" />
              {cloneOperation && ["queued", "running"].includes(cloneOperation.state) ? (
                <>
                  <span role="status">{cloneOperation.message ?? t("Cloning…")}</span>
                  <button type="button" onClick={() => cancelOperation(cloneOperation.operationId)}>{t("Cancel")}</button>
                </>
              ) : (
                <button type="submit" disabled={!cloneUrl.trim()}>{t("Choose destination and clone")}</button>
              )}
            </form>
          )}
          {appInfo.status === "error" && <ErrorBanner title={t("Could not reach the GitAcorn core.")} message={appInfo.message} />}
          {error && <ErrorBanner title={t("Repository session needs attention.")} message={error.message} detail={error.details} actionLabel={error.code === "repositoryNotFound" ? t("Choose another folder") : undefined} onAction={handleOpenRepository} />}
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
  return <div className="sidebar-read-group"><div className="sidebar-group"><span>›</span>{label}{count !== undefined && <small>{t("{visible} of {total}", { visible: Math.min(count, 5), total: count })}</small>}</div>{children && <div className="sidebar-items">{children}</div>}</div>;
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
        <span>›</span>{t("Stashes")}
        {snapshot && <small>{t("{visible} of {total}", { visible: Math.min(snapshot.stashCount, 5), total: snapshot.stashCount })}</small>}
      </div>
      {snapshot && (
        <form
          aria-label={t("Create stash")}
          onSubmit={(event) => {
            event.preventDefault();
            onCreate(message, includeUntracked);
            setMessage("");
          }}
        >
          <input
            aria-label={t("Stash message")}
            placeholder={t("Stash message")}
            value={message}
            onChange={(event) => setMessage(event.currentTarget.value)}
          />
          <label>
            <input
              type="checkbox"
              checked={includeUntracked}
              onChange={(event) => setIncludeUntracked(event.currentTarget.checked)}
            />
            {t("Include untracked")}
          </label>
          <button type="submit" disabled={snapshot.changes.length === 0}>{t("Stash changes")}</button>
        </form>
      )}
      <div className="sidebar-items">
        {stashes.map((stash) => (
          <div className="stash-item" key={stash.reference} title={stash.message}>
            <span>{stash.reference} · {stash.message}</span>
            <div>
              <button type="button" onClick={() => onApply(stash.reference)}>{t("Apply")}</button>
              <button
                type="button"
                className="danger-button"
                onClick={() => {
                  if (window.confirm(t("Drop {reference}? The stash entry cannot be recovered.", { reference: stash.reference }))) {
                    onDrop(stash.reference);
                  }
                }}
              >
                {t("Drop")}
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
      setCopyState(t("Diagnostics copied"));
    } catch (reason: unknown) {
      onError(reason);
    }
  }

  return (
    <section className="operations-view" aria-labelledby="operations-title">
      <div className="operations-heading">
        <div>
          <span className="eyebrow">{t("Recovery and diagnostics")}</span>
          <h1 id="operations-title">{t("Operation center")}</h1>
        </div>
        <div>
          <button type="button" onClick={refresh}>{t("Refresh")}</button>
          <button type="button" onClick={() => void copyDiagnostics()}>{t("Copy diagnostics")}</button>
        </div>
      </div>
      {copyState && <p role="status">{copyState}</p>}
      {loading ? (
        <div className="history-state" role="status">{t("Loading operations…")}</div>
      ) : operations.length === 0 ? (
        <div className="history-state">{t("No operations have been recorded.")}</div>
      ) : (
        <ol className="operation-list">
          {operations.map((operation) => (
            <li key={operation.id}>
              <span className={`operation-state ${operation.state}`}>{operationTerm(operation.state)}</span>
              <div>
                <strong>{operationTerm(operation.kind)}</strong>
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
  const name = repositoryName(tab.worktreePath);
  return <div className="welcome-panel"><p className="eyebrow">{t("Repository unavailable")}</p><h1>{t("{name} moved or was deleted.", { name })}</h1><p>{tab.worktreePath}</p><button type="button" onClick={onLocate}>{t("Locate repository")}</button></div>;
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
    void mutate(selectedTarget === "staged" ? t("Unstaging lines…") : t("Staging lines…"), () =>
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
        t("Discard the displayed working-tree changes in {path}? This cannot be undone by GitAcorn.", { path: selected.path }),
      )
    ) {
      return;
    }
    void mutate(t("Discarding…"), () =>
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
    void mutate(amend ? t("Amending…") : t("Committing…"), () =>
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
      <section className="file-panel" aria-label={t("Changed files")}>
        <label className="panel-width-control">
          <span>{t("File panel width")}</span>
          <input
            aria-label={t("Changed files panel width")}
            type="range"
            min="190"
            max="420"
            value={panelWidth}
            onChange={(event) => onPanelWidth(Number(event.currentTarget.value))}
          />
        </label>
        <ChangeSection
          title={t("Unstaged")}
          target="unstaged"
          changes={unstaged}
          selectedPath={selectedPath}
          selectedTarget={selectedTarget}
          onSelect={onSelect}
        />
        <ChangeSection
          title={t("Staged")}
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
                  {selectedTarget === "staged" ? t("Staged diff") : t("Unstaged diff")}
                </span>
                <strong>{selected.path}</strong>
              </div>
              {selected.conflict ? (
                <div className="conflict-actions" aria-label={t("Conflict resolution")}>
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate(t("Using our version…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "ours",
                        ),
                      )
                    }
                  >
                    {t("Use ours")}
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate(t("Using their version…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "theirs",
                        ),
                      )
                    }
                  >
                    {t("Use theirs")}
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={() =>
                      void mutate(t("Marking resolved…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "markResolved",
                        ),
                      )
                    }
                  >
                    {t("Mark current content resolved")}
                  </button>
                  <button
                    type="button"
                    className="danger-button"
                    disabled={Boolean(operation)}
                    onClick={() => {
                      if (window.confirm(t("Abort this merge and restore the pre-merge working tree?"))) {
                        void mutate(t("Aborting merge…"), () =>
                          abortMerge(snapshot.repository.id, snapshot.revision),
                        );
                      }
                    }}
                  >
                    {t("Abort merge…")}
                  </button>
                </div>
              ) : (
              <div>
                <button
                  type="button"
                  disabled={Boolean(operation)}
                  onClick={() =>
                    void mutate(
                      selectedTarget === "staged" ? t("Unstaging file…") : t("Staging file…"),
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
                  {selectedTarget === "staged" ? t("Unstage file") : t("Stage file")}
                </button>
                <button
                  type="button"
                  disabled={selectedLines.size === 0 || Boolean(operation)}
                  onClick={applyLines}
                >
                  {selectedTarget === "staged" ? t("Unstage selected lines") : t("Stage selected lines")}
                </button>
                {selectedTarget === "unstaged" && (
                  <button
                    className="danger-button"
                    type="button"
                    disabled={Boolean(operation)}
                    onClick={discardSelected}
                  >
                    {t("Discard…")}
                  </button>
                )}
              </div>
              )}
            </div>
            {operation && <div className="operation-status" role="status">{operation}</div>}
            {selected.conflict ? (
              <div className="conflict-panel" role="region" aria-label={t("Conflict resolution guidance")}>
                <h2>{t("Resolve merge conflict")}</h2>
                <p>
                  {t("Choose one side, or edit the file in your editor and mark the current content resolved. Aborting restores the state from before the merge.")}
                </p>
              </div>
            ) : diffLoading ? (
              <div className="diff-state" role="status">{t("Loading diff…")}</div>
            ) : diff?.binary ? (
              <div className="diff-state">{t("Binary file. Use the whole-file action.")}</div>
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
                    selectedTarget === "staged" ? t("Unstaging hunk…") : t("Staging hunk…"),
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
                actionLabel={selectedTarget === "staged" ? t("Unstage hunk") : t("Stage hunk")}
              />
            ) : (
              <div className="diff-state">{t("No text diff is available for this side.")}</div>
            )}
          </>
        ) : (
          <div className="empty-selection">
            <span className="file-glyph" aria-hidden="true" />
            <h1>
              {snapshot.changes.length === 0
                ? t("Working tree clean")
                : t("Select a changed file")}
            </h1>
            <p>
              {snapshot.changes.length === 0
                ? t("There are no staged or unstaged changes.")
                : t("Choose a file to inspect and stage its diff.")}
            </p>
          </div>
        )}
      </section>
      <aside className="commit-panel" aria-label={t("Commit form")}>
        <div className="panel-heading"><h2>{t("Commit")}</h2><span>{staged.length}</span></div>
        <textarea
          aria-label={t("Commit summary")}
          placeholder={t("Summary")}
          value={summary}
          onChange={(event) => setSummary(event.currentTarget.value)}
        />
        <textarea
          aria-label={t("Commit description")}
          placeholder={t("Description (optional)")}
          value={description}
          onChange={(event) => setDescription(event.currentTarget.value)}
        />
        <label className="amend-control">
          <input
            type="checkbox"
            checked={amend}
            onChange={(event) => setAmend(event.currentTarget.checked)}
          />
          {t("Amend previous commit")}
        </label>
        <button
          className="primary-action"
          type="button"
          disabled={!summary.trim() || (!amend && staged.length === 0) || Boolean(operation)}
          onClick={submitCommit}
        >
          {t("{action} to {branch}", {
            action: amend ? t("Amend") : t("Commit"),
            branch: snapshot.head.name ?? "HEAD",
          })}
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
        <div className="panel-empty">
          {target === "staged" ? t("No staged changes.") : t("No unstaged changes.")}
        </div>
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
                {partial && <small>{t("partial")}</small>}
                {!partial && change.submodule && <small>{t("submodule")}</small>}
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
    <div className="diff-scroll" aria-label={t("File diff")}>
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
  return <div className="changes-layout"><section className="file-panel" aria-label={t("Changed files")}><ChangeSection title={t("Unstaged")} target="unstaged" changes={[]} onSelect={() => undefined} /><ChangeSection title={t("Staged")} target="staged" changes={[]} onSelect={() => undefined} /></section><section className="welcome-panel"><div className="welcome-art" aria-hidden="true"><span className="branch-line" /><span className="branch-node node-one" /><span className="branch-node node-two" /><span className="branch-node node-three" /></div><p className="eyebrow">{t("A calmer Git workflow")}</p><h1>{t("Your repositories, clearly in view.")}</h1><p>{t("Open a local repository to inspect its real staged, unstaged, and untracked changes.")}</p><button type="button" onClick={onOpen} disabled={opening}>{opening ? t("Opening…") : t("Open a repository")}</button><small>{t("Git 2.40.0 or newer is required.")}</small></section><aside className="commit-panel" aria-label={t("Commit form preview")}><div className="panel-heading"><h2>{t("Commit")}</h2></div><textarea aria-label={t("Commit summary")} placeholder={t("Summary")} disabled /><textarea aria-label={t("Commit description")} placeholder={t("Description (optional)")} disabled /><button type="button" disabled>{t("Commit")}</button></aside></div>;
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
  const graph = useMemo(
    () =>
      layoutCommitGraph(
        query
          ? commits.map((commit) => ({ oid: commit.oid, parents: [] }))
          : commits,
      ),
    [commits, query],
  );
  const graphWidth = Math.max(44, graph.laneCount * 16 + 16);

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
      <section className="history-list-panel" aria-label={t("Commit history")}>
        <form
          className="history-filterbar"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery(draftQuery.trim());
            persistFilter(reference, draftQuery.trim());
          }}
        >
          <select
            aria-label={t("Branch or tag reference")}
            value={reference}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setReference(value);
              persistFilter(value, query);
            }}
          >
            <option value="">{t("All branches and tags")}</option>
            {references.map((item) => (
              <option key={item.fullName} value={item.fullName}>
                {item.kind === "tag" ? t("tag: ") : ""}{item.shortName}
              </option>
            ))}
          </select>
          <input
            aria-label={t("Search commit messages")}
            value={draftQuery}
            onChange={(event) => setDraftQuery(event.currentTarget.value)}
            placeholder={t("Search subject or body")}
          />
          <button type="submit">{t("Search")}</button>
        </form>
        {loading && commits.length === 0 ? (
          <div className="history-state" role="status">{t("Loading history…")}</div>
        ) : commits.length === 0 ? (
          <div className="history-state">{t("No commits match this filter.")}</div>
        ) : (
          <div
            className="commit-list"
            style={{ "--graph-width": `${graphWidth}px` } as CSSProperties}
          >
            {commits.map((commit, index) => (
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
                <CommitGraph
                  row={graph.rows[index]}
                  width={graphWidth}
                  label={t("Graph lane {lane} of {total}", {
                    lane: graph.rows[index].nodeLane + 1,
                    total: graph.rows[index].laneCount,
                  })}
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
            {loading ? t("Loading…") : t("Load older commits")}
          </button>
        )}
      </section>

      <aside className="commit-detail" aria-label={t("Commit details")}>
        {hasConflicts && (
          <div className="merge-conflict" role="alert">
            {t("Merge stopped with conflicts. Resolve the highlighted files in Changes.")}
          </div>
        )}
        {selected ? (
          <>
            <span className="eyebrow">{selected.oid}</span>
            <h1>{selected.subject}</h1>
            <p>{selected.authorName} &lt;{selected.authorEmail}&gt;</p>
            <time dateTime={new Date(selected.authoredAt * 1000).toISOString()}>
              {new Date(selected.authoredAt * 1000).toLocaleString(localeTag())}
            </time>
            {selected.body && <pre>{selected.body}</pre>}
            <div className="detail-refs">
              {selected.references.map((item) => <span key={item}>{shortRef(item)}</span>)}
            </div>
          </>
        ) : (
          <div className="history-state">{t("Select a commit to inspect it.")}</div>
        )}

        <div className="reference-actions">
          <h2>{t("Reference actions")}</h2>
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
                      void mutate(t("Checking out…"), () =>
                        checkoutBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    {t("Checkout")}
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation) || selectedReference.shortName === currentBranch}
                    onClick={() =>
                      window.confirm(t("Delete merged branch {branch}?", { branch: selectedReference.shortName })) &&
                      void mutate(t("Deleting branch…"), () =>
                        deleteBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    {t("Delete")}
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(operation) || selectedReference.shortName === currentBranch}
                    onClick={() =>
                      window.confirm(t("Merge {branch} into {target}?", { branch: selectedReference.shortName, target: currentBranch ?? "HEAD" })) &&
                      void mutate(t("Merging…"), () =>
                        mergeBranch(
                          snapshot.repository.id,
                          snapshot.revision,
                          selectedReference.shortName,
                        ),
                      )
                    }
                  >
                    {t("Merge")}
                  </button>
                </div>
              )}
            </>
          ) : (
            <span>{t("Choose a branch or tag to enable explicit actions.")}</span>
          )}
          <form
            onSubmit={(event) => {
              event.preventDefault();
              const name = branchName.trim();
              if (!name) return;
              void mutate(t("Creating branch…"), () =>
                createBranch(snapshot.repository.id, snapshot.revision, {
                  name,
                  startPoint: selected?.oid,
                }),
              ).then(() => setBranchName(""));
            }}
          >
            <input
              aria-label={t("New branch name")}
              value={branchName}
              onChange={(event) => setBranchName(event.currentTarget.value)}
              placeholder="new/branch-name"
            />
            <button type="submit" disabled={!branchName.trim() || Boolean(operation)}>
              {t("Create at selected commit")}
            </button>
          </form>
          {operation && <span role="status">{operation}</span>}
        </div>
      </aside>
    </div>
  );
}

const GRAPH_ROW_HEIGHT = 56;
const GRAPH_NODE_Y = GRAPH_ROW_HEIGHT / 2;
const GRAPH_LANE_GAP = 16;
const GRAPH_LANE_OFFSET = 12;

function CommitGraph({
  row,
  width,
  label,
}: {
  row: CommitGraphRow;
  width: number;
  label: string;
}) {
  const laneX = (lane: number) => GRAPH_LANE_OFFSET + lane * GRAPH_LANE_GAP;

  return (
    <svg
      className="commit-graph"
      viewBox={`0 0 ${width} ${GRAPH_ROW_HEIGHT}`}
      role="img"
      aria-label={label}
    >
      {row.segments.map((segment, index) => (
        <path
          key={`${segment.from}-${segment.to}-${segment.fromLane}-${segment.toLane}-${index}`}
          className={`graph-edge graph-color-${segment.color % 8}`}
          d={graphSegmentPath(segment, laneX)}
        />
      ))}
      <circle
        className={`graph-commit-node graph-color-${row.nodeColor % 8}`}
        cx={laneX(row.nodeLane)}
        cy={GRAPH_NODE_Y}
        r="5"
      />
    </svg>
  );
}

function graphSegmentPath(
  segment: GraphSegment,
  laneX: (lane: number) => number,
) {
  const startY = segment.from === "top" ? 0 : GRAPH_NODE_Y;
  const endY = segment.to === "node" ? GRAPH_NODE_Y : GRAPH_ROW_HEIGHT;
  const startX = laneX(segment.fromLane);
  const endX = laneX(segment.toLane);
  const middleY = (startY + endY) / 2;
  return `M ${startX} ${startY} C ${startX} ${middleY}, ${endX} ${middleY}, ${endX} ${endY}`;
}

function runWindowCommand(command: () => Promise<void>) {
  void command().catch((reason: unknown) => {
    console.error("Window command failed", reason);
  });
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
  if (seconds < 60) return t("just now");
  if (seconds < 3600) return t("{count}m ago", { count: Math.floor(seconds / 60) });
  if (seconds < 86_400) return t("{count}h ago", { count: Math.floor(seconds / 3600) });
  if (seconds < 2_592_000) return t("{count}d ago", { count: Math.floor(seconds / 86_400) });
  return new Date(timestamp * 1000).toLocaleDateString(localeTag());
}

function operationTerm(value: string): string {
  switch (value.toLowerCase()) {
    case "fetch": return t("Fetch");
    case "pull": return t("Pull");
    case "push": return t("Push");
    case "clone": return t("clone");
    case "queued": return t("queued");
    case "running": return t("running");
    case "succeeded": return t("succeeded");
    case "failed": return t("failed");
    case "cancelled": return t("cancelled");
    case "interrupted": return t("interrupted");
    default: return value;
  }
}

function HistoryEmpty() {
  return <div className="history-empty"><div className="history-lines" aria-hidden="true"><i /><i /><i /></div><p className="eyebrow">{t("Commit graph")}</p><h1>{t("History will appear here.")}</h1><p>{t("Open a repository to explore commits, branches, tags, and authors.")}</p></div>;
}
