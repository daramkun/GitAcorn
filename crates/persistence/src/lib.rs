//! SQLite-backed application session persistence.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceSettings {
    pub schema_version: u16,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self { schema_version: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTab {
    pub repo_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub opened_from_repository_name: Option<String>,
    pub opened_from_worktree_path: Option<String>,
    pub tab_order: i64,
    pub active: bool,
    pub page: String,
    pub selected_path: Option<String>,
    pub selected_diff: String,
    pub panel_width: f64,
    pub history_cursor: Option<String>,
    pub selected_commit: Option<String>,
    pub history_filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecord {
    pub id: String,
    pub repo_id: Option<String>,
    pub kind: String,
    pub state: String,
    pub summary: String,
    pub diagnostic: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub recovery_action: Option<String>,
    pub recovery_state: Option<String>,
    pub before_head_oid: Option<String>,
    pub after_head_oid: Option<String>,
    pub before_head_ref: Option<String>,
    pub after_head_ref: Option<String>,
    pub recovery_ref: Option<String>,
    pub recovery_oid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccountRecord {
    pub id: String,
    pub provider: String,
    pub host: String,
    pub login: String,
    pub display_name: String,
    pub auth_username: String,
    pub scope: Option<String>,
    pub avatar_url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRepositoryRecord {
    pub path: String,
    pub clone_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub repositories: Vec<WorkspaceRepositoryRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecovery<'a> {
    pub action: &'a str,
    pub before_head_oid: Option<&'a str>,
    pub after_head_oid: Option<&'a str>,
    pub before_head_ref: Option<&'a str>,
    pub after_head_ref: Option<&'a str>,
    pub recovery_ref: Option<&'a str>,
    pub recovery_oid: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_tabs (
                repo_id TEXT PRIMARY KEY NOT NULL,
                worktree_id TEXT NOT NULL DEFAULT '',
                worktree_path TEXT NOT NULL,
                opened_from_repository_name TEXT,
                opened_from_worktree_path TEXT,
                tab_order INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 0,
                page TEXT NOT NULL DEFAULT 'changes',
                selected_path TEXT,
                selected_diff TEXT NOT NULL DEFAULT 'unstaged',
                panel_width REAL NOT NULL DEFAULT 280,
                history_cursor TEXT,
                selected_commit TEXT,
                history_filter TEXT
            )",
        )
        .execute(&pool)
        .await?;
        create_operation_table(&pool).await?;
        create_forge_account_table(&pool).await?;
        create_workspace_tables(&pool).await?;
        let has_worktree_id: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('session_tabs') WHERE name = 'worktree_id'",
        )
        .fetch_one(&pool)
        .await?;
        if has_worktree_id == 0 {
            sqlx::query("ALTER TABLE session_tabs ADD COLUMN worktree_id TEXT NOT NULL DEFAULT ''")
                .execute(&pool)
                .await?;
        }
        let has_selected_diff: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('session_tabs') WHERE name = 'selected_diff'",
        )
        .fetch_one(&pool)
        .await?;
        if has_selected_diff == 0 {
            sqlx::query(
                "ALTER TABLE session_tabs ADD COLUMN selected_diff TEXT NOT NULL DEFAULT 'unstaged'",
            )
            .execute(&pool)
            .await?;
        }
        for column in ["history_cursor", "selected_commit", "history_filter"] {
            let exists: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM pragma_table_info('session_tabs') WHERE name = '{column}'"
            ))
            .fetch_one(&pool)
            .await?;
            if exists == 0 {
                sqlx::query(&format!(
                    "ALTER TABLE session_tabs ADD COLUMN {column} TEXT"
                ))
                .execute(&pool)
                .await?;
            }
        }
        for column in ["opened_from_repository_name", "opened_from_worktree_path"] {
            let exists: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM pragma_table_info('session_tabs') WHERE name = '{column}'"
            ))
            .fetch_one(&pool)
            .await?;
            if exists == 0 {
                sqlx::query(&format!(
                    "ALTER TABLE session_tabs ADD COLUMN {column} TEXT"
                ))
                .execute(&pool)
                .await?;
            }
        }
        Ok(Self { pool })
    }

    pub async fn memory() -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new().filename(":memory:");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query(
            "CREATE TABLE session_tabs (
                repo_id TEXT PRIMARY KEY NOT NULL,
                worktree_id TEXT NOT NULL DEFAULT '',
                worktree_path TEXT NOT NULL,
                opened_from_repository_name TEXT,
                opened_from_worktree_path TEXT,
                tab_order INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 0,
                page TEXT NOT NULL DEFAULT 'changes',
                selected_path TEXT,
                selected_diff TEXT NOT NULL DEFAULT 'unstaged',
                panel_width REAL NOT NULL DEFAULT 280,
                history_cursor TEXT,
                selected_commit TEXT,
                history_filter TEXT
            )",
        )
        .execute(&pool)
        .await?;
        create_operation_table(&pool).await?;
        create_forge_account_table(&pool).await?;
        create_workspace_tables(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn start_operation(
        &self,
        id: &str,
        repo_id: Option<&str>,
        kind: &str,
        summary: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO operation_history
                (id, repo_id, kind, state, summary, started_at)
             VALUES (?, ?, ?, 'running', ?, CURRENT_TIMESTAMP)",
        )
        .bind(id)
        .bind(repo_id)
        .bind(kind)
        .bind(summary)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn finish_operation(
        &self,
        id: &str,
        state: &str,
        summary: &str,
        diagnostic: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE operation_history
             SET state = ?, summary = ?, diagnostic = ?, finished_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(state)
        .bind(summary)
        .bind(diagnostic)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_operations(&self, limit: usize) -> Result<Vec<OperationRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, repo_id, kind, state, summary, diagnostic, started_at, finished_at,
                    recovery_action, recovery_state, before_head_oid, after_head_oid,
                    before_head_ref, after_head_ref, recovery_ref, recovery_oid
             FROM operation_history
             ORDER BY started_at DESC, rowid DESC
             LIMIT ?",
        )
        .bind(limit.clamp(1, 200) as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| OperationRecord {
                id: row.get("id"),
                repo_id: row.get("repo_id"),
                kind: row.get("kind"),
                state: row.get("state"),
                summary: row.get("summary"),
                diagnostic: row.get("diagnostic"),
                started_at: row.get("started_at"),
                finished_at: row.get("finished_at"),
                recovery_action: row.get("recovery_action"),
                recovery_state: row.get("recovery_state"),
                before_head_oid: row.get("before_head_oid"),
                after_head_oid: row.get("after_head_oid"),
                before_head_ref: row.get("before_head_ref"),
                after_head_ref: row.get("after_head_ref"),
                recovery_ref: row.get("recovery_ref"),
                recovery_oid: row.get("recovery_oid"),
            })
            .collect())
    }

    pub async fn operation(&self, id: &str) -> Result<Option<OperationRecord>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, repo_id, kind, state, summary, diagnostic, started_at, finished_at,
                    recovery_action, recovery_state, before_head_oid, after_head_oid,
                    before_head_ref, after_head_ref, recovery_ref, recovery_oid
             FROM operation_history WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| OperationRecord {
            id: row.get("id"),
            repo_id: row.get("repo_id"),
            kind: row.get("kind"),
            state: row.get("state"),
            summary: row.get("summary"),
            diagnostic: row.get("diagnostic"),
            started_at: row.get("started_at"),
            finished_at: row.get("finished_at"),
            recovery_action: row.get("recovery_action"),
            recovery_state: row.get("recovery_state"),
            before_head_oid: row.get("before_head_oid"),
            after_head_oid: row.get("after_head_oid"),
            before_head_ref: row.get("before_head_ref"),
            after_head_ref: row.get("after_head_ref"),
            recovery_ref: row.get("recovery_ref"),
            recovery_oid: row.get("recovery_oid"),
        }))
    }

    pub async fn attach_recovery(
        &self,
        id: &str,
        recovery: &OperationRecovery<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE operation_history
             SET recovery_action = ?, recovery_state = 'ready',
                 before_head_oid = ?, after_head_oid = ?,
                 before_head_ref = ?, after_head_ref = ?,
                 recovery_ref = ?, recovery_oid = ?
             WHERE id = ?",
        )
        .bind(recovery.action)
        .bind(recovery.before_head_oid)
        .bind(recovery.after_head_oid)
        .bind(recovery.before_head_ref)
        .bind(recovery.after_head_ref)
        .bind(recovery.recovery_ref)
        .bind(recovery.recovery_oid)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_recovery_state(&self, id: &str, state: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE operation_history SET recovery_state = ? WHERE id = ?")
            .bind(state)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn recover_interrupted_operations(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE operation_history
             SET state = 'interrupted',
                 summary = 'Interrupted when GitAcorn last exited',
                 finished_at = CURRENT_TIMESTAMP
             WHERE state IN ('queued', 'running')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn load_tabs(&self) -> Result<Vec<SessionTab>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT repo_id, worktree_id, worktree_path, opened_from_repository_name, opened_from_worktree_path, tab_order, active, page, selected_path, selected_diff, panel_width, history_cursor, selected_commit, history_filter
             FROM session_tabs ORDER BY tab_order",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SessionTab {
                repo_id: row.get("repo_id"),
                worktree_id: row.get("worktree_id"),
                worktree_path: row.get("worktree_path"),
                opened_from_repository_name: row.get("opened_from_repository_name"),
                opened_from_worktree_path: row.get("opened_from_worktree_path"),
                tab_order: row.get("tab_order"),
                active: row.get::<i64, _>("active") != 0,
                page: row.get("page"),
                selected_path: row.get("selected_path"),
                selected_diff: row.get("selected_diff"),
                panel_width: row.get("panel_width"),
                history_cursor: row.get("history_cursor"),
                selected_commit: row.get("selected_commit"),
                history_filter: row.get("history_filter"),
            })
            .collect())
    }

    pub async fn upsert_tab(&self, tab: &SessionTab) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        if tab.active {
            sqlx::query("UPDATE session_tabs SET active = 0")
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "INSERT INTO session_tabs
                (repo_id, worktree_id, worktree_path, opened_from_repository_name, opened_from_worktree_path, tab_order, active, page, selected_path, selected_diff, panel_width, history_cursor, selected_commit, history_filter)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(repo_id) DO UPDATE SET
                worktree_id = excluded.worktree_id,
                worktree_path = excluded.worktree_path,
                opened_from_repository_name = excluded.opened_from_repository_name,
                opened_from_worktree_path = excluded.opened_from_worktree_path,
                tab_order = excluded.tab_order,
                active = excluded.active,
                page = excluded.page,
                selected_path = excluded.selected_path,
                selected_diff = excluded.selected_diff,
                panel_width = excluded.panel_width,
                history_cursor = excluded.history_cursor,
                selected_commit = excluded.selected_commit,
                history_filter = excluded.history_filter",
        )
        .bind(&tab.repo_id)
        .bind(&tab.worktree_id)
        .bind(&tab.worktree_path)
        .bind(&tab.opened_from_repository_name)
        .bind(&tab.opened_from_worktree_path)
        .bind(tab.tab_order)
        .bind(tab.active)
        .bind(&tab.page)
        .bind(&tab.selected_path)
        .bind(&tab.selected_diff)
        .bind(tab.panel_width)
        .bind(&tab.history_cursor)
        .bind(&tab.selected_commit)
        .bind(&tab.history_filter)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }

    pub async fn activate(&self, repo_id: &str) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("UPDATE session_tabs SET active = 0")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE session_tabs SET active = 1 WHERE repo_id = ?")
            .bind(repo_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await
    }

    pub async fn list_forge_accounts(&self) -> Result<Vec<ForgeAccountRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, provider, host, login, display_name, auth_username, scope, avatar_url
             FROM forge_accounts ORDER BY provider, display_name, login",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ForgeAccountRecord {
                id: row.get("id"),
                provider: row.get("provider"),
                host: row.get("host"),
                login: row.get("login"),
                display_name: row.get("display_name"),
                auth_username: row.get("auth_username"),
                scope: row.get("scope"),
                avatar_url: row.get("avatar_url"),
            })
            .collect())
    }

    pub async fn upsert_forge_account(
        &self,
        account: &ForgeAccountRecord,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO forge_accounts
                (id, provider, host, login, display_name, auth_username, scope, avatar_url)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider, host = excluded.host, login = excluded.login,
                display_name = excluded.display_name, auth_username = excluded.auth_username,
                scope = excluded.scope, avatar_url = excluded.avatar_url",
        )
        .bind(&account.id)
        .bind(&account.provider)
        .bind(&account.host)
        .bind(&account.login)
        .bind(&account.display_name)
        .bind(&account.auth_username)
        .bind(&account.scope)
        .bind(&account.avatar_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_forge_account(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM forge_accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn list_workspaces(&self) -> Result<Vec<WorkspaceRecord>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, name FROM workspaces ORDER BY name, id")
            .fetch_all(&self.pool)
            .await?;
        let mut workspaces = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let repository_rows = sqlx::query(
                "SELECT path, clone_url FROM workspace_repositories WHERE workspace_id = ? ORDER BY position, path",
            )
            .bind(&id)
            .fetch_all(&self.pool)
            .await?;
            workspaces.push(WorkspaceRecord {
                id,
                name: row.get("name"),
                repositories: repository_rows
                    .into_iter()
                    .map(|repository| WorkspaceRepositoryRecord {
                        path: repository.get("path"),
                        clone_url: repository.get("clone_url"),
                    })
                    .collect(),
            });
        }
        Ok(workspaces)
    }

    pub async fn upsert_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO workspaces (id, name) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name",
        )
        .bind(&workspace.id)
        .bind(&workspace.name)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM workspace_repositories WHERE workspace_id = ?")
            .bind(&workspace.id)
            .execute(&mut *transaction)
            .await?;
        for (position, repository) in workspace.repositories.iter().enumerate() {
            sqlx::query(
                "INSERT INTO workspace_repositories (workspace_id, path, clone_url, position)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&workspace.id)
            .bind(&repository.path)
            .bind(&repository.clone_url)
            .bind(position as i64)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await
    }

    pub async fn delete_workspace(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM workspaces WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn close(&self, repo_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM session_tabs WHERE repo_id = ?")
            .bind(repo_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn reorder(&self, repo_ids: &[String]) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        for (index, repo_id) in repo_ids.iter().enumerate() {
            sqlx::query("UPDATE session_tabs SET tab_order = ? WHERE repo_id = ?")
                .bind(index as i64)
                .bind(repo_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await
    }
}

async fn create_forge_account_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS forge_accounts (
            id TEXT PRIMARY KEY NOT NULL,
            provider TEXT NOT NULL,
            host TEXT NOT NULL,
            login TEXT NOT NULL,
            display_name TEXT NOT NULL,
            auth_username TEXT NOT NULL,
            scope TEXT,
            avatar_url TEXT
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}
async fn create_workspace_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspace_repositories (
            workspace_id TEXT NOT NULL,
            path TEXT NOT NULL,
            clone_url TEXT,
            position INTEGER NOT NULL,
            PRIMARY KEY (workspace_id, path),
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_operation_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS operation_history (
            id TEXT PRIMARY KEY NOT NULL,
            repo_id TEXT,
            kind TEXT NOT NULL,
            state TEXT NOT NULL,
            summary TEXT NOT NULL,
            diagnostic TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT
        )",
    )
    .execute(pool)
    .await?;
    for column in [
        "recovery_action TEXT",
        "recovery_state TEXT",
        "before_head_oid TEXT",
        "after_head_oid TEXT",
        "before_head_ref TEXT",
        "after_head_ref TEXT",
        "recovery_ref TEXT",
        "recovery_oid TEXT",
    ] {
        let name = column.split_whitespace().next().unwrap_or_default();
        let exists: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('operation_history') WHERE name = '{name}'"
        ))
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE operation_history ADD COLUMN {column}"
            ))
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ForgeAccountRecord, OperationRecovery, PersistenceSettings, SessionStore, SessionTab,
        SqliteConnectOptions, SqlitePool, WorkspaceRecord, WorkspaceRepositoryRecord,
    };

    #[test]
    fn settings_start_at_schema_version_one() {
        assert_eq!(PersistenceSettings::default().schema_version, 1);
    }

    #[tokio::test]
    async fn persists_order_active_tab_and_per_repository_ui_state() {
        let store = SessionStore::memory().await.expect("memory store");
        for (index, repo_id) in ["one", "two"].iter().enumerate() {
            store
                .upsert_tab(&SessionTab {
                    repo_id: (*repo_id).to_owned(),
                    worktree_id: format!("{repo_id}-worktree"),
                    worktree_path: format!("C:/{repo_id}"),
                    opened_from_repository_name: (*repo_id == "two").then(|| "parent".to_owned()),
                    opened_from_worktree_path: (*repo_id == "two").then(|| "C:/parent".to_owned()),
                    tab_order: index as i64,
                    active: *repo_id == "two",
                    page: if *repo_id == "one" {
                        "history"
                    } else {
                        "changes"
                    }
                    .to_owned(),
                    selected_path: Some(format!("{repo_id}.txt")),
                    selected_diff: "staged".to_owned(),
                    panel_width: 320.0,
                    history_cursor: Some("offset:50".to_owned()),
                    selected_commit: Some(format!("{repo_id}-oid")),
                    history_filter: Some("author:Ada".to_owned()),
                })
                .await
                .expect("save tab");
        }

        let tabs = store.load_tabs().await.expect("load tabs");
        assert_eq!(
            tabs.iter()
                .map(|tab| tab.repo_id.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert!(tabs[1].active);
        assert_eq!(tabs[0].page, "history");
        assert_eq!(tabs[0].selected_path.as_deref(), Some("one.txt"));
        assert_eq!(tabs[0].selected_diff, "staged");
        assert_eq!(tabs[0].history_cursor.as_deref(), Some("offset:50"));
        assert_eq!(tabs[0].selected_commit.as_deref(), Some("one-oid"));
        assert_eq!(
            tabs[1].opened_from_repository_name.as_deref(),
            Some("parent")
        );
        assert_eq!(
            tabs[1].opened_from_worktree_path.as_deref(),
            Some("C:/parent")
        );
    }

    #[tokio::test]
    async fn closes_and_reorders_tabs_without_losing_the_registry_record() {
        let store = SessionStore::memory().await.expect("memory store");
        for (index, repo_id) in ["one", "two", "three"].iter().enumerate() {
            store
                .upsert_tab(&SessionTab {
                    repo_id: (*repo_id).to_owned(),
                    worktree_id: format!("{repo_id}-worktree"),
                    worktree_path: format!("C:/{repo_id}"),
                    opened_from_repository_name: None,
                    opened_from_worktree_path: None,
                    tab_order: index as i64,
                    active: false,
                    page: "changes".to_owned(),
                    selected_path: None,
                    selected_diff: "unstaged".to_owned(),
                    panel_width: 280.0,
                    history_cursor: None,
                    selected_commit: None,
                    history_filter: None,
                })
                .await
                .expect("save tab");
        }
        store
            .reorder(&["three".to_owned(), "one".to_owned(), "two".to_owned()])
            .await
            .expect("reorder");
        store.close("one").await.expect("close");

        let tabs = store.load_tabs().await.expect("load tabs");
        assert_eq!(
            tabs.iter()
                .map(|tab| tab.repo_id.as_str())
                .collect::<Vec<_>>(),
            ["three", "two"]
        );
    }

    #[tokio::test]
    async fn migrates_sessions_created_before_worktree_ids() {
        let directory = tempfile::tempdir().expect("temporary database directory");
        let path = directory.path().join("session.sqlite3");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("legacy pool");
        sqlx::query(
            "CREATE TABLE session_tabs (
                repo_id TEXT PRIMARY KEY NOT NULL,
                worktree_path TEXT NOT NULL,
                tab_order INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 0,
                page TEXT NOT NULL DEFAULT 'changes',
                selected_path TEXT,
                panel_width REAL NOT NULL DEFAULT 280
            )",
        )
        .execute(&pool)
        .await
        .expect("legacy schema");
        sqlx::query(
            "INSERT INTO session_tabs
                (repo_id, worktree_path, tab_order, active, page, panel_width)
             VALUES ('legacy', 'C:/legacy', 0, 1, 'changes', 280)",
        )
        .execute(&pool)
        .await
        .expect("legacy row");
        pool.close().await;

        let store = SessionStore::open(&path).await.expect("migrated store");
        let tabs = store.load_tabs().await.expect("load migrated tabs");

        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].repo_id, "legacy");
        assert_eq!(tabs[0].worktree_id, "");
        assert_eq!(tabs[0].selected_diff, "unstaged");
        assert!(tabs[0].history_cursor.is_none());
    }

    #[tokio::test]
    async fn persists_operation_history_and_recovers_interrupted_work() {
        let store = SessionStore::memory().await.expect("memory store");
        store
            .start_operation("one", Some("repo"), "stash-create", "Creating stash")
            .await
            .expect("start operation");
        assert_eq!(
            store
                .recover_interrupted_operations()
                .await
                .expect("recover"),
            1
        );
        let operations = store.list_operations(20).await.expect("list operations");
        assert_eq!(operations[0].state, "interrupted");
        assert!(operations[0].finished_at.is_some());
    }

    #[tokio::test]
    async fn persists_and_transitions_typed_recovery_metadata() {
        let store = SessionStore::memory().await.expect("memory store");
        store
            .start_operation("commit-one", Some("repo"), "commit", "Creating commit")
            .await
            .expect("start operation");
        store
            .finish_operation("commit-one", "succeeded", "Created commit", None)
            .await
            .expect("finish operation");
        store
            .attach_recovery(
                "commit-one",
                &OperationRecovery {
                    action: "checkout",
                    before_head_oid: Some("before"),
                    after_head_oid: Some("after"),
                    before_head_ref: Some("main"),
                    after_head_ref: Some("topic"),
                    recovery_ref: None,
                    recovery_oid: None,
                },
            )
            .await
            .expect("attach recovery");

        let ready = store
            .operation("commit-one")
            .await
            .expect("read operation")
            .expect("operation");
        assert_eq!(ready.recovery_action.as_deref(), Some("checkout"));
        assert_eq!(ready.recovery_state.as_deref(), Some("ready"));
        assert_eq!(ready.before_head_oid.as_deref(), Some("before"));
        assert_eq!(ready.after_head_oid.as_deref(), Some("after"));
        assert_eq!(ready.before_head_ref.as_deref(), Some("main"));
        assert_eq!(ready.after_head_ref.as_deref(), Some("topic"));

        store
            .set_recovery_state("commit-one", "undone")
            .await
            .expect("mark undone");
        assert_eq!(
            store
                .operation("commit-one")
                .await
                .expect("read operation")
                .expect("operation")
                .recovery_state
                .as_deref(),
            Some("undone")
        );
    }

    #[tokio::test]
    async fn persists_forge_account_metadata_without_secret_columns() {
        use sqlx::Row;
        let store = SessionStore::memory().await.expect("memory store");
        let account = ForgeAccountRecord {
            id: "github:acorn".to_owned(),
            provider: "github".to_owned(),
            host: "github.com".to_owned(),
            login: "acorn".to_owned(),
            display_name: "Acorn User".to_owned(),
            auth_username: "acorn".to_owned(),
            scope: None,
            avatar_url: Some("https://example.invalid/avatar".to_owned()),
        };
        store
            .upsert_forge_account(&account)
            .await
            .expect("save account");
        assert_eq!(
            store.list_forge_accounts().await.expect("list accounts"),
            vec![account]
        );

        let columns = sqlx::query("SELECT name FROM pragma_table_info('forge_accounts')")
            .fetch_all(&store.pool)
            .await
            .expect("account columns")
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert!(columns.iter().all(|name| {
            !["token", "password", "secret", "credential"]
                .iter()
                .any(|forbidden| name.contains(forbidden))
        }));

        store
            .delete_forge_account("github:acorn")
            .await
            .expect("delete account");
        assert!(
            store
                .list_forge_accounts()
                .await
                .expect("list after delete")
                .is_empty()
        );
    }
    #[tokio::test]
    async fn recovery_schema_only_allows_head_and_reference_metadata() {
        use sqlx::Row;

        let store = SessionStore::memory().await.expect("memory store");
        let columns = sqlx::query("SELECT name FROM pragma_table_info('operation_history')")
            .fetch_all(&store.pool)
            .await
            .expect("operation columns")
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .filter(|name| {
                name.starts_with("recovery_")
                    || name.ends_with("_head_oid")
                    || name.ends_with("_head_ref")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            columns,
            [
                "recovery_action",
                "recovery_state",
                "before_head_oid",
                "after_head_oid",
                "before_head_ref",
                "after_head_ref",
                "recovery_ref",
                "recovery_oid",
            ]
        );
        assert!(columns.iter().all(|name| {
            !["credential", "secret", "content", "remote_url", "file_path"]
                .iter()
                .any(|forbidden| name.contains(forbidden))
        }));
    }
    #[tokio::test]
    async fn persists_workspace_repository_order_and_cascades_delete() {
        let store = SessionStore::memory().await.expect("memory store");
        let workspace = WorkspaceRecord {
            id: "workspace-one".to_owned(),
            name: "Client application".to_owned(),
            repositories: vec![
                WorkspaceRepositoryRecord {
                    path: "C:/repos/frontend".to_owned(),
                    clone_url: Some("https://example.invalid/frontend.git".to_owned()),
                },
                WorkspaceRepositoryRecord {
                    path: "C:/repos/backend".to_owned(),
                    clone_url: None,
                },
            ],
        };

        store
            .upsert_workspace(&workspace)
            .await
            .expect("save workspace");
        assert_eq!(
            store.list_workspaces().await.expect("list workspaces"),
            vec![workspace.clone()]
        );

        let renamed = WorkspaceRecord {
            name: "Renamed workspace".to_owned(),
            repositories: workspace.repositories.into_iter().rev().collect(),
            ..workspace
        };
        store
            .upsert_workspace(&renamed)
            .await
            .expect("update workspace");
        assert_eq!(
            store
                .list_workspaces()
                .await
                .expect("list updated workspace"),
            vec![renamed]
        );

        store
            .delete_workspace("workspace-one")
            .await
            .expect("delete workspace");
        assert!(
            store
                .list_workspaces()
                .await
                .expect("list deleted workspace")
                .is_empty()
        );
        let repository_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspace_repositories")
                .fetch_one(&store.pool)
                .await
                .expect("count workspace repositories");
        assert_eq!(repository_count, 0);
    }
}
