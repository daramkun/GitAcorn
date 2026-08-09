import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  connectForgeAccount,
  disconnectForgeAccount,
  getForgeAccounts,
  getForgeRepositories,
  normalizeAppError,
  type ForgeAccountDto,
  type ForgeProvider,
  type ForgeRepositoryDto,
} from "./repository";
import { localeTag, t } from "./i18n";

type Props = {
  onClose: () => void;
  onClone: (url: string) => void;
};

const providerDefaults: Record<ForgeProvider, string> = {
  github: "github.com",
  gitlab: "gitlab.com",
  bitbucket: "bitbucket.org",
  azureDevOps: "dev.azure.com",
};

const providerLabels: Record<ForgeProvider, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
  azureDevOps: "Azure DevOps",
};

export function ForgeBrowser({ onClose, onClone }: Props) {
  const [accounts, setAccounts] = useState<ForgeAccountDto[]>([]);
  const [selectedId, setSelectedId] = useState<string>();
  const [repositories, setRepositories] = useState<ForgeRepositoryDto[]>([]);
  const [loadingAccounts, setLoadingAccounts] = useState(true);
  const [loadingRepositories, setLoadingRepositories] = useState(false);
  const [showAdd, setShowAdd] = useState(false);
  const [provider, setProvider] = useState<ForgeProvider>("github");
  const [host, setHost] = useState(providerDefaults.github);
  const [scope, setScope] = useState("");
  const [authUsername, setAuthUsername] = useState("");
  const [token, setToken] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [pendingDisconnect, setPendingDisconnect] = useState(false);
  const [query, setQuery] = useState("");
  const [message, setMessage] = useState("");

  const selected = accounts.find((account) => account.id === selectedId);
  const filteredRepositories = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return repositories;
    return repositories.filter((repository) =>
      `${repository.name} ${repository.fullName}`.toLocaleLowerCase().includes(normalized),
    );
  }, [query, repositories]);

  useEffect(() => {
    let active = true;
    getForgeAccounts()
      .then(({ accounts: loaded }) => {
        if (!active) return;
        setAccounts(loaded);
        setShowAdd(loaded.length === 0);
        setSelectedId(loaded[0]?.id);
      })
      .catch((reason: unknown) => active && setMessage(normalizeAppError(reason).message))
      .finally(() => active && setLoadingAccounts(false));
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!selectedId || showAdd) {
      setRepositories([]);
      return;
    }
    let active = true;
    setLoadingRepositories(true);
    setMessage("");
    getForgeRepositories(selectedId)
      .then(({ repositories: loaded }) => active && setRepositories(loaded))
      .catch((reason: unknown) => active && setMessage(normalizeAppError(reason).message))
      .finally(() => active && setLoadingRepositories(false));
    return () => {
      active = false;
    };
  }, [selectedId, showAdd]);

  function changeProvider(next: ForgeProvider) {
    setProvider(next);
    setHost(providerDefaults[next]);
    setScope("");
    setAuthUsername("");
    setToken("");
    setMessage("");
  }

  async function connect(event: FormEvent) {
    event.preventDefault();
    if (!host.trim() || !token || (needsScope(provider) && !scope.trim())) return;
    try {
      setConnecting(true);
      setMessage("");
      const account = await connectForgeAccount({
        provider,
        host: host.trim(),
        authUsername: authUsername.trim(),
        token,
        scope: scope.trim() || undefined,
      });
      setAccounts((current) => [
        ...current.filter((item) => item.id !== account.id),
        account,
      ]);
      setSelectedId(account.id);
      setShowAdd(false);
      setToken("");
      setMessage(t("Account connected."));
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setConnecting(false);
    }
  }

  async function disconnect() {
    if (!selected) return;
    try {
      setDisconnecting(true);
      setMessage("");
      await disconnectForgeAccount(selected.id);
      const next = accounts.filter((account) => account.id !== selected.id);
      setAccounts(next);
      setSelectedId(next[0]?.id);
      setShowAdd(next.length === 0);
      setPendingDisconnect(false);
      setRepositories([]);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setDisconnecting(false);
    }
  }

  async function refreshRepositories() {
    if (!selectedId) return;
    try {
      setLoadingRepositories(true);
      setMessage("");
      const response = await getForgeRepositories(selectedId);
      setRepositories(response.repositories);
    } catch (reason: unknown) {
      setMessage(normalizeAppError(reason).message);
    } finally {
      setLoadingRepositories(false);
    }
  }

  return (
    <div className="modal-overlay" role="presentation" onClick={onClose}>
      <section
        className="settings-modal forge-browser-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="forge-browser-title"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="settings-modal-header">
          <div>
            <h2 id="forge-browser-title">{t("Hosted repositories")}</h2>
            <p>{t("Browse repositories from your Git hosting accounts.")}</p>
          </div>
          <button className="settings-close-btn" type="button" aria-label={t("Close hosted repositories")} onClick={onClose}>×</button>
        </header>
        <div className="forge-browser-layout">
          <aside className="forge-account-pane" aria-label={t("Hosting accounts")}>
            <div className="forge-pane-heading">
              <span>{t("Accounts")}</span>
              <button type="button" onClick={() => { setShowAdd(true); setPendingDisconnect(false); setMessage(""); }} aria-label={t("Add hosting account")}>＋</button>
            </div>
            {loadingAccounts ? (
              <p className="forge-empty" role="status">{t("Loading accounts…")}</p>
            ) : accounts.length === 0 ? (
              <p className="forge-empty">{t("No hosting accounts connected.")}</p>
            ) : (
              <div className="forge-account-list">
                {accounts.map((account) => (
                  <button
                    className={account.id === selectedId && !showAdd ? "forge-account active" : "forge-account"}
                    key={account.id}
                    type="button"
                    onClick={() => { setSelectedId(account.id); setShowAdd(false); setPendingDisconnect(false); setQuery(""); }}
                  >
                    <span className={`forge-provider-mark forge-provider-mark--${account.provider}`} aria-hidden="true">{providerLabels[account.provider].slice(0, 1)}</span>
                    <span><strong>{account.displayName}</strong><small>{providerLabels[account.provider]} · {account.scope ?? account.login}</small></span>
                  </button>
                ))}
              </div>
            )}
            <button className="control-button control-button--secondary forge-add-account" type="button" onClick={() => { setShowAdd(true); setPendingDisconnect(false); setMessage(""); }}>＋ {t("Add account")}</button>
          </aside>
          <div className="forge-content-pane">
            {showAdd ? (
              <form className="forge-connect-form" onSubmit={connect}>
                <div className="forge-content-heading">
                  <div><h3>{t("Connect hosting account")}</h3><p>{t("Verify a token over HTTPS and keep it in Git Credential Manager.")}</p></div>
                </div>
                <div className="forge-form-grid">
                  <label><span>{t("Provider")}</span><select className="control-input" value={provider} onChange={(event) => changeProvider(event.target.value as ForgeProvider)}><option value="github">GitHub</option><option value="gitlab">GitLab</option><option value="bitbucket">Bitbucket</option><option value="azureDevOps">Azure DevOps</option></select></label>
                  <label><span>{t("Host")}</span><input className="control-input" value={host} onChange={(event) => setHost(event.target.value)} autoCapitalize="none" spellCheck={false} /></label>
                  {needsScope(provider) && <label><span>{provider === "bitbucket" ? t("Workspace") : t("Organization")}</span><input className="control-input" value={scope} onChange={(event) => setScope(event.target.value)} autoCapitalize="none" spellCheck={false} /></label>}
                  <label><span>{t("Authentication username")}{!requiresUsername(provider) && ` (${t("optional")})`}</span><input className="control-input" value={authUsername} onChange={(event) => setAuthUsername(event.target.value)} autoCapitalize="none" spellCheck={false} /></label>
                  <label className="forge-token-field"><span>{t("Access token")}</span><input className="control-input" type="password" value={token} onChange={(event) => setToken(event.target.value)} autoComplete="off" /></label>
                </div>
                <p className="init-repository-note">{t("GitAcorn does not store tokens in its database. Git's configured credential helper stores them for this Windows user.")}</p>
                {message && <p className="forge-message" role="status">{message}</p>}
                <div className="forge-form-actions"><button className="control-button control-button--secondary" type="button" disabled={connecting} onClick={() => accounts.length ? setShowAdd(false) : onClose()}>{t("Cancel")}</button><button className="control-button control-button--primary" type="submit" disabled={connecting || !host.trim() || !token || (needsScope(provider) && !scope.trim()) || (requiresUsername(provider) && !authUsername.trim())}>{connecting ? t("Connecting…") : t("Connect account")}</button></div>
              </form>
            ) : selected ? (
              <>
                <div className="forge-content-heading">
                  <div><h3>{selected.displayName}</h3><p>{providerLabels[selected.provider]} · {selected.host}{selected.scope ? ` · ${selected.scope}` : ""}</p></div>
                  <div className="forge-account-actions"><button className="control-button control-button--secondary" type="button" disabled={loadingRepositories} onClick={() => void refreshRepositories()}>{t("Refresh")}</button><button className="control-button control-button--danger" type="button" onClick={() => setPendingDisconnect(true)}>{t("Disconnect")}</button></div>
                </div>
                <div className="forge-repository-toolbar"><input className="control-input" aria-label={t("Search hosted repositories")} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Search repositories")} /><span>{t("{count} repositories", { count: filteredRepositories.length })}</span></div>
                {message && <p className="forge-message" role="status">{message}</p>}
                <div className="forge-repository-list">
                  {loadingRepositories ? <p className="forge-empty" role="status">{t("Loading repositories…")}</p> : filteredRepositories.length === 0 ? <p className="forge-empty">{query ? t("No repositories match your search.") : t("No repositories found.")}</p> : filteredRepositories.map((repository) => (
                    <article className="forge-repository" key={repository.id}>
                      <div><strong>{repository.fullName}</strong><span>{repository.private ? t("Private") : t("Public")}{repository.archived ? ` · ${t("Archived")}` : ""}{repository.updatedAt ? ` · ${t("Updated {date}", { date: formatDate(repository.updatedAt) })}` : ""}</span></div>
                      <button className="control-button control-button--secondary" type="button" disabled={repository.archived} onClick={() => onClone(repository.cloneUrl)}>{t("Clone")}</button>
                    </article>
                  ))}
                </div>
              </>
            ) : null}
          </div>
        </div>
        {pendingDisconnect && selected && (
          <div className="forge-disconnect-confirm" role="alertdialog" aria-modal="true" aria-labelledby="forge-disconnect-title">
            <div><h3 id="forge-disconnect-title">{t("Disconnect {name}?", { name: selected.displayName })}</h3><p>{t("The account metadata and its token in Git Credential Manager will be removed.")}</p></div>
            <div><button className="control-button control-button--secondary" type="button" disabled={disconnecting} onClick={() => setPendingDisconnect(false)}>{t("Cancel")}</button><button className="control-button control-button--danger" type="button" disabled={disconnecting} onClick={() => void disconnect()}>{disconnecting ? t("Disconnecting…") : t("Disconnect account")}</button></div>
          </div>
        )}
      </section>
    </div>
  );
}

function needsScope(provider: ForgeProvider) {
  return provider === "bitbucket" || provider === "azureDevOps";
}

function requiresUsername(provider: ForgeProvider) {
  return provider === "bitbucket" || provider === "azureDevOps";
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat(localeTag(), { dateStyle: "medium" }).format(date);
}