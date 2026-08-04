//! xAI **Management API** credential store + console team billing meters.
//!
//! Surfaces:
//! - **Prepaid** ledger balance (`GET …/prepaid/balance`)
//! - **Postpaid** invoice preview (`GET …/postpaid/invoice/preview`) for OAuth
//!   vs API class team Usage dollars (distinct from prepaid $ and SuperGrok)
//! - **Usage series** (`POST …/usage` with `analyticsRequest`) for spend over
//!   time / by description class (not a GET invent)
//!
//! Distinct from:
//! - Console **inference** keys (`XAI_API_KEY` / [`super::xai_console`]) on `api.x.ai`
//! - SuperGrok OIDC session tokens
//! - Enterprise `GROK_DEPLOYMENT_KEY` managed-config attribution
//!
//! Management keys come from Console → Settings → Management Keys and authorize
//! `https://management-api.x.ai` (billing prepaid balance, postpaid preview, …).
//! The browser console dashboard (`console.x.ai`) uses a **session cookie**, not
//! this key; product cannot read ~team prepaid remaining from an inference key.
//!
//! Storage: OS keyring / file mirror via [`CredentialsStore`], keyed by
//! [`MANAGEMENT_API_BASE_URL`] (not the inference console URL). Resolve order:
//! `[endpoints] management_api_key` → `XAI_MANAGEMENT_API_KEY` env → secret store.
//! Team id: `[endpoints] management_team_id` → `XAI_MANAGEMENT_TEAM_ID` env →
//! auto from `GET /auth/management-keys/validation` when a key is present.
//!
//! Secrets are never accepted as CLI argv values. Debug of store entries redacts
//! secrets (see [`CredentialsStore`]).

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::credentials_store::{BEARER_USERNAME, CredentialsStore, CredentialsStoreError};

/// Public Management API host (no path suffix).
pub const MANAGEMENT_API_BASE_URL: &str = "https://management-api.x.ai";

/// Credential store key for the management Bearer secret (URL-keyed like console).
pub const MANAGEMENT_CREDENTIAL_URL: &str = "https://management-api.x.ai";

/// Env var for Management API Bearer (billing). Distinct from `XAI_API_KEY`.
pub const XAI_MANAGEMENT_API_KEY_ENV: &str = "XAI_MANAGEMENT_API_KEY";

/// Env var for Management API team id (path param). Distinct from SuperGrok OIDC.
pub const XAI_MANAGEMENT_TEAM_ID_ENV: &str = "XAI_MANAGEMENT_TEAM_ID";

/// Path template under the Management API base for team prepaid balance.
/// Use [`prepaid_balance_path`] to fill `team_id`.
pub const PREPAID_BALANCE_PATH_TEMPLATE: &str = "/v1/billing/teams/{team_id}/prepaid/balance";

/// Path template for team postpaid invoice preview (OAuth vs API attribution).
/// Use [`postpaid_invoice_preview_path`] to fill `team_id`.
pub const POSTPAID_INVOICE_PREVIEW_PATH_TEMPLATE: &str =
    "/v1/billing/teams/{team_id}/postpaid/invoice/preview";

/// Path template for team usage analytics series (POST with analyticsRequest).
/// Use [`usage_analytics_path`] to fill `team_id`. Documented; not a GET invent.
pub const USAGE_ANALYTICS_PATH_TEMPLATE: &str = "/v1/billing/teams/{team_id}/usage";

/// Documented validation path (returns `teamId` / `scopeId` for the key).
pub const MANAGEMENT_KEY_VALIDATION_PATH: &str = "/auth/management-keys/validation";

/// Default day window for spend series on `/limits` (rolling calendar days).
pub const USAGE_SERIES_DEFAULT_DAY_WINDOW: i64 = 7;

/// Shared soft window (seconds) for Management **prepaid** and **postpaid**
/// process caches.
///
/// Background TUI billing polls reuse a warm entry for this long. Explicit
/// `grok limits` collect busts both caches so dollars are not stuck until TTL
/// expiry or process restart. Honesty surfaces may cite this value.
pub const CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS: u64 = 60;

/// Alias of [`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`] for prepaid lag copy
/// and older call sites. Same 60s window as postpaid process cache.
pub const CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS: u64 = CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS;

/// How long a successful prepaid balance stays in the process cache.
const PREPAID_CACHE_TTL: Duration = Duration::from_secs(CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS);

/// How long a successful postpaid preview stays in the process cache (same
/// soft window as prepaid; explicit limits collect busts both).
const POSTPAID_CACHE_TTL: Duration = Duration::from_secs(CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS);

/// How long a discovered team id from key validation stays in process cache.
const TEAM_ID_CACHE_TTL: Duration = Duration::from_secs(3600);

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

/// `GET` path for team postpaid invoice preview (team id path-segment encoded).
pub fn postpaid_invoice_preview_path(team_id: &str) -> String {
    let id = team_id.trim();
    format!(
        "/v1/billing/teams/{}/postpaid/invoice/preview",
        urlencoding_path_segment(id)
    )
}

/// `POST` path for team usage analytics series (team id path-segment encoded).
pub fn usage_analytics_path(team_id: &str) -> String {
    let id = team_id.trim();
    format!("/v1/billing/teams/{}/usage", urlencoding_path_segment(id))
}

/// Path-segment escape for team ids used in Management billing paths.
///
/// Only unreserved alphanumerics plus `-` / `_` pass through unencoded. Dots and
/// tildes are percent-encoded so a team id of `..` cannot form a `/../` segment
/// after stack path normalization. Slashes and other bytes are also encoded.
fn urlencoding_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => {
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
/// key, a different product surface and store slot. Refuses when
/// `XAI_MANAGEMENT_API_KEY` is set (env wins, OpenRouter/console parity).
pub fn store_management_api_key(
    store: &CredentialsStore,
    management_key: &str,
) -> Result<(), ManagementAuthError> {
    if has_management_api_key_env() {
        return Err(ManagementAuthError::EnvVarSet);
    }
    let key = management_key.trim();
    if key.is_empty() {
        return Err(ManagementAuthError::EmptyKey);
    }
    // New key may map to a different team; drop stale billing process meters.
    clear_management_billing_process_caches();
    let url = management_credential_url();
    store
        .write(&url, BEARER_USERNAME, key)
        .map_err(ManagementAuthError::Store)
}

/// Clear the stored management API key (does not clear config).
///
/// Also drops process caches that depended on the prior key/team (prepaid,
/// postpaid preview, discovered team id) so a rotate does not leave stale meters.
pub fn clear_management_api_key(store: &CredentialsStore) -> Result<(), CredentialsStoreError> {
    clear_management_billing_process_caches();
    store.delete(&management_credential_url())
}

/// Drop Management billing process caches (prepaid, postpaid, discovered team id).
///
/// Called when the management key is cleared or replaced. Safe for tests.
pub fn clear_management_billing_process_caches() {
    clear_console_team_prepaid_cache();
    clear_console_team_postpaid_cache();
    clear_discovered_team_id_cache();
}

/// Fingerprint-only description for logs (never the raw key).
pub fn fingerprint_management_key(key: &str) -> String {
    blake3::hash(key.trim().as_bytes()).to_hex().to_string()
}

/// True when `XAI_MANAGEMENT_API_KEY` is set and non-empty.
pub fn has_management_api_key_env() -> bool {
    std::env::var(XAI_MANAGEMENT_API_KEY_ENV)
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Non-empty `XAI_MANAGEMENT_API_KEY` value, if set.
pub fn management_api_key_from_env() -> Option<String> {
    std::env::var(XAI_MANAGEMENT_API_KEY_ENV)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Non-empty `XAI_MANAGEMENT_TEAM_ID` value, if set.
pub fn management_team_id_from_env() -> Option<String> {
    std::env::var(XAI_MANAGEMENT_TEAM_ID_ENV)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Resolve management key: non-empty config first, then env, then secret store.
///
/// Config is the existing `[endpoints] management_api_key` load path. Env is
/// `XAI_MANAGEMENT_API_KEY`. Store is the keyring-backed management slot. Never
/// returns the inference console key (`XAI_API_KEY`).
pub fn resolve_management_api_key(
    config_key: Option<&str>,
    store: &CredentialsStore,
) -> Result<Option<String>, CredentialsStoreError> {
    if let Some(k) = config_key.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(k.to_owned()));
    }
    if let Some(k) = management_api_key_from_env() {
        return Ok(Some(k));
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

/// Explicit Management API team id from config / env (never SuperGrok OIDC).
///
/// Does **not** call the network. For auto team id from key validation, use
/// [`resolve_management_team_id_with_discovery`] / the prepaid fetch path.
pub fn resolve_management_team_id(config_team_id: Option<&str>) -> Option<String> {
    if let Some(t) = config_team_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    {
        return Some(t);
    }
    management_team_id_from_env()
}

/// Resolve team id from config → env → process-cached validation discovery.
///
/// Sync only: never hits the network. After a successful validation fetch, the
/// discovered id is available here until TTL expiry.
pub fn resolve_management_team_id_default() -> Option<String> {
    if let Some(t) =
        resolve_management_team_id(crate::util::config::load_management_team_id_sync().as_deref())
    {
        return Some(t);
    }
    cached_discovered_team_id()
}

/// Config/env team id, else process cache, else validate the management key
/// (network) and remember `teamId` / `scopeId`.
pub async fn resolve_management_team_id_with_discovery(
    base_url: &str,
    management_key: Option<&str>,
    config_team_id: Option<&str>,
) -> Option<String> {
    if let Some(t) = resolve_management_team_id(config_team_id) {
        return Some(t);
    }
    if let Some(t) = cached_discovered_team_id() {
        return Some(t);
    }
    let key = management_key.map(str::trim).filter(|s| !s.is_empty())?;
    let meta = validate_management_key_at(base_url, key).await?;
    let team = meta.team_id_for_billing()?;
    remember_discovered_team_id(&team);
    Some(team)
}

#[derive(Debug, thiserror::Error)]
pub enum ManagementAuthError {
    #[error("management API key is empty")]
    EmptyKey,
    #[error("XAI_MANAGEMENT_API_KEY is set; refuse to write the secret store (env wins)")]
    EnvVarSet,
    #[error(transparent)]
    Store(#[from] CredentialsStoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// --- Management key validation (team id discovery) --------------------------

/// Documented `GET /auth/management-keys/validation` body (subset).
///
/// Accepts camelCase (docs) and snake_case (defensive) field names.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagementKeyValidation {
    /// Deprecated field; still returned. Prefer [`Self::scope_id`] when present.
    #[serde(default, alias = "team_id")]
    pub team_id: Option<String>,
    /// Scope id (team or organization). Used as billing team when SCOPE_TEAM.
    #[serde(default, alias = "scope_id")]
    pub scope_id: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "api_key_id")]
    pub api_key_id: Option<String>,
}

impl ManagementKeyValidation {
    /// Team id for billing path params: `scopeId` when team-scoped, else `teamId`.
    pub fn team_id_for_billing(&self) -> Option<String> {
        let scope = self.scope.as_deref().unwrap_or("");
        // Prefer scopeId for SCOPE_TEAM (docs mark teamId deprecated).
        // Also take scopeId when scope is empty/unspecified (common live shape).
        if scope == "SCOPE_TEAM" || scope.is_empty() || scope == "SCOPE_UNSPECIFIED" {
            if let Some(id) = self
                .scope_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return Some(id.to_owned());
            }
        }
        if let Some(id) = self
            .team_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(id.to_owned());
        }
        // Last resort: non-empty scopeId even for other scopes (org keys sometimes
        // only return scopeId; operator can still pin team id explicitly).
        self.scope_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    }
}

/// Why `GET /auth/management-keys/validation` did not yield a usable team id.
///
/// Used for honest login / setup copy. Discovery still returns `None` on any
/// failure; this is the structured reason for operators (not a secret dump).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagementKeyValidateFailure {
    /// Empty / whitespace-only key string.
    EmptyKey,
    /// Network / client send failed (offline, DNS, TLS, timeout).
    Network,
    /// HTTP 401/403: token is not a valid management key (often an inference
    /// API key pasted by mistake).
    InvalidManagementKey {
        status: u16,
        /// API `message` when present (never includes the bearer secret).
        message: Option<String>,
    },
    /// Other non-success HTTP status.
    Http {
        status: u16,
        message: Option<String>,
    },
    /// 2xx body could not be parsed as [`ManagementKeyValidation`].
    UnparseableBody,
    /// Key validated but neither `teamId` nor `scopeId` was usable.
    NoTeamIdInResponse,
}

