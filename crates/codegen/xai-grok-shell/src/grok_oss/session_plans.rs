//! Live session plan documents in `grok_oss.db`.
//!
//! Additive Surmount table (schema v6). Stores Isolated Preview title, body,
//! dock-open, and comments so `/plan --soft` and present do not require a
//! markdown write lock on session `plan.md`. This is not the Token Economy
//! spend ledger. `/spend` still uses schema v1 `local_usage_event`,
//! `remote_meter_sample`, and `reconciliation_run`.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;

use crate::grok_oss::{GrokOssStore, SESSION_PLAN_IDENTITY};

/// Additive schema v6: one live plan document per session identity.
pub(crate) const SCHEMA_V6: &str = r#"
CREATE TABLE IF NOT EXISTS session_plans (
  session_id TEXT NOT NULL,
  plan_identity TEXT NOT NULL,
  title TEXT,
  body TEXT NOT NULL,
  dock_open INTEGER NOT NULL,
  comments TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (session_id, plan_identity)
);
"#;

/// One session plan document row. Identity stays `"plan.md"` so
/// `plan_recorded_choice` still joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionPlanRow {
    pub session_id: String,
    pub plan_identity: String,
    pub title: Option<String>,
    pub body: String,
    pub dock_open: bool,
    pub comments: String,
    pub updated_at: String,
}

/// Apply additive v6 `session_plans` without dropping prior tables.
pub(crate) fn apply_schema_v6(store: &GrokOssStore) -> Result<()> {
    store
        .connection()
        .execute_batch(SCHEMA_V6)
        .context("apply schema v6 session_plans")?;
    Ok(())
}

/// Isolated Preview / present: SQL body first, then disk `plan.md`.
pub(crate) fn resolve_plan_body_sql_then_disk(
    store: &GrokOssStore,
    session_id: &str,
    plan_identity: &str,
    disk_plan_md: Option<&Path>,
) -> Option<String> {
    if let Ok(Some(row)) = store.load_session_plan(session_id, plan_identity)
        && !row.body.trim().is_empty()
    {
        return Some(row.body);
    }
    disk_plan_md
        .and_then(|p| std::fs::read_to_string(p).ok())
        .filter(|s| !s.trim().is_empty())
}

/// Fail-open present helper. Prefers SQL, then an already-read disk body.
/// Tests skip opening the operator store unless `[token_economy]
/// grok_oss_database_path` is set.
pub fn prefer_sql_plan_body_fail_open(
    session_id: &str,
    disk_body: Option<String>,
) -> Option<String> {
    let cfg = crate::token_economy::token_economy_from_disk();
    if !(cfg!(test) && cfg.grok_oss_database_path.is_none())
        && let Some(store) = crate::grok_oss::try_open_from_token_economy_config(&cfg)
        && let Ok(Some(row)) = store.load_session_plan(session_id, SESSION_PLAN_IDENTITY)
        && !row.body.trim().is_empty()
    {
        return Some(row.body);
    }
    disk_body.filter(|s| !s.trim().is_empty())
}

impl GrokOssStore {
    fn ensure_session_plans_table(&self) -> Result<()> {
        apply_schema_v6(self)
    }

