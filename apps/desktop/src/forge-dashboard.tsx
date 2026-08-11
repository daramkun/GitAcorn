import { useEffect, useMemo, useState } from "react";
import {
  getForgeDashboard,
  normalizeAppError,
  type ForgeAttentionReason,
  type ForgeDashboardDto,
  type ForgeDashboardItemDto,
  type ForgeProvider,
} from "./repository";
import { localeTag, t } from "./i18n";

type Props = {
  onClose: () => void;
  onUnreadChange?: (count: number) => void;
};

type DashboardFilter = "attention" | "mine" | "team" | "all";

const READ_KEY = "git-acorn.forge-dashboard.read.v1";
const providerLabels: Record<ForgeProvider, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
  azureDevOps: "Azure DevOps",
};

export function ForgeDashboard({ onClose, onUnreadChange }: Props) {
  const [dashboard, setDashboard] = useState<ForgeDashboardDto>();
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [filter, setFilter] = useState<DashboardFilter>("attention");
  const [query, setQuery] = useState("");
  const [readIds, setReadIds] = useState<Set<string>>(() => loadReadIds());

  function refresh() {
    setLoading(true);
    setMessage("");
    getForgeDashboard()
      .then(setDashboard)
      .catch((reason: unknown) => setMessage(normalizeAppError(reason).message))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    refresh();
  }, []);

  const unreadCount = useMemo(
    () => dashboard?.items.filter((item) => item.attention && !readIds.has(item.id)).length ?? 0,
    [dashboard, readIds],
  );

  useEffect(() => {
    onUnreadChange?.(unreadCount);
  }, [onUnreadChange, unreadCount]);

  const counts = useMemo(() => {
    const items = dashboard?.items ?? [];
    return {
      pullRequests: items.filter((item) => item.kind === "pullRequest").length,
      issues: items.filter((item) => item.kind === "issue").length,
      ciAttention: items.filter((item) => item.attention === "ciFailed").length,
    };
  }, [dashboard]);

  const filteredItems = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return (dashboard?.items ?? []).filter((item) => {
      if (filter === "attention" && !item.attention) return false;
      if (filter === "mine" && !item.personal) return false;
      if (filter === "team" && item.personal) return false;
      return !normalized || `${item.repositoryName} ${item.title} ${item.author} ${item.accountLogin}`.toLocaleLowerCase().includes(normalized);
    });
  }, [dashboard, filter, query]);

  function markRead(ids: string[]) {
    setReadIds((current) => {
      const next = new Set(current);
      ids.forEach((id) => next.add(id));
      persistReadIds(next);
      return next;
    });
  }

  const attentionIds = dashboard?.items.filter((item) => item.attention).map((item) => item.id) ?? [];

  return (
    <div className="modal-overlay" role="presentation" onClick={onClose}>
      <section className="settings-modal forge-dashboard-modal" role="dialog" aria-modal="true" aria-labelledby="forge-dashboard-title" onClick={(event) => event.stopPropagation()}>
        <header className="settings-modal-header">
          <div>
            <span className="eyebrow">{t("Collaboration")}</span>
            <h2 id="forge-dashboard-title">{t("Team dashboard")}</h2>
            <p>{t("Pull requests, issues, CI, and notifications across connected hosting accounts.")}</p>
          </div>
          <button className="settings-close-btn" type="button" aria-label={t("Close team dashboard")} onClick={onClose}>×</button>
        </header>

        <div className="forge-dashboard-body">
          <div className="forge-dashboard-summary" aria-label={t("Dashboard summary")}>
            <SummaryCard label={t("Pull requests")} value={counts.pullRequests} />
            <SummaryCard label={t("Issues")} value={counts.issues} />
            <SummaryCard label={t("CI needs attention")} value={counts.ciAttention} tone={counts.ciAttention ? "danger" : undefined} />
            <SummaryCard label={t("Unread notifications")} value={unreadCount} tone={unreadCount ? "warning" : undefined} />
          </div>

          <div className="forge-dashboard-toolbar">
            <div className="forge-dashboard-filters" role="tablist" aria-label={t("Dashboard view")}>
              {(["attention", "mine", "team", "all"] as const).map((value) => (
                <button key={value} role="tab" aria-selected={filter === value} className={filter === value ? "active" : ""} type="button" onClick={() => setFilter(value)}>{t(filterLabel(value))}</button>
              ))}
            </div>
            <input className="control-input" type="search" aria-label={t("Search dashboard")} placeholder={t("Search repository, title, or author")} value={query} onChange={(event) => setQuery(event.target.value)} />
            <button className="control-button control-button--secondary" type="button" disabled={loading} onClick={refresh}>{loading ? t("Refreshing…") : t("Refresh")}</button>
            <button className="control-button control-button--secondary" type="button" disabled={!unreadCount} onClick={() => markRead(attentionIds)}>{t("Mark all read")}</button>
          </div>

          {message && <p className="forge-message" role="alert">{message}</p>}
          {dashboard && <p className="forge-dashboard-coverage">{t("Showing {count} recent repositories", { count: dashboard.coveredRepositories })}{dashboard.skippedRepositories ? ` · ${t("{count} older repositories skipped", { count: dashboard.skippedRepositories })}` : ""}</p>}

          <div className="forge-dashboard-list" aria-busy={loading}>
            {loading ? <p className="forge-empty" role="status">{t("Loading team activity…")}</p> : filteredItems.length === 0 ? <p className="forge-empty">{t("No dashboard items match this view.")}</p> : filteredItems.map((item) => (
              <DashboardItem key={item.id} item={item} unread={Boolean(item.attention && !readIds.has(item.id))} onRead={() => markRead([item.id])} />
            ))}
          </div>

          {dashboard && dashboard.failures.length > 0 && (
            <details className="forge-dashboard-failures">
              <summary>{t("{count} sources could not be refreshed", { count: dashboard.failures.length })}</summary>
              <ul>{dashboard.failures.map((failure, index) => <li key={`${failure.accountId}-${failure.repositoryName ?? "account"}-${index}`}><strong>{failure.repositoryName ?? failure.accountId}</strong><span>{failure.message}</span></li>)}</ul>
            </details>
          )}
        </div>
      </section>
    </div>
  );
}