/// Outcome of management-key validation (team id discovery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagementKeyValidateOutcome {
    Ok(ManagementKeyValidation),
    Err(ManagementKeyValidateFailure),
}

/// Operator-facing help for a validation failure (no secrets).
pub fn format_management_key_validate_failure(fail: &ManagementKeyValidateFailure) -> String {
    match fail {
        ManagementKeyValidateFailure::EmptyKey => "Management key is empty.".into(),
        ManagementKeyValidateFailure::Network => {
            "Could not reach management-api.x.ai (offline, DNS, or timeout). \
             Retry when online, or pin team id without discovery:\n  [endpoints]\n  \
             management_team_id = \"<console-team-uuid>\""
                .into()
        }
        ManagementKeyValidateFailure::InvalidManagementKey { status, message } => {
            let api = message
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|m| format!(" API said: {m}"))
                .unwrap_or_default();
            format!(
                "Management API rejected this key (HTTP {status}).{api}\n\
                 Create a **Management key** at Console → Settings → Management Keys \
                 (not an inference API key from API Keys), then re-run \
                 `grok login --management-key`.\n\
                 Team id is separate — pin when known:\n  [endpoints]\n  \
                 management_team_id = \"<console-team-uuid from team settings>\""
            )
        }
        ManagementKeyValidateFailure::Http { status, message } => {
            let api = message
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|m| format!(" ({m})"))
                .unwrap_or_default();
            format!(
                "Management key validation failed (HTTP {status}{api}). \
                 Pin team id if you have it:\n  [endpoints]\n  \
                 management_team_id = \"<console-team-uuid>\""
            )
        }
        ManagementKeyValidateFailure::UnparseableBody => {
            "Management key validation returned an unexpected body; could not read team id. \
             Pin team id:\n  [endpoints]\n  management_team_id = \"<console-team-uuid>\""
                .into()
        }
        ManagementKeyValidateFailure::NoTeamIdInResponse => {
            "Management key is valid but the response had no team id / scope id. \
             Pin it from Console → team settings:\n  [endpoints]\n  \
             management_team_id = \"<console-team-uuid>\""
                .into()
        }
    }
}

/// Truncate API error messages for UI (never log full bodies that might hold keys).
fn short_api_message(raw: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct ErrBody {
        message: Option<String>,
    }
    let parsed = serde_json::from_str::<ErrBody>(raw).ok();
    let msg = parsed.and_then(|b| b.message).or_else(|| {
        let t = raw.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_owned())
        }
    })?;
    let t = msg.trim();
    if t.is_empty() {
        return None;
    }
    // Refuse to echo anything that looks like a pasted secret.
    if t.starts_with("xai-") || t.len() > 240 {
        return Some("rejected by server".into());
    }
    Some(t.chars().take(200).collect())
}

struct TeamIdCacheEntry {
    team_id: String,
    fetched_at: Instant,
}

static TEAM_ID_CACHE: Mutex<Option<TeamIdCacheEntry>> = Mutex::new(None);

/// Clear discovered team id cache (tests / logout).
pub fn clear_discovered_team_id_cache() {
    if let Ok(mut g) = TEAM_ID_CACHE.lock() {
        *g = None;
    }
}

/// Last discovered team id from validation, if still fresh.
pub fn cached_discovered_team_id() -> Option<String> {
    let g = TEAM_ID_CACHE.lock().ok()?;
    let entry = g.as_ref()?;
    if entry.fetched_at.elapsed() > TEAM_ID_CACHE_TTL {
        return None;
    }
    Some(entry.team_id.clone())
}

fn remember_discovered_team_id(team_id: &str) {
    let id = team_id.trim();
    if id.is_empty() {
        return;
    }
    if let Ok(mut g) = TEAM_ID_CACHE.lock() {
        *g = Some(TeamIdCacheEntry {
            team_id: id.to_owned(),
            fetched_at: Instant::now(),
        });
    }
}

/// Validate a management key and return meta (includes team id when present).
pub async fn validate_management_key(management_key: &str) -> Option<ManagementKeyValidation> {
    validate_management_key_at(MANAGEMENT_API_BASE_URL, management_key).await
}

/// Injectable-base validation (hermetic tests). `None` on any failure.
pub async fn validate_management_key_at(
    base_url: &str,
    management_key: &str,
) -> Option<ManagementKeyValidation> {
    match validate_management_key_outcome_at(base_url, management_key).await {
        ManagementKeyValidateOutcome::Ok(meta) => Some(meta),
        ManagementKeyValidateOutcome::Err(fail) => {
            tracing::debug!(?fail, "management key validation failed");
            None
        }
    }
}

/// Validate with a structured outcome (login messaging / diagnostics).
pub async fn validate_management_key_outcome(management_key: &str) -> ManagementKeyValidateOutcome {
    validate_management_key_outcome_at(MANAGEMENT_API_BASE_URL, management_key).await
}

/// Injectable-base structured validation.
pub async fn validate_management_key_outcome_at(
    base_url: &str,
    management_key: &str,
) -> ManagementKeyValidateOutcome {
    let key = management_key.trim();
    if key.is_empty() {
        return ManagementKeyValidateOutcome::Err(ManagementKeyValidateFailure::EmptyKey);
    }
    let base = management_api_base(Some(base_url));
    let url = format!("{base}{MANAGEMENT_KEY_VALIDATION_PATH}");
    let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, key);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let client = crate::http::shared_client();
    let response = match client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "management key validation: network error");
            return ManagementKeyValidateOutcome::Err(ManagementKeyValidateFailure::Network);
        }
    };
    let status = response.status();
    if !status.is_success() {
        let code = status.as_u16();
        let headers = response.headers().clone();
        // 429 publishes shared cooldown. Bare 401/403 stay invalid-key (no
        // Retry-After) so peers are not poisoned by a bad secret.
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            code,
            &headers,
            "Management API key validation rate limit",
        );
        let body = response.text().await.unwrap_or_default();
        let message = short_api_message(&body);
        tracing::debug!(
            status = code,
            "management key validation: non-success status"
        );
        let fail = if code == 401 || code == 403 {
            ManagementKeyValidateFailure::InvalidManagementKey {
                status: code,
                message,
            }
        } else {
            ManagementKeyValidateFailure::Http {
                status: code,
                message,
            }
        };
        return ManagementKeyValidateOutcome::Err(fail);
    }
    match response.json::<ManagementKeyValidation>().await {
        Ok(meta) => ManagementKeyValidateOutcome::Ok(meta),
        Err(e) => {
            tracing::debug!(error = %e, "management key validation: unparseable body");
            ManagementKeyValidateOutcome::Err(ManagementKeyValidateFailure::UnparseableBody)
        }
    }
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

/// Structured fields for a successful Management prepaid fetch (no secrets).
///
/// Logs **team_id**, absolute remaining **balance_cents**, and the signed wire
/// **total.val** as an integer (`total_val_cents`). Never includes the
/// management key. Safe for unified.jsonl / info-level tracing.
pub fn management_prepaid_success_log_fields(
    team_id: &str,
    balance_cents: i64,
    total_val: &str,
) -> serde_json::Value {
    let total_val_cents: Option<i64> = total_val.trim().parse().ok();
    serde_json::json!({
        "team_id": team_id,
        "balance_cents": balance_cents,
        "total_val_cents": total_val_cents,
    })
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

/// Clear the process **prepaid** cache only.
///
/// Building block for tests, management key clear/rotate, and
/// [`clear_console_team_billing_meter_caches`]. Product force-refresh for
/// explicit `grok limits` is the **combined** billing-meter clear (prepaid +
/// postpaid), not this prepaid-only helper.
pub fn clear_console_team_prepaid_cache() {
    if let Ok(mut g) = PREPAID_CACHE.lock() {
        *g = None;
    }
}

/// Bust prepaid + postpaid process caches without dropping discovered team id.
///
/// Used by explicit `grok limits` collect so Management dollars are not served
/// from a ≤[`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`]s warm entry. Background
/// TUI `FetchBilling` polls must **not** call this (they honor the TTL).
pub fn clear_console_team_billing_meter_caches() {
    clear_console_team_prepaid_cache();
    clear_console_team_postpaid_cache();
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
/// When `team_id` is missing but the key is present, attempts key validation to
/// discover the team id (docs: `GET /auth/management-keys/validation`).
///
/// Returns `None` when key is missing, discovery fails, HTTP fails, or body is
/// unusable. Callers map that to honest not-configured / unavailable gap copy
/// (never invent $).
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
    let team = match team_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t.to_owned(),
        None => resolve_management_team_id_with_discovery(base_url, Some(key), None).await?,
    };

    if let Some(cached) = cached_console_team_prepaid(&team) {
        return Some(cached);
    }

    let base = management_api_base(Some(base_url));
    let url = format!("{base}{}", prepaid_balance_path(&team));
    let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, key);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let client = crate::http::shared_client();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            status,
            &headers,
            "Management API prepaid balance rate limit",
        );
        tracing::debug!(status, "management prepaid balance: non-success status");
        return None;
    }
    let parsed: PrepaidBalanceResponse = response.json().await.ok()?;
    let meter = console_team_prepaid_from_response(&team, &parsed)?;
    remember_prepaid(&meter);
    // Info-level dogfood: which team + cents (signed wire + abs remaining).
    // Never log the management key. Dashboard "Credits remaining" may differ
    // from this prepaid ledger total; product reports prepaid only.
    let log_fields = management_prepaid_success_log_fields(
        &meter.team_id,
        meter.balance_cents,
        &parsed.total.val,
    );
    tracing::info!(
        team_id = %meter.team_id,
        balance_cents = meter.balance_cents,
        total_val_cents = log_fields.get("total_val_cents").and_then(|v| v.as_i64()),
        "management prepaid: fetched console team balance"
    );
    xai_grok_telemetry::unified_log::info(
        "management prepaid: fetched console team balance",
        None,
        Some(log_fields),
    );
    Some(meter)
}

