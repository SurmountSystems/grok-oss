//! Uniquely grok-oss durable SQLite store (`$GROK_HOME/grok_oss.db`).
//!
//! Not an upstream session database. Session trees stay directory + jsonl.
//! Token Economy ledger tables are the first schema family; later Surmount-only
//! durable state can add tables via additive migrations.
//!
//! Open is multiproc-safe (busy timeout via journal helper) and **fail-open**
//! for callers that treat open/write errors as non-fatal.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use xai_sqlite_journal::JournalMode;

/// Current schema version stamped in `meta`.
pub const SCHEMA_VERSION: i64 = 1;

/// Default filename under `$GROK_HOME`.
pub const GROK_OSS_DB_FILE: &str = "grok_oss.db";

/// Default path: `$GROK_HOME/grok_oss.db`.
pub fn default_database_path() -> PathBuf {
    xai_grok_config::grok_home().join(GROK_OSS_DB_FILE)
}

/// Open (or create) the uniquely grok-oss store at `path`.
///
/// Creates parent dirs, applies NFS-safe journal mode, runs additive migrations.
/// Does **not** store secrets.
pub fn open_at(path: &Path) -> Result<GrokOssStore> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir for grok_oss.db: {}", parent.display()))?;
    }
    let journal_mode = JournalMode::for_db_path(path);
    let effective = journal_mode.effective_db_path(path);
    let conn = journal_mode
        .open(&effective)
        .with_context(|| format!("open grok_oss.db: {}", effective.display()))?;
    let store = GrokOssStore {
        conn,
        path: effective,
    };
    store.migrate()?;
    Ok(store)
}

/// Open at default `$GROK_HOME/grok_oss.db`.
pub fn open_default() -> Result<GrokOssStore> {
    open_at(&default_database_path())
}

/// Open at path from Token Economy config (override or default).
pub fn open_from_token_economy_config(
    cfg: &crate::token_economy::config::TokenEconomyConfig,
) -> Result<GrokOssStore> {
    let path = crate::token_economy::config::resolve_grok_oss_database_path(cfg);
    open_at(&path)
}

/// Fail-open open: `None` on any error (logs at debug).
pub fn try_open_at(path: &Path) -> Option<GrokOssStore> {
    match open_at(path) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::debug!(error = %e, path = %path.display(), "grok_oss.db open failed (fail-open)");
            None
        }
    }
}

/// Fail-open open using Token Economy config.
pub fn try_open_from_token_economy_config(
    cfg: &crate::token_economy::config::TokenEconomyConfig,
) -> Option<GrokOssStore> {
    let path = crate::token_economy::config::resolve_grok_oss_database_path(cfg);
    try_open_at(&path)
}

/// Live connection to `grok_oss.db`.
pub struct GrokOssStore {
    conn: Connection,
    path: PathBuf,
}

impl GrokOssStore {
    /// Absolute path actually opened (may be per-host sibling on network FS).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Borrow the connection (ledger / future features).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Mutable connection for writes.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Schema version in `meta`, or 0 if missing.
    pub fn schema_version(&self) -> Result<i64> {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional_context()?;
        Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#,
            )
            .context("create meta")?;

        let version = self.schema_version().unwrap_or(0);
        if version < 1 {
            self.conn
                .execute_batch(SCHEMA_V1)
                .context("apply schema v1")?;
            self.conn
                .execute(
                    "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [SCHEMA_VERSION.to_string()],
                )
                .context("stamp schema_version")?;
        }
        Ok(())
    }
}

/// Token Economy tables (schema version 1). Additive only.
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS local_usage_event (
  event_ulid TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  work_ulid TEXT,
  timestamp_utc TEXT NOT NULL,
  turn_type TEXT NOT NULL,
  agent_kind TEXT NOT NULL,
  model_id TEXT,
  input_tokens INTEGER,
  output_tokens INTEGER,
  cached_tokens INTEGER,
  reasoning_tokens INTEGER,
  total_tokens INTEGER,
  cost_usd_ticks INTEGER,
  cost_missing INTEGER NOT NULL,
  incomplete INTEGER NOT NULL,
  sampling_identity TEXT,
  ingested_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_local_usage_session ON local_usage_event(session_id);
CREATE INDEX IF NOT EXISTS idx_local_usage_ts ON local_usage_event(timestamp_utc);

CREATE TABLE IF NOT EXISTS remote_meter_sample (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL,
  sampled_at TEXT NOT NULL,
  window_start TEXT,
  window_end TEXT,
  payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_remote_meter_source_ts ON remote_meter_sample(source, sampled_at);

CREATE TABLE IF NOT EXISTS reconciliation_run (
  id INTEGER PRIMARY KEY,
  ran_at TEXT NOT NULL,
  window_start TEXT NOT NULL,
  window_end TEXT NOT NULL,
  local_cost_usd_ticks INTEGER,
  local_events INTEGER NOT NULL,
  local_cost_missing_events INTEGER NOT NULL,
  remote_api_class_usd_cents INTEGER,
  remote_oauth_class_usd_cents INTEGER,
  notes TEXT
);
"#;

/// Helper: query_row optional without pulling rusqlite OptionalExtension everywhere.
trait OptionalContext<T> {
    fn optional_context(self) -> Result<Option<T>>;
}

impl<T> OptionalContext<T> for rusqlite::Result<T> {
    fn optional_context(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_creates_schema_and_version() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("grok_oss.db");
        let store = open_at(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
        // Tables exist
        let n: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='local_usage_event'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n2: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_meter_sample'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n2, 1);
    }

    #[test]
    fn reopen_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("grok_oss.db");
        {
            let s = open_at(&path).unwrap();
            assert_eq!(s.schema_version().unwrap(), 1);
        }
        let s2 = open_at(&path).unwrap();
        assert_eq!(s2.schema_version().unwrap(), 1);
    }

    #[test]
    fn path_override_honored() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("custom.db");
        let store = open_at(&path).unwrap();
        assert!(
            store.path().ends_with("custom.db")
                || store.path().to_string_lossy().contains("custom")
        );
        assert!(
            store.path().exists()
                || JournalMode::for_db_path(&path)
                    .effective_db_path(&path)
                    .exists()
        );
    }

    #[test]
    fn no_secret_columns_in_schema() {
        // Guard: schema SQL must not mention secret-like columns.
        let lower = SCHEMA_V1.to_ascii_lowercase();
        for banned in [
            "api_key",
            "jwt",
            "password",
            "bearer",
            "management_key",
            "secret",
        ] {
            assert!(
                !lower.contains(banned),
                "schema must not store secrets ({banned})"
            );
        }
    }
}
