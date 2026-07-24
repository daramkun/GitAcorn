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
        Ok(Self { pool })
    }

    pub async fn load_tabs(&self) -> Result<Vec<SessionTab>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT repo_id, worktree_id, worktree_path, tab_order, active, page, selected_path, selected_diff, panel_width, history_cursor, selected_commit, history_filter
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
                (repo_id, worktree_id, worktree_path, tab_order, active, page, selected_path, selected_diff, panel_width, history_cursor, selected_commit, history_filter)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(repo_id) DO UPDATE SET
                worktree_id = excluded.worktree_id,
                worktree_path = excluded.worktree_path,
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

#[cfg(test)]
mod tests {
    use super::{PersistenceSettings, SessionStore, SessionTab, SqliteConnectOptions, SqlitePool};

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
}