/// Resolve credentials from config/store/env defaults and fetch prepaid.
///
/// Auto-discovers team id via management key validation when config/env team
/// id is unset.
pub async fn fetch_console_team_prepaid_balance_default() -> Option<ConsoleTeamPrepaidMeter> {
    let key = resolve_management_api_key_default();
    let config_team = crate::util::config::load_management_team_id_sync();
    let team = resolve_management_team_id_with_discovery(
        MANAGEMENT_API_BASE_URL,
        key.as_deref(),
        config_team.as_deref(),
    )
    .await;
    fetch_console_team_prepaid_balance(key.as_deref(), team.as_deref()).await
}

/// `grok login --management-key` — store Management API key (billing meter).
///
/// `management_key` is `Some` only for library callers/tests or after stdin
/// materialize (`--management-key -`). Interactive CLI uses `None` → no-echo
/// TTY prompt. Never accepts raw argv secrets (bin refuses those before
/// calling). Never prints raw keys.
///
/// Prints fingerprint + optional discovered team id (when network validate
/// succeeds). Team id is remembered in-process; also print config hint so the
/// operator can pin `[endpoints] management_team_id` for cold starts.
pub fn run_management_key_login(
    grok_home: &Path,
    management_key: Option<&str>,
) -> Result<(), ManagementAuthError> {
    let store = CredentialsStore::at_grok_home(grok_home);
    if has_management_api_key_env() {
        eprintln!(
            "XAI_MANAGEMENT_API_KEY is set; console team prepaid uses the \
             environment (not writing to the secret store)."
        );
        eprintln!("Management API key ready via XAI_MANAGEMENT_API_KEY.");
        return Ok(());
    }
    let key = if let Some(k) = management_key {
        k.to_owned()
    } else {
        super::secret_entry::prompt_api_key_no_echo(
            "Enter your xAI Management API key (Console → Settings → Management Keys): ",
        )
        .map_err(ManagementAuthError::Io)?
    };
    let show_progress = super::secret_store_progress::should_show_secret_store_progress();
    super::secret_store_progress::with_secret_store_progress(show_progress, || {
        store_management_api_key(&store, &key)
    })?;
    let fp = fingerprint_management_key(&key);
    eprintln!("Management API key saved (fingerprint {fp}).");
    eprintln!(
        "This is for console team prepaid / Business Usage remaining, not SuperGrok $ extras \
         and not the inference XAI_API_KEY."
    );
    // Best-effort live validate on a dedicated runtime (CLI may already be
    // inside tokio; never block_on the current handle).
    let outcome = std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            Some(rt.block_on(validate_management_key_outcome(&key)))
        })
        .join()
        .ok()
        .flatten()
    });
    match outcome {
        Some(ManagementKeyValidateOutcome::Ok(meta)) => {
            if let Some(team) = meta.team_id_for_billing() {
                remember_discovered_team_id(&team);
                eprintln!("Team id from management key: {team}");
                eprintln!(
                    "Pin for cold starts in ~/.grok/config.toml:\n  [endpoints]\n  management_team_id = \"{team}\""
                );
                eprintln!("Or: export XAI_MANAGEMENT_TEAM_ID={team}");
            } else {
                eprintln!(
                    "{}",
                    format_management_key_validate_failure(
                        &ManagementKeyValidateFailure::NoTeamIdInResponse
                    )
                );
            }
        }
        Some(ManagementKeyValidateOutcome::Err(fail)) => {
            eprintln!("{}", format_management_key_validate_failure(&fail));
        }
        None => {
            eprintln!(
                "{}",
                format_management_key_validate_failure(&ManagementKeyValidateFailure::Network)
            );
        }
    }
    Ok(())
}

/// Short operator-facing setup note when console team prepaid is unconfigured.
///
/// Used by `grok limits` / notes — not a Balance-line lecture wall.
pub fn console_team_prepaid_setup_note(
    gap_missing_key: bool,
    gap_missing_team: bool,
) -> Option<String> {
    if gap_missing_key {
        return Some(
            "Console team prepaid (business credits remaining on console.x.ai) needs a \
             Management API key: Console → Settings → Management Keys, then \
             `grok login --management-key` or [endpoints] management_api_key / \
             XAI_MANAGEMENT_API_KEY. Not XAI_API_KEY and not SuperGrok $ extras."
                .into(),
        );
    }
    if gap_missing_team {
        return Some(
            "Management key is set but team id is unknown. Set [endpoints] management_team_id \
             or XAI_MANAGEMENT_TEAM_ID (console team UUID from Console → team settings), or re-run \
             `grok login --management-key` so a **valid Management key** can discover it. \
             Inference API keys never auto-read team id (HTTP 401 on management-api.x.ai)."
                .into(),
        );
    }
    None
}

// --- Postpaid invoice preview (OAuth vs API class) --------------------------

/// Line class for postpaid invoice attribution (team Usage dollars).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostpaidLineClass {
    /// SuperGrok / Grok Build OAuth settlement class (description contains OAuth).
    Oauth,
    /// Console API key / public API class (`product=api` or description starts with API).
    Api,
    /// Other lines (file storage without API prefix, unknown products, …).
    Other,
}

/// Classify one postpaid line for OAuth vs API aggregation.
///
/// Dogfood shape: `description` like `"Grok Build OAuth grok-4.5-build"` vs
/// `"API grok-4.5"`, optional `product` `"grok-build"` / `"api"`.
///
/// Order: description contains `oauth` → OAuth; `product=grok-build` → OAuth
/// (mirrors `product=api` → API when the description omits the word OAuth);
/// then `product=api` or description starts with `api` → API; else Other.
pub fn classify_postpaid_line(description: &str, product: Option<&str>) -> PostpaidLineClass {
    let desc = description.trim();
    let desc_lower = desc.to_ascii_lowercase();
    if desc_lower.contains("oauth") {
        return PostpaidLineClass::Oauth;
    }
    let product = product.map(str::trim).filter(|p| !p.is_empty());
    if product.is_some_and(|p| p.eq_ignore_ascii_case("grok-build")) {
        return PostpaidLineClass::Oauth;
    }
    let product_is_api = product.is_some_and(|p| p.eq_ignore_ascii_case("api"));
    if product_is_api || desc_lower.starts_with("api") {
        return PostpaidLineClass::Api;
    }
    PostpaidLineClass::Other
}

/// One invoice line (subset of Management postpaid preview body).
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostpaidInvoiceLine {
    /// Human description (e.g. `Grok Build OAuth grok-4.5-build`, `API grok-4.5`).
    #[serde(default)]
    pub description: String,
    /// Wire product tag when present (`grok-build`, `api`, …).
    #[serde(default)]
    pub product: Option<String>,
    /// Line amount in USD cents as a decimal string (dogfood / live).
    #[serde(default)]
    pub amount: Option<String>,
}

/// Nested `coreInvoice` on the postpaid preview response.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PostpaidCoreInvoice {
    #[serde(default)]
    pub lines: Vec<PostpaidInvoiceLine>,
    /// Period total with corrections (`totalWithCorr.val`, cents string).
    #[serde(default)]
    pub total_with_corr: Option<UsdCentsVal>,
    /// Free/default credits issued this period (cents string; often negative).
    #[serde(default)]
    pub default_credits_issued: Option<String>,
    /// Prepaid credits applied on the postpaid invoice (not SuperGrok extras).
    #[serde(default)]
    pub prepaid_credits: Option<UsdCentsVal>,
}

/// Calendar billing cycle on the preview response.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostpaidBillingCycle {
    pub year: Option<i32>,
    pub month: Option<u32>,
}

/// Documented postpaid invoice preview response (fields we parse only).
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PostpaidInvoicePreviewResponse {
    #[serde(default)]
    pub core_invoice: Option<PostpaidCoreInvoice>,
    /// Free pool / default credits remaining or cap (cents string when present).
    #[serde(default)]
    pub default_credits: Option<String>,
    #[serde(default)]
    pub billing_cycle: Option<PostpaidBillingCycle>,
    /// Soft/hard spending limit string when present (not required for OAuth/API).
    #[serde(default)]
    pub effective_spending_limit: Option<String>,
}

/// Plain console-team postpaid preview meter for TUI / cache / limits JSON.
///
/// Distinct from [`ConsoleTeamPrepaidMeter`] (ledger remaining) and from
/// SuperGrok included % / SuperGrok $ extras. Amounts are non-negative USD
/// cents aggregates for the current invoice period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleTeamPostpaidPreview {
    pub team_id: String,
    /// Period total (from `totalWithCorr` when present, else sum of line amounts).
    pub period_total_cents: i64,
    /// Sum of line amounts classified as OAuth (Grok Build OAuth class).
    pub oauth_class_cents: i64,
    /// Sum of line amounts classified as API / ApiKey class.
    pub api_class_cents: i64,
    /// Sum of line amounts not OAuth and not API.
    pub other_class_cents: i64,
    /// Top-level `defaultCredits` when parseable (cents).
    pub default_credits_cents: Option<i64>,
    /// `coreInvoice.defaultCreditsIssued` abs when parseable (cents).
    pub default_credits_issued_cents: Option<i64>,
    pub billing_cycle_year: Option<i32>,
    pub billing_cycle_month: Option<u32>,
}

impl ConsoleTeamPostpaidPreview {
    /// True when OAuth class spend is strictly greater than API class and &gt; 0.
    ///
    /// Used for honesty C6: SuperGrok session can still move team Usage dollars.
    pub fn oauth_class_dominates(&self) -> bool {
        self.oauth_class_cents > 0 && self.oauth_class_cents > self.api_class_cents
    }
}