    /// Insert or replace the live plan document.
    pub fn upsert_session_plan(
        &self,
        session_id: &str,
        plan_identity: &str,
        title: Option<&str>,
        body: &str,
        dock_open: bool,
        comments: &str,
    ) -> Result<()> {
        self.ensure_session_plans_table()?;
        let now = Utc::now().to_rfc3339();
        let dock = i64::from(dock_open);
        self.connection()
            .execute(
                "INSERT INTO session_plans
                   (session_id, plan_identity, title, body, dock_open, comments, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(session_id, plan_identity) DO UPDATE SET
                   title = excluded.title,
                   body = excluded.body,
                   dock_open = excluded.dock_open,
                   comments = excluded.comments,
                   updated_at = excluded.updated_at",
                rusqlite::params![session_id, plan_identity, title, body, dock, comments, now],
            )
            .context("upsert session_plans")?;
        Ok(())
    }

    /// Load the live plan document, if any.
    pub(crate) fn load_session_plan(
        &self,
        session_id: &str,
        plan_identity: &str,
    ) -> Result<Option<SessionPlanRow>> {
        self.ensure_session_plans_table()?;
        self.connection()
            .query_row(
                "SELECT session_id, plan_identity, title, body, dock_open, comments, updated_at
                 FROM session_plans
                 WHERE session_id = ?1 AND plan_identity = ?2",
                rusqlite::params![session_id, plan_identity],
                |row| {
                    let dock: i64 = row.get(4)?;
                    Ok(SessionPlanRow {
                        session_id: row.get(0)?,
                        plan_identity: row.get(1)?,
                        title: row.get(2)?,
                        body: row.get(3)?,
                        dock_open: dock != 0,
                        comments: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("load session_plans")
    }

    /// Isolated Preview / present: non-empty SQL body, if any.
    pub fn load_session_plan_body(
        &self,
        session_id: &str,
        plan_identity: &str,
    ) -> Result<Option<String>> {
        Ok(self
            .load_session_plan(session_id, plan_identity)?
            .map(|row| row.body)
            .filter(|body| !body.trim().is_empty()))
    }

    /// Isolated Preview / present: SQL body first, then disk `plan.md`.
    pub fn plan_body_sql_then_disk(
        &self,
        session_id: &str,
        plan_identity: &str,
        disk_plan_md: Option<&Path>,
    ) -> Option<String> {
        resolve_plan_body_sql_then_disk(self, session_id, plan_identity, disk_plan_md)
    }

    /// Present / plan-save: write the live body. Keeps dock_open, title, and
    /// comments when a row already exists.
    pub fn upsert_session_plan_body(&self, session_id: &str, body: &str) -> Result<()> {
        let existing = self
            .load_session_plan(session_id, SESSION_PLAN_IDENTITY)
            .ok()
            .flatten();
        let title = existing
            .as_ref()
            .and_then(|row| row.title.as_deref())
            .map(str::to_owned);
        let dock_open = existing.as_ref().is_some_and(|row| row.dock_open);
        let comments = existing
            .as_ref()
            .map(|row| row.comments.clone())
            .unwrap_or_else(|| "[]".to_string());
        self.upsert_session_plan(
            session_id,
            SESSION_PLAN_IDENTITY,
            title.as_deref(),
            body,
            dock_open,
            &comments,
        )
    }

    /// Present / plan-save fail-open. Tests skip the operator store unless
    /// `[token_economy] grok_oss_database_path` is set.
    pub fn upsert_session_plan_body_fail_open(session_id: &str, body: &str) {
        if body.trim().is_empty() {
            return;
        }
        let cfg = crate::token_economy::token_economy_from_disk();
        if cfg!(test) && cfg.grok_oss_database_path.is_none() {
            return;
        }
        let Some(store) = crate::grok_oss::try_open_from_token_economy_config(&cfg) else {
            return;
        };
        if let Err(e) = store.upsert_session_plan_body(session_id, body) {
            tracing::debug!(error = %e, "session_plans body upsert failed (fail-open)");
        }
    }

    /// `/plan --soft` sets Isolated Preview dock-open without writing `plan.md`.
    pub fn set_session_plan_dock_open(
        &self,
        session_id: &str,
        plan_identity: &str,
        dock_open: bool,
    ) -> Result<bool> {
        self.ensure_session_plans_table()?;
        let now = Utc::now().to_rfc3339();
        let dock = i64::from(dock_open);
        self.connection()
            .execute(
                "INSERT INTO session_plans
                   (session_id, plan_identity, title, body, dock_open, comments, updated_at)
                 VALUES (?1, ?2, NULL, '', ?3, '[]', ?4)
                 ON CONFLICT(session_id, plan_identity) DO UPDATE SET
                   dock_open = excluded.dock_open,
                   updated_at = excluded.updated_at",
                rusqlite::params![session_id, plan_identity, dock, now],
            )
            .context("set session_plans.dock_open")?;
        Ok(dock_open)
    }
}

#[cfg(test)]
mod session_plans_tests {
    use super::*;
    use crate::grok_oss::{SCHEMA_VERSION, SESSION_PLAN_IDENTITY, open_at};
    use tempfile::TempDir;
    use xai_grok_tools::implementations::editor_infra::per_path_write_lock::try_acquire_write;

    fn table_exists(store: &GrokOssStore, name: &str) -> bool {
        let n: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .unwrap();
        n == 1
    }

    fn stamp_v5_file(path: &Path) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#,
        )
        .unwrap();
        conn.execute_batch(super::super::SCHEMA_V1).unwrap();
        conn.execute_batch(super::super::SCHEMA_V2).unwrap();
        conn.execute_batch(super::super::SCHEMA_V3).unwrap();
        conn.execute_batch(super::super::SCHEMA_V4).unwrap();
        conn.execute_batch(super::super::SCHEMA_V5).unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '5')",
            [],
        )
        .unwrap();
    }

    /// Named contract: additive v6 creates `session_plans` (session_id,
    /// plan_identity, title, body, dock_open, comments, updated_at) without
    /// dropping v5 tables.
    #[test]
    fn migrate_v5_file_to_v6_adds_session_plans_without_dropping_v5_tables() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("grok_oss.db");
        stamp_v5_file(&path);
        let store = open_at(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        // session_plans is additive schema v6 and later versions keep it
        const {
            assert!(SCHEMA_VERSION >= 6);
        }
        assert!(
            table_exists(&store, "session_plans"),
            "additive v6 must create session_plans"
        );
        let cols: Vec<String> = {
            let mut stmt = store
                .connection()
                .prepare("PRAGMA table_info(session_plans)")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        for name in [
            "session_id",
            "plan_identity",
            "title",
            "body",
            "dock_open",
            "comments",
            "updated_at",
        ] {
            assert!(
                cols.iter().any(|c| c == name),
                "session_plans must have column {name}; got {cols:?}"
            );
        }
        for name in [
            "local_usage_event",
            "remote_meter_sample",
            "reconciliation_run",
            "prompt_tasks",
            "prompt_exec_metrics",
            "plan_recorded_choice",
            "session_id_map",
        ] {
            assert!(
                table_exists(&store, name),
                "v6 must not drop v5 table {name}"
            );
        }
        let store2 = open_at(&path).unwrap();
        assert!(
            table_exists(&store2, "session_plans"),
            "v6 migrate must be idempotent on reopen"
        );
    }

    /// Named contract: `/plan --soft` sets dock_open without requiring a
    /// markdown write lock on session `plan.md`.
    #[test]
    fn plan_soft_sets_dock_open_without_requiring_markdown_write_lock() {
        let tmp = TempDir::new().unwrap();
        let plan_md = tmp.path().join("plan.md");
        std::fs::write(&plan_md, "# Disk plan\nheld by another writer\n").unwrap();
        let _guard = try_acquire_write(&plan_md, "nested-l2-plan-md-holder")
            .expect("hold the markdown write lock for another agent");
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let before = std::fs::read_to_string(&plan_md).unwrap();
        let docked = store
            .set_session_plan_dock_open("sess-soft", SESSION_PLAN_IDENTITY, true)
            .expect("dock_open must succeed while plan.md is write-locked");
        assert!(docked, "`/plan --soft` must set session_plans.dock_open");
        assert_eq!(
            std::fs::read_to_string(&plan_md).unwrap(),
            before,
            "`/plan --soft` must not rewrite plan.md"
        );
        let loaded = store
            .load_session_plan("sess-soft", SESSION_PLAN_IDENTITY)
            .unwrap()
            .expect("dock_open row");
        assert!(loaded.dock_open);
    }

    /// Named contract: present / plan-save upserts the live body via
    /// `upsert_session_plan` and keeps Isolated Preview dock_open.
    #[test]
    fn present_upserts_session_plan_body_without_rewriting_plan_md() {
        let tmp = TempDir::new().unwrap();
        let plan_md = tmp.path().join("plan.md");
        std::fs::write(&plan_md, "# Disk leftover\nnot the live present\n").unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .set_session_plan_dock_open("sess-present", SESSION_PLAN_IDENTITY, true)
            .unwrap();
        let before = std::fs::read_to_string(&plan_md).unwrap();
        store
            .upsert_session_plan_body("sess-present", "# SQL live plan\npresent this body\n")
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&plan_md).unwrap(),
            before,
            "present must not rewrite plan.md"
        );
        let loaded = store
            .load_session_plan("sess-present", SESSION_PLAN_IDENTITY)
            .unwrap()
            .expect("present body row");
        assert!(
            loaded.body.contains("present this body"),
            "present must upsert session_plans.body; got {:?}",
            loaded.body
        );
        assert!(
            loaded.dock_open,
            "present body upsert must keep Isolated Preview dock_open"
        );
        assert!(
            !loaded.body.contains("Disk leftover"),
            "disk leftover must not become the SQL body"
        );
    }

