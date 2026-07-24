import { useEffect, useState } from "react";
import { getAppInfo, type AppInfoDto } from "./app-info";

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
  const [page, setPage] = useState<Page>("changes");
  const [appInfo, setAppInfo] = useState<AppInfoState>({ status: "loading" });

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

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="brand">
          <span className="acorn-mark" aria-hidden="true">
            <span />
          </span>
          <span>GitAcorn</span>
          <span className="alpha-badge">ALPHA</span>
        </div>
        <div className="window-drag-region" />
      </header>

      <div className="tabbar" aria-label="Repository tabs">
        <div className="tabbar-empty">No repositories open</div>
        <button className="open-button" type="button" disabled title="Available in M1">
          <span aria-hidden="true">＋</span> Open repository
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

          <div className="sidebar-groups" aria-hidden="true">
            {["Worktrees", "Branches", "Tags", "Stashes"].map((label) => (
              <div className="sidebar-group" key={label}>
                <span>›</span>
                {label}
              </div>
            ))}
          </div>

          <div className="runtime-status" role="status">
            <span className={appInfo.status === "error" ? "status-dot error" : "status-dot"} />
            {appInfo.status === "loading" && "Connecting to core…"}
            {appInfo.status === "ready" &&
              `${appInfo.value.runtime} · v${appInfo.value.version}`}
            {appInfo.status === "error" && "Core unavailable"}
          </div>
        </aside>

        <section className="content" aria-live="polite">
          <div className="contextbar">
            <div>
              <span className="eyebrow">Local workspace</span>
              <strong>{page === "changes" ? "Changes" : "History"}</strong>
            </div>
            <div className="remote-actions" aria-label="Remote actions">
              <button type="button" disabled>Fetch</button>
              <button type="button" disabled>Pull</button>
              <button type="button" disabled>Push</button>
            </div>
          </div>

          {appInfo.status === "error" && (
            <div className="error-banner" role="alert">
              <strong>Could not reach the GitAcorn core.</strong>
              <span>{appInfo.message}</span>
            </div>
          )}

          {page === "changes" ? <ChangesEmpty /> : <HistoryEmpty />}
        </section>
      </main>
    </div>
  );
}

function ChangesEmpty() {
  return (
    <div className="changes-layout">
      <section className="file-panel" aria-label="Changed files">
        <div className="change-section">
          <div className="panel-heading">
            <h2>Unstaged</h2>
            <span>0</span>
          </div>
          <div className="panel-empty">Open a repository to inspect working tree changes.</div>
        </div>
        <div className="change-section">
          <div className="panel-heading">
            <h2>Staged</h2>
            <span>0</span>
          </div>
          <div className="panel-empty">No staged changes.</div>
        </div>
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
        <p>
          Open a local repository to review changes, stage precise edits, and keep every
          operation explainable.
        </p>
        <button type="button" disabled title="Repository discovery arrives in M1">
          Open a repository
        </button>
        <small>Repository discovery arrives in the next milestone.</small>
      </section>
      <aside className="commit-panel" aria-label="Commit form preview">
        <div className="panel-heading">
          <h2>Commit</h2>
        </div>
        <textarea aria-label="Commit summary" placeholder="Summary" disabled />
        <textarea aria-label="Commit description" placeholder="Description (optional)" disabled />
        <button type="button" disabled>Commit</button>
      </aside>
    </div>
  );
}

function HistoryEmpty() {
  return (
    <div className="history-empty">
      <div className="history-lines" aria-hidden="true">
        <i />
        <i />
        <i />
      </div>
      <p className="eyebrow">Commit graph</p>
      <h1>History will appear here.</h1>
      <p>Open a repository to explore commits, branches, tags, and authors.</p>
    </div>
  );
}