/// Parse a signed/unsigned cents string to non-negative display cents.
fn postpaid_cents_abs(val: &str) -> Option<i64> {
    let n: i64 = val.trim().parse().ok()?;
    Some(n.saturating_abs())
}

/// Aggregate OAuth / API / other line totals from a preview body.
///
/// Returns `None` when no line amount and no `totalWithCorr` parse as cents
/// (never invents a zero meter from unparseable body shape).
pub fn console_team_postpaid_from_response(
    team_id: &str,
    body: &PostpaidInvoicePreviewResponse,
) -> Option<ConsoleTeamPostpaidPreview> {
    let team = team_id.trim();
    if team.is_empty() {
        return None;
    }
    let core = body.core_invoice.as_ref()?;
    let mut oauth: i64 = 0;
    let mut api: i64 = 0;
    let mut other: i64 = 0;
    let mut any_line_amount = false;
    for line in &core.lines {
        let Some(amt) = line.amount.as_deref().and_then(postpaid_cents_abs) else {
            continue;
        };
        any_line_amount = true;
        match classify_postpaid_line(&line.description, line.product.as_deref()) {
            PostpaidLineClass::Oauth => oauth = oauth.saturating_add(amt),
            PostpaidLineClass::Api => api = api.saturating_add(amt),
            PostpaidLineClass::Other => other = other.saturating_add(amt),
        }
    }
    let line_sum = oauth.saturating_add(api).saturating_add(other);
    let period_from_total = core
        .total_with_corr
        .as_ref()
        .and_then(|v| postpaid_cents_abs(&v.val));
    // Need at least one parseable cents signal so we do not invent $0.00.
    if !any_line_amount && period_from_total.is_none() {
        return None;
    }
    let period_total = period_from_total.unwrap_or(line_sum);
    Some(ConsoleTeamPostpaidPreview {
        team_id: team.to_owned(),
        period_total_cents: period_total,
        oauth_class_cents: oauth,
        api_class_cents: api,
        other_class_cents: other,
        default_credits_cents: body.default_credits.as_deref().and_then(postpaid_cents_abs),
        default_credits_issued_cents: core
            .default_credits_issued
            .as_deref()
            .and_then(postpaid_cents_abs),
        billing_cycle_year: body.billing_cycle.as_ref().and_then(|c| c.year),
        billing_cycle_month: body.billing_cycle.as_ref().and_then(|c| c.month),
    })
}

struct PostpaidCacheEntry {
    meter: ConsoleTeamPostpaidPreview,
    fetched_at: Instant,
}

static POSTPAID_CACHE: Mutex<Option<PostpaidCacheEntry>> = Mutex::new(None);

/// Clear the process postpaid preview cache (tests / management key clear).
pub fn clear_console_team_postpaid_cache() {
    if let Ok(mut g) = POSTPAID_CACHE.lock() {
        *g = None;
    }
}

/// Last successful postpaid preview from process cache, if still fresh.
pub fn cached_console_team_postpaid(team_id: &str) -> Option<ConsoleTeamPostpaidPreview> {
    let team = team_id.trim();
    if team.is_empty() {
        return None;
    }
    let g = POSTPAID_CACHE.lock().ok()?;
    let entry = g.as_ref()?;
    if entry.meter.team_id != team {
        return None;
    }
    if entry.fetched_at.elapsed() > POSTPAID_CACHE_TTL {
        return None;
    }
    Some(entry.meter.clone())
}

/// Process-cache postpaid when team id is known and the entry is still fresh.
pub fn cached_console_team_postpaid_default() -> Option<ConsoleTeamPostpaidPreview> {
    let team = resolve_management_team_id_default()?;
    cached_console_team_postpaid(&team)
}

fn remember_postpaid(meter: &ConsoleTeamPostpaidPreview) {
    if let Ok(mut g) = POSTPAID_CACHE.lock() {
        *g = Some(PostpaidCacheEntry {
            meter: meter.clone(),
            fetched_at: Instant::now(),
        });
    }
}

/// Structured fields for a successful postpaid preview fetch (no secrets).
pub fn management_postpaid_success_log_fields(
    meter: &ConsoleTeamPostpaidPreview,
) -> serde_json::Value {
    serde_json::json!({
        "team_id": meter.team_id,
        "period_total_cents": meter.period_total_cents,
        "oauth_class_cents": meter.oauth_class_cents,
        "api_class_cents": meter.api_class_cents,
        "other_class_cents": meter.other_class_cents,
        "oauth_dominates": meter.oauth_class_dominates(),
    })
}

/// Fetch console team postpaid invoice preview when management key + team_id
/// are present. Same resolve rules as prepaid (discovery when team id missing).
///
/// Returns `None` when key is missing, discovery fails, HTTP fails, or body is
/// unusable. Never invents OAuth/API dollars.
pub async fn fetch_console_team_postpaid_preview(
    management_key: Option<&str>,
    team_id: Option<&str>,
) -> Option<ConsoleTeamPostpaidPreview> {
    fetch_console_team_postpaid_preview_at(MANAGEMENT_API_BASE_URL, management_key, team_id).await
}

/// Same as [`fetch_console_team_postpaid_preview`] with an injectable base URL
/// (hermetic HTTP mock tests).
pub async fn fetch_console_team_postpaid_preview_at(
    base_url: &str,
    management_key: Option<&str>,
    team_id: Option<&str>,
) -> Option<ConsoleTeamPostpaidPreview> {
    let key = management_key.map(str::trim).filter(|s| !s.is_empty())?;
    let team = match team_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t.to_owned(),
        None => resolve_management_team_id_with_discovery(base_url, Some(key), None).await?,
    };

    if let Some(cached) = cached_console_team_postpaid(&team) {
        return Some(cached);
    }

    let base = management_api_base(Some(base_url));
    let url = format!("{base}{}", postpaid_invoice_preview_path(&team));
    let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, key);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let client = crate::http::shared_client();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            status,
            &headers,
            "Management API postpaid preview rate limit",
        );
        tracing::debug!(status, "management postpaid preview: non-success status");
        return None;
    }
    let parsed: PostpaidInvoicePreviewResponse = response.json().await.ok()?;
    let meter = console_team_postpaid_from_response(&team, &parsed)?;
    remember_postpaid(&meter);
    let log_fields = management_postpaid_success_log_fields(&meter);
    tracing::info!(
        team_id = %meter.team_id,
        period_total_cents = meter.period_total_cents,
        oauth_class_cents = meter.oauth_class_cents,
        api_class_cents = meter.api_class_cents,
        "management postpaid: fetched invoice preview"
    );
    xai_grok_telemetry::unified_log::info(
        "management postpaid: fetched invoice preview",
        None,
        Some(log_fields),
    );
    Some(meter)
}

/// Resolve credentials from config/store/env defaults and fetch postpaid preview.
pub async fn fetch_console_team_postpaid_preview_default() -> Option<ConsoleTeamPostpaidPreview> {
    let key = resolve_management_api_key_default();
    let config_team = crate::util::config::load_management_team_id_sync();
    let team = resolve_management_team_id_with_discovery(
        MANAGEMENT_API_BASE_URL,
        key.as_deref(),
        config_team.as_deref(),
    )
    .await;
    fetch_console_team_postpaid_preview(key.as_deref(), team.as_deref()).await
}

/// Short setup note when postpaid preview is unconfigured (same key as prepaid).
pub fn console_team_postpaid_setup_note(
    gap_missing_key: bool,
    gap_missing_team: bool,
) -> Option<String> {
    if gap_missing_key {
        return Some(
            "Console team postpaid (OAuth vs API Usage dollars on console.x.ai) needs a \
             Management API key: Console → Settings → Management Keys, then \
             `grok login --management-key` or [endpoints] management_api_key / \
             XAI_MANAGEMENT_API_KEY. Distinct from prepaid remaining and SuperGrok $ extras."
                .into(),
        );
    }
    if gap_missing_team {
        return Some(
            "Management key is set but team id is unknown for postpaid preview. Set \
             [endpoints] management_team_id or XAI_MANAGEMENT_TEAM_ID, or re-run \
             `grok login --management-key` so a valid Management key can discover it."
                .into(),
        );
    }
    None
}

// ---------------------------------------------------------------------------
// Usage series (POST …/billing/teams/{team_id}/usage)
// ---------------------------------------------------------------------------

/// One value aggregation on a usage analytics request (`usd` + sum, etc.).
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsValueSpec {
    pub name: String,
    pub aggregation: String,
}

/// Local-timezone time range for Management usage analytics.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsTimeRange {
    /// From-time `YYYY-MM-DD HH:MM:SS` in [`Self::timezone`].
    pub start_time: String,
    /// To-time `YYYY-MM-DD HH:MM:SS` (not including) in [`Self::timezone`].
    pub end_time: String,
    /// IANA timezone id (e.g. `Etc/GMT`).
    pub timezone: String,
}

/// Documented `analyticsRequest` body (fields we send).
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsRequestInner {
    pub time_range: UsageAnalyticsTimeRange,
    pub time_unit: String,
    pub values: Vec<UsageAnalyticsValueSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
}

/// Top-level POST body for usage analytics.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsRequestBody {
    pub analytics_request: UsageAnalyticsRequestInner,
}

/// One dense data point on a series (docs: values are USD floats, not cents).
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsDataPoint {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub values: Vec<f64>,
}

/// One group-by time series from the usage analytics response.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsTimeSeries {
    #[serde(default)]
    pub group: Vec<String>,
    #[serde(default)]
    pub group_labels: Vec<String>,
    #[serde(default)]
    pub data_points: Vec<UsageAnalyticsDataPoint>,
}

/// Documented POST usage analytics response (fields we parse only).
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsResponse {
    #[serde(default)]
    pub time_series: Vec<UsageAnalyticsTimeSeries>,
    #[serde(default)]
    pub limit_reached: bool,
}

/// One aggregated description row for TUI / limits (class + window total USD).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTeamUsageSeriesRow {
    /// Description / group label from Management (e.g. `Grok Build OAuth …`).
    pub label: String,
    /// OAuth / Grok Build class vs API-key class vs other (same classifier as postpaid).
    pub class: PostpaidLineClass,
    /// Sum of `usd` values across the window for this group.
    pub total_usd: f64,
}

/// Plain console-team usage series summary for limits / cache.
///
/// Distinct from prepaid ledger remaining, postpaid invoice period totals,
/// SuperGrok free period allowance, and SuperGrok prepaid top-up dollars.
/// Amounts are **USD** (docs sample values are floats, not cents).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTeamUsageSeries {
    pub team_id: String,
    pub start_time: String,
    pub end_time: String,
    pub timezone: String,
    /// Rows sorted by total_usd descending (non-zero first).
    pub rows: Vec<ConsoleTeamUsageSeriesRow>,
    pub oauth_class_usd: f64,
    pub api_class_usd: f64,
    pub other_class_usd: f64,
    pub limit_reached: bool,
}

