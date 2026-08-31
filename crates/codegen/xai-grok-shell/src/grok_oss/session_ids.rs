//! 1:1 ACP session UUID ↔ grok-oss session ULID map in `grok_oss.db`.
//!
//! Additive Surmount table (schema v5). Not `{session_dir}/work_ulid`.
//! The ACP / Grok Build wire session id stays a UUID.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;

use super::GrokOssStore;

/// One mapped pair: ACP session UUID and grok-oss session ULID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdPair {
    pub session_uuid: String,
    pub session_ulid: String,
    pub created_at: String,
}

impl GrokOssStore {
    /// Lookup-or-insert: same UUID always returns the same ULID.
    ///
    /// Mints a 26-character Crockford ULID when the UUID is new.
    /// Rejects an empty UUID. Does not reinterpret UUID bytes as a ULID.
    pub fn ensure_session_ids(&self, session_uuid: &str) -> Result<SessionIdPair> {
        if session_uuid.is_empty() {
            anyhow::bail!("session_uuid must not be empty");
        }
        if let Some(pair) = self.lookup_by_uuid(session_uuid)? {
            return Ok(pair);
        }
        let session_ulid = xai_grok_tools::util::ulid::mint();
        if !xai_grok_tools::util::ulid::is_valid(&session_ulid) {
            anyhow::bail!("minted session_ulid is not a valid Crockford ULID");
        }
        let created_at = Utc::now().to_rfc3339();
        match self.connection().execute(
            "INSERT INTO session_id_map (session_uuid, session_ulid, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![session_uuid, session_ulid, created_at],
        ) {
            Ok(_) => Ok(SessionIdPair {
                session_uuid: session_uuid.to_owned(),
                session_ulid,
                created_at,
            }),
            Err(e) => {
                if let Some(pair) = self.lookup_by_uuid(session_uuid)? {
                    return Ok(pair);
                }
                Err(e).context("insert session_id_map")
            }
        }
    }

    /// Pair for this ACP session UUID, if mapped.
    pub fn lookup_by_uuid(&self, session_uuid: &str) -> Result<Option<SessionIdPair>> {
        if session_uuid.is_empty() {
            return Ok(None);
        }
        self.load_pair(
            "SELECT session_uuid, session_ulid, created_at
             FROM session_id_map WHERE session_uuid = ?1",
            session_uuid,
        )
    }

    /// Pair for this grok-oss session ULID, if mapped.
    pub fn lookup_by_ulid(&self, session_ulid: &str) -> Result<Option<SessionIdPair>> {
        if session_ulid.is_empty() {
            return Ok(None);
        }
        self.load_pair(
            "SELECT session_uuid, session_ulid, created_at
             FROM session_id_map WHERE session_ulid = ?1",
            session_ulid,
        )
    }