    /// Named contract: Isolated Preview / present reads SQL first, then
    /// disk `plan.md` fallback.
    #[test]
    fn isolated_preview_and_present_read_sql_first_then_disk_plan_md_fallback() {
        let tmp = TempDir::new().unwrap();
        let plan_md = tmp.path().join("plan.md");
        std::fs::write(&plan_md, "# Disk leftover\nnot the live plan\n").unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .upsert_session_plan(
                "sess-preview",
                SESSION_PLAN_IDENTITY,
                Some("SQL title"),
                "# SQL live plan\npresent this body\n",
                true,
                r#"[{"start_line":1,"end_line":1,"text":"note"}]"#,
            )
            .unwrap();
        let sql_first = resolve_plan_body_sql_then_disk(
            &store,
            "sess-preview",
            SESSION_PLAN_IDENTITY,
            Some(&plan_md),
        )
        .expect("SQL body");
        assert!(
            sql_first.contains("present this body"),
            "Isolated Preview / present must prefer SQL over disk; got {sql_first:?}"
        );
        assert!(
            !sql_first.contains("Disk leftover"),
            "disk must not win when SQL has a body; got {sql_first:?}"
        );

        let other_md = tmp.path().join("other-plan.md");
        std::fs::write(&other_md, "# Disk fallback body\n").unwrap();
        let fallback = resolve_plan_body_sql_then_disk(
            &store,
            "sess-missing",
            SESSION_PLAN_IDENTITY,
            Some(&other_md),
        )
        .expect("disk fallback");
        assert!(
            fallback.contains("Disk fallback body"),
            "present must fall back to disk plan.md when SQL is empty; got {fallback:?}"
        );
        assert!(
            resolve_plan_body_sql_then_disk(
                &store,
                "sess-missing",
                SESSION_PLAN_IDENTITY,
                Some(&tmp.path().join("absent.md")),
            )
            .is_none(),
            "missing SQL and missing disk must not invent a plan body"
        );
    }

