//! xAI **Management API** credential store + console team prepaid balance fetch.
//!
//! Distinct from:
//! - Console **inference** keys (`XAI_API_KEY` / [`super::xai_console`]) on `api.x.ai`
//! - SuperGrok OIDC session tokens
//! - Enterprise `GROK_DEPLOYMENT_KEY` managed-config attribution
//!
//! Management keys come from Console → Settings → Management Keys and authorize
//! `https://management-api.x.ai` (billing prepaid balance, usage analytics, …).
//!
//! Storage: OS keyring / file mirror via [`CredentialsStore`], keyed by
//! [`MANAGEMENT_API_BASE_URL`] (not the inference console URL). Optional
//! `[endpoints] management_api_key` in config still wins when set (CI / headless).
//!
//! Secrets are never accepted as CLI argv values. Debug of store entries redacts
//! secrets (see [`CredentialsStore`]).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::credentials_store::{BEARER_USERNAME, CredentialsStore, CredentialsStoreError};

/// Public Management API host (no path suffix).
pub const MANAGEMENT_API_BASE_URL: &str = "https://management-api.x.ai";

/// Credential store key for the management Bearer secret (URL-keyed like console).
pub const MANAGEMENT_CREDENTIAL_URL: &str = "https://management-api.x.ai";

/// Path template under the Management API base for team prepaid balance.
/// Use [`prepaid_balance_path`] to fill `team_id`.
pub const PREPAID_BALANCE_PATH_TEMPLATE: &str = "/v1/billing/teams/{team_id}/prepaid/balance";

/// How long a successful prepaid balance stays in the process cache.
const PREPAID_CACHE_TTL: Duration = Duration::from_secs(60);

/// Normalize the Management API base URL (trim trailing `/`).
pub fn management_api_base(base_url: Option<&str>) -> String {
    let url = base_url
        .unwrap_or(MANAGEMENT_API_BASE_URL)
        .trim_end_matches('/');
    if url.is_empty() {
        MANAGEMENT_API_BASE_URL.to_owned()
    } else {
        url.to_owned()
    }
}

/// Store key for management credentials (distinct from inference `api.x.ai/v1`).
pub fn management_credential_url() -> String {
    MANAGEMENT_CREDENTIAL_URL.to_owned()
}

/// `GET` path for team prepaid balance (team id path-segment encoded).
pub fn prepaid_balance_path(team_id: &str) -> String {
    let id = team_id.trim();
    format!(
        "/v1/billing/teams/{}/prepaid/balance",
        urlencoding_path_segment(id)
    )
}

/// Minimal path-segment escape for team ids (UUID / safe id chars pass through).
fn urlencoding_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Load management key from the secret store only (not config).
pub fn load_stored_management_api_key(
    store: &CredentialsStore,
) -> Result<Option<String>, CredentialsStoreError> {
    let url = management_credential_url();
    Ok(store.read(&url)?.map(|(_, secret)| secret))
}

/// Store a management API key (replaces store blob). Empty refused.
///
/// Does **not** refuse when `XAI_API_KEY` is set: that is the inference console
/// key, a different product surface and store slot.
pub fn store_management_api_key(
    store: &CredentialsStore,
    management_key: &str,
) -> Result<(), ManagementAuthError> {
    let key = management_key.trim();
    if key.is_empty() {
        return Err(ManagementAuthError::EmptyKey);
    }
    let url = management_credential_url();
    store
        .write(&url, BEARER_USERNAME, key)
        .map_err(ManagementAuthError::Store)
}

/// Clear the stored management API key (does not clear config).
pub fn clear_management_api_key(store: &CredentialsStore) -> Result<(), CredentialsStoreError> {
    store.delete(&management_credential_url())
}

/// Fingerprint-only description for logs (never the raw key).
pub fn fingerprint_management_key(key: &str) -> String {
    blake3::hash(key.trim().as_bytes()).to_hex().to_string()
}

/// Resolve management key: non-empty config first, then secret store.
///
/// Config is the existing `[endpoints] management_api_key` load path. Store is
/// the keyring-backed management slot. Never returns the inference console key.
pub fn resolve_management_api_key(
    config_key: Option<&str>,
    store: &CredentialsStore,
) -> Result<Option<String>, CredentialsStoreError> {
    if let Some(k) = config_key.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(k.to_owned()));
    }
    load_stored_management_api_key(store)
}

/// Resolve using the process default config loader + grok-home store.
pub fn resolve_management_api_key_default() -> Option<String> {
    let config_key = crate::util::config::load_management_api_key_sync();
    let store = CredentialsStore::default_store();
    resolve_management_api_key(config_key.as_deref(), &store)
        .ok()
        .flatten()
}

