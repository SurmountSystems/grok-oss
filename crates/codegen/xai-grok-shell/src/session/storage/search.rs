//! Binds the `xai-grok-session-search` index to this crate's JSONL session
//! store: a process-wide manager plus the two entry points the rest of the
//! shell calls.
//!
//! Everything below the seam (the SQLite FTS5 cache, the cross-process
//! bootstrap lease, the debounced upsert worker) lives in the crate; this
//! module supplies the store binding and re-exports the request/response
//! types at their original paths.

use std::io;
use std::path::Path;
use std::sync::LazyLock;

use agent_client_protocol as acp;

use super::StorageAdapter;
use super::jsonl::JsonlStorageAdapter;
use crate::session::info::Info;
use crate::session::persistence::Summary;
use xai_grok_session_search::{IndexableSession, SearchIndexManager, SessionSource};

pub use xai_grok_session_search::{SearchIndexStatus, SessionSearchRequest, SessionSearchResponse};

/// Global singleton — lazily started on first use.
///
/// Requires an active tokio runtime on first access (spawns tasks).
pub static SEARCH_INDEX_MANAGER: LazyLock<SearchIndexManager> = LazyLock::new(|| {
    SearchIndexManager::start(
        |root| -> Box<dyn SessionSource> {
            Box::new(JsonlSessionSource(JsonlStorageAdapter::with_root(root)))
        },
        super::search_content::collect_all_indexable_content_single_pass,
    )
});

/// Projects the JSONL store's `Summary` down to the handful of fields the
/// index reads, so the index never sees the full session record.
struct JsonlSessionSource(JsonlStorageAdapter);

impl JsonlSessionSource {
    fn to_indexable(&self, summary: &Summary) -> IndexableSession {
        IndexableSession {
            session_id: summary.info.id.to_string(),
            cwd: summary.info.cwd.clone(),
            updated_at_unix: summary.updated_at.timestamp(),
            title: summary.display_title().to_owned(),
            updates_path: self.0.updates_file_path(&summary.info),
        }
    }
}

#[async_trait::async_trait]
impl SessionSource for JsonlSessionSource {
    async fn list_sessions(&self) -> io::Result<Vec<IndexableSession>> {
        let summaries = self.0.list_sessions(None).await?;
        Ok(summaries.iter().map(|s| self.to_indexable(s)).collect())
    }