    /// Named contract: grok_oss.db is not the Token Economy spend ledger;
    /// `/spend` is unchanged.
    #[test]
    fn grok_oss_db_session_plans_are_not_the_token_economy_spend_ledger() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .upsert_session_plan(
                "sess-spend",
                SESSION_PLAN_IDENTITY,
                None,
                "# Plan body is not a spend event\n",
                false,
                "[]",
            )
            .unwrap();
        for name in [
            "local_usage_event",
            "remote_meter_sample",
            "reconciliation_run",
        ] {
            assert!(
                table_exists(&store, name),
                "/spend ledger table {name} must stay after v6"
            );
        }
        let usage_rows: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM local_usage_event", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            usage_rows, 0,
            "upserting session_plans must not write Token Economy local_usage_event rows"
        );
        let lower = SCHEMA_V6.to_ascii_lowercase();
        for banned in ["cost_usd_ticks", "local_usage_event", "spend"] {
            assert!(
                !lower.contains(banned),
                "session_plans schema must not become the spend ledger ({banned})"
            );
        }
        let cols: Vec<String> = {
            let mut stmt = store
                .connection()
                .prepare("PRAGMA table_info(session_plans)")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert!(
            !cols
                .iter()
                .any(|c| c.contains("cost") || c.contains("token")),
            "session_plans must not grow spend columns; got {cols:?}"
        );
    }
}