/// Explicit Management API team id from config (never SuperGrok OIDC team id).
pub fn resolve_management_team_id(config_team_id: Option<&str>) -> Option<String> {
    config_team_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Resolve team id from the process default config loader.
pub fn resolve_management_team_id_default() -> Option<String> {
    resolve_management_team_id(crate::util::config::load_management_team_id_sync().as_deref())
}

#[derive(Debug, thiserror::Error)]
pub enum ManagementAuthError {
    #[error("management API key is empty")]
    EmptyKey,
    #[error(transparent)]
    Store(#[from] CredentialsStoreError),
}

// --- Prepaid balance response + fetch ---------------------------------------

/// USD cents wrapper as documented on Management API (`{ "val": "<cents>" }`).
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct UsdCentsVal {
    /// Cents as a decimal string in public docs (may be negative for remaining credit).
    pub val: String,
}

/// Documented prepaid balance response body.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct PrepaidBalanceResponse {
    /// Remaining prepaid balance in USD cents (`total.val`).
    pub total: UsdCentsVal,
    /// Optional change history (ignored for footer / `/limits` v1).
    #[serde(default)]
    pub changes: Vec<serde_json::Value>,
}

/// Plain console-team prepaid meter for TUI / cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleTeamPrepaidMeter {
    pub team_id: String,
    /// Remaining prepaid in USD cents (non-negative absolute remaining).
    pub balance_cents: i64,
}

/// Map documented `total.val` (USD cents string) to remaining cents for display.
///
/// Public docs show remaining credit as a **negative** total after PURCHASE
/// (e.g. `"-1000"` for $10 prepaid). UI meters want absolute remaining.
pub fn prepaid_remaining_cents_from_total_val(val: &str) -> Option<i64> {
    let n: i64 = val.trim().parse().ok()?;
    Some(n.saturating_abs())
}

/// Parse a full prepaid JSON body into a meter for `team_id`.
pub fn console_team_prepaid_from_response(
    team_id: &str,
    body: &PrepaidBalanceResponse,
) -> Option<ConsoleTeamPrepaidMeter> {
    let balance_cents = prepaid_remaining_cents_from_total_val(&body.total.val)?;
    let team = team_id.trim();
    if team.is_empty() {
        return None;
    }
    Some(ConsoleTeamPrepaidMeter {
        team_id: team.to_owned(),
        balance_cents,
    })
}

struct PrepaidCacheEntry {
    team_id: String,
    balance_cents: i64,
    fetched_at: Instant,
}

static PREPAID_CACHE: Mutex<Option<PrepaidCacheEntry>> = Mutex::new(None);

/// Clear the process prepaid cache (tests / logout).
pub fn clear_console_team_prepaid_cache() {
    if let Ok(mut g) = PREPAID_CACHE.lock() {
        *g = None;
    }
}

/// Last successful prepaid meter from process cache, if still fresh.
pub fn cached_console_team_prepaid(team_id: &str) -> Option<ConsoleTeamPrepaidMeter> {
    let team = team_id.trim();
    if team.is_empty() {
        return None;
    }
    let g = PREPAID_CACHE.lock().ok()?;
    let entry = g.as_ref()?;
    if entry.team_id != team {
        return None;
    }
    if entry.fetched_at.elapsed() > PREPAID_CACHE_TTL {
        return None;
    }
    Some(ConsoleTeamPrepaidMeter {
        team_id: entry.team_id.clone(),
        balance_cents: entry.balance_cents,
    })
}

/// Process-cache cents when `[endpoints] management_team_id` is set and the
/// entry is still fresh. Used by footer / `/limits` without another HTTP round
/// trip. Returns `None` when team_id is unset or cache is cold/stale.
pub fn cached_console_team_prepaid_cents_default() -> Option<i64> {
    let team = resolve_management_team_id_default()?;
    cached_console_team_prepaid(&team).map(|m| m.balance_cents)
}

fn remember_prepaid(meter: &ConsoleTeamPrepaidMeter) {
    if let Ok(mut g) = PREPAID_CACHE.lock() {
        *g = Some(PrepaidCacheEntry {
            team_id: meter.team_id.clone(),
            balance_cents: meter.balance_cents,
            fetched_at: Instant::now(),
        });
    }
}