impl ConsoleTeamUsageSeries {
    /// Window total across all classes.
    pub fn period_total_usd(&self) -> f64 {
        self.oauth_class_usd + self.api_class_usd + self.other_class_usd
    }
}

/// Build a day-bucketed `usd` sum request grouped by description for a window.
///
/// `day_window` is how many calendar days before today (UTC) to include, ending
/// at the start of tomorrow UTC so the current day is covered. Docs require
/// local-timezone wall clocks; we use `Etc/GMT` so the strings match UTC.
pub fn usage_analytics_day_sum_by_description_request(
    day_window: i64,
) -> UsageAnalyticsRequestBody {
    let days = day_window.max(1);
    let today = chrono::Utc::now().date_naive();
    let start = today - chrono::Duration::days(days.saturating_sub(1));
    let end_exclusive = today + chrono::Duration::days(1);
    UsageAnalyticsRequestBody {
        analytics_request: UsageAnalyticsRequestInner {
            time_range: UsageAnalyticsTimeRange {
                start_time: format!("{} 00:00:00", start.format("%Y-%m-%d")),
                end_time: format!("{} 00:00:00", end_exclusive.format("%Y-%m-%d")),
                timezone: "Etc/GMT".into(),
            },
            time_unit: "TIME_UNIT_DAY".into(),
            values: vec![UsageAnalyticsValueSpec {
                name: "usd".into(),
                aggregation: "AGGREGATION_SUM".into(),
            }],
            group_by: vec!["description".into()],
            filters: vec![],
        },
    }
}

/// Sum the first value channel of a data point (USD).
fn datapoint_usd_sum(points: &[UsageAnalyticsDataPoint]) -> f64 {
    points
        .iter()
        .map(|p| p.values.first().copied().unwrap_or(0.0))
        .sum()
}

/// Primary label for a series row (groupLabels first, else group).
fn series_row_label(series: &UsageAnalyticsTimeSeries) -> String {
    series
        .group_labels
        .first()
        .or_else(|| series.group.first())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(ungrouped)".into())
}

/// Aggregate a usage analytics response into class totals + per-description rows.
///
/// Returns `None` when there is no parseable non-empty series and no zero-only
/// empty body that still proves a successful shape (empty `timeSeries` is a
/// valid empty window → empty series with zeros, not `None`).
///
/// `None` is reserved for unusable responses that callers should treat as a gap
/// (caller already failed HTTP / JSON). Empty successful body → Some with zeros.
pub fn console_team_usage_series_from_response(
    team_id: &str,
    body: &UsageAnalyticsResponse,
    request: &UsageAnalyticsRequestBody,
) -> ConsoleTeamUsageSeries {
    let team = team_id.trim();
    let mut rows: Vec<ConsoleTeamUsageSeriesRow> = Vec::new();
    let mut oauth = 0.0_f64;
    let mut api = 0.0_f64;
    let mut other = 0.0_f64;
    for series in &body.time_series {
        let label = series_row_label(series);
        let total = datapoint_usd_sum(&series.data_points);
        // product is not on the series group; classify from description only.
        let class = classify_postpaid_line(&label, None);
        match class {
            PostpaidLineClass::Oauth => oauth += total,
            PostpaidLineClass::Api => api += total,
            PostpaidLineClass::Other => other += total,
        }
        if total.abs() > f64::EPSILON {
            rows.push(ConsoleTeamUsageSeriesRow {
                label,
                class,
                total_usd: total,
            });
        }
    }
    rows.sort_by(|a, b| {
        b.total_usd
            .partial_cmp(&a.total_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let tr = &request.analytics_request.time_range;
    ConsoleTeamUsageSeries {
        team_id: team.to_owned(),
        start_time: tr.start_time.clone(),
        end_time: tr.end_time.clone(),
        timezone: tr.timezone.clone(),
        rows,
        oauth_class_usd: oauth,
        api_class_usd: api,
        other_class_usd: other,
        limit_reached: body.limit_reached,
    }
}

/// Structured fields for a successful usage series fetch (no secrets).
pub fn management_usage_series_success_log_fields(
    series: &ConsoleTeamUsageSeries,
) -> serde_json::Value {
    serde_json::json!({
        "team_id": series.team_id,
        "oauth_class_usd": series.oauth_class_usd,
        "api_class_usd": series.api_class_usd,
        "other_class_usd": series.other_class_usd,
        "row_count": series.rows.len(),
        "limit_reached": series.limit_reached,
        "start_time": series.start_time,
        "end_time": series.end_time,
    })
}

/// Fetch console team usage series via documented POST usage analytics.
///
/// Returns `None` when key is missing, discovery fails, HTTP fails, or body is
/// unusable. Never invents spend dollars.
pub async fn fetch_console_team_usage_series(
    management_key: Option<&str>,
    team_id: Option<&str>,
    day_window: i64,
) -> Option<ConsoleTeamUsageSeries> {
    fetch_console_team_usage_series_at(MANAGEMENT_API_BASE_URL, management_key, team_id, day_window)
        .await
}

/// Same as [`fetch_console_team_usage_series`] with an injectable base URL
/// (hermetic HTTP mock tests).
pub async fn fetch_console_team_usage_series_at(
    base_url: &str,
    management_key: Option<&str>,
    team_id: Option<&str>,
    day_window: i64,
) -> Option<ConsoleTeamUsageSeries> {
    let key = management_key.map(str::trim).filter(|s| !s.is_empty())?;
    let team = match team_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t.to_owned(),
        None => resolve_management_team_id_with_discovery(base_url, Some(key), None).await?,
    };

    let request = usage_analytics_day_sum_by_description_request(day_window);
    let base = management_api_base(Some(base_url));
    let url = format!("{base}{}", usage_analytics_path(&team));
    let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, key);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let client = crate::http::shared_client();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            status,
            &headers,
            "Management API usage series rate limit",
        );
        tracing::debug!(status, "management usage series: non-success status");
        return None;
    }
    let parsed: UsageAnalyticsResponse = response.json().await.ok()?;
    let series = console_team_usage_series_from_response(&team, &parsed, &request);
    let log_fields = management_usage_series_success_log_fields(&series);
    tracing::info!(
        team_id = %series.team_id,
        oauth_class_usd = series.oauth_class_usd,
        api_class_usd = series.api_class_usd,
        row_count = series.rows.len(),
        "management usage series: fetched POST analytics"
    );
    xai_grok_telemetry::unified_log::info(
        "management usage series: fetched POST analytics",
        None,
        Some(log_fields),
    );
    Some(series)
}

/// Resolve credentials from config/store/env defaults and fetch usage series.
pub async fn fetch_console_team_usage_series_default(
    day_window: i64,
) -> Option<ConsoleTeamUsageSeries> {
    let key = resolve_management_api_key_default();
    let config_team = crate::util::config::load_management_team_id_sync();
    let team = resolve_management_team_id_with_discovery(
        MANAGEMENT_API_BASE_URL,
        key.as_deref(),
        config_team.as_deref(),
    )
    .await;
    fetch_console_team_usage_series(key.as_deref(), team.as_deref(), day_window).await
}