    fn load_pair(&self, sql: &str, key: &str) -> Result<Option<SessionIdPair>> {
        self.connection()
            .query_row(sql, rusqlite::params![key], |row| {
                Ok(SessionIdPair {
                    session_uuid: row.get(0)?,
                    session_ulid: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })
            .optional()
            .context("load session_id_map")
    }
}

/// Fail-open map insert for a new ACP session UUID.
///
/// Opens grok_oss from live Token Economy config. Tests skip unless
/// `grok_oss_database_path` is overridden so crate tests do not write the
/// operator store. Does not change the wire session id.
pub fn ensure_session_ids_fail_open(session_uuid: &str) {
    if session_uuid.is_empty() {
        return;
    }
    let cfg = crate::token_economy::token_economy_from_disk();
    if cfg!(test) && cfg.grok_oss_database_path.is_none() {
        return;
    }
    let Some(store) = super::try_open_from_token_economy_config(&cfg) else {
        return;
    };
    if let Err(e) = store.ensure_session_ids(session_uuid) {
        tracing::debug!(
            error = %e,
            session_uuid,
            "session_id_map ensure failed (fail-open)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    fn sample_uuid() -> String {
        uuid::Uuid::now_v7().to_string()
    }

    #[test]
    fn ensure_session_ids_mints_ulid_and_round_trips() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let uuid = sample_uuid();
        let pair = store.ensure_session_ids(&uuid).unwrap();
        assert_eq!(pair.session_uuid, uuid);
        assert_eq!(pair.session_ulid.len(), 26);
        assert!(
            xai_grok_tools::util::ulid::is_valid(&pair.session_ulid),
            "minted session_ulid must be a valid Crockford ULID"
        );
        assert!(!pair.created_at.is_empty());

        let by_uuid = store.lookup_by_uuid(&uuid).unwrap().expect("by uuid");
        assert_eq!(by_uuid, pair);
        let by_ulid = store
            .lookup_by_ulid(&pair.session_ulid)
            .unwrap()
            .expect("by ulid");
        assert_eq!(by_ulid, pair);
    }

    #[test]
    fn ensure_session_ids_same_uuid_returns_same_ulid() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let uuid = sample_uuid();
        let first = store.ensure_session_ids(&uuid).unwrap();
        let second = store.ensure_session_ids(&uuid).unwrap();
        assert_eq!(first.session_ulid, second.session_ulid);
        assert_eq!(first.session_uuid, second.session_uuid);
        assert_eq!(first, second);
    }

    #[test]
    fn ensure_session_ids_distinct_uuids_get_distinct_ulids() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let a = store.ensure_session_ids(&sample_uuid()).unwrap();
        let b = store.ensure_session_ids(&sample_uuid()).unwrap();
        assert_ne!(a.session_uuid, b.session_uuid);
        assert_ne!(a.session_ulid, b.session_ulid);
    }

    #[test]
    fn ensure_session_ids_rejects_empty_uuid() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let err = store.ensure_session_ids("").unwrap_err();
        assert!(
            err.to_string().contains("empty"),
            "empty uuid must be rejected: {err}"
        );
        assert!(store.lookup_by_uuid("").unwrap().is_none());
    }

    #[test]
    fn attach_and_new_session_both_call_ensure_session_ids_fail_open() {
        let src = include_str!("../agent/mvp_agent/session_setup.rs");
        let count = src.matches("ensure_session_ids_fail_open").count();
        assert!(
            count >= 2,
            "new_session and load/attach must both map UUID sessions; found {count} calls"
        );
        assert!(
            src.contains("crate::grok_oss::ensure_session_ids_fail_open(session_id.0.as_ref())"),
            "load/attach must map existing UUID sessions without changing acp::SessionId"
        );
    }

    #[test]
    #[serial_test::serial(TOKEN_ECONOMY_LIVE)]
    fn existing_uuid_session_gets_mapped_ulid_on_load_path_helper() {
        let tmp = TempDir::new().unwrap();
        let db = tmp.path().join("grok_oss.db");
        crate::token_economy::set_token_economy_live(crate::token_economy::TokenEconomyConfig {
            grok_oss_database_path: Some(db.clone()),
            ..crate::token_economy::TokenEconomyConfig::default()
        });
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                crate::token_economy::reset_token_economy_live_to_defaults();
            }
        }
        let _guard = Guard;
        let uuid = sample_uuid();
        ensure_session_ids_fail_open(&uuid);
        let store = open_at(&db).unwrap();
        let pair = store
            .lookup_by_uuid(&uuid)
            .unwrap()
            .expect("load-path helper must persist a mapped ULID");
        assert_eq!(pair.session_uuid, uuid);
        assert_eq!(pair.session_ulid.len(), 26);
        assert!(xai_grok_tools::util::ulid::is_valid(&pair.session_ulid));
        ensure_session_ids_fail_open(&uuid);
        let again = store.lookup_by_uuid(&uuid).unwrap().unwrap();
        assert_eq!(again.session_ulid, pair.session_ulid);
    }
}