/// Fetch console team prepaid balance when management key + team_id are present.
///
/// Returns `None` when key or team_id is missing, HTTP fails, or body is unusable.
/// Callers map that to honest not-configured / unavailable gap copy (never invent $).
pub async fn fetch_console_team_prepaid_balance(
    management_key: Option<&str>,
    team_id: Option<&str>,
) -> Option<ConsoleTeamPrepaidMeter> {
    fetch_console_team_prepaid_balance_at(MANAGEMENT_API_BASE_URL, management_key, team_id).await
}

/// Same as [`fetch_console_team_prepaid_balance`] with an injectable base URL
/// (hermetic HTTP mock tests).
pub async fn fetch_console_team_prepaid_balance_at(
    base_url: &str,
    management_key: Option<&str>,
    team_id: Option<&str>,
) -> Option<ConsoleTeamPrepaidMeter> {
    let key = management_key.map(str::trim).filter(|s| !s.is_empty())?;
    let team = team_id.map(str::trim).filter(|s| !s.is_empty())?;

    if let Some(cached) = cached_console_team_prepaid(team) {
        return Some(cached);
    }

    let base = management_api_base(Some(base_url));
    let url = format!("{base}{}", prepaid_balance_path(team));
    let client = crate::http::shared_client();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        tracing::debug!(
            status = response.status().as_u16(),
            "management prepaid balance: non-success status"
        );
        return None;
    }
    let parsed: PrepaidBalanceResponse = response.json().await.ok()?;
    let meter = console_team_prepaid_from_response(team, &parsed)?;
    remember_prepaid(&meter);
    Some(meter)
}