    async fn load_session(
        &self,
        session_id: &str,
        cwd: &str,
    ) -> io::Result<Option<IndexableSession>> {
        let info = Info {
            id: acp::SessionId::new(session_id.to_string()),
            cwd: cwd.to_string(),
        };
        match self.0.load_summary(&info).await {
            Ok(summary) => Ok(Some(self.to_indexable(&summary))),
            // A missing session is a delete, not a failure: the index drops
            // its row. Every other error leaves the row alone.
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Session search stays on unless the operator turns it off.
///
/// `GROK_SESSION_SEARCH` of `0` or `false` (any ASCII case), or config
/// `[features] session_search = false`, keeps the index off. Unset env and
/// absent config stay on. Either explicit off wins. Callers must check this
/// before touching [`SEARCH_INDEX_MANAGER`], or the lazy index starts.
fn session_search_enabled() -> bool {
    if env_disables_session_search(std::env::var("GROK_SESSION_SEARCH").ok().as_deref()) {
        return false;
    }
    !config_disables_session_search()
}

fn env_disables_session_search(value: Option<&str>) -> bool {
    let Some(raw) = value else {
        return false;
    };
    let trimmed = raw.trim();
    trimmed.eq_ignore_ascii_case("0") || trimmed.eq_ignore_ascii_case("false")
}

fn config_disables_session_search() -> bool {
    let Ok(layers) = crate::config::ConfigLayers::load() else {
        return false;
    };
    config_value_disables_session_search(&layers.user)
        || config_value_disables_session_search(&layers.managed)
        || config_value_disables_session_search(&layers.system_managed)
        || layers
            .user_requirements
            .as_ref()
            .is_some_and(config_value_disables_session_search)
        || layers
            .system_requirements
            .as_ref()
            .is_some_and(config_value_disables_session_search)
        || layers
            .mdm_requirements
            .as_ref()
            .is_some_and(config_value_disables_session_search)
}

fn config_value_disables_session_search(config: &toml::Value) -> bool {
    config
        .get("features")
        .and_then(|features| features.get("session_search"))
        .and_then(toml::Value::as_bool)
        == Some(false)
}

fn empty_session_search_response() -> SessionSearchResponse {
    SessionSearchResponse {
        results: Vec::new(),
        next_offset: None,
        total_estimate: Some(0),
        bootstrapping: false,
    }
}

/// Trigger indexing for a session that was just saved or updated.
pub fn notify_session_updated(session_id: &str, cwd: &str) {
    if !session_search_enabled() {
        return;
    }
    let root = crate::util::grok_home::grok_home();
    SEARCH_INDEX_MANAGER.enqueue(root, session_id.to_string(), cwd.to_string());
}

/// Execute a session search query against the shared index.
pub async fn execute_search(
    root_dir: &Path,
    req: &SessionSearchRequest,
) -> io::Result<SessionSearchResponse> {
    if !session_search_enabled() {
        return Ok(empty_session_search_response());
    }
    xai_grok_session_search::execute_search(&SEARCH_INDEX_MANAGER, root_dir, req).await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::session::storage::search_content::test_summary;

    fn indexable_of(summary: &Summary) -> IndexableSession {
        JsonlSessionSource(JsonlStorageAdapter::with_root(PathBuf::from(
            "/nonexistent",
        )))
        .to_indexable(summary)
    }

    /// The index stores one title; the store decides which one, and a
    /// generated title outranks the session summary.
    #[test]
    fn indexable_prefers_generated_title() {
        let mut summary = test_summary("s1", "/workspace", "session summary");
        summary.generated_title = Some("Generated Title".to_string());
        assert_eq!(indexable_of(&summary).title, "Generated Title");

        summary.generated_title = Some(String::new());
        assert_eq!(indexable_of(&summary).title, "session summary");
    }

    #[test]
    fn indexable_carries_identity_and_recency() {
        let summary = test_summary("s1", "/workspace", "a title");
        let session = indexable_of(&summary);
        assert_eq!(session.session_id, "s1");
        assert_eq!(session.cwd, "/workspace");
        assert_eq!(session.updated_at_unix, summary.updated_at.timestamp());
    }

    fn enabled(env_value: Option<&str>, config_off: bool) -> bool {
        !env_disables_session_search(env_value) && !config_off
    }

    #[test]
    fn session_search_stays_on_when_env_and_config_are_absent() {
        assert!(enabled(None, false));
    }

    #[test]
    fn session_search_env_zero_or_false_turns_the_index_off() {
        for value in ["0", "false", "FALSE", "False", "  false  "] {
            assert!(!enabled(Some(value), false), "{value}");
        }
    }

    #[test]
    fn session_search_other_env_values_stay_on() {
        for value in ["1", "true", "TRUE", "yes", ""] {
            assert!(enabled(Some(value), false), "{value}");
        }
    }

    #[test]
    fn session_search_config_false_wins_even_when_env_is_on() {
        assert!(!enabled(None, true));
        assert!(!enabled(Some("true"), true));
        assert!(!enabled(Some("0"), false));
    }

    #[test]
    fn session_search_config_false_is_the_bool_only() {
        let off: toml::Value = toml::from_str("[features]\nsession_search = false\n").unwrap();
        assert!(config_value_disables_session_search(&off));
        let on: toml::Value = toml::from_str("[features]\nsession_search = true\n").unwrap();
        assert!(!config_value_disables_session_search(&on));
        assert!(!config_value_disables_session_search(&toml::Value::Table(
            Default::default()
        )));
        let empty = empty_session_search_response();
        assert!(empty.results.is_empty());
        assert_eq!(empty.total_estimate, Some(0));
        assert!(!empty.bootstrapping);
        assert!(empty.next_offset.is_none());
    }
}