function SummaryCard({ label, value, tone }: { label: string; value: number; tone?: "danger" | "warning" }) {
  return <div className={`forge-dashboard-card${tone ? ` forge-dashboard-card--${tone}` : ""}`}><span>{label}</span><strong>{value}</strong></div>;
}

function DashboardItem({ item, unread, onRead }: { item: ForgeDashboardItemDto; unread: boolean; onRead: () => void }) {
  const webUrl = safeWebUrl(item.webUrl);
  return (
    <article className={`forge-dashboard-item${unread ? " forge-dashboard-item--unread" : ""}`}>
      <span className={`forge-provider-mark forge-provider-mark--${item.provider}`} aria-hidden="true">{providerLabels[item.provider].slice(0, 1)}</span>
      <div className="forge-dashboard-item-main">
        <div className="forge-dashboard-item-title"><strong>{item.kind === "pullRequest" ? "PR" : t("Issue")} #{item.number} · {item.title}</strong>{unread && <span className="forge-dashboard-unread">{t("New")}</span>}</div>
        <span>{item.repositoryName} · {item.author} · {providerLabels[item.provider]}</span>
        <div className="forge-pr-statuses">
          <span className="forge-status-badge">{item.state}</span>
          {item.reviewStatus && <StatusBadge label={t("Review")} value={item.reviewStatus} />}
          {item.ciStatus && <StatusBadge label="CI" value={item.ciStatus} />}
          {item.attention && <span className={`forge-status-badge forge-status-badge--${attentionTone(item.attention)}`}>{t(attentionLabel(item.attention))}</span>}
          <span className="forge-status-badge">{item.personal ? t("Mine") : t("Team")}</span>
        </div>
      </div>
      <div className="forge-dashboard-item-actions">
        {unread && <button className="control-button control-button--secondary" type="button" onClick={onRead}>{t("Mark read")}</button>}
        {webUrl ? <a className="control-button control-button--secondary" href={webUrl} target="_blank" rel="noreferrer" onClick={onRead}>{t("Open")}</a> : <button className="control-button control-button--secondary" type="button" disabled>{t("Open")}</button>}
      </div>
      {item.updatedAt && <time dateTime={item.updatedAt}>{new Intl.DateTimeFormat(localeTag(), { dateStyle: "medium", timeStyle: "short" }).format(new Date(item.updatedAt))}</time>}
    </article>
  );
}

function StatusBadge({ label, value }: { label: string; value: string }) {
  return <span className={`forge-status-badge forge-status-badge--${value}`}>{label}: {t(statusLabel(value))}</span>;
}

const statusLabels = { approved: "Approved", changesRequested: "Changes requested", pending: "Pending", success: "Passed", failure: "Failed", cancelled: "Cancelled", unknown: "Unknown" } as const;
function statusLabel(value: string): (typeof statusLabels)[keyof typeof statusLabels] {
  return statusLabels[value as keyof typeof statusLabels] ?? "Unknown";
}

function filterLabel(filter: DashboardFilter) {
  return ({ attention: "Needs attention", mine: "Mine", team: "Team", all: "All" } as const)[filter];
}

function attentionLabel(reason: ForgeAttentionReason) {
  return ({ changesRequested: "Changes requested", ciFailed: "CI failed", assignedIssue: "Assigned to you" } as const)[reason];
}

function attentionTone(reason: ForgeAttentionReason) {
  return reason === "assignedIssue" ? "pending" : "failure";
}

function loadReadIds() {
  try {
    const value = JSON.parse(localStorage.getItem(READ_KEY) ?? "[]");
    return new Set<string>(Array.isArray(value) ? value.filter((item): item is string => typeof item === "string").slice(-2000) : []);
  } catch {
    return new Set<string>();
  }
}

function persistReadIds(ids: Set<string>) {
  try {
    localStorage.setItem(READ_KEY, JSON.stringify(Array.from(ids).slice(-2000)));
  } catch {
    // Notification read state is optional when browser storage is unavailable.
  }
}

function safeWebUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}