/// Resolve credentials from config/store defaults and fetch prepaid (product path).
pub async fn fetch_console_team_prepaid_balance_default() -> Option<ConsoleTeamPrepaidMeter> {
    let key = resolve_management_api_key_default();
    let team = resolve_management_team_id_default();
    fetch_console_team_prepaid_balance(key.as_deref(), team.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::credentials_store::CredentialsStore;
    use crate::auth::xai_console::{
        XAI_CONSOLE_API_URL, load_stored_console_api_key, store_console_api_key,
    };
    use axum::Json;
    use axum::Router;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use serial_test::serial;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    #[test]
    fn management_credential_url_is_not_inference_console_url() {
        assert_eq!(management_credential_url(), MANAGEMENT_CREDENTIAL_URL);
        assert_ne!(management_credential_url(), XAI_CONSOLE_API_URL);
        assert!(!management_credential_url().contains("api.x.ai/v1"));
        assert!(management_credential_url().contains("management-api.x.ai"));
    }

    #[test]
    #[serial]
    fn management_key_store_and_load_round_trip() {
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));

        store_management_api_key(&store, "  mgmt-secret-key  ").unwrap();
        let loaded = load_stored_management_api_key(&store).unwrap();
        assert_eq!(loaded.as_deref(), Some("mgmt-secret-key"));

        clear_management_api_key(&store).unwrap();
        assert!(load_stored_management_api_key(&store).unwrap().is_none());
    }

    /// Named contract: management store slot is not the inference console slot.
    #[test]
    #[serial]
    fn management_key_not_conflated_with_inference_console_key() {
        let _xai = EnvGuard::set("XAI_API_KEY", "inference-env-key");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));

        // Management write must succeed even when inference env is set.
        store_management_api_key(&store, "mgmt-only-secret").unwrap();
        // Inference console store is a different URL; leave empty here.
        assert!(
            load_stored_console_api_key(&store).unwrap().is_none(),
            "management write must not populate inference console slot"
        );
        assert_eq!(
            load_stored_management_api_key(&store).unwrap().as_deref(),
            Some("mgmt-only-secret")
        );

        // Writing a console inference key (when env unset) stays on console URL.
        drop(_xai);
        let _xai = EnvGuard::unset("XAI_API_KEY");
        store_console_api_key(&store, "console-inference-secret").unwrap();
        assert_eq!(
            load_stored_console_api_key(&store).unwrap().as_deref(),
            Some("console-inference-secret")
        );
        assert_eq!(
            load_stored_management_api_key(&store).unwrap().as_deref(),
            Some("mgmt-only-secret"),
            "console store must not overwrite management slot"
        );
        assert_ne!(
            load_stored_console_api_key(&store).unwrap().unwrap(),
            load_stored_management_api_key(&store).unwrap().unwrap()
        );
    }

    #[test]
    fn resolve_prefers_config_over_store() {
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        store_management_api_key(&store, "from-store").unwrap();
        let resolved = resolve_management_api_key(Some("from-config"), &store).unwrap();
        assert_eq!(resolved.as_deref(), Some("from-config"));
        let from_store_only = resolve_management_api_key(None, &store).unwrap();
        assert_eq!(from_store_only.as_deref(), Some("from-store"));
        let blank_config = resolve_management_api_key(Some("  "), &store).unwrap();
        assert_eq!(blank_config.as_deref(), Some("from-store"));
    }

    #[test]
    fn team_id_resolve_ignores_blank_and_does_not_invent() {
        assert_eq!(resolve_management_team_id(None), None);
        assert_eq!(resolve_management_team_id(Some("")), None);
        assert_eq!(resolve_management_team_id(Some("  ")), None);
        assert_eq!(
            resolve_management_team_id(Some("  team-uuid-1  ")).as_deref(),
            Some("team-uuid-1")
        );
    }

    #[test]
    fn fingerprint_is_not_raw_key() {
        let fp = fingerprint_management_key("super-secret-mgmt-key");
        assert!(!fp.contains("super-secret"));
        assert!(!fp.is_empty());
    }

    #[test]
    fn prepaid_remaining_cents_handles_negative_doc_convention() {
        assert_eq!(prepaid_remaining_cents_from_total_val("-1000"), Some(1000));
        assert_eq!(prepaid_remaining_cents_from_total_val("2500"), Some(2500));
        assert_eq!(prepaid_remaining_cents_from_total_val("0"), Some(0));
        assert_eq!(prepaid_remaining_cents_from_total_val("not-a-number"), None);
    }

    #[test]
    fn prepaid_path_includes_team_id() {
        let p = prepaid_balance_path("65c1e471-205f-4566-9c5a-07198bcdf4ce");
        assert_eq!(
            p,
            "/v1/billing/teams/65c1e471-205f-4566-9c5a-07198bcdf4ce/prepaid/balance"
        );
    }

    #[test]
    fn parse_prepaid_response_body() {
        let body: PrepaidBalanceResponse = serde_json::from_value(serde_json::json!({
            "total": { "val": "-4500" },
            "changes": []
        }))
        .unwrap();
        let meter = console_team_prepaid_from_response("team-abc", &body).unwrap();
        assert_eq!(meter.team_id, "team-abc");
        assert_eq!(meter.balance_cents, 4500);
    }

    /// RED/GREEN: missing management key or team_id leaves meter absent.
    #[tokio::test]
    async fn fetch_missing_key_or_team_id_returns_none() {
        clear_console_team_prepaid_cache();
        assert!(
            fetch_console_team_prepaid_balance_at("http://127.0.0.1:1", None, Some("team-1"),)
                .await
                .is_none()
        );
        assert!(
            fetch_console_team_prepaid_balance_at("http://127.0.0.1:1", Some("mgmt-key"), None,)
                .await
                .is_none()
        );
        assert!(
            fetch_console_team_prepaid_balance_at("http://127.0.0.1:1", Some(""), Some("team-1"),)
                .await
                .is_none()
        );
    }

    /// Hermetic mock: prepaid balance → cents for ConsoleMeter.
    #[tokio::test]
    async fn fetch_prepaid_balance_hermetic_mock_returns_cents() {
        clear_console_team_prepaid_cache();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_h = hits.clone();
        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/prepaid/balance",
            get(move |Path(team_id): Path<String>, headers: HeaderMap| {
                let hits = hits_h.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let auth = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if auth != "Bearer hermetic-mgmt-key" {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    if team_id != "team-hermetic-1" {
                        return Err(StatusCode::NOT_FOUND);
                    }
                    Ok(Json(serde_json::json!({
                        "total": { "val": "-12500" },
                        "changes": []
                    })))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let base = format!("http://{addr}");
        let meter = fetch_console_team_prepaid_balance_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-hermetic-1"),
        )
        .await
        .expect("prepaid meter");
        assert_eq!(meter.team_id, "team-hermetic-1");
        assert_eq!(meter.balance_cents, 12500);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // Process cache: second call does not hit HTTP again.
        let again = fetch_console_team_prepaid_balance_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-hermetic-1"),
        )
        .await
        .expect("cached");
        assert_eq!(again.balance_cents, 12500);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "cache must skip second HTTP"
        );

        clear_console_team_prepaid_cache();
        server.abort();
    }

    #[tokio::test]
    async fn fetch_http_error_returns_none() {
        clear_console_team_prepaid_cache();
        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/prepaid/balance",
            get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");
        assert!(
            fetch_console_team_prepaid_balance_at(&base, Some("key"), Some("team-err"),)
                .await
                .is_none()
        );
        server.abort();
    }
}