/// Short setup note when usage series cannot run (same key as prepaid/postpaid).
pub fn console_team_usage_series_setup_note(
    gap_missing_key: bool,
    gap_missing_team: bool,
) -> Option<String> {
    if gap_missing_key {
        return Some(
            "Team usage spend series needs a Management API key (POST billing usage \
             analytics): Console → Settings → Management Keys, then \
             `grok login --management-key`. Distinct from prepaid wallet and SuperGrok \
             free period allowance."
                .into(),
        );
    }
    if gap_missing_team {
        return Some(
            "Management key is set but team id is unknown for usage series. Set \
             [endpoints] management_team_id or XAI_MANAGEMENT_TEAM_ID, or re-run \
             `grok login --management-key`."
                .into(),
        );
    }
    None
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
        let _mgmt = EnvGuard::unset(XAI_MANAGEMENT_API_KEY_ENV);
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
        let _mgmt = EnvGuard::unset(XAI_MANAGEMENT_API_KEY_ENV);
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
    #[serial]
    fn store_refuses_when_management_env_set() {
        let _mgmt = EnvGuard::set(XAI_MANAGEMENT_API_KEY_ENV, "env-mgmt-key");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        match store_management_api_key(&store, "should-not-write") {
            Err(ManagementAuthError::EnvVarSet) => {}
            other => panic!("expected EnvVarSet, got {other:?}"),
        }
    }

    #[test]
    #[serial]
    fn resolve_prefers_config_over_env_over_store() {
        let _mgmt = EnvGuard::set(XAI_MANAGEMENT_API_KEY_ENV, "from-env");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        // Env set: store write refused — seed store by unsetting env briefly.
        drop(_mgmt);
        let _unset = EnvGuard::unset(XAI_MANAGEMENT_API_KEY_ENV);
        store_management_api_key(&store, "from-store").unwrap();
        drop(_unset);
        let _mgmt = EnvGuard::set(XAI_MANAGEMENT_API_KEY_ENV, "from-env");

        let resolved = resolve_management_api_key(Some("from-config"), &store).unwrap();
        assert_eq!(resolved.as_deref(), Some("from-config"));
        let from_env = resolve_management_api_key(None, &store).unwrap();
        assert_eq!(from_env.as_deref(), Some("from-env"));
        drop(_mgmt);
        let _unset = EnvGuard::unset(XAI_MANAGEMENT_API_KEY_ENV);
        let from_store_only = resolve_management_api_key(None, &store).unwrap();
        assert_eq!(from_store_only.as_deref(), Some("from-store"));
        let blank_config = resolve_management_api_key(Some("  "), &store).unwrap();
        assert_eq!(blank_config.as_deref(), Some("from-store"));
    }

    #[test]
    #[serial]
    fn team_id_resolve_config_then_env_ignores_blank() {
        let _team = EnvGuard::unset(XAI_MANAGEMENT_TEAM_ID_ENV);
        assert_eq!(resolve_management_team_id(None), None);
        assert_eq!(resolve_management_team_id(Some("")), None);
        assert_eq!(resolve_management_team_id(Some("  ")), None);
        assert_eq!(
            resolve_management_team_id(Some("  team-uuid-1  ")).as_deref(),
            Some("team-uuid-1")
        );
        let _team = EnvGuard::set(XAI_MANAGEMENT_TEAM_ID_ENV, "  team-from-env  ");
        assert_eq!(
            resolve_management_team_id(None).as_deref(),
            Some("team-from-env")
        );
        // Config wins over env.
        assert_eq!(
            resolve_management_team_id(Some("team-from-config")).as_deref(),
            Some("team-from-config")
        );
    }

    #[test]
    fn validation_body_prefers_scope_id_for_team_billing() {
        let meta: ManagementKeyValidation = serde_json::from_value(serde_json::json!({
            "teamId": "old-team",
            "scope": "SCOPE_TEAM",
            "scopeId": "scope-team-uuid",
            "name": "test key"
        }))
        .unwrap();
        assert_eq!(
            meta.team_id_for_billing().as_deref(),
            Some("scope-team-uuid")
        );
        let legacy: ManagementKeyValidation = serde_json::from_value(serde_json::json!({
            "teamId": "legacy-only-team"
        }))
        .unwrap();
        assert_eq!(
            legacy.team_id_for_billing().as_deref(),
            Some("legacy-only-team")
        );
        // snake_case defensive parse (if wire ever flips).
        let snake: ManagementKeyValidation = serde_json::from_value(serde_json::json!({
            "team_id": "snake-team",
            "scope_id": "snake-scope",
            "scope": "SCOPE_TEAM"
        }))
        .unwrap();
        assert_eq!(snake.team_id_for_billing().as_deref(), Some("snake-scope"));
    }

    /// Named contract: login help for HTTP 401 must not claim "offline / billing
    /// scope" — live Management API returns invalid bearer for inference keys.
    #[test]
    fn validate_failure_help_distinguishes_invalid_key_from_offline() {
        let invalid = format_management_key_validate_failure(
            &ManagementKeyValidateFailure::InvalidManagementKey {
                status: 401,
                message: Some(
                    "Invalid bearer token. Please ensure you use a valid management key.".into(),
                ),
            },
        );
        assert!(invalid.contains("401"), "{invalid}");
        assert!(
            invalid.contains("Management key") || invalid.contains("Management Keys"),
            "{invalid}"
        );
        assert!(
            invalid.contains("not an inference") || invalid.contains("not an inference API"),
            "{invalid}"
        );
        assert!(
            invalid.contains("management_team_id"),
            "must still show config pin path: {invalid}"
        );
        assert!(
            !invalid.contains("offline or key lacks billing scope"),
            "retired mushy copy must not return: {invalid}"
        );
        let offline =
            format_management_key_validate_failure(&ManagementKeyValidateFailure::Network);
        assert!(
            offline.contains("offline") || offline.contains("management-api"),
            "{offline}"
        );
        let no_team = format_management_key_validate_failure(
            &ManagementKeyValidateFailure::NoTeamIdInResponse,
        );
        assert!(
            no_team.contains("no team id") || no_team.contains("team id"),
            "{no_team}"
        );
    }

    #[test]
    fn setup_note_names_console_business_not_supergrok_extras() {
        let note = console_team_prepaid_setup_note(true, false).expect("key note");
        assert!(note.contains("Management API key"), "{note}");
        assert!(
            note.contains("management-key") || note.contains("management_api_key"),
            "{note}"
        );
        assert!(note.contains("not SuperGrok"), "{note}");
        assert!(
            note.contains("not XAI_API_KEY") || note.contains("Not XAI_API_KEY"),
            "{note}"
        );
        let team_note = console_team_prepaid_setup_note(false, true).expect("team note");
        assert!(team_note.contains("team id"), "{team_note}");
        assert!(
            team_note.contains("valid Management key") || team_note.contains("401"),
            "{team_note}"
        );
        assert!(console_team_prepaid_setup_note(false, false).is_none());
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

    /// Named contract: successful prepaid info log carries team_id, abs
    /// balance_cents, and signed wire total.val as integer — never a key.
    #[test]
    fn management_prepaid_success_log_fields_are_honest_and_keyless() {
        let fields = management_prepaid_success_log_fields(
            "61fab250-b2c1-40cf-b5b8-628e673a2eeb",
            34_000,
            "-34000",
        );
        assert_eq!(fields["team_id"], "61fab250-b2c1-40cf-b5b8-628e673a2eeb");
        assert_eq!(fields["balance_cents"], 34_000);
        assert_eq!(fields["total_val_cents"], -34_000);
        let s = fields.to_string();
        assert!(
            !s.contains("Bearer") && !s.contains("xai-") && !s.to_lowercase().contains("key"),
            "must never embed key material: {s}"
        );
        // Dogfood-class $1,317.15 remaining (hermetic fixture shape).
        let dogfood = management_prepaid_success_log_fields("team-1", 131_715, "-131715");
        assert_eq!(dogfood["balance_cents"], 131_715);
        assert_eq!(dogfood["total_val_cents"], -131_715);
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

    /// RED/GREEN: missing management key leaves meter absent (no invented $).
    #[tokio::test]
    #[serial]
    async fn fetch_missing_key_or_team_id_returns_none() {
        clear_console_team_prepaid_cache();
        clear_discovered_team_id_cache();
        assert!(
            fetch_console_team_prepaid_balance_at("http://127.0.0.1:1", None, Some("team-1"),)
                .await
                .is_none()
        );
        // Key present, no team, unreachable host → discovery fails → None.
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

    /// RED/GREEN: HTTP 401 from validation is InvalidManagementKey, not silent
    /// network-ish None only — login can print the real reason.
    #[tokio::test]
    async fn validate_outcome_maps_401_to_invalid_management_key() {
        let app = Router::new().route(
            MANAGEMENT_KEY_VALIDATION_PATH,
            get(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "code": 16,
                        "message": "Invalid bearer token. Please ensure you use a valid management key.",
                        "details": []
                    })),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");
        let outcome = validate_management_key_outcome_at(&base, "not-a-mgmt-key").await;
        match outcome {
            ManagementKeyValidateOutcome::Err(
                ManagementKeyValidateFailure::InvalidManagementKey { status, message },
            ) => {
                assert_eq!(status, 401);
                let msg = message.expect("api message");
                assert!(
                    msg.to_lowercase().contains("management key")
                        || msg.to_lowercase().contains("invalid bearer"),
                    "{msg}"
                );
            }
            other => panic!("expected InvalidManagementKey, got {other:?}"),
        }
        // Thin Option API still returns None (discovery path).
        assert!(
            validate_management_key_at(&base, "not-a-mgmt-key")
                .await
                .is_none()
        );
        server.abort();
    }

    /// Named contract: 2xx body with teamId/scopeId still discovers team id.
    #[tokio::test]
    async fn validate_outcome_ok_parses_team_id() {
        let app = Router::new().route(
            MANAGEMENT_KEY_VALIDATION_PATH,
            get(|| async {
                Json(serde_json::json!({
                    "teamId": "team-ok-1",
                    "scope": "SCOPE_TEAM",
                    "scopeId": "team-ok-1",
                    "name": "ok"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");
        match validate_management_key_outcome_at(&base, "good-mgmt-key").await {
            ManagementKeyValidateOutcome::Ok(meta) => {
                assert_eq!(meta.team_id_for_billing().as_deref(), Some("team-ok-1"));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
        server.abort();
    }

    /// Named contract: management key alone can discover team id via validation
    /// and still return console team **prepaid ledger** cents (not a claim that
    /// dashboard Credits remaining always equals this field alone).
    #[tokio::test]
    #[serial]
    async fn fetch_prepaid_discovers_team_id_from_key_validation() {
        clear_console_team_prepaid_cache();
        clear_discovered_team_id_cache();
        let app = Router::new()
            .route(
                MANAGEMENT_KEY_VALIDATION_PATH,
                get(|headers: HeaderMap| async move {
                    let auth = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if auth != "Bearer hermetic-mgmt-key" {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    Ok(Json(serde_json::json!({
                        "teamId": "team-discovered-1",
                        "scope": "SCOPE_TEAM",
                        "scopeId": "team-discovered-1",
                        "name": "hermetic"
                    })))
                }),
            )
            .route(
                "/v1/billing/teams/{team_id}/prepaid/balance",
                get(
                    |Path(team_id): Path<String>, headers: HeaderMap| async move {
                        let auth = headers
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        if auth != "Bearer hermetic-mgmt-key" {
                            return Err(StatusCode::UNAUTHORIZED);
                        }
                        if team_id != "team-discovered-1" {
                            return Err(StatusCode::NOT_FOUND);
                        }
                        // $1,317.15 remaining (operator dogfood class) as cents abs.
                        Ok(Json(serde_json::json!({
                            "total": { "val": "-131715" },
                            "changes": []
                        })))
                    },
                ),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let base = format!("http://{addr}");
        // No team_id arg — discovery from validation must fill it.
        let meter = fetch_console_team_prepaid_balance_at(&base, Some("hermetic-mgmt-key"), None)
            .await
            .expect("prepaid after discovery");
        assert_eq!(meter.team_id, "team-discovered-1");
        assert_eq!(meter.balance_cents, 131_715);
        assert_eq!(
            cached_discovered_team_id().as_deref(),
            Some("team-discovered-1")
        );

        clear_console_team_prepaid_cache();
        clear_discovered_team_id_cache();
        server.abort();
    }

    /// Hermetic mock: prepaid balance → cents for ConsoleMeter.
    #[tokio::test]
    #[serial]
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

        // Named contract: clear/bust (explicit `grok limits` path) forces the
        // next fetch to hit Management again instead of serving ≤60s warm cache.
        clear_console_team_billing_meter_caches();
        assert!(
            cached_console_team_prepaid("team-hermetic-1").is_none(),
            "bust must drop warm prepaid entry"
        );
        let forced = fetch_console_team_prepaid_balance_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-hermetic-1"),
        )
        .await
        .expect("force re-fetch after bust");
        assert_eq!(forced.balance_cents, 12500);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            2,
            "after clear_console_team_billing_meter_caches, next fetch must hit HTTP"
        );

        clear_console_team_prepaid_cache();
        server.abort();
    }

    #[test]
    fn billing_meter_cache_ttl_secs_is_sixty_and_prepaid_alias_matches() {
        assert_eq!(CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS, 60);
        assert_eq!(
            CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS, CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS,
            "prepaid alias must track shared billing-meter TTL"
        );
    }

    #[tokio::test]
    #[serial]
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

    /// Named contract: Management prepaid 429 publishes multi-process shared
    /// cooldown (flock store). Two logical store handles share the wait; kill
    /// switch is covered in `shared_http_rate_limit` unit tests.
    #[tokio::test]
    #[serial]
    async fn prepaid_429_observes_shared_rate_limit_store() {
        use grok_rate_limit::{DISABLE_ENV, SharedRateLimitStore};
        use std::time::Duration;

        clear_console_team_prepaid_cache();
        // Ensure shared limits are enabled for this serial test.
        let prev_disable = std::env::var_os(DISABLE_ENV);
        if prev_disable.is_some() {
            // SAFETY: serial test; restored at end.
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }

        let dir = tempfile::TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let _override =
            crate::shared_http_rate_limit::override_shared_store_for_test(store.clone());

        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/prepaid/balance",
            get(|| async {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(axum::http::header::RETRY_AFTER, "3")],
                    "rate limited",
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");
        let mgmt_key = "hermetic-rate-limit-key";
        assert!(
            fetch_console_team_prepaid_balance_at(&base, Some(mgmt_key), Some("team-rl"),)
                .await
                .is_none()
        );

        let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, mgmt_key);
        let rem = store.remaining(&rate_key);
        assert!(
            rem >= Duration::from_secs(1),
            "429 must leave shared remaining for peers, got {rem:?}"
        );
        // Second logical handle (simulates another process) sees the same file.
        let peer = SharedRateLimitStore::open(dir.path()).unwrap();
        let peer_rem = peer.remaining(&rate_key);
        assert!(
            peer_rem >= Duration::from_secs(1),
            "peer handle must see same cooldown, got {peer_rem:?}"
        );

        server.abort();
        match prev_disable {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => {}
        }
    }

    /// Bare 403 without Retry-After (invalid key) must not publish cooldown.
    #[tokio::test]
    #[serial]
    async fn prepaid_403_without_retry_after_does_not_observe() {
        use grok_rate_limit::{DISABLE_ENV, SharedRateLimitStore};
        use std::time::Duration;

        clear_console_team_prepaid_cache();
        let prev_disable = std::env::var_os(DISABLE_ENV);
        if prev_disable.is_some() {
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }

        let dir = tempfile::TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let _override =
            crate::shared_http_rate_limit::override_shared_store_for_test(store.clone());

        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/prepaid/balance",
            get(|| async { StatusCode::FORBIDDEN }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");
        let mgmt_key = "hermetic-forbidden-key";
        assert!(
            fetch_console_team_prepaid_balance_at(&base, Some(mgmt_key), Some("team-f"),)
                .await
                .is_none()
        );
        let rate_key = crate::shared_http_rate_limit::management_provider_key(&base, mgmt_key);
        assert_eq!(
            store.remaining(&rate_key),
            Duration::ZERO,
            "bare 403 must not poison shared cooldown"
        );
        server.abort();
        match prev_disable {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => {}
        }
    }

    #[test]
    fn postpaid_path_includes_team_id() {
        let p = postpaid_invoice_preview_path("65c1e471-205f-4566-9c5a-07198bcdf4ce");
        assert_eq!(
            p,
            "/v1/billing/teams/65c1e471-205f-4566-9c5a-07198bcdf4ce/postpaid/invoice/preview"
        );
        assert_eq!(
            POSTPAID_INVOICE_PREVIEW_PATH_TEMPLATE,
            "/v1/billing/teams/{team_id}/postpaid/invoice/preview"
        );
    }

    #[test]
    fn classify_postpaid_line_oauth_vs_api() {
        assert_eq!(
            classify_postpaid_line("Grok Build OAuth grok-4.5-build", Some("grok-build")),
            PostpaidLineClass::Oauth
        );
        assert_eq!(
            classify_postpaid_line("API grok-4.5", Some("api")),
            PostpaidLineClass::Api
        );
        assert_eq!(
            classify_postpaid_line("API Speech-to-Text (WebSocket)", Some("api")),
            PostpaidLineClass::Api
        );
        assert_eq!(
            classify_postpaid_line("File Storage", Some("api")),
            PostpaidLineClass::Api
        );
        assert_eq!(
            classify_postpaid_line("Mystery line", None),
            PostpaidLineClass::Other
        );
        // OAuth wins even if product says api (should not happen on wire).
        assert_eq!(
            classify_postpaid_line("Grok Build OAuth via API path", Some("api")),
            PostpaidLineClass::Oauth
        );
    }

    /// Named contract: product=grok-build alone (no "oauth" in description) → Oauth,
    /// mirroring product=api → Api.
    #[test]
    fn classify_postpaid_line_product_grok_build_is_oauth() {
        assert_eq!(
            classify_postpaid_line("Grok Build tokens", Some("grok-build")),
            PostpaidLineClass::Oauth
        );
        assert_eq!(
            classify_postpaid_line("Grok Build tokens", Some("GROK-BUILD")),
            PostpaidLineClass::Oauth
        );
        // Still Other when product absent and description has no oauth/api cue.
        assert_eq!(
            classify_postpaid_line("Grok Build tokens", None),
            PostpaidLineClass::Other
        );
    }

    #[test]
    fn postpaid_path_encodes_slash_in_team_id() {
        let p = postpaid_invoice_preview_path("ab/cd");
        assert!(
            p.contains("%2F"),
            "slash in team id must be percent-encoded: {p}"
        );
        assert!(
            !p.contains("/ab/cd/"),
            "raw slash must not open an extra path segment: {p}"
        );
        assert_eq!(p, "/v1/billing/teams/ab%2Fcd/postpaid/invoice/preview");
    }

    #[test]
    fn postpaid_path_encodes_or_rejects_dotdot_team_id() {
        let p = postpaid_invoice_preview_path("..");
        assert!(
            !p.contains("/../"),
            "dotdot team id must not form a parent path segment: {p}"
        );
        assert!(
            p.contains("%2E%2E") || p.contains("%2e%2e"),
            "dots must be percent-encoded: {p}"
        );
        // Prepaid path shares the same encoder.
        let prepaid = prepaid_balance_path("..");
        assert!(
            !prepaid.contains("/../"),
            "prepaid path must also encode dotdot: {prepaid}"
        );
    }

    /// Named contract: postpaid success log fields never embed key material
    /// (parity with prepaid keyless log test).
    #[test]
    fn management_postpaid_success_log_fields_are_honest_and_keyless() {
        let meter = ConsoleTeamPostpaidPreview {
            team_id: "61fab250-b2c1-40cf-b5b8-628e673a2eeb".into(),
            period_total_cents: 20_756,
            oauth_class_cents: 20_176,
            api_class_cents: 580,
            other_class_cents: 0,
            default_credits_cents: Some(150_000),
            default_credits_issued_cents: Some(20_756),
            billing_cycle_year: Some(2026),
            billing_cycle_month: Some(8),
        };
        let fields = management_postpaid_success_log_fields(&meter);
        assert_eq!(fields["team_id"], "61fab250-b2c1-40cf-b5b8-628e673a2eeb");
        assert_eq!(fields["period_total_cents"], 20_756);
        assert_eq!(fields["oauth_class_cents"], 20_176);
        assert_eq!(fields["api_class_cents"], 580);
        assert_eq!(fields["oauth_dominates"], true);
        let s = fields.to_string();
        assert!(
            !s.contains("Bearer") && !s.contains("xai-") && !s.to_lowercase().contains("key"),
            "must never embed key material: {s}"
        );
    }

    /// Named contract: unparseable line amounts + no totalWithCorr → None (no $0 invent).
    #[test]
    fn postpaid_preview_none_when_line_amounts_unparseable() {
        let body: PostpaidInvoicePreviewResponse = serde_json::from_value(serde_json::json!({
            "coreInvoice": {
                "lines": [
                    {
                        "description": "Grok Build OAuth grok-4.5-build",
                        "product": "grok-build",
                        "amount": "not-cents"
                    },
                    {
                        "description": "API grok-4.5",
                        "product": "api",
                        "amount": "also-bad"
                    }
                ]
            }
        }))
        .expect("parse fixture");
        assert!(
            console_team_postpaid_from_response("team-bad", &body).is_none(),
            "unparseable amounts without totalWithCorr must not invent a zero meter"
        );
    }

    /// Named contract: dogfood-shaped postpaid body → OAuth vs API class totals.
    #[test]
    fn parse_postpaid_preview_aggregates_oauth_vs_api() {
        let body: PostpaidInvoicePreviewResponse = serde_json::from_value(serde_json::json!({
            "coreInvoice": {
                "lines": [
                    {
                        "description": "Grok Build OAuth grok-4.5-build",
                        "product": "grok-build",
                        "amount": "20100"
                    },
                    {
                        "description": "Grok Build OAuth grok-4.5-build",
                        "product": "grok-build",
                        "amount": "76"
                    },
                    {
                        "description": "API grok-4.5",
                        "product": "api",
                        "amount": "500"
                    },
                    {
                        "description": "API Speech-to-Text (WebSocket)",
                        "product": "api",
                        "amount": "80"
                    }
                ],
                "totalWithCorr": { "val": "20756" },
                "defaultCreditsIssued": "-20756"
            },
            "defaultCredits": "150000",
            "billingCycle": { "year": 2026, "month": 8 }
        }))
        .expect("parse fixture");
        let meter = console_team_postpaid_from_response("team-dogfood", &body).expect("meter");
        assert_eq!(meter.team_id, "team-dogfood");
        assert_eq!(meter.oauth_class_cents, 20176);
        assert_eq!(meter.api_class_cents, 580);
        assert_eq!(meter.other_class_cents, 0);
        assert_eq!(meter.period_total_cents, 20756);
        assert_eq!(meter.default_credits_cents, Some(150_000));
        assert_eq!(meter.default_credits_issued_cents, Some(20_756));
        assert_eq!(meter.billing_cycle_year, Some(2026));
        assert_eq!(meter.billing_cycle_month, Some(8));
        assert!(
            meter.oauth_class_dominates(),
            "dogfood OAuth-heavy must dominate: oauth={} api={}",
            meter.oauth_class_cents,
            meter.api_class_cents
        );
    }

    /// Named contract (Slice 3 M3): hermetic HTTP → OAuth vs API totals.
    #[tokio::test]
    #[serial]
    async fn fetch_postpaid_preview_hermetic_parses_oauth_vs_api_totals() {
        clear_console_team_postpaid_cache();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_h = hits.clone();
        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/postpaid/invoice/preview",
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
                    if team_id != "team-postpaid-1" {
                        return Err(StatusCode::NOT_FOUND);
                    }
                    Ok(Json(serde_json::json!({
                        "coreInvoice": {
                            "lines": [
                                {
                                    "description": "Grok Build OAuth grok-4.5-build",
                                    "unitType": "Prompt text tokens",
                                    "amount": "20100",
                                    "product": "grok-build"
                                },
                                {
                                    "description": "API grok-4.5",
                                    "unitType": "Prompt text tokens",
                                    "amount": "580",
                                    "product": "api"
                                }
                            ],
                            "totalWithCorr": { "val": "20680" },
                            "defaultCreditsIssued": "-20680"
                        },
                        "defaultCredits": "150000",
                        "billingCycle": { "year": 2026, "month": 8 }
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
        let meter = fetch_console_team_postpaid_preview_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-postpaid-1"),
        )
        .await
        .expect("postpaid preview");
        assert_eq!(meter.team_id, "team-postpaid-1");
        assert_eq!(meter.oauth_class_cents, 20100);
        assert_eq!(meter.api_class_cents, 580);
        assert_eq!(meter.period_total_cents, 20680);
        assert!(meter.oauth_class_dominates());
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // Process cache: second call does not hit HTTP again.
        let again = fetch_console_team_postpaid_preview_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-postpaid-1"),
        )
        .await
        .expect("cached postpaid");
        assert_eq!(again.oauth_class_cents, 20100);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "postpaid cache must skip second HTTP"
        );

        // Named contract: combined clear (explicit `grok limits` path) busts
        // **postpaid** warm entry too, not only prepaid.
        clear_console_team_billing_meter_caches();
        assert!(
            cached_console_team_postpaid("team-postpaid-1").is_none(),
            "combined clear must drop warm postpaid entry"
        );
        let forced = fetch_console_team_postpaid_preview_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-postpaid-1"),
        )
        .await
        .expect("force re-fetch postpaid after combined clear");
        assert_eq!(forced.oauth_class_cents, 20100);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            2,
            "after clear_console_team_billing_meter_caches, next postpaid fetch must hit HTTP"
        );

        clear_console_team_postpaid_cache();
        server.abort();
    }

    /// Named contract (Slice 3 M3): no management key → no invented postpaid $.
    #[tokio::test]
    #[serial]
    async fn postpaid_preview_gap_when_no_management_key() {
        clear_console_team_postpaid_cache();
        clear_discovered_team_id_cache();
        assert!(
            fetch_console_team_postpaid_preview_at("http://127.0.0.1:1", None, Some("team-1"),)
                .await
                .is_none(),
            "missing management key must not invent postpaid dollars"
        );
        assert!(
            fetch_console_team_postpaid_preview_at("http://127.0.0.1:1", Some(""), Some("team-1"),)
                .await
                .is_none(),
            "empty management key must not invent postpaid dollars"
        );
        // Setup note for missing key is honest and distinct from prepaid-only copy.
        let note = console_team_postpaid_setup_note(true, false).expect("key gap note");
        assert!(
            note.contains("postpaid") || note.contains("OAuth"),
            "gap note must name postpaid / OAuth: {note}"
        );
        assert!(
            note.contains("Management API key"),
            "gap note must name Management key: {note}"
        );
        assert!(
            !note.to_ascii_lowercase().contains("xai_api_key is enough"),
            "must not claim inference key is enough: {note}"
        );
    }

    /// Named contract (Item 5a): parse usage analytics fixture into OAuth vs
    /// API class USD totals (docs values are USD floats, not cents).
    #[test]
    fn parse_usage_series_aggregates_oauth_vs_api_usd() {
        let body: UsageAnalyticsResponse = serde_json::from_value(serde_json::json!({
            "timeSeries": [
                {
                    "group": ["Grok Build OAuth grok-4.5-build"],
                    "groupLabels": ["Grok Build OAuth grok-4.5-build"],
                    "dataPoints": [
                        { "timestamp": "2026-08-01T00:00:00Z", "values": [12.5] },
                        { "timestamp": "2026-08-02T00:00:00Z", "values": [7.25] }
                    ]
                },
                {
                    "group": ["API grok-4.5"],
                    "groupLabels": ["API grok-4.5"],
                    "dataPoints": [
                        { "timestamp": "2026-08-01T00:00:00Z", "values": [3.0] },
                        { "timestamp": "2026-08-02T00:00:00Z", "values": [1.5] }
                    ]
                },
                {
                    "group": ["File Storage"],
                    "groupLabels": ["File Storage"],
                    "dataPoints": [
                        { "timestamp": "2026-08-01T00:00:00Z", "values": [0.1] }
                    ]
                }
            ],
            "limitReached": false
        }))
        .expect("parse fixture");
        let request = usage_analytics_day_sum_by_description_request(7);
        let series = console_team_usage_series_from_response("team-series", &body, &request);
        assert_eq!(series.team_id, "team-series");
        assert!((series.oauth_class_usd - 19.75).abs() < 1e-9);
        assert!((series.api_class_usd - 4.5).abs() < 1e-9);
        assert!((series.other_class_usd - 0.1).abs() < 1e-9);
        assert_eq!(series.rows.len(), 3);
        assert_eq!(series.rows[0].label, "Grok Build OAuth grok-4.5-build");
        assert_eq!(series.rows[0].class, PostpaidLineClass::Oauth);
        assert!(!series.limit_reached);
    }

    /// Named contract: empty successful series body is honest zeros, not invent.
    #[test]
    fn parse_usage_series_empty_time_series_is_zero_not_none() {
        let body: UsageAnalyticsResponse = serde_json::from_value(serde_json::json!({
            "timeSeries": [],
            "limitReached": false
        }))
        .expect("parse empty");
        let request = usage_analytics_day_sum_by_description_request(7);
        let series = console_team_usage_series_from_response("team-empty", &body, &request);
        assert_eq!(series.oauth_class_usd, 0.0);
        assert_eq!(series.api_class_usd, 0.0);
        assert!(series.rows.is_empty());
    }

    /// Named contract: usage path encodes team id like prepaid/postpaid.
    #[test]
    fn usage_analytics_path_encodes_slash_in_team_id() {
        let p = usage_analytics_path("ab/cd");
        assert!(p.contains("%2F"), "slash must be encoded: {p}");
        assert_eq!(p, "/v1/billing/teams/ab%2Fcd/usage");
    }

    /// Named contract: day window request uses POST-shaped body (no GET invent).
    #[test]
    fn usage_analytics_request_is_day_sum_by_description() {
        let body = usage_analytics_day_sum_by_description_request(7);
        let v = serde_json::to_value(&body).expect("serialize");
        assert_eq!(v["analyticsRequest"]["timeUnit"], "TIME_UNIT_DAY");
        assert_eq!(v["analyticsRequest"]["values"][0]["name"], "usd");
        assert_eq!(
            v["analyticsRequest"]["values"][0]["aggregation"],
            "AGGREGATION_SUM"
        );
        assert_eq!(v["analyticsRequest"]["groupBy"][0], "description");
        assert_eq!(v["analyticsRequest"]["timeRange"]["timezone"], "Etc/GMT");
        let start = v["analyticsRequest"]["timeRange"]["startTime"]
            .as_str()
            .expect("start");
        let end = v["analyticsRequest"]["timeRange"]["endTime"]
            .as_str()
            .expect("end");
        assert!(
            start.contains(" 00:00:00") && end.contains(" 00:00:00"),
            "wall-clock format: {start} .. {end}"
        );
    }

    /// Named contract: success log fields never embed key material.
    #[test]
    fn management_usage_series_success_log_fields_are_honest_and_keyless() {
        let series = ConsoleTeamUsageSeries {
            team_id: "61fab250-b2c1-40cf-b5b8-628e673a2eeb".into(),
            start_time: "2026-07-28 00:00:00".into(),
            end_time: "2026-08-04 00:00:00".into(),
            timezone: "Etc/GMT".into(),
            rows: vec![],
            oauth_class_usd: 19.75,
            api_class_usd: 4.5,
            other_class_usd: 0.1,
            limit_reached: false,
        };
        let fields = management_usage_series_success_log_fields(&series);
        assert_eq!(fields["team_id"], "61fab250-b2c1-40cf-b5b8-628e673a2eeb");
        assert_eq!(fields["oauth_class_usd"], 19.75);
        let s = fields.to_string();
        assert!(
            !s.contains("Bearer") && !s.contains("xai-") && !s.to_lowercase().contains("key"),
            "must never embed key material: {s}"
        );
    }

    /// Named contract (Item 5a): hermetic POST usage → class totals.
    #[tokio::test]
    #[serial]
    async fn fetch_usage_series_hermetic_parses_oauth_vs_api_totals() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_h = hits.clone();
        let app = Router::new().route(
            "/v1/billing/teams/{team_id}/usage",
            axum::routing::post(
                move |Path(team_id): Path<String>, headers: HeaderMap, body: axum::body::Bytes| {
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
                        if team_id != "team-usage-1" {
                            return Err(StatusCode::NOT_FOUND);
                        }
                        // Must be a POST body with analyticsRequest (not empty GET).
                        let v: serde_json::Value =
                            serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
                        if v.get("analyticsRequest").is_none() {
                            return Err(StatusCode::BAD_REQUEST);
                        }
                        Ok(Json(serde_json::json!({
                            "timeSeries": [
                                {
                                    "group": ["Grok Build OAuth grok-4.5-build"],
                                    "groupLabels": ["Grok Build OAuth grok-4.5-build"],
                                    "dataPoints": [
                                        { "timestamp": "2026-08-01T00:00:00Z", "values": [10.0] },
                                        { "timestamp": "2026-08-02T00:00:00Z", "values": [5.0] }
                                    ]
                                },
                                {
                                    "group": ["API grok-4.5"],
                                    "groupLabels": ["API grok-4.5"],
                                    "dataPoints": [
                                        { "timestamp": "2026-08-01T00:00:00Z", "values": [2.0] }
                                    ]
                                }
                            ],
                            "limitReached": false
                        })))
                    }
                },
            ),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let base = format!("http://{addr}");
        let series = fetch_console_team_usage_series_at(
            &base,
            Some("hermetic-mgmt-key"),
            Some("team-usage-1"),
            7,
        )
        .await
        .expect("usage series");
        assert_eq!(series.team_id, "team-usage-1");
        assert!((series.oauth_class_usd - 15.0).abs() < 1e-9);
        assert!((series.api_class_usd - 2.0).abs() < 1e-9);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        clear_console_team_postpaid_cache();
        server.abort();
    }

    /// Named contract: no management key → no invented usage series dollars.
    #[tokio::test]
    #[serial]
    async fn usage_series_gap_when_no_management_key() {
        assert!(
            fetch_console_team_usage_series_at("http://127.0.0.1:1", None, Some("team-1"), 7,)
                .await
                .is_none(),
            "missing management key must not invent usage series"
        );
        assert!(
            fetch_console_team_usage_series_at("http://127.0.0.1:1", Some(""), Some("team-1"), 7,)
                .await
                .is_none(),
            "empty management key must not invent usage series"
        );
        let note = console_team_usage_series_setup_note(true, false).expect("key gap note");
        assert!(
            note.contains("usage") || note.contains("series") || note.contains("POST"),
            "gap note must name usage series: {note}"
        );
        assert!(
            note.contains("Management API key"),
            "gap note must name Management key: {note}"
        );
    }
}
