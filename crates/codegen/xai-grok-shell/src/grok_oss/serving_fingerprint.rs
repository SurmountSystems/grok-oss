//! Official serving-path fingerprints in `grok_oss.db` (schema v7).
//!
//! Additive Surmount tables. Not the Token Economy spend ledger. `/spend`
//! still uses schema v1 `local_usage_event`, `remote_meter_sample`, and
//! `reconciliation_run`.
//!
//! Chat Completions `system_fingerprint` is backend configuration. A flip
//! means the serving path changed. It is not a SHA of the weights. Behavior
//! still decides if weights moved.
//!
//! `GET /v1/language-models` fingerprint is the xAI system configuration
//! hosting the model. See
//! [List language models](https://docs.x.ai/docs/api-reference#list-language-models)
//! (accessed: 2026-09-09).

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;
use xai_grok_sampling_types::{
    LanguageModelServing, language_models_url_from_models_list_url, parse_language_models_json,
};

use super::GrokOssStore;

/// Additive schema v7: last completion fingerprint, last language-models
/// serving row, and flip history. Does not drop prior tables.
pub(crate) const SCHEMA_V7: &str = r#"
CREATE TABLE IF NOT EXISTS completion_system_fingerprint (
  model_id TEXT NOT NULL PRIMARY KEY,
  system_fingerprint TEXT NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS language_model_serving (
  model_id TEXT NOT NULL PRIMARY KEY,
  fingerprint TEXT,
  version TEXT,
  created INTEGER,
  aliases_json TEXT NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS serving_fingerprint_flip (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,
  model_id TEXT NOT NULL,
  previous_fingerprint TEXT,
  new_fingerprint TEXT NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_serving_fingerprint_flip_model
  ON serving_fingerprint_flip(model_id, observed_at);
"#;

pub const SOURCE_COMPLETION: &str = "completion";
pub const SOURCE_LANGUAGE_MODELS: &str = "language_models";

/// Last stored serving-path fields for one sampling model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServingMetadataSnapshot {
    pub public_id: String,
    pub completion_system_fingerprint: Option<String>,
    pub completion_observed_at: Option<String>,
    pub language_models_id: Option<String>,
    pub language_models_fingerprint: Option<String>,
    pub language_models_version: Option<String>,
    pub language_models_created: Option<i64>,
    pub language_models_observed_at: Option<String>,
}

/// Apply additive v7 serving-fingerprint tables without dropping prior tables.
pub(crate) fn apply_schema_v7(store: &GrokOssStore) -> Result<()> {
    store
        .connection()
        .execute_batch(SCHEMA_V7)
        .context("apply schema v7 serving fingerprints")?;
    Ok(())
}

impl GrokOssStore {
    fn ensure_serving_fingerprint_tables(&self) -> Result<()> {
        apply_schema_v7(self)
    }

    /// Store a Chat Completions `system_fingerprint` for `model_id`.
    /// Returns true when this observation is a flip (previous value differed).
    pub fn record_completion_system_fingerprint(
        &self,
        model_id: &str,
        system_fingerprint: &str,
        observed_at: Option<&str>,
    ) -> Result<bool> {
        if model_id.is_empty() || system_fingerprint.is_empty() {
            return Ok(false);
        }
        self.ensure_serving_fingerprint_tables()?;
        let observed = observed_at
            .map(str::to_owned)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let previous: Option<String> = self
            .connection()
            .query_row(
                "SELECT system_fingerprint FROM completion_system_fingerprint WHERE model_id = ?1",
                rusqlite::params![model_id],
                |row| row.get(0),
            )
            .optional()
            .context("load previous completion system_fingerprint")?;
        let flipped = previous
            .as_deref()
            .is_some_and(|prev| prev != system_fingerprint);
        if flipped {
            self.insert_flip(
                SOURCE_COMPLETION,
                model_id,
                previous.as_deref(),
                system_fingerprint,
                &observed,
            )?;
        }
        if previous.is_none() || flipped {
            tracing::info!(
                model_id,
                system_fingerprint,
                flipped,
                "Chat Completions system_fingerprint (serving path; not a SHA of the weights)"
            );
        } else {
            tracing::debug!(
                model_id,
                system_fingerprint,
                "Chat Completions system_fingerprint unchanged"
            );
        }
        self.connection()
            .execute(
                "INSERT INTO completion_system_fingerprint
                   (model_id, system_fingerprint, observed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(model_id) DO UPDATE SET
                   system_fingerprint = excluded.system_fingerprint,
                   observed_at = excluded.observed_at",
                rusqlite::params![model_id, system_fingerprint, observed],
            )
            .context("upsert completion_system_fingerprint")?;
        Ok(flipped)
    }

    /// Persist parsed language-models serving rows. Returns how many flips.
    pub fn persist_language_models_list(
        &self,
        models: &[LanguageModelServing],
        observed_at: Option<&str>,
    ) -> Result<usize> {
        self.ensure_serving_fingerprint_tables()?;
        let observed = observed_at
            .map(str::to_owned)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let mut flips = 0usize;
        for model in models {
            if model.id.is_empty() {
                continue;
            }
            if self.upsert_language_model_row(model, &observed)? {
                flips += 1;
            }
        }
        Ok(flips)
    }

    /// Parse official language-models JSON and persist serving rows.
    pub fn persist_language_models_json(
        &self,
        json: &str,
        observed_at: Option<&str>,
    ) -> Result<usize> {
        let list = parse_language_models_json(json).context("parse language-models JSON")?;
        self.persist_language_models_list(&list.models, observed_at)
    }

    /// Last stored serving fields for a public model id or alias.
    pub fn serving_snapshot_for_model(
        &self,
        model_id_or_alias: &str,
    ) -> Result<Option<ServingMetadataSnapshot>> {
        if model_id_or_alias.is_empty() {
            return Ok(None);
        }
        self.ensure_serving_fingerprint_tables()?;
        let language = self
            .load_language_model_row(model_id_or_alias)?
            .or_else(|| self.find_language_model_by_alias(model_id_or_alias).ok()?);
        let mut completion = self.load_completion_row(model_id_or_alias)?;
        if completion.is_none() {
            if let Some(row) = language.as_ref() {
                if row.model_id != model_id_or_alias {
                    completion = self.load_completion_row(&row.model_id)?;
                }
                if completion.is_none() {
                    let aliases: Vec<String> =
                        serde_json::from_str(&row.aliases_json).unwrap_or_default();
                    for alias in aliases {
                        if alias == model_id_or_alias {
                            continue;
                        }
                        if let Some(stored) = self.load_completion_row(&alias)? {
                            completion = Some(stored);
                            break;
                        }
                    }
                }
            }
        }
        if completion.is_none() && language.is_none() {
            return Ok(None);
        }
        let public_id = language
            .as_ref()
            .map(|row| row.model_id.clone())
            .unwrap_or_else(|| model_id_or_alias.to_owned());
        Ok(Some(ServingMetadataSnapshot {
            public_id,
            completion_system_fingerprint: completion.as_ref().map(|c| c.0.clone()),
            completion_observed_at: completion.as_ref().map(|c| c.1.clone()),
            language_models_id: language.as_ref().map(|r| r.model_id.clone()),
            language_models_fingerprint: language.as_ref().and_then(|r| r.fingerprint.clone()),
            language_models_version: language.as_ref().and_then(|r| r.version.clone()),
            language_models_created: language.as_ref().and_then(|r| r.created),
            language_models_observed_at: language.as_ref().map(|r| r.observed_at.clone()),
        }))
    }

    /// Latest flip for tests: (previous, new).
    pub fn latest_serving_fingerprint_flip(
        &self,
        source: &str,
        model_id: &str,
    ) -> Result<Option<(Option<String>, String)>> {
        self.connection()
            .query_row(
                "SELECT previous_fingerprint, new_fingerprint
                 FROM serving_fingerprint_flip
                 WHERE source = ?1 AND model_id = ?2
                 ORDER BY id DESC LIMIT 1",
                rusqlite::params![source, model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("load serving_fingerprint_flip")
    }

    fn upsert_language_model_row(
        &self,
        model: &LanguageModelServing,
        observed_at: &str,
    ) -> Result<bool> {
        let fingerprint = model.serving_fingerprint().map(str::to_owned);
        let previous: Option<String> = self
            .connection()
            .query_row(
                "SELECT fingerprint FROM language_model_serving WHERE model_id = ?1",
                rusqlite::params![model.id],
                |row| row.get(0),
            )
            .optional()
            .context("load previous language-models fingerprint")?;
        let flipped = match (
            previous.as_deref().filter(|s| !s.is_empty()),
            fingerprint.as_deref(),
        ) {
            (Some(prev), Some(new)) if prev != new => {
                self.insert_flip(
                    SOURCE_LANGUAGE_MODELS,
                    &model.id,
                    Some(prev),
                    new,
                    observed_at,
                )?;
                true
            }
            _ => false,
        };
        let aliases_json =
            serde_json::to_string(&model.aliases).unwrap_or_else(|_| "[]".to_string());
        let version = model.version_str();
        self.connection()
            .execute(
                "INSERT INTO language_model_serving
                   (model_id, fingerprint, version, created, aliases_json, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(model_id) DO UPDATE SET
                   fingerprint = excluded.fingerprint,
                   version = excluded.version,
                   created = excluded.created,
                   aliases_json = excluded.aliases_json,
                   observed_at = excluded.observed_at",
                rusqlite::params![
                    model.id,
                    fingerprint,
                    version,
                    model.created,
                    aliases_json,
                    observed_at
                ],
            )
            .context("upsert language_model_serving")?;
        Ok(flipped)
    }

    fn load_completion_row(&self, model_id: &str) -> Result<Option<(String, String)>> {
        self.connection()
            .query_row(
                "SELECT system_fingerprint, observed_at
                 FROM completion_system_fingerprint WHERE model_id = ?1",
                rusqlite::params![model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("load completion_system_fingerprint")
    }

    fn load_language_model_row(&self, model_id: &str) -> Result<Option<LanguageModelRow>> {
        self.connection()
            .query_row(
                "SELECT model_id, fingerprint, version, created, aliases_json, observed_at
                 FROM language_model_serving WHERE model_id = ?1",
                rusqlite::params![model_id],
                row_to_language_model,
            )
            .optional()
            .context("load language_model_serving")
    }

    fn find_language_model_by_alias(&self, alias: &str) -> Result<Option<LanguageModelRow>> {
        let mut stmt = self
            .connection()
            .prepare(
                "SELECT model_id, fingerprint, version, created, aliases_json, observed_at
                 FROM language_model_serving",
            )
            .context("prepare language_model_serving scan")?;
        let rows = stmt
            .query_map([], row_to_language_model)
            .context("scan language_model_serving")?;
        for row in rows {
            let row = row.context("language_model_serving row")?;
            let aliases: Vec<String> = serde_json::from_str(&row.aliases_json).unwrap_or_default();
            if aliases.iter().any(|a| a == alias) {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }

    fn insert_flip(
        &self,
        source: &str,
        model_id: &str,
        previous: Option<&str>,
        new: &str,
        observed_at: &str,
    ) -> Result<()> {
        tracing::info!(
            source,
            model_id,
            previous,
            new,
            "serving fingerprint flipped (serving path changed; not a SHA of the weights)"
        );
        self.connection()
            .execute(
                "INSERT INTO serving_fingerprint_flip
                   (source, model_id, previous_fingerprint, new_fingerprint, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![source, model_id, previous, new, observed_at],
            )
            .context("insert serving_fingerprint_flip")?;
        Ok(())
    }
}

struct LanguageModelRow {
    model_id: String,
    fingerprint: Option<String>,
    version: Option<String>,
    created: Option<i64>,
    aliases_json: String,
    observed_at: String,
}

fn row_to_language_model(row: &rusqlite::Row<'_>) -> rusqlite::Result<LanguageModelRow> {
    Ok(LanguageModelRow {
        model_id: row.get(0)?,
        fingerprint: row.get(1)?,
        version: row.get(2)?,
        created: row.get(3)?,
        aliases_json: row.get(4)?,
        observed_at: row.get(5)?,
    })
}

/// Fail-open persist of a completion `system_fingerprint`.
///
/// Tests skip the operator store unless `[token_economy] grok_oss_database_path`
/// is overridden. After a successful write, one process-wide fail-open refresh
/// of `GET /v1/language-models` may run (not in crate tests).
pub fn record_completion_system_fingerprint_fail_open(model_id: &str, system_fingerprint: &str) {
    if model_id.is_empty() || system_fingerprint.is_empty() {
        return;
    }
    let cfg = crate::token_economy::token_economy_from_disk();
    if cfg!(test) && cfg.grok_oss_database_path.is_none() {
        return;
    }
    let Some(store) = super::try_open_from_token_economy_config(&cfg) else {
        return;
    };
    if let Err(e) = store.record_completion_system_fingerprint(model_id, system_fingerprint, None) {
        tracing::debug!(
            error = %e,
            model_id,
            "completion system_fingerprint persist failed (fail-open)"
        );
        return;
    }
    try_refresh_language_models_fail_open();
}

/// Fail-open lookup of stored serving metadata for `/metadata`.
pub fn lookup_serving_snapshot_fail_open(
    model_id_or_alias: &str,
) -> Option<ServingMetadataSnapshot> {
    if model_id_or_alias.is_empty() {
        return None;
    }
    let cfg = crate::token_economy::token_economy_from_disk();
    if cfg!(test) && cfg.grok_oss_database_path.is_none() {
        return None;
    }
    let store = super::try_open_from_token_economy_config(&cfg)?;
    match store.serving_snapshot_for_model(model_id_or_alias) {
        Ok(row) => row,
        Err(e) => {
            tracing::debug!(
                error = %e,
                model_id_or_alias,
                "serving snapshot lookup failed (fail-open)"
            );
            None
        }
    }
}

fn try_refresh_language_models_fail_open() {
    if cfg!(test) {
        return;
    }
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Err(e) = refresh_language_models_once() {
            tracing::debug!(error = %e, "language-models refresh failed (fail-open)");
        }
    });
}

fn refresh_language_models_once() -> Result<()> {
    let endpoints = crate::agent::config::EndpointsConfig::from_effective_config();
    let models_url = format!(
        "{}/models",
        endpoints.xai_api_base_url.trim_end_matches('/')
    );
    let url = language_models_url_from_models_list_url(&models_url)
        .context("language-models URL from models list")?;
    let api_key = crate::agent::auth_method::read_xai_api_key_env()
        .context("XAI_API_KEY for language-models")?;
    let client = crate::http::shared_startup_blocking_client();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .context("GET language-models")?;
    if !response.status().is_success() {
        anyhow::bail!("language-models HTTP {}", response.status());
    }
    let body = response.text().context("language-models body")?;
    let cfg = crate::token_economy::token_economy_from_disk();
    let store =
        super::open_from_token_economy_config(&cfg).context("open grok_oss for language-models")?;
    store.persist_language_models_json(&body, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::{SCHEMA_VERSION, open_at};
    use tempfile::TempDir;

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

    fn stamp_v6_file(path: &std::path::Path) {
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
        conn.execute_batch(crate::grok_oss::SCHEMA_V1).unwrap();
        conn.execute_batch(crate::grok_oss::SCHEMA_V2).unwrap();
        conn.execute_batch(crate::grok_oss::SCHEMA_V3).unwrap();
        conn.execute_batch(crate::grok_oss::SCHEMA_V4).unwrap();
        conn.execute_batch(crate::grok_oss::SCHEMA_V5).unwrap();
        conn.execute_batch(crate::grok_oss::session_plans::SCHEMA_V6)
            .unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '6')",
            [],
        )
        .unwrap();
    }

    /// Named contract: additive v7 creates serving-fingerprint tables without
    /// dropping `/spend` schema v1 tables.
    #[test]
    fn migrate_v6_file_to_v7_adds_serving_tables_without_dropping_spend() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("grok_oss.db");
        stamp_v6_file(&path);
        let store = open_at(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(
            SCHEMA_VERSION, 7,
            "serving fingerprints are additive schema v7"
        );
        for name in [
            "completion_system_fingerprint",
            "language_model_serving",
            "serving_fingerprint_flip",
            "local_usage_event",
            "remote_meter_sample",
            "reconciliation_run",
            "session_plans",
        ] {
            assert!(table_exists(&store, name), "expected table {name}");
        }
    }

    /// Named contract: persist a flip when completion system_fingerprint changes.
    #[test]
    fn record_completion_fingerprint_persists_a_flip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let first = store
            .record_completion_system_fingerprint(
                "grok-4.6",
                "fp_aaa",
                Some("2026-09-09T00:00:00Z"),
            )
            .unwrap();
        assert!(!first, "first observation is not a flip");
        let flipped = store
            .record_completion_system_fingerprint(
                "grok-4.6",
                "fp_bbb",
                Some("2026-09-09T01:00:00Z"),
            )
            .unwrap();
        assert!(flipped, "second distinct fingerprint must persist a flip");
        let snap = store
            .serving_snapshot_for_model("grok-4.6")
            .unwrap()
            .expect("snapshot");
        assert_eq!(
            snap.completion_system_fingerprint.as_deref(),
            Some("fp_bbb")
        );
        let flip = store
            .latest_serving_fingerprint_flip(SOURCE_COMPLETION, "grok-4.6")
            .unwrap()
            .expect("flip row");
        assert_eq!(flip.0.as_deref(), Some("fp_aaa"));
        assert_eq!(flip.1, "fp_bbb");
    }

    /// Named contract: persist language-models JSON and record a fingerprint flip.
    #[test]
    fn persist_language_models_list_upserts_and_records_flip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let json_a = r#"{
          "models": [{
            "id": "latest",
            "fingerprint": "fp_old",
            "version": "1.0",
            "created": 1776556800,
            "aliases": ["grok-4.6"]
          }]
        }"#;
        let json_b = r#"{
          "models": [{
            "id": "latest",
            "fingerprint": "fp_new",
            "version": "1.0",
            "created": 1776556800,
            "aliases": ["grok-4.6"]
          }]
        }"#;
        assert_eq!(store.persist_language_models_json(json_a, None).unwrap(), 0);
        assert_eq!(store.persist_language_models_json(json_b, None).unwrap(), 1);
        let snap = store
            .serving_snapshot_for_model("grok-4.6")
            .unwrap()
            .expect("alias match");
        assert_eq!(snap.language_models_id.as_deref(), Some("latest"));
        assert_eq!(snap.language_models_fingerprint.as_deref(), Some("fp_new"));
        assert_eq!(snap.language_models_version.as_deref(), Some("1.0"));
        assert_eq!(snap.language_models_created, Some(1_776_556_800));
        let flip = store
            .latest_serving_fingerprint_flip(SOURCE_LANGUAGE_MODELS, "latest")
            .unwrap()
            .expect("language-models flip");
        assert_eq!(flip.0.as_deref(), Some("fp_old"));
        assert_eq!(flip.1, "fp_new");
    }

    /// Named contract: `/metadata` lookup by public alias still finds a
    /// completion fingerprint stored under the language-models id.
    #[test]
    fn serving_snapshot_for_alias_finds_completion_stored_under_language_models_id() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        store
            .record_completion_system_fingerprint(
                "latest",
                "fp_84ff176447",
                Some("2026-09-09T00:00:00Z"),
            )
            .unwrap();
        store
            .persist_language_models_json(
                r#"{
                  "models": [{
                    "id": "latest",
                    "fingerprint": "fp_777a9f8466",
                    "version": "1.0",
                    "created": 1776556800,
                    "aliases": ["grok-4.6"]
                  }]
                }"#,
                Some("2026-09-09T00:05:00Z"),
            )
            .unwrap();
        let snap = store
            .serving_snapshot_for_model("grok-4.6")
            .unwrap()
            .expect("alias snapshot");
        assert_eq!(snap.public_id, "latest");
        assert_eq!(
            snap.completion_system_fingerprint.as_deref(),
            Some("fp_84ff176447")
        );
        assert_eq!(
            snap.language_models_fingerprint.as_deref(),
            Some("fp_777a9f8466")
        );
    }
}
