//! `x.ai/billing` extension handler.
//!
//! Fetches the authenticated user's Grok Build billing configuration (credit limit, usage, on-demand cap, billing period, history) from the backend.
//! The pager and desktop use it to display credits and usage.

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;
use crate::auth::supergrok_identity_rank::AuthManagerRankAlign;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCycle {
    pub year: i32,
    pub month: i32,
}

/// Cent value from the billing API (USD cents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cent {
    /// proto3 JSON omits zero-valued scalars, so a `$0` Cent arrives as `{}`; default to 0 rather than failing the whole parse.
    #[serde(default)]
    pub val: i64,
}

/// A usage period (weekly or monthly) from the newer credits config.
/// `start`/`end` are RFC 3339 timestamps.
/// `period_type` is the proto enum name (e.g. `USAGE_PERIOD_TYPE_WEEKLY`); it is kept so callers can distinguish weekly from monthly cycles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePeriod {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

/// Usage summary for one past billing period.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingPeriodUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<BillingCycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_used: Option<Cent>,
}

/// Wire product id for Grok Build included usage inside `productUsage`.
pub const PRODUCT_GROK_BUILD: &str = "PRODUCT_GROK_BUILD";

/// One `productUsage` row from the credits config (camelCase on the wire).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductUsageEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_percent: Option<f64>,
}

/// Current billing configuration for Grok Build coding credits. Carries the newer credits-config fields (`credit_usage_percent`, `current_period`).
/// It also carries the deprecated `GrokBuildBillingConfig` fields (`monthly_limit`, `used`, `billing_period_*`). Consumers should prefer the new fields and fall back to the deprecated ones.
/// The same struct then works against both the new `GetGrokCreditsConfig` and the legacy `GetGrokBuildBillingConfig` responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingConfig {
    /// Included credit usage as a percentage of the allowance (0.0 to 100.0).
    /// Prefer it over deriving from `monthly_limit`/`used`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_usage_percent: Option<f64>,
    /// Current usage period (weekly or monthly).
    /// Prefer it over `billing_period_start`/`billing_period_end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_period: Option<UsagePeriod>,
    /// Deprecated: included monthly credit budget. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_limit: Option<Cent>,
    /// Deprecated: credits used this period. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_cap: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    /// Remaining prepaid (purchased) credit balance, positive: the "bought credits" the user has topped up.
    /// It comes from the credits config (`GetGrokCreditsConfig.prepaid_balance`) and is absent in the legacy billing shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepaid_balance: Option<Cent>,
    /// Whether this user is on unified usage billing (a shared weekly/monthly pool).
    /// It comes from `GrokCreditsConfig.is_unified_billing_user`, which billing sets from the remote setting `unified_consumer_billing_enabled`.
    /// `None` when absent (legacy `GetGrokBuildBillingConfig` shape or older servers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unified_billing_user: Option<bool>,
    /// Per-product included usage when the server sends `productUsage` (e.g.
    /// Grok Build %). Absent on legacy shapes; empty when omitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub product_usage: Vec<ProductUsageEntry>,
    /// Deprecated: use `current_period.start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_start: Option<String>,
    /// Deprecated: use `current_period.end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<BillingPeriodUsage>,
}

/// Top-level response (primarily from `GET /rest/grok/credits` and the auto-topup-rule fetch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfigResponse {
    pub config: Option<BillingConfig>,
    /// Whether on-demand credit usage is enabled; when `false`, the pager should hide on-demand controls.
    /// It comes from `RemoteSettings`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    /// User-friendly subscription tier name (e.g. "SuperGrok Heavy").
    /// It comes from `RemoteSettings` so the pager can update its cached tier on every billing fetch without an extra request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_tier: Option<String>,
}

/// Auto top-up configuration (from GetAutoTopupRule).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTopupRule {
    /// proto3 JSON omits `false`, so a disabled rule arrives without this field; default to `false` rather than failing the parse.
    /// A failed parse would keep a stale cached rule in the pager.
    #[serde(default)]
    pub enabled: bool,
    pub min_before_hitting_sl: Option<Cent>,
    pub topup_amount: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount_per_month: Option<Cent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutoTopupRuleResponse {
    #[serde(default)]
    pub rule: Option<AutoTopupRule>,
}

/// Optional params for `x.ai/billing`. Empty `{}` is a background poll.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBillingParams {
    /// Explicit `grok-oss limits` / TUI `/limits` open: bust a fresh-by-TTL
    /// disk snapshot so `usagePct` 100 is not applied as included exhaust.
    #[serde(default)]
    pub force_refresh: bool,
    /// Successful login only. Not `force_refresh`. The wire flag is
    /// `login_immediate_included_period_limits_refresh`.
    #[serde(default, alias = "login_immediate_included_period_limits_refresh")]
    pub login_immediate_included_period_limits_refresh: bool,
}

/// Snapshot hub mode for `x.ai/billing`. Explicit collect/open is ForceRefresh.
pub fn limits_snapshot_mode_for_get_billing(
    force_refresh: bool,
) -> crate::auth::LimitsSnapshotMode {
    if force_refresh {
        crate::auth::LimitsSnapshotMode::ForceRefresh
    } else {
        crate::auth::LimitsSnapshotMode::HonorTtl
    }
}

/// Whether the hub leader fetch should bust Management process caches.
///
/// True only for explicit ForceRefresh when a management key is present.
/// Background HonorTtl polls must not clear those caches (they are 60s
/// in-process paint helpers, not the shared hourly snapshot TTL).
pub fn should_clear_management_caches_on_billing_leader_fetch(
    force_refresh: bool,
    has_management_key: bool,
) -> bool {
    force_refresh && has_management_key
}

/// Parse `forceRefresh` from the billing extension params JSON.
pub fn force_refresh_from_billing_params(params_json: &str) -> bool {
    serde_json::from_str::<GetBillingParams>(params_json)
        .map(|p| p.force_refresh)
        .unwrap_or(false)
}

/// Parse the login-only included-period flag. Not `force_refresh`, and not
/// mapped through [`limits_snapshot_mode_for_get_billing`].
pub fn login_immediate_included_period_limits_refresh_from_billing_params(
    params_json: &str,
) -> bool {
    serde_json::from_str::<GetBillingParams>(params_json)
        .map(|p| p.login_immediate_included_period_limits_refresh)
        .unwrap_or(false)
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/billing" => {
            tracing::info!("handling billing config request");
            let params = args.params.get();
            let force_refresh = force_refresh_from_billing_params(params);
            let login_immediate_included_period_limits_refresh =
                login_immediate_included_period_limits_refresh_from_billing_params(params);
            handle_get_billing(
                agent,
                force_refresh,
                login_immediate_included_period_limits_refresh,
            )
            .await
        }
        "x.ai/auto-topup-rule" => {
            tracing::info!("handling auto top-up rule request");
            handle_get_auto_topup_rule(agent).await
        }
        _ => Err(acp::Error::method_not_found()),
    }
}

/// Included usage % + period-end RFC 3339 from a credits / billing config.
///
/// Prefers `credit_usage_percent` + `current_period.end`; falls back to
/// `monthly_limit`/`used` and `billing_period_end`. Returns `None` usage when
/// neither shape is present (honest absence — do not invent 0%).
///
/// Only **included** SuperGrok allowance; not dollar extras or console $.
pub fn included_usage_and_period_end(config: &BillingConfig) -> (Option<f64>, Option<String>) {
    let usage_pct = match config.credit_usage_percent {
        Some(pct) => Some(pct),
        None => {
            let limit = config.monthly_limit.as_ref().map(|c| c.val).unwrap_or(0);
            let used = config.used.as_ref().map(|c| c.val).unwrap_or(0);
            if limit > 0 {
                Some((used as f64 / limit as f64 * 100.0).min(100.0))
            } else {
                None
            }
        }
    };
    let period_end = config
        .current_period
        .as_ref()
        .and_then(|p| p.end.clone())
        .or_else(|| config.billing_period_end.clone());
    (usage_pct, period_end)
}

/// Grok Build included usage percent when `productUsage` names [`PRODUCT_GROK_BUILD`].
pub fn grok_build_usage_percent(config: &BillingConfig) -> Option<f64> {
    config.product_usage.iter().find_map(|entry| {
        if entry.product.as_deref() == Some(PRODUCT_GROK_BUILD) {
            entry.usage_percent
        } else {
            None
        }
    })
}

/// Append one included-poll sample from a credits config. Empty identity is a no-op.
fn record_included_poll_history_from_config(identity_id: &str, config: &BillingConfig) {
    let (usage_pct, _) = included_usage_and_period_end(config);
    let Some(credit_usage_percent) = usage_pct else {
        return;
    };
    crate::auth::included_poll_history::record_included_poll_now(
        identity_id,
        credit_usage_percent,
        grok_build_usage_percent(config),
        config.prepaid_balance.as_ref().map(|cent| cent.val),
    );
}

/// Refresh SuperGrok included billing through the flock snapshot hub.
///
/// Leader (exclusive flock) runs `fetch_all` once (active + siblings).
/// Followers read `$GROK_HOME/limits_snapshot.json` and apply remember maps.
/// Isolated tests set `GROK_DISABLE_SHARED_RATE_LIMIT` so each process fetches.
pub async fn collect_billing_via_snapshot_hub<F, Fut>(
    grok_home: &std::path::Path,
    mode: crate::auth::LimitsSnapshotMode,
    now_unix_ms: u64,
    fetch_all: F,
) -> Result<
    (
        crate::auth::LimitsSnapshotRole,
        crate::auth::LimitsSnapshotDocument,
    ),
    String,
>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = crate::auth::LimitsSnapshotDocument>,
{
    crate::auth::coordinate_limits_snapshot(grok_home, mode, now_unix_ms, fetch_all)
        .await
        .map_err(|e| format!("limits snapshot hub: {e}"))
}

fn poll_outcome_class_from_error(err: &str) -> &'static str {
    use crate::auth::SupergrokBillingPollOutcomeKind;
    match crate::auth::classify_supergrok_billing_poll_error(err) {
        SupergrokBillingPollOutcomeKind::AuthFailed => crate::auth::POLL_OUTCOME_AUTH,
        SupergrokBillingPollOutcomeKind::OtherFailed => {
            let e = err.to_ascii_lowercase();
            if e.contains("timeout")
                || e.contains("connection")
                || e.contains("dns")
                || e.contains("network")
                || e.contains("connect")
            {
                crate::auth::POLL_OUTCOME_NETWORK
            } else {
                crate::auth::POLL_OUTCOME_OTHER
            }
        }
        SupergrokBillingPollOutcomeKind::Ok | SupergrokBillingPollOutcomeKind::Never => {
            crate::auth::POLL_OUTCOME_OTHER
        }
    }
}

/// Build a snapshot identity row from a credits config (no tokens).
pub fn limits_identity_from_credits_config(
    identity_id: &str,
    config: &BillingConfig,
    poll_outcome: &str,
) -> crate::auth::LimitsSnapshotIdentity {
    let (usage_pct, period_end) = included_usage_and_period_end(config);
    crate::auth::LimitsSnapshotIdentity {
        identity_id: identity_id.trim().to_owned(),
        usage_pct,
        period_end,
        period_type: config
            .current_period
            .as_ref()
            .and_then(|p| p.period_type.clone()),
        dollar_credits_cents: config.prepaid_balance.as_ref().map(|c| c.val),
        grok_build_usage_pct: grok_build_usage_percent(config),
        is_unified_billing_user: config.is_unified_billing_user,
        poll_outcome: poll_outcome.to_owned(),
    }
}

/// Reconstruct an ACP billing response from a hub snapshot (follower path).
pub fn billing_response_from_limits_snapshot(
    doc: &crate::auth::LimitsSnapshotDocument,
    identity_id: &str,
) -> BillingConfigResponse {
    let wanted = identity_id.trim();
    let row = doc
        .identities
        .iter()
        .find(|i| i.identity_id == wanted)
        .or_else(|| doc.identities.first());
    let Some(row) = row else {
        return BillingConfigResponse {
            config: None,
            on_demand_enabled: None,
            subscription_tier: None,
        };
    };
    BillingConfigResponse {
        config: Some(BillingConfig {
            credit_usage_percent: row.usage_pct,
            current_period: Some(UsagePeriod {
                period_type: row.period_type.clone(),
                start: None,
                end: row.period_end.clone(),
            }),
            monthly_limit: None,
            used: None,
            on_demand_cap: None,
            on_demand_used: None,
            prepaid_balance: row.dollar_credits_cents.map(|val| Cent { val }),
            is_unified_billing_user: row.is_unified_billing_user,
            product_usage: row
                .grok_build_usage_pct
                .map(|usage_percent| ProductUsageEntry {
                    product: Some(PRODUCT_GROK_BUILD.into()),
                    usage_percent: Some(usage_percent),
                })
                .into_iter()
                .collect(),
            billing_period_start: None,
            billing_period_end: row.period_end.clone(),
            history: vec![],
        }),
        on_demand_enabled: None,
        subscription_tier: None,
    }
}

fn personal_identity_ids_from_auth(grok_home: &std::path::Path) -> Vec<String> {
    let Ok(map) = crate::auth::read_auth_json(&grok_home.join("auth.json")) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for listing in crate::auth::list_supergrok_principal_listings(&map) {
        if listing.role_label != "personal" {
            continue;
        }
        let id = listing.identity_id.trim();
        if id.is_empty() || ids.iter().any(|existing: &String| existing == id) {
            continue;
        }
        ids.push(id.to_owned());
    }
    ids
}

fn row_has_finite_ok_usage(row: &crate::auth::LimitsSnapshotIdentity) -> bool {
    row.poll_outcome == crate::auth::POLL_OUTCOME_OK
        && row.usage_pct.is_some_and(|percent| percent.is_finite())
}

/// Personal rows this poll recorded as `auth` still have no finite used percent.
///
/// Refresh that personal session, then POST `GetGrokCreditsConfig` with
/// `Authorization: Bearer`. A finite `credit_usage_percent` (percent used)
/// replaces that personal row through [`limits_identity_from_credits_config`].
/// A POST with no `Authorization` is not this store. Another personal
/// identity that also lacks a percent does not skip this row. Business-team
/// rows and `management` stay. When refresh returns no token, do not POST.
/// That personal row's poll outcome is `reauth`, not `auth`, and `usage_pct`
/// stays empty. This does not set `use_console` and does not hop to the
/// console API key, team postpaid Billing Credits, or SuperGrok dollar
/// credits. A percent below 100 does not store Extra Usage Credits cents
/// on the row.
///
/// `credits_post_url` empty uses [`GET_GROK_CREDITS_CONFIG_URL`]. Tests pass
/// a local URL so the store is not a live grok.com call.
async fn record_personal_credits_post_on_the_personal_row(
    grok_home: &std::path::Path,
    doc: &mut crate::auth::LimitsSnapshotDocument,
    credits_post_url: Option<&str>,
) {
    let personal_ids = personal_identity_ids_from_auth(grok_home);
    let targets: Vec<String> = doc
        .identities
        .iter()
        .filter(|row| {
            personal_ids.iter().any(|id| id == &row.identity_id)
                && row.poll_outcome == crate::auth::POLL_OUTCOME_AUTH
                && !row_has_finite_ok_usage(row)
        })
        .map(|row| row.identity_id.clone())
        .collect();
    for identity_id in targets {
        let Some((access_token, _user_id)) =
            crate::auth::ensure_fresh_access_token_for_supergrok_billing_poll(
                grok_home,
                &identity_id,
            )
            .await
        else {
            if let Some(existing) = doc
                .identities
                .iter_mut()
                .find(|existing| existing.identity_id == identity_id)
            {
                existing.poll_outcome =
                    crate::auth::limits_snapshot_hub::POLL_OUTCOME_REAUTH.to_owned();
                existing.usage_pct = None;
            }
            continue;
        };
        let token = access_token.trim().to_owned();
        if token.is_empty() {
            continue;
        }
        let Ok(mut config) = post_get_grok_credits_config(credits_post_url, Some(&token)).await
        else {
            continue;
        };
        let Some(percent) = finite_posted_credit_usage_percent(&config) else {
            continue;
        };
        if percent < 100.0 {
            config.prepaid_balance = None;
        }
        let row = limits_identity_from_credits_config(
            &identity_id,
            &config,
            crate::auth::POLL_OUTCOME_OK,
        );
        if let Some(existing) = doc
            .identities
            .iter_mut()
            .find(|existing| existing.identity_id == identity_id)
        {
            *existing = row;
        } else {
            doc.identities.push(row);
        }
    }
}

/// Leader-only: poll stored SuperGrok JWTs plus optional active token into a snapshot.
///
/// Call only from a hub fetch callback. Does not remember (apply does that).
/// A finite `GetGrokCreditsConfig` percent is that identity's `usage_pct`.
/// A personal identity this poll recorded as `auth` can take a finite
/// percent from a POST that sends that personal session. That POST does
/// not replace the business-team row. A POST with no `Authorization` is
/// not that store.
pub async fn fetch_supergrok_credits_snapshot_document(
    grok_home: &std::path::Path,
    proxy_base: &str,
    active: Option<(&str, &str, &str)>,
    live_active: Option<&std::sync::Mutex<Option<BillingConfigResponse>>>,
    live_active_err: Option<&std::sync::Mutex<Option<String>>>,
) -> crate::auth::LimitsSnapshotDocument {
    let now = crate::auth::limits_snapshot_hub::now_unix_ms();
    let mut doc = crate::auth::LimitsSnapshotDocument::empty(now);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    if let Some((token, user_id, identity_id)) = active {
        match fetch_credits_config_with_posted_included_percent(proxy_base, token, user_id).await {
            Ok(resp) => {
                if let Some(config) = resp.config.as_ref() {
                    doc.identities.push(limits_identity_from_credits_config(
                        identity_id,
                        config,
                        crate::auth::POLL_OUTCOME_OK,
                    ));
                    seen.insert(identity_id.trim().to_owned());
                }
                if let Some(slot) = live_active {
                    *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(resp);
                }
            }
            Err(e) => {
                doc.identities.push(crate::auth::LimitsSnapshotIdentity {
                    identity_id: identity_id.trim().to_owned(),
                    usage_pct: None,
                    period_end: None,
                    period_type: None,
                    dollar_credits_cents: None,
                    grok_build_usage_pct: None,
                    is_unified_billing_user: None,
                    poll_outcome: poll_outcome_class_from_error(&e).to_owned(),
                });
                seen.insert(identity_id.trim().to_owned());
                if let Some(slot) = live_active_err {
                    *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(e);
                }
            }
        }
    }

    let targets = crate::auth::load_supergrok_billing_poll_targets(grok_home);
    for target in targets {
        if seen.contains(&target.identity_id) {
            continue;
        }
        let (access_token, user_id) =
            match crate::auth::ensure_fresh_access_token_for_supergrok_billing_poll(
                grok_home,
                &target.identity_id,
            )
            .await
            {
                Some((tok, uid)) => (tok, uid),
                None => continue,
            };
        match fetch_credits_config_with_posted_included_percent(proxy_base, &access_token, &user_id)
            .await
        {
            Ok(resp) => {
                if let Some(config) = resp.config.as_ref() {
                    doc.identities.push(limits_identity_from_credits_config(
                        &target.identity_id,
                        config,
                        crate::auth::POLL_OUTCOME_OK,
                    ));
                }
            }
            Err(e) => {
                doc.identities.push(crate::auth::LimitsSnapshotIdentity {
                    identity_id: target.identity_id.clone(),
                    usage_pct: None,
                    period_end: None,
                    period_type: None,
                    dollar_credits_cents: None,
                    grok_build_usage_pct: None,
                    is_unified_billing_user: None,
                    poll_outcome: poll_outcome_class_from_error(&e).to_owned(),
                });
            }
        }
    }
    doc.management = crate::auth::fetch_management_into_snapshot().await;
    record_personal_credits_post_on_the_personal_row(grok_home, &mut doc, None).await;
    doc
}

/// Fetch `GetGrokCreditsConfig` for one SuperGrok session token (included-safe).
///
/// Same CLI proxy path as the active `x.ai/billing` handler:
/// `GET {proxy}/billing?format=credits`. Does not burn SuperGrok dollar credits
/// (not an inference call). Used for non-active dual-principal polls.
///
/// Multi-principal / multi-process collect must go through
/// [`collect_billing_via_snapshot_hub`] so only the flock leader calls this
/// URL. Isolated tests set `GROK_DISABLE_SHARED_RATE_LIMIT`.
///
/// Honors multi-process shared cooldowns ([`crate::shared_http_rate_limit`]) so
/// concurrent `limits` / TUI polls do not stampede the proxy after a 429.
pub async fn fetch_credits_config_with_session(
    proxy_base: &str,
    access_token: &str,
    user_id: &str,
) -> Result<BillingConfigResponse, String> {
    let token = access_token.trim();
    if token.is_empty() {
        return Err("empty SuperGrok session token".into());
    }
    let base = proxy_base.trim_end_matches('/');
    let credits_url = format!("{base}/billing?format=credits");
    let rate_key = crate::shared_http_rate_limit::billing_provider_key(base, token);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let credits_resp = crate::http::shared_client()
        .get(&credits_url)
        .header("Authorization", format!("Bearer {token}"))
        .header(
            "X-XAI-Token-Auth",
            crate::auth::GrokComConfig::default().token_header,
        )
        .header("x-userid", user_id)
        .header("x-grok-client-version", xai_grok_version::VERSION)
        .header(
            crate::http::CLIENT_MODE_HEADER,
            crate::http::process_client_mode(),
        )
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch billing data: {e}"))?;

    if !credits_resp.status().is_success() {
        let status = credits_resp.status().as_u16();
        let headers = credits_resp.headers().clone();
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            status,
            &headers,
            "SuperGrok billing credits rate limit",
        );
        let body = credits_resp.text().await.unwrap_or_default();
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!("Billing service error: {detail}"));
    }

    credits_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse billing data: {e}"))
}

/// Default POST target for `GetGrokCreditsConfig`.
pub const GET_GROK_CREDITS_CONFIG_URL: &str =
    "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";

/// POST the five-byte grpc-web frame and decode `GetGrokCreditsConfig`.
///
/// `url` empty uses [`GET_GROK_CREDITS_CONFIG_URL`]. A bearer, when
/// present, is `Authorization: Bearer`. This does not set `Cookie`.
/// The decoded percent is percent used. Callers subtract that from 100.
pub async fn post_get_grok_credits_config(
    url: Option<&str>,
    bearer: Option<&str>,
) -> Result<BillingConfig, String> {
    let target = url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(GET_GROK_CREDITS_CONFIG_URL);
    let mut request = crate::http::shared_client()
        .post(target)
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .timeout(std::time::Duration::from_secs(15))
        .body(vec![0u8; 5]);
    if let Some(token) = bearer.map(str::trim).filter(|token| !token.is_empty()) {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to fetch billing data: {e}"))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!("Billing service error: HTTP {status}"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to parse billing data: {e}"))?;
    decode_get_grok_credits_config_frame(&bytes)
}

fn finite_posted_credit_usage_percent(config: &BillingConfig) -> Option<f64> {
    config
        .credit_usage_percent
        .filter(|percent| percent.is_finite())
}

/// Put a successful `GetGrokCreditsConfig` onto the legacy GET response.
///
/// A finite posted percent becomes `credit_usage_percent`, including when the
/// GET omitted that field. Prepaid cents on the POST stay. A percent below
/// 100 is not removed.
fn apply_posted_credits_to_get_response(
    response: &mut BillingConfigResponse,
    posted: BillingConfig,
) {
    let posted_percent = finite_posted_credit_usage_percent(&posted);
    if let Some(config) = response.config.as_mut() {
        if let Some(percent) = posted_percent {
            config.credit_usage_percent = Some(percent);
        }
        if let Some(prepaid) = posted.prepaid_balance {
            config.prepaid_balance = Some(prepaid);
        }
        return;
    }
    if posted_percent.is_some() || posted.prepaid_balance.is_some() {
        response.config = Some(posted);
    }
}

/// Credits for one SuperGrok identity the snapshot or non-active poll targets.
///
/// Posts `GetGrokCreditsConfig` with that bearer and no `Cookie`. A finite
/// percent on `Ok` is this identity's used percent. A failed POST keeps the
/// legacy GET, so a GET that already carried a percent still stores it.
async fn fetch_credits_config_with_posted_included_percent(
    proxy_base: &str,
    access_token: &str,
    user_id: &str,
) -> Result<BillingConfigResponse, String> {
    let posted = post_get_grok_credits_config(None, Some(access_token)).await;
    let Ok(posted) = posted else {
        return fetch_credits_config_with_session(proxy_base, access_token, user_id).await;
    };
    match fetch_credits_config_with_session(proxy_base, access_token, user_id).await {
        Ok(mut response) => {
            apply_posted_credits_to_get_response(&mut response, posted);
            Ok(response)
        }
        Err(get_error) => {
            if finite_posted_credit_usage_percent(&posted).is_some()
                || posted.prepaid_balance.is_some()
            {
                Ok(BillingConfigResponse {
                    config: Some(posted),
                    on_demand_enabled: None,
                    subscription_tier: None,
                })
            } else {
                Err(get_error)
            }
        }
    }
}

fn decode_get_grok_credits_config_frame(bytes: &[u8]) -> Result<BillingConfig, String> {
    let mut offset = 0usize;
    let mut payload = Vec::new();
    let mut saw_ok = false;
    if bytes.is_empty() {
        return Err("empty GetGrokCreditsConfig frame".into());
    }
    while offset + 5 <= bytes.len() {
        let flag = bytes
            .get(offset)
            .copied()
            .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?;
        let len = u32::from_be_bytes([
            bytes
                .get(offset + 1)
                .copied()
                .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?,
            bytes
                .get(offset + 2)
                .copied()
                .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?,
            bytes
                .get(offset + 3)
                .copied()
                .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?,
            bytes
                .get(offset + 4)
                .copied()
                .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?,
        ]) as usize;
        offset += 5;
        if offset + len > bytes.len() {
            return Err("truncated GetGrokCreditsConfig frame".into());
        }
        let body = bytes
            .get(offset..offset + len)
            .ok_or_else(|| String::from("truncated GetGrokCreditsConfig frame"))?;
        offset += len;
        if flag & 0x80 != 0 {
            let text = String::from_utf8_lossy(body);
            if !grpc_status_is_zero(&text) {
                return Err(format!("GetGrokCreditsConfig grpc-status is not 0: {text}"));
            }
            saw_ok = true;
        } else if flag == 0 {
            payload.extend_from_slice(body);
        } else {
            return Err(format!("unexpected grpc-web flag {flag}"));
        }
    }
    if offset != bytes.len() {
        return Err("truncated GetGrokCreditsConfig frame".into());
    }
    if !saw_ok {
        return Err("GetGrokCreditsConfig response missing grpc-status 0".into());
    }
    let config_bytes = config_message_bytes(&payload)?;
    billing_config_from_credits_proto(config_bytes)
}

fn grpc_status_is_zero(trailer: &str) -> bool {
    trailer.split(['\n', '\r']).any(|line| {
        let line = line.trim();
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("grpc-status") && value.trim() == "0"
    })
}

struct ProtoCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ProtoCursor<'a> {
    fn remaining(&self) -> bool {
        self.pos < self.data.len()
    }

    fn read_varint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        let mut shift = 0u32;
        while self.pos < self.data.len() {
            let byte = self
                .data
                .get(self.pos)
                .copied()
                .ok_or_else(|| String::from("truncated varint"))?;
            self.pos += 1;
            let Some(part) = u64::from(byte & 0x7f).checked_shl(shift) else {
                return Err("varint too long".into());
            };
            value |= part;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift > 63 {
                return Err("varint too long".into());
            }
        }
        Err("truncated varint".into())
    }

    fn read_fixed32(&mut self) -> Result<[u8; 4], String> {
        if self.pos + 4 > self.data.len() {
            return Err("truncated fixed32".into());
        }
        let mut raw = [0u8; 4];
        raw.copy_from_slice(
            self.data
                .get(self.pos..self.pos + 4)
                .ok_or_else(|| String::from("truncated fixed32"))?,
        );
        self.pos += 4;
        Ok(raw)
    }

    fn read_len_bytes(&mut self) -> Result<&'a [u8], String> {
        let len = self.read_varint()? as usize;
        if self.pos + len > self.data.len() {
            return Err("truncated length-delimited field".into());
        }
        let slice = self
            .data
            .get(self.pos..self.pos + len)
            .ok_or_else(|| String::from("truncated length-delimited field"))?;
        self.pos += len;
        Ok(slice)
    }

    fn skip(&mut self, wire: u8) -> Result<(), String> {
        match wire {
            0 => {
                self.read_varint()?;
                Ok(())
            }
            1 => {
                if self.pos + 8 > self.data.len() {
                    return Err("truncated fixed64".into());
                }
                self.pos += 8;
                Ok(())
            }
            2 => {
                self.read_len_bytes()?;
                Ok(())
            }
            5 => {
                self.read_fixed32()?;
                Ok(())
            }
            _ => Err(format!("unsupported wire type {wire}")),
        }
    }
}

fn config_message_bytes(payload: &[u8]) -> Result<&[u8], String> {
    let mut cursor = ProtoCursor {
        data: payload,
        pos: 0,
    };
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        if field == 1 && wire == 2 {
            return cursor.read_len_bytes();
        }
        cursor.skip(wire)?;
    }
    Err("GetGrokCreditsConfig payload missing config message".into())
}

fn billing_config_from_credits_proto(bytes: &[u8]) -> Result<BillingConfig, String> {
    let mut cursor = ProtoCursor {
        data: bytes,
        pos: 0,
    };
    let mut credit_usage_percent = None;
    let mut product_usage = Vec::new();
    let mut current_period = None;
    let mut prepaid_balance = None;
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        match (field, wire) {
            (1, 5) => {
                let raw = cursor.read_fixed32()?;
                credit_usage_percent = Some(f64::from(f32::from_le_bytes(raw)));
            }
            (7, 2) => {
                let row = cursor.read_len_bytes()?;
                if let Some(entry) = product_usage_from_proto(row)? {
                    product_usage.push(entry);
                }
            }
            (8, 2) => {
                let row = cursor.read_len_bytes()?;
                current_period = Some(usage_period_from_proto(row)?);
            }
            (12, 2) => {
                let row = cursor.read_len_bytes()?;
                prepaid_balance = prepaid_from_proto(row)?;
            }
            _ => cursor.skip(wire)?,
        }
    }
    Ok(BillingConfig {
        credit_usage_percent,
        current_period,
        monthly_limit: None,
        used: None,
        on_demand_cap: None,
        on_demand_used: None,
        prepaid_balance,
        is_unified_billing_user: None,
        product_usage,
        billing_period_start: None,
        billing_period_end: None,
        history: vec![],
    })
}

fn product_usage_from_proto(bytes: &[u8]) -> Result<Option<ProductUsageEntry>, String> {
    let mut cursor = ProtoCursor {
        data: bytes,
        pos: 0,
    };
    let mut id = None;
    let mut percent = None;
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        match (field, wire) {
            (1, 0) => id = Some(cursor.read_varint()?),
            (2, 5) => {
                let raw = cursor.read_fixed32()?;
                percent = Some(f64::from(f32::from_le_bytes(raw)));
            }
            _ => cursor.skip(wire)?,
        }
    }
    let Some(percent) = percent else {
        return Ok(None);
    };
    let name = match id {
        Some(4) => "Chat".to_owned(),
        Some(6) => "Voice".to_owned(),
        Some(other) => other.to_string(),
        None => return Ok(None),
    };
    Ok(Some(ProductUsageEntry {
        product: Some(name),
        usage_percent: Some(percent),
    }))
}

fn usage_period_from_proto(bytes: &[u8]) -> Result<UsagePeriod, String> {
    let mut cursor = ProtoCursor {
        data: bytes,
        pos: 0,
    };
    let mut kind = None;
    let mut start = None;
    let mut end = None;
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        match (field, wire) {
            (1, 0) => kind = Some(cursor.read_varint()?),
            (2, 2) => {
                let row = cursor.read_len_bytes()?;
                start = Some(timestamp_rfc3339(row)?);
            }
            (3, 2) => {
                let row = cursor.read_len_bytes()?;
                end = Some(timestamp_rfc3339(row)?);
            }
            _ => cursor.skip(wire)?,
        }
    }
    let period_type = kind.map(|kind| match kind {
        1 => "USAGE_PERIOD_TYPE_MONTHLY".to_owned(),
        2 => "USAGE_PERIOD_TYPE_WEEKLY".to_owned(),
        other => format!("USAGE_PERIOD_TYPE_{other}"),
    });
    Ok(UsagePeriod {
        period_type,
        start,
        end,
    })
}

fn timestamp_rfc3339(bytes: &[u8]) -> Result<String, String> {
    let mut cursor = ProtoCursor {
        data: bytes,
        pos: 0,
    };
    let mut seconds = 0i64;
    let mut nanos = 0u32;
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        match (field, wire) {
            (1, 0) => seconds = cursor.read_varint()? as i64,
            (2, 0) => nanos = cursor.read_varint()? as u32,
            _ => cursor.skip(wire)?,
        }
    }
    Ok(rfc3339_from_unix(seconds, nanos))
}

fn prepaid_from_proto(bytes: &[u8]) -> Result<Option<Cent>, String> {
    let mut cursor = ProtoCursor {
        data: bytes,
        pos: 0,
    };
    let mut cents = None;
    while cursor.remaining() {
        let tag = cursor.read_varint()?;
        let field = tag >> 3;
        let wire = (tag & 7) as u8;
        match (field, wire) {
            (1, 0) => cents = Some(cursor.read_varint()? as i64),
            _ => cursor.skip(wire)?,
        }
    }
    Ok(cents.map(|val| Cent { val }))
}

fn rfc3339_from_unix(seconds: i64, nanos: u32) -> String {
    let days = seconds.div_euclid(86_400);
    let tod = seconds.rem_euclid(86_400) as u32;
    let hour = tod / 3_600;
    let min = (tod % 3_600) / 60;
    let sec = tod % 60;
    let (year, month, day) = civil_from_unix_days(days);
    if nanos == 0 {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
    } else {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{nanos:09}Z")
    }
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

/// Poll non-active SuperGrok principals and remember their included fields.
///
/// Best-effort: a failed sibling poll does not fail the active billing path.
/// No-op when fewer than two SuperGrok principals are stored.
/// A finite `GetGrokCreditsConfig` percent is remembered on that identity.
///
/// Before each sibling credits request, OIDC-refreshes a multi-slot JWT that
/// is past the early-invalidation buffer when refresh credentials are present
/// ([`crate::auth::ensure_fresh_access_token_for_supergrok_billing_poll`]).
pub async fn poll_and_remember_non_active_supergrok_included_billing(
    grok_home: &std::path::Path,
    proxy_base: &str,
) {
    let targets = crate::auth::load_non_active_supergrok_billing_poll_targets(grok_home);
    if targets.is_empty() {
        return;
    }
    for target in targets {
        // Multi-slot OIDC refresh before sibling poll: AuthManager only keeps
        // the active base fresh; siblings can hold a stale multi-slot JWT.
        // A rejected grant returns no token. Do not send the stored one.
        let (access_token, user_id) =
            match crate::auth::ensure_fresh_access_token_for_supergrok_billing_poll(
                grok_home,
                &target.identity_id,
            )
            .await
            {
                Some((tok, uid)) => (tok, uid),
                None => continue,
            };
        match fetch_credits_config_with_posted_included_percent(proxy_base, &access_token, &user_id)
            .await
        {
            Ok(resp) => {
                let Some(config) = resp.config.as_ref() else {
                    tracing::debug!(
                        identity_id = %target.identity_id,
                        "sibling SuperGrok billing: no config in response"
                    );
                    continue;
                };
                let (usage_pct, period_end) = included_usage_and_period_end(config);
                let period_type = config
                    .current_period
                    .as_ref()
                    .and_then(|p| p.period_type.as_deref());
                // Prepaid (Extra Usage Credits) is independent of included % —
                // remember when present even if usage % is absent.
                if let Some(prepaid) = config.prepaid_balance.as_ref() {
                    crate::auth::remember_supergrok_dollar_credits(
                        &target.identity_id,
                        prepaid.val,
                    );
                }
                let Some(pct) = usage_pct else {
                    tracing::debug!(
                        identity_id = %target.identity_id,
                        "sibling SuperGrok billing: no included usage in response"
                    );
                    continue;
                };
                crate::auth::remember_supergrok_included_billing(
                    &target.identity_id,
                    pct,
                    period_end.as_deref(),
                    period_type,
                );
                crate::auth::remember_supergrok_billing_poll_ok(&target.identity_id);
                if let Some(build_pct) = grok_build_usage_percent(config) {
                    crate::auth::remember_supergrok_build_usage(&target.identity_id, build_pct);
                }
                record_included_poll_history_from_config(&target.identity_id, config);
                tracing::debug!(
                    identity_id = %target.identity_id,
                    usage_pct = pct,
                    prepaid = config.prepaid_balance.as_ref().map(|c| c.val),
                    "remembered non-active SuperGrok included + dollar extras billing"
                );
            }
            Err(e) => {
                let err_text = e.to_string();
                crate::auth::remember_supergrok_billing_poll_failed(&target.identity_id, &err_text);
                // Process outcome map is the operator surface for TUI /limits
                // and doctor (not debug-only). Keep debug for active path.
                tracing::debug!(
                    identity_id = %target.identity_id,
                    error = %err_text,
                    "sibling SuperGrok billing poll failed (active path unchanged)"
                );
            }
        }
    }
}

/// Structured context for unified-log entries from a successful billing fetch.
/// Keeps history to a count + the most recent period so `~/.grok/logs/unified.jsonl` stays useful without dumping unbounded period arrays.
fn billing_unified_log_ctx(billing: &BillingConfigResponse) -> serde_json::Value {
    let history_len = billing
        .config
        .as_ref()
        .map(|c| c.history.len())
        .unwrap_or(0);
    let latest_history = billing
        .config
        .as_ref()
        .and_then(|c| c.history.last())
        .and_then(|p| serde_json::to_value(p).ok());

    let mut config_value = billing
        .config
        .as_ref()
        .and_then(|c| serde_json::to_value(c).ok())
        .unwrap_or(serde_json::Value::Null);
    if let Some(obj) = config_value.as_object_mut() {
        // Drop the full history array; log its length and the latest entry instead
        obj.remove("history");
        obj.insert("historyLen".into(), serde_json::json!(history_len));
        if let Some(latest) = latest_history {
            obj.insert("latestHistory".into(), latest);
        }
    }

    serde_json::json!({
        "config": config_value,
        "onDemandEnabled": billing.on_demand_enabled,
        "subscriptionTier": billing.subscription_tier,
    })
}

/// Unified-log context for a successful `billing: fetched credits config` line.
///
/// Always includes the credits snapshot. When known, also records which SuperGrok
/// principal was polled (`identity_id`) and its role (`personal` / `business`)
/// so dogfood can end "wrong JWT" debates. Hoists Grok Build product usage %
/// when `productUsage` is on the wire (top-level `creditUsagePercent` alone is
/// not enough to prove Build-specific debit).
///
/// **Top-level key naming:** hoist fields use **snake_case** (`identity_id`,
/// `role`, `grok_build_usage_percent`) so operators can `rg identity_id`
/// in `unified.jsonl`. Nested `config` / response flags keep wire **camelCase**
/// (`creditUsagePercent`, `onDemandEnabled`, `subscriptionTier`) from serde of
/// [`BillingConfig`]. The mix is intentional — do not churn nested keys to snake.
pub fn billing_fetched_credits_log_ctx(
    billing: &BillingConfigResponse,
    identity_id: Option<&str>,
    role: Option<&str>,
) -> serde_json::Value {
    let mut ctx = billing_unified_log_ctx(billing);
    let Some(obj) = ctx.as_object_mut() else {
        return ctx;
    };
    if let Some(id) = identity_id.map(str::trim).filter(|s| !s.is_empty()) {
        obj.insert("identity_id".into(), serde_json::json!(id));
    }
    if let Some(r) = role.map(str::trim).filter(|s| !s.is_empty()) {
        obj.insert("role".into(), serde_json::json!(r));
    }
    if let Some(pct) = billing.config.as_ref().and_then(grok_build_usage_percent) {
        obj.insert("grok_build_usage_percent".into(), serde_json::json!(pct));
    }
    ctx
}

/// Identity id + role for billing success logs from the **credential that just
/// polled**, not a second disk-only `auth.json` scan.
///
/// Uses the same rules as [`crate::auth::supergrok_identity_id_from_auth`]
/// (team_id → user_id → store_scope fallback) and
/// [`crate::auth::role_from_session_fields`]. Never invents `productUsage`.
/// Callers may still cross-check disk listings; prefer this when SuperGrok
/// auth just hit the network so success lines keep `identity_id` even if
/// `active_supergrok_identity_id` cannot resolve.
pub fn billing_log_identity_from_auth(auth: &crate::auth::GrokAuth) -> (String, &'static str) {
    let scope = crate::auth::GrokComConfig::default().auth_scope();
    let identity_id = crate::auth::supergrok_identity_id_from_auth(auth, &scope);
    let role = crate::auth::role_label(crate::auth::role_from_session_fields(
        auth.principal_type.as_deref(),
        auth.team_id.as_deref(),
    ));
    (identity_id, role)
}

/// `config.toml` `[auth]` or `[grok_com_config]` `auto_use_included_limits`.
/// Missing file, unreadable file, or a missing key stays on the default (true).
fn auto_use_included_limits_from_disk() -> bool {
    let path = crate::util::grok_home::grok_home().join("config.toml");
    let Ok(text) = std::fs::read_to_string(path) else {
        return true;
    };
    let Ok(value) = text.parse::<toml::Value>() else {
        return true;
    };
    for table_name in ["auth", "grok_com_config"] {
        if let Some(flag) = value
            .get(table_name)
            .and_then(|table| table.get("auto_use_included_limits"))
            .and_then(|item| item.as_bool())
        {
            return flag;
        }
    }
    true
}

async fn handle_get_billing(
    agent: &MvpAgent,
    force_refresh: bool,
    login_immediate_included_period_limits_refresh: bool,
) -> ExtResult {
    let auth = super::auth_gate::require_xai_auth(
        &agent.auth_manager,
        "Authentication required to fetch billing data",
        "Billing data requires auth with grok.com. Run `grok login` to authenticate.",
    )?;

    let proxy_base = agent.cli_chat_proxy_base_url();
    let base = proxy_base.trim_end_matches('/');
    let grok_home = crate::util::grok_home::grok_home();
    let (active_identity_id, _) = billing_log_identity_from_auth(&auth);
    let now = crate::auth::limits_snapshot_hub::now_unix_ms();
    let live_active: std::sync::Mutex<Option<BillingConfigResponse>> = std::sync::Mutex::new(None);
    let live_active_err: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let token = auth.key.clone();
    let user_id = auth.user_id.clone();
    let identity_for_fetch = active_identity_id.clone();
    let home_for_fetch = grok_home.clone();
    let base_for_fetch = base.to_owned();

    // One flock leader fetches SuperGrok credits (active + siblings) and
    // Management meters. Followers apply the snapshot into remember maps.
    let mode = if login_immediate_included_period_limits_refresh {
        crate::auth::LimitsSnapshotMode::LoginImmediateIncludedPeriodLimitsRefresh
    } else {
        limits_snapshot_mode_for_get_billing(force_refresh)
    };
    let (role, doc) = match collect_billing_via_snapshot_hub(&grok_home, mode, now, || {
        let live_active = &live_active;
        let live_active_err = &live_active_err;
        let token = token.clone();
        let user_id = user_id.clone();
        let identity_for_fetch = identity_for_fetch.clone();
        let home_for_fetch = home_for_fetch.clone();
        let base_for_fetch = base_for_fetch.clone();
        async move {
            if !login_immediate_included_period_limits_refresh
                && should_clear_management_caches_on_billing_leader_fetch(
                    force_refresh,
                    crate::auth::resolve_management_api_key_default().is_some(),
                )
            {
                crate::auth::clear_console_team_billing_meter_caches();
            }
            fetch_supergrok_credits_snapshot_document(
                &home_for_fetch,
                &base_for_fetch,
                Some((
                    token.as_str(),
                    user_id.as_str(),
                    identity_for_fetch.as_str(),
                )),
                Some(live_active),
                Some(live_active_err),
            )
            .await
        }
    })
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!(error = %e, "billing: limits snapshot hub failed");
            xai_grok_telemetry::unified_log::warn(
                "billing: limits snapshot hub failed",
                None,
                Some(serde_json::json!({ "error": e })),
            );
            return Err(acp::Error::internal_error().data(e));
        }
    };

    let active_err = live_active_err
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if matches!(
        role,
        crate::auth::LimitsSnapshotRole::LeaderFetched
            | crate::auth::LimitsSnapshotRole::UncoordinatedFetch
    ) && let Some(e) = active_err
    {
        tracing::error!(error = %e, "billing: upstream request failed");
        xai_grok_telemetry::unified_log::warn(
            "billing: upstream request failed",
            None,
            Some(serde_json::json!({ "error": e })),
        );
        return Err(acp::Error::internal_error().data(e));
    }

    let mut billing = live_active
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_else(|| billing_response_from_limits_snapshot(&doc, &active_identity_id));

    // Enrich with fields from remote settings.
    let rs = agent.cfg.borrow().remote_settings.clone();
    billing.on_demand_enabled = rs.as_ref().and_then(|rs| rs.on_demand_enabled);
    billing.subscription_tier = rs.as_ref().and_then(|rs| {
        rs.subscription_tier_display
            .clone()
            .or_else(|| rs.subscription_tier.clone())
    });

    // Hub apply already filled remember maps. Re-apply exhaust + align so
    // sibling included SuperGrok period remaining can hop the SessionToken.
    if let Some(ref config) = billing.config {
        let (usage_pct, period_end) = included_usage_and_period_end(config);
        if let Some(pct) = usage_pct {
            let _ = crate::auth::apply_billing_usage_to_session_exhaust_with_period(
                pct,
                &grok_home,
                period_end.as_deref(),
            );
        }
    }
    if auto_use_included_limits_from_disk() {
        let _ = agent.auth_manager.align_to_ranked_free_period_primary();
    }

    // Leader or follower: log the credits snapshot so support can correlate
    // limit UX with real balances. HTTP only runs when this process is leader.
    // Prefer identity from the GrokAuth that just polled (not disk-only scan)
    // so success lines keep identity_id even when auth.json listing lags.
    // Include productUsage / Build % when present so flat top-level % cannot
    // hide principal or product mismatch.
    let (identity_id, role) = billing_log_identity_from_auth(&auth);
    xai_grok_telemetry::unified_log::info(
        "billing: fetched credits config",
        None,
        Some(billing_fetched_credits_log_ctx(
            &billing,
            Some(identity_id.as_str()),
            Some(role),
        )),
    );

    to_raw_response(&billing)
}

async fn handle_get_auto_topup_rule(agent: &MvpAgent) -> ExtResult {
    let auth = super::auth_gate::require_xai_auth(
        &agent.auth_manager,
        "Authentication required to fetch auto top-up rule",
        "Auto top-up data requires auth with grok.com. Run `grok login` to authenticate.",
    )?;

    let proxy_base = agent.cli_chat_proxy_base_url();
    let base = proxy_base.trim_end_matches('/');

    // Fetch the auto top-up rule via the CLI proxy, which forwards to the backend `GetAutoTopupRule`
    let url = format!("{}/auto-topup-rule", base);
    let rate_key = crate::shared_http_rate_limit::billing_provider_key(base, &auth.key);
    crate::shared_http_rate_limit::wait_before_http(&rate_key).await;
    let response = crate::http::shared_client()
        .get(&url)
        .header("Authorization", format!("Bearer {}", auth.key))
        .header(
            "X-XAI-Token-Auth",
            xai_grok_login::GrokComConfig::default().token_header,
        )
        .header("x-userid", &auth.user_id)
        .header("x-grok-client-version", xai_grok_version::VERSION)
        .header(
            crate::http::CLIENT_MODE_HEADER,
            crate::http::process_client_mode(),
        )
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "auto-topup: upstream request failed");
            acp::Error::internal_error().data(format!("Failed to fetch auto top-up rule: {e}"))
        })?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        crate::shared_http_rate_limit::observe_http_rate_limit(
            &rate_key,
            status,
            &headers,
            "SuperGrok auto-topup rate limit",
        );
        let body = response.text().await.unwrap_or_default();
        tracing::warn!(status, url = %url, "auto-topup: upstream error");

        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| format!("HTTP {status}"));

        return Err(
            acp::Error::internal_error().data(format!("Auto top-up service error: {detail}"))
        );
    }

    // Return the upstream response body verbatim (as a JSON value) so `/usage` can print the exact data from this request unformatted
    let body_text = response.text().await.unwrap_or_default();
    let value: serde_json::Value =
        serde_json::from_str(&body_text).unwrap_or(serde_json::json!({"raw": body_text}));
    to_raw_response(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::GrokAuth;

    fn empty_config() -> BillingConfig {
        BillingConfig {
            credit_usage_percent: None,
            current_period: None,
            monthly_limit: None,
            used: None,
            on_demand_cap: None,
            on_demand_used: None,
            prepaid_balance: None,
            is_unified_billing_user: None,
            product_usage: vec![],
            billing_period_start: None,
            billing_period_end: None,
            history: vec![],
        }
    }

    fn decode_standard_base64(input: &str) -> Vec<u8> {
        fn val(byte: u8) -> u8 {
            match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => 0,
            }
        }
        let bytes = input.as_bytes();
        let mut out = Vec::new();
        let mut index = 0;
        while index + 4 <= bytes.len() {
            let packed = (u32::from(val(bytes[index])) << 18)
                | (u32::from(val(bytes[index + 1])) << 12)
                | (u32::from(val(bytes[index + 2])) << 6)
                | u32::from(val(bytes[index + 3]));
            out.push((packed >> 16) as u8);
            if bytes[index + 2] != b'=' {
                out.push((packed >> 8) as u8);
            }
            if bytes[index + 3] != b'=' {
                out.push(packed as u8);
            }
            index += 4;
        }
        out
    }

    fn grpc_web_frame(payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        let trailer = b"grpc-status:0\r\n";
        out.push(0x80);
        out.extend_from_slice(&(trailer.len() as u32).to_be_bytes());
        out.extend_from_slice(trailer);
        out
    }

    fn frame_with_used_percent(percent: f32) -> Vec<u8> {
        let mut config = Vec::new();
        config.push(0x0d);
        config.extend_from_slice(&percent.to_le_bytes());
        let mut outer = Vec::new();
        outer.push(0x0a);
        outer.push(config.len() as u8);
        outer.extend_from_slice(&config);
        grpc_web_frame(&outer)
    }

    #[test]
    fn get_grok_credits_config_fixture_decodes_percent_products_period_and_cents() {
        const FIXTURE: &str = "AAAAAGoKaA0AACBBEgAaACIMCIyckdYGEMD//dcBKgwIjJG21gYQwP/91wE6BwgEFQAA4EA6BwgGFQAAQEA6AggFOgIICEIeCAISDAiMnJHWBhDA//3XARoMCIyRttYGEMD//dcBWAFiAwihPWgBgAAAAA9ncnBjLXN0YXR1czowDQo=";
        let config = decode_get_grok_credits_config_frame(&decode_standard_base64(FIXTURE))
            .expect("fixture frame");
        assert_eq!(config.credit_usage_percent, Some(10.0));
        let chat = config
            .product_usage
            .iter()
            .find(|entry| entry.product.as_deref() == Some("Chat"))
            .expect("Chat");
        let voice = config
            .product_usage
            .iter()
            .find(|entry| entry.product.as_deref() == Some("Voice"))
            .expect("Voice");
        assert_eq!(chat.usage_percent, Some(7.0));
        assert_eq!(voice.usage_percent, Some(3.0));
        assert_eq!(
            config.prepaid_balance.as_ref().map(|cent| cent.val),
            Some(7841)
        );
        let end = config
            .current_period
            .as_ref()
            .and_then(|period| period.end.as_deref())
            .unwrap_or("");
        assert!(end.contains("2026-10-13T01:25:32"), "period end {end}");
        let again = decode_get_grok_credits_config_frame(&frame_with_used_percent(28.0))
            .expect("28 percent frame");
        assert_eq!(again.credit_usage_percent, Some(28.0));
    }

    fn frame_with_used_percent_and_prepaid_cents(percent: f32, cents: u64) -> Vec<u8> {
        let mut config = Vec::new();
        config.push(0x0d);
        config.extend_from_slice(&percent.to_le_bytes());
        let mut cent = Vec::new();
        cent.push(0x08);
        let mut value = cents;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            cent.push(byte);
            if value == 0 {
                break;
            }
        }
        config.push(0x62);
        config.push(cent.len() as u8);
        config.extend_from_slice(&cent);
        let mut outer = Vec::new();
        outer.push(0x0a);
        outer.push(config.len() as u8);
        outer.extend_from_slice(&config);
        grpc_web_frame(&outer)
    }

    /// A personal identity whose poll recorded `auth`, after a fresh personal
    /// session, stores `usage_pct` from that session's finite
    /// `GetGrokCreditsConfig` `credit_usage_percent`. The stored value is
    /// percent used. A POST with no `Authorization` is not that store.
    /// Another personal identity that also lacks a finite ok percent does
    /// not skip this row. The business-team row and `management` stay.
    #[tokio::test]
    async fn personal_auth_poll_after_a_fresh_session_stores_credit_usage_percent_not_a_no_authorization_post()
     {
        use axum::response::IntoResponse;
        use std::sync::{Arc, Mutex};

        const PERSONAL_USER: &str = "personal-poll-auth-user";
        const OTHER_PERSONAL_USER: &str = "other-personal-lacking-percent";
        const BUSINESS_TEAM: &str = "business-team-kept";
        const PERSONAL_SESSION: &str = "personal-session-for-credits-post";
        const POSTED_USED_PERCENT: f64 = 37.0;
        const NO_AUTHORIZATION_DECOY_PERCENT: f32 = 4.0;
        const POSTED_PREPAID_CENTS: u64 = 3217;

        let seen = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let seen_for_handler = Arc::clone(&seen);
        let authorized_body = frame_with_used_percent_and_prepaid_cents(
            POSTED_USED_PERCENT as f32,
            POSTED_PREPAID_CENTS,
        );
        let no_authorization_body = frame_with_used_percent(NO_AUTHORIZATION_DECOY_PERCENT);
        let router = axum::Router::new().route(
            "/GetGrokCreditsConfig",
            axum::routing::post(move |headers: axum::http::HeaderMap| {
                let seen = Arc::clone(&seen_for_handler);
                let authorized_body = authorized_body.clone();
                let no_authorization_body = no_authorization_body.clone();
                async move {
                    let authorization = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    seen.lock()
                        .expect("authorization log")
                        .push(authorization.clone());
                    let body = if authorization.is_some() {
                        authorized_body
                    } else {
                        no_authorization_body
                    };
                    (axum::http::StatusCode::OK, body).into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local credits listener");
        let addr = listener.local_addr().expect("local credits address");
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("local credits server");
        });
        let credits_url = format!("http://{addr}/GetGrokCreditsConfig");

        let home = tempfile::TempDir::new().expect("temp home");
        let mut map = xai_grok_login::model::AuthStore::new();
        let personal = GrokAuth {
            key: PERSONAL_SESSION.into(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            ..sample_auth(PERSONAL_USER, None, None)
        };
        let other_personal = GrokAuth {
            key: "other-personal-session-not-this-store".into(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            ..sample_auth(OTHER_PERSONAL_USER, None, None)
        };
        let business = GrokAuth {
            key: "business-session-not-this-store".into(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            ..sample_auth("business-user", Some(BUSINESS_TEAM), Some("Team"))
        };
        crate::auth::upsert_supergrok_session(
            &mut map,
            "https://auth.example.invalid/personal-poll",
            personal,
        );
        crate::auth::upsert_supergrok_session(
            &mut map,
            "https://auth.example.invalid/other-personal",
            other_personal,
        );
        crate::auth::upsert_supergrok_session(
            &mut map,
            "https://auth.example.invalid/business",
            business,
        );
        xai_grok_login::storage::write_auth_json(&home.path().join("auth.json"), &map)
            .expect("auth.json");

        let personal_ids = personal_identity_ids_from_auth(home.path());
        assert!(
            personal_ids.iter().any(|id| id == PERSONAL_USER),
            "the auth personal identity must be listed"
        );
        assert!(
            personal_ids.iter().any(|id| id == OTHER_PERSONAL_USER),
            "another personal identity also lacks a finite ok percent"
        );
        assert!(
            !personal_ids.iter().any(|id| id == BUSINESS_TEAM),
            "the business-team id is not a personal identity"
        );

        let business_row = crate::auth::LimitsSnapshotIdentity {
            identity_id: BUSINESS_TEAM.into(),
            usage_pct: None,
            period_end: None,
            period_type: None,
            dollar_credits_cents: None,
            grok_build_usage_pct: None,
            is_unified_billing_user: None,
            poll_outcome: crate::auth::POLL_OUTCOME_OTHER.into(),
        };
        let management = crate::auth::LimitsSnapshotManagement {
            team_id: Some("kept-management-team".into()),
            ..crate::auth::LimitsSnapshotManagement::default()
        };
        let mut doc = crate::auth::LimitsSnapshotDocument::empty(1);
        doc.identities.push(crate::auth::LimitsSnapshotIdentity {
            identity_id: PERSONAL_USER.into(),
            usage_pct: None,
            period_end: None,
            period_type: None,
            dollar_credits_cents: None,
            grok_build_usage_pct: None,
            is_unified_billing_user: None,
            poll_outcome: crate::auth::POLL_OUTCOME_AUTH.into(),
        });
        doc.identities.push(business_row.clone());
        doc.management = Some(management.clone());

        record_personal_credits_post_on_the_personal_row(home.path(), &mut doc, Some(&credits_url))
            .await;

        let personal_row = doc
            .identities
            .iter()
            .find(|row| row.identity_id == PERSONAL_USER)
            .expect("personal row");
        assert_eq!(
            personal_row.usage_pct,
            Some(POSTED_USED_PERCENT),
            "usage_pct is the posted credit_usage_percent, percent used"
        );
        assert_ne!(
            personal_row.usage_pct,
            Some(f64::from(NO_AUTHORIZATION_DECOY_PERCENT)),
            "a POST with no Authorization is not the store"
        );
        assert_eq!(
            personal_row.dollar_credits_cents, None,
            "a finite used percent below 100 does not store Extra Usage Credits cents"
        );
        assert_eq!(personal_row.poll_outcome, crate::auth::POLL_OUTCOME_OK);
        let posts = seen.lock().expect("authorization log").clone();
        let expected_authorization = format!("Bearer {PERSONAL_SESSION}");
        assert_eq!(posts.len(), 1, "one credits POST stores the percent");
        assert_eq!(
            posts[0].as_deref(),
            Some(expected_authorization.as_str()),
            "the request that stored usage_pct sent the personal session bearer"
        );
        assert!(
            posts
                .iter()
                .all(|header| header.as_deref().is_some_and(|value| !value.is_empty())),
            "a POST with no Authorization is not the store"
        );
        assert!(
            !doc.identities
                .iter()
                .any(|row| row.identity_id == OTHER_PERSONAL_USER),
            "the other personal identity that lacks a percent is not this poll's row"
        );
        assert_eq!(
            doc.identities
                .iter()
                .find(|row| row.identity_id == BUSINESS_TEAM),
            Some(&business_row),
            "the business-team row stays"
        );
        assert_eq!(doc.management, Some(management), "management stays");
    }

    /// A rejected personal refresh (`invalid_grant`) does not send the expired
    /// access token. Personal `usage_pct` stays empty. The personal poll
    /// outcome is `reauth`, not `auth`.
    #[tokio::test]
    async fn rejected_personal_refresh_does_not_send_the_expired_access_token() {
        use axum::response::IntoResponse;
        use std::sync::{Arc, Mutex};

        const PERSONAL_USER: &str = "personal-rejected-refresh-user";
        const EXPIRED_ACCESS: &str = "expired-personal-access-token";
        const BUSINESS_ACCESS: &str = "business-session-not-this-bearer";

        let seen = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let seen_for_handler = Arc::clone(&seen);
        let credits = axum::Router::new().route(
            "/GetGrokCreditsConfig",
            axum::routing::post(move |headers: axum::http::HeaderMap| {
                let seen = Arc::clone(&seen_for_handler);
                async move {
                    let authorization = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    seen.lock().expect("authorization log").push(authorization);
                    axum::http::StatusCode::OK.into_response()
                }
            }),
        );
        let credits_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local credits listener");
        let credits_addr = credits_listener
            .local_addr()
            .expect("local credits address");
        tokio::spawn(async move {
            axum::serve(credits_listener, credits)
                .await
                .expect("local credits server");
        });
        let credits_url = format!("http://{credits_addr}/GetGrokCreditsConfig");

        let oidc_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local oidc listener");
        let issuer = format!(
            "http://127.0.0.1:{}",
            oidc_listener.local_addr().expect("oidc port").port()
        );
        let issuer_disc = issuer.clone();
        let oidc = axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let iss = issuer_disc.clone();
                    async move {
                        axum::Json(serde_json::json!({
                            "authorization_endpoint": format!("{iss}/authorize"),
                            "token_endpoint": format!("{iss}/token"),
                            "jwks_uri": format!("{iss}/jwks"),
                            "id_token_signing_alg_values_supported": ["RS256"],
                        }))
                    }
                }),
            )
            .route(
                "/token",
                axum::routing::post(|| async {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        axum::Json(serde_json::json!({
                            "error": "invalid_grant",
                            "error_description": "refresh token revoked"
                        })),
                    )
                }),
            );
        tokio::spawn(async move {
            axum::serve(oidc_listener, oidc)
                .await
                .expect("local oidc server");
        });

        let home = tempfile::TempDir::new().expect("temp home");
        let mut map = xai_grok_login::model::AuthStore::new();
        let personal = GrokAuth {
            key: EXPIRED_ACCESS.into(),
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            refresh_token: Some("personal-refresh-token".into()),
            oidc_issuer: Some(issuer),
            oidc_client_id: Some("personal-limits-client".into()),
            ..sample_auth(PERSONAL_USER, None, None)
        };
        let business = GrokAuth {
            key: BUSINESS_ACCESS.into(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            ..sample_auth("business-user", Some("business-team-kept"), Some("Team"))
        };
        crate::auth::upsert_supergrok_session(
            &mut map,
            "https://auth.example.invalid/rejected-personal",
            personal,
        );
        crate::auth::upsert_supergrok_session(
            &mut map,
            "https://auth.example.invalid/rejected-personal",
            business,
        );
        xai_grok_login::storage::write_auth_json(&home.path().join("auth.json"), &map)
            .expect("auth.json");

        let mut doc = crate::auth::LimitsSnapshotDocument::empty(1);
        doc.identities.push(crate::auth::LimitsSnapshotIdentity {
            identity_id: PERSONAL_USER.into(),
            usage_pct: None,
            period_end: None,
            period_type: None,
            dollar_credits_cents: None,
            grok_build_usage_pct: None,
            is_unified_billing_user: None,
            poll_outcome: crate::auth::POLL_OUTCOME_AUTH.into(),
        });

        record_personal_credits_post_on_the_personal_row(home.path(), &mut doc, Some(&credits_url))
            .await;

        let posts = seen.lock().expect("authorization log").clone();
        assert!(
            posts.iter().all(|header| {
                header.as_deref().is_none_or(|value| {
                    !value.ends_with(EXPIRED_ACCESS) && !value.ends_with(BUSINESS_ACCESS)
                })
            }),
            "TerminalError must not send the expired access token or the business token"
        );
        let personal_row = doc
            .identities
            .iter()
            .find(|row| row.identity_id == PERSONAL_USER)
            .expect("personal row");
        assert_eq!(
            personal_row.usage_pct, None,
            "personal usage_pct stays empty"
        );
        assert_eq!(
            personal_row.poll_outcome,
            crate::auth::limits_snapshot_hub::POLL_OUTCOME_REAUTH,
            "a rejected grant is reauth, not auth"
        );
        assert_ne!(
            personal_row.poll_outcome,
            crate::auth::POLL_OUTCOME_AUTH,
            "auth stays a not-yet-decided or other auth miss"
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // SharedSnapshotEnvGuard serializes GROK_HOME.
    // Grok OSS: x.ai/billing uses the snapshot hub instead of unconditionally HTTP-ing siblings. This diverges from upstream xAI because automatic limits fetch is at most once an hour per machine through the snapshot hub.
    async fn billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http() {
        use crate::auth::limits_snapshot_hub::SharedSnapshotEnvGuard;
        use crate::auth::{
            LimitsSnapshotDocument, LimitsSnapshotIdentity, LimitsSnapshotMode, LimitsSnapshotRole,
            POLL_OUTCOME_OK, clear_included_billing_cache, included_billing_fields_snapshot,
            write_limits_snapshot_file,
        };
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = crate::auth::limits_snapshot_hub::now_unix_ms();
        let mut fresh = LimitsSnapshotDocument::empty(now);
        fresh.identities.push(LimitsSnapshotIdentity {
            identity_id: "sibling-biz".into(),
            usage_pct: Some(18.0),
            period_end: Some("2026-09-01T00:00:00Z".into()),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            dollar_credits_cents: Some(0),
            grok_build_usage_pct: None,
            is_unified_billing_user: Some(false),
            poll_outcome: POLL_OUTCOME_OK.into(),
        });
        write_limits_snapshot_file(home, &fresh).expect("seed fresh snapshot");

        let http = Arc::new(AtomicU32::new(0));
        let http_cb = Arc::clone(&http);
        let (role, doc) = collect_billing_via_snapshot_hub(
            home,
            LimitsSnapshotMode::HonorTtl,
            now.saturating_add(500),
            || {
                let http_cb = Arc::clone(&http_cb);
                async move {
                    http_cb.fetch_add(1, Ordering::SeqCst);
                    let mut leaked = LimitsSnapshotDocument::empty(now);
                    leaked.identities.push(LimitsSnapshotIdentity {
                        identity_id: "sibling-biz".into(),
                        usage_pct: Some(90.0),
                        period_end: None,
                        period_type: None,
                        dollar_credits_cents: None,
                        grok_build_usage_pct: None,
                        is_unified_billing_user: None,
                        poll_outcome: POLL_OUTCOME_OK.into(),
                    });
                    leaked
                }
            },
        )
        .await
        .expect("hub collect");
        assert_eq!(role, LimitsSnapshotRole::FollowerRead);
        assert_eq!(
            http.load(Ordering::SeqCst),
            0,
            "billing handler must not unconditionally HTTP siblings when a fresh snapshot exists"
        );
        assert_eq!(doc.identities[0].usage_pct, Some(18.0));
        let remembered = included_billing_fields_snapshot();
        assert_eq!(
            remembered.get("sibling-biz").and_then(|f| f.usage_pct),
            Some(18.0)
        );
    }

    #[test]
    fn limits_snapshot_mode_for_get_billing_explicit_is_force_refresh() {
        assert_eq!(
            limits_snapshot_mode_for_get_billing(true),
            crate::auth::LimitsSnapshotMode::ForceRefresh
        );
        assert_eq!(
            limits_snapshot_mode_for_get_billing(false),
            crate::auth::LimitsSnapshotMode::HonorTtl
        );
        assert!(force_refresh_from_billing_params(
            r#"{"forceRefresh":true}"#
        ));
        assert!(!force_refresh_from_billing_params("{}"));
        assert!(!force_refresh_from_billing_params("not-json"));
    }

    /// Background billing poll HonorTtl does not clear Management caches
    /// and does not ForceRefresh.
    #[test]
    fn background_billing_poll_honor_ttl_does_not_force_refresh_or_clear_management_caches() {
        assert_eq!(
            limits_snapshot_mode_for_get_billing(false),
            crate::auth::LimitsSnapshotMode::HonorTtl
        );
        assert!(
            !should_clear_management_caches_on_billing_leader_fetch(false, true),
            "HonorTtl background poll must not clear Management process caches"
        );
        assert!(
            !should_clear_management_caches_on_billing_leader_fetch(false, false),
            "HonorTtl without a management key must not clear"
        );
        assert!(
            should_clear_management_caches_on_billing_leader_fetch(true, true),
            "ForceRefresh with a management key still busts process caches"
        );
        assert!(
            !should_clear_management_caches_on_billing_leader_fetch(true, false),
            "ForceRefresh without a management key must not clear"
        );
    }

    /// Explicit `/limits` collect must ForceRefresh a fresh-by-TTL disk
    /// snapshot whose `usagePct` is 100. That disk field is not Usage and
    /// must not fill the remember map as included SuperGrok period exhaust.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // SharedSnapshotEnvGuard serializes GROK_HOME.
    async fn explicit_limits_force_refresh_does_not_apply_disk_usage_pct_100() {
        use crate::auth::limits_snapshot_hub::SharedSnapshotEnvGuard;
        use crate::auth::{
            LimitsSnapshotDocument, LimitsSnapshotIdentity, LimitsSnapshotMode, LimitsSnapshotRole,
            POLL_OUTCOME_OK, clear_included_billing_cache, included_billing_fields_snapshot,
            write_limits_snapshot_file,
        };
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = crate::auth::limits_snapshot_hub::now_unix_ms();
        let mut stale_100 = LimitsSnapshotDocument::empty(now);
        stale_100.identities.push(LimitsSnapshotIdentity {
            identity_id: "user-personal".into(),
            usage_pct: Some(100.0),
            period_end: Some("2026-09-01T00:00:00Z".into()),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            dollar_credits_cents: Some(10_029),
            grok_build_usage_pct: None,
            is_unified_billing_user: Some(false),
            poll_outcome: POLL_OUTCOME_OK.into(),
        });
        stale_100.identities.push(LimitsSnapshotIdentity {
            identity_id: "team-biz".into(),
            usage_pct: Some(100.0),
            period_end: Some("2026-09-01T00:00:00Z".into()),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            dollar_credits_cents: None,
            grok_build_usage_pct: None,
            is_unified_billing_user: Some(false),
            poll_outcome: POLL_OUTCOME_OK.into(),
        });
        write_limits_snapshot_file(home, &stale_100).expect("seed disk usagePct 100");

        let http = Arc::new(AtomicU32::new(0));
        let http_cb = Arc::clone(&http);
        let mode = limits_snapshot_mode_for_get_billing(true);
        assert_eq!(mode, LimitsSnapshotMode::ForceRefresh);
        let (role, doc) =
            collect_billing_via_snapshot_hub(home, mode, now.saturating_add(500), || {
                let http_cb = Arc::clone(&http_cb);
                async move {
                    http_cb.fetch_add(1, Ordering::SeqCst);
                    let mut live = LimitsSnapshotDocument::empty(now.saturating_add(500));
                    live.identities.push(LimitsSnapshotIdentity {
                        identity_id: "user-personal".into(),
                        usage_pct: Some(100.0),
                        period_end: Some("2026-09-01T00:00:00Z".into()),
                        period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                        dollar_credits_cents: Some(10_029),
                        grok_build_usage_pct: None,
                        is_unified_billing_user: Some(false),
                        poll_outcome: POLL_OUTCOME_OK.into(),
                    });
                    live.identities.push(LimitsSnapshotIdentity {
                        identity_id: "team-biz".into(),
                        usage_pct: Some(41.0),
                        period_end: Some("2026-09-01T00:00:00Z".into()),
                        period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                        dollar_credits_cents: None,
                        grok_build_usage_pct: None,
                        is_unified_billing_user: Some(false),
                        poll_outcome: POLL_OUTCOME_OK.into(),
                    });
                    live
                }
            })
            .await
            .expect("explicit collect");
        assert_eq!(role, LimitsSnapshotRole::LeaderFetched);
        assert_eq!(
            http.load(Ordering::SeqCst),
            1,
            "explicit ForceRefresh must fetch instead of applying disk usagePct 100"
        );
        let team = doc
            .identities
            .iter()
            .find(|i| i.identity_id == "team-biz")
            .expect("team row");
        assert_eq!(
            team.usage_pct,
            Some(41.0),
            "live included SuperGrok period used percent must replace disk 100"
        );
        let remembered = included_billing_fields_snapshot();
        assert_eq!(
            remembered.get("team-biz").and_then(|f| f.usage_pct),
            Some(41.0),
            "remember map must fill Team/Business from the live collect"
        );
        assert_eq!(
            remembered.get("user-personal").and_then(|f| f.usage_pct),
            Some(100.0)
        );

        use crate::auth::supergrok_identity_rank::{
            SupergrokAccountRole, SupergrokIdentityHeadroom, SupergrokSessionCandidate,
            enrich_candidates_with_included_billing, order_credentials_for_preferred_auto,
        };
        let mut candidates = vec![
            SupergrokSessionCandidate {
                headroom: SupergrokIdentityHeadroom {
                    identity_id: "user-personal".into(),
                    role: SupergrokAccountRole::Personal,
                    included_remaining: 1,
                    reset_at: None,
                },
                access_token: "tok-personal".into(),
                prepaid_balance_cents: Some(10_029),
                hard_expired: false,
            },
            SupergrokSessionCandidate {
                headroom: SupergrokIdentityHeadroom {
                    identity_id: "team-biz".into(),
                    role: SupergrokAccountRole::Business,
                    included_remaining: 1,
                    reset_at: None,
                },
                access_token: "tok-team".into(),
                prepaid_balance_cents: None,
                hard_expired: false,
            },
        ];
        let _ = enrich_candidates_with_included_billing(&mut candidates, &remembered, |_| false);
        let team_row = candidates
            .iter()
            .find(|c| c.headroom.identity_id == "team-biz")
            .expect("team candidate");
        assert!(
            team_row.headroom.included_remaining > 0,
            "live 41% must not be treated as included exhaust"
        );
        let order =
            order_credentials_for_preferred_auto(&candidates, &["console-must-wait".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-personal"),
            "next-turn rank must stay on the personal SuperGrok paying JWT: {order:?}"
        );
        assert_ne!(
            order.primary.as_deref(),
            Some("tok-team"),
            "must not hop to Team JWT (Billing Credits settlement) while a personal SuperGrok login exists"
        );
        assert_ne!(
            order.primary.as_deref(),
            Some("console-must-wait"),
            "must not make console primary while personal SuperGrok included remaining stands"
        );
    }

    #[test]
    fn included_usage_prefers_credit_usage_percent_and_period_end() {
        let config = BillingConfig {
            credit_usage_percent: Some(33.5),
            current_period: Some(UsagePeriod {
                period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                start: Some("2026-07-01T00:00:00Z".into()),
                end: Some("2026-07-08T00:00:00Z".into()),
            }),
            monthly_limit: Some(Cent { val: 2000 }),
            used: Some(Cent { val: 999 }),
            billing_period_end: Some("2026-08-01T00:00:00Z".into()),
            ..empty_config()
        };
        let (pct, end) = included_usage_and_period_end(&config);
        assert_eq!(pct, Some(33.5));
        assert_eq!(end.as_deref(), Some("2026-07-08T00:00:00Z"));
    }

    #[test]
    fn included_usage_falls_back_to_limit_used_and_billing_period_end() {
        let config = BillingConfig {
            monthly_limit: Some(Cent { val: 1000 }),
            used: Some(Cent { val: 250 }),
            billing_period_start: Some("2026-07-01T00:00:00Z".into()),
            billing_period_end: Some("2026-08-01T00:00:00Z".into()),
            ..empty_config()
        };
        let (pct, end) = included_usage_and_period_end(&config);
        assert_eq!(pct, Some(25.0));
        assert_eq!(end.as_deref(), Some("2026-08-01T00:00:00Z"));
    }

    #[test]
    fn included_usage_honest_absence_when_no_meters() {
        let config = empty_config();
        let (pct, end) = included_usage_and_period_end(&config);
        assert_eq!(pct, None);
        assert_eq!(end, None);
    }

    #[test]
    fn auto_topup_disabled_rule_omits_enabled_field() {
        // proto3 JSON omits `false` and `0`, so a disabled rule arrives without `enabled` (and zero Cents arrive as `{}`)
        // It must still deserialize (as disabled) rather than erroring; otherwise the pager keeps a stale cached rule
        let json = serde_json::json!({
            "rule": { "topupAmount": {"val": 500}, "minBeforeHittingSl": {} }
        });
        let resp: GetAutoTopupRuleResponse = serde_json::from_value(json).unwrap();
        let rule = resp.rule.expect("rule present");
        assert!(!rule.enabled);
        assert_eq!(rule.topup_amount.unwrap().val, 500);
        assert_eq!(rule.min_before_hitting_sl.unwrap().val, 0);
    }

    #[test]
    fn billing_config_response_deserializes_from_backend_json() {
        let json = serde_json::json!({
            "config": {
                "monthlyLimit": {"val": 2000},
                "used": {"val": 1234},
                "onDemandCap": {"val": 500},
                "billingPeriodStart": "2025-04-01T00:00:00Z",
                "billingPeriodEnd": "2025-05-01T00:00:00Z",
                "history": [
                    {
                        "billingCycle": {"year": 2025, "month": 3},
                        "includedUsed": {"val": 1800},
                        "onDemandUsed": {"val": 0},
                        "totalUsed": {"val": 1800}
                    }
                ]
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.monthly_limit.unwrap().val, 2000);
        assert_eq!(config.used.unwrap().val, 1234);
        assert_eq!(config.on_demand_cap.unwrap().val, 500);
        assert_eq!(
            config.billing_period_start.as_deref(),
            Some("2025-04-01T00:00:00Z")
        );
        assert_eq!(config.history.len(), 1);
        let Some(period) = config.history.first() else {
            panic!("expected one billing period");
        };
        let cycle = period.billing_cycle.as_ref().unwrap();
        assert_eq!(cycle.year, 2025);
        assert_eq!(cycle.month, 3);
        assert_eq!(period.included_used.as_ref().unwrap().val, 1800);
        assert_eq!(period.total_used.as_ref().unwrap().val, 1800);
    }

    #[test]
    fn billing_unified_log_ctx_includes_credits_and_collapses_history() {
        let resp = BillingConfigResponse {
            config: Some(BillingConfig {
                credit_usage_percent: Some(42.5),
                current_period: Some(UsagePeriod {
                    period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                    start: Some("2025-04-01T00:00:00Z".into()),
                    end: Some("2025-04-08T00:00:00Z".into()),
                }),
                monthly_limit: Some(Cent { val: 2000 }),
                used: Some(Cent { val: 850 }),
                on_demand_cap: Some(Cent { val: 500 }),
                on_demand_used: Some(Cent { val: 0 }),
                prepaid_balance: Some(Cent { val: 100 }),
                is_unified_billing_user: Some(true),
                history: vec![
                    BillingPeriodUsage {
                        billing_cycle: Some(BillingCycle {
                            year: 2025,
                            month: 2,
                        }),
                        included_used: Some(Cent { val: 1000 }),
                        on_demand_used: Some(Cent { val: 0 }),
                        total_used: Some(Cent { val: 1000 }),
                    },
                    BillingPeriodUsage {
                        billing_cycle: Some(BillingCycle {
                            year: 2025,
                            month: 3,
                        }),
                        included_used: Some(Cent { val: 1800 }),
                        on_demand_used: Some(Cent { val: 0 }),
                        total_used: Some(Cent { val: 1800 }),
                    },
                ],
                ..empty_config()
            }),
            on_demand_enabled: Some(true),
            subscription_tier: Some("SuperGrok".into()),
        };
        let ctx = billing_unified_log_ctx(&resp);
        assert_eq!(
            ctx.get("onDemandEnabled").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            ctx.get("subscriptionTier").and_then(|v| v.as_str()),
            Some("SuperGrok")
        );
        let config = ctx
            .get("config")
            .and_then(|v| v.as_object())
            .expect("config object");
        assert!(
            config.get("history").is_none(),
            "full history must be collapsed"
        );
        assert_eq!(config.get("historyLen").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(
            config
                .get("latestHistory")
                .and_then(|h| h.get("billingCycle"))
                .and_then(|c| c.get("month"))
                .and_then(|v| v.as_u64()),
            Some(3),
            "latest history period retained"
        );
        assert_eq!(
            config.get("creditUsagePercent").and_then(|v| v.as_f64()),
            Some(42.5)
        );
        assert_eq!(
            config
                .get("prepaidBalance")
                .and_then(|b| b.get("val"))
                .and_then(|v| v.as_u64()),
            Some(100)
        );
    }

    /// SuperGrok `GetGrokCreditsConfig.prepaidBalance.val` is SuperGrok dollar
    /// credits. It is not the console Billing Credits card. Public docs do
    /// not bind that card to this field.
    #[test]
    fn prepaid_balance_maps_to_supergrok_dollar_credits_never_billing_credits_card() {
        use xai_grok_sampling_types::{
            BillingCreditsCard, billing_credits_card_from_supergrok_prepaid_balance,
            billing_credits_usd_from_named_json_field, current_billing_credits_usd,
        };

        let config = BillingConfig {
            prepaid_balance: Some(Cent { val: 4703 }),
            ..empty_config()
        };
        let row = limits_identity_from_credits_config("principal-1", &config, "ok");
        assert_eq!(
            row.dollar_credits_cents,
            Some(4703),
            "prepaidBalance.val is SuperGrok dollar credits"
        );
        assert_eq!(
            billing_credits_card_from_supergrok_prepaid_balance(row.dollar_credits_cents.unwrap()),
            BillingCreditsCard::NotFetched
        );
        assert_eq!(
            billing_credits_usd_from_named_json_field("prepaidBalance.val", 47.03),
            None
        );
        assert_eq!(
            current_billing_credits_usd(Some(89.94), Some(47.03), None),
            None,
            "operator-visible Billing Credits $47.03 is not this SuperGrok field"
        );
    }

    fn sample_auth(user_id: &str, team_id: Option<&str>, principal_type: Option<&str>) -> GrokAuth {
        GrokAuth {
            key: "session-token-not-a-secret-for-tests".into(),
            auth_mode: crate::auth::AuthMode::Oidc,
            create_time: chrono::Utc::now(),
            user_id: user_id.into(),
            email: None,
            first_name: None,
            last_name: None,
            profile_image_asset_id: None,
            principal_type: principal_type.map(str::to_owned),
            principal_id: None,
            team_id: team_id.map(str::to_owned),
            team_name: None,
            team_role: None,
            organization_id: None,
            organization_name: None,
            organization_role: None,
            user_blocked_reason: None,
            team_blocked_reasons: vec![],
            coding_data_retention_opt_out: true,
            can_administer_team: None,
            has_grok_code_access: None,
            refresh_token: None,
            expires_at: None,
            oidc_issuer: None,
            oidc_client_id: None,
        }
    }

    /// Named contract: log identity comes from the polled GrokAuth (team/user),
    /// not a second auth.json scan — keeps identity_id when SuperGrok auth
    /// succeeded even if disk listing is missing.
    #[test]
    fn billing_log_identity_from_auth_uses_polled_credential() {
        let personal = sample_auth("user-abc", None, None);
        let (id, role) = billing_log_identity_from_auth(&personal);
        assert_eq!(id, "user-abc");
        assert_eq!(role, "personal");

        let business = sample_auth(
            "user-abc",
            Some("61fab250-b2c1-40cf-b5b8-628e673a2eeb"),
            Some("Team"),
        );
        let (id, role) = billing_log_identity_from_auth(&business);
        assert_eq!(id, "61fab250-b2c1-40cf-b5b8-628e673a2eeb");
        assert_eq!(role, "business");

        // Log ctx still gets non-blank identity without inventing Build %.
        let resp = BillingConfigResponse {
            config: Some(BillingConfig {
                credit_usage_percent: Some(65.0),
                ..empty_config()
            }),
            on_demand_enabled: None,
            subscription_tier: None,
        };
        let ctx = billing_fetched_credits_log_ctx(&resp, Some(id.as_str()), Some(role));
        assert_eq!(ctx["identity_id"], "61fab250-b2c1-40cf-b5b8-628e673a2eeb");
        assert_eq!(ctx["role"], "business");
        assert!(
            ctx.get("grok_build_usage_percent").is_none(),
            "no productUsage → no Build invent: {ctx}"
        );
    }

    #[test]
    fn billing_fetched_credits_log_ctx_includes_identity_and_build_product_usage() {
        // Named contract: successful credits log carries principal id (+ role)
        // and surfaces PRODUCT_GROK_BUILD % when productUsage is on the wire.
        let resp = BillingConfigResponse {
            config: Some(BillingConfig {
                credit_usage_percent: Some(65.0),
                product_usage: vec![
                    ProductUsageEntry {
                        product: Some("PRODUCT_OTHER".into()),
                        usage_percent: Some(10.0),
                    },
                    ProductUsageEntry {
                        product: Some(PRODUCT_GROK_BUILD.into()),
                        usage_percent: Some(61.2),
                    },
                ],
                prepaid_balance: Some(Cent { val: 10029 }),
                ..empty_config()
            }),
            on_demand_enabled: None,
            subscription_tier: Some("SuperGrok Heavy".into()),
        };
        let ctx = billing_fetched_credits_log_ctx(
            &resp,
            Some("user-abc::team::61fab250-b2c1-40cf-b5b8-628e673a2eeb"),
            Some("business"),
        );
        assert_eq!(
            ctx["identity_id"],
            "user-abc::team::61fab250-b2c1-40cf-b5b8-628e673a2eeb"
        );
        assert_eq!(ctx["role"], "business");
        assert_eq!(ctx["grok_build_usage_percent"], 61.2);
        let config = ctx["config"].as_object().expect("config");
        assert_eq!(config["creditUsagePercent"], 65.0);
        let products = config["productUsage"].as_array().expect("productUsage");
        assert_eq!(products.len(), 2);
        assert_eq!(products[1]["product"], PRODUCT_GROK_BUILD);
        assert_eq!(products[1]["usagePercent"], 61.2);
    }

    #[test]
    fn billing_fetched_credits_log_ctx_omits_blank_identity_and_missing_build() {
        let resp = BillingConfigResponse {
            config: Some(BillingConfig {
                credit_usage_percent: Some(42.0),
                ..empty_config()
            }),
            on_demand_enabled: None,
            subscription_tier: None,
        };
        let ctx = billing_fetched_credits_log_ctx(&resp, Some("  "), Some(""));
        assert!(
            ctx.get("identity_id").is_none(),
            "blank identity must not be logged: {ctx}"
        );
        assert!(
            ctx.get("role").is_none(),
            "blank role must not be logged: {ctx}"
        );
        assert!(
            ctx.get("grok_build_usage_percent").is_none(),
            "no productUsage → no Build hoist: {ctx}"
        );
    }

    #[test]
    fn grok_build_usage_percent_reads_product_usage_wire() {
        let with_build = BillingConfig {
            product_usage: vec![ProductUsageEntry {
                product: Some(PRODUCT_GROK_BUILD.into()),
                usage_percent: Some(61.2),
            }],
            ..empty_config()
        };
        assert_eq!(grok_build_usage_percent(&with_build), Some(61.2));
        assert_eq!(grok_build_usage_percent(&empty_config()), None);
        let other_only = BillingConfig {
            product_usage: vec![ProductUsageEntry {
                product: Some("PRODUCT_OTHER".into()),
                usage_percent: Some(99.0),
            }],
            ..empty_config()
        };
        assert_eq!(grok_build_usage_percent(&other_only), None);
    }

    #[test]
    fn billing_config_response_roundtrips_through_json() {
        let config = BillingConfig {
            monthly_limit: Some(Cent { val: 5000 }),
            used: Some(Cent { val: 123 }),
            on_demand_cap: Some(Cent { val: 0 }),
            on_demand_used: Some(Cent { val: 50 }),
            prepaid_balance: Some(Cent { val: 750 }),
            billing_period_start: Some("2025-04-01T00:00:00Z".to_string()),
            billing_period_end: Some("2025-05-01T00:00:00Z".to_string()),
            history: vec![BillingPeriodUsage {
                billing_cycle: Some(BillingCycle {
                    year: 2025,
                    month: 3,
                }),
                included_used: Some(Cent { val: 4500 }),
                on_demand_used: Some(Cent { val: 100 }),
                total_used: Some(Cent { val: 4600 }),
            }],
            ..empty_config()
        };
        let resp = BillingConfigResponse {
            config: Some(config),
            on_demand_enabled: None,
            subscription_tier: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        let roundtripped: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let rt_config = roundtripped.config.unwrap();
        assert_eq!(rt_config.monthly_limit.unwrap().val, 5000);
        assert_eq!(rt_config.used.unwrap().val, 123);
        assert_eq!(rt_config.prepaid_balance.unwrap().val, 750);
        assert_eq!(rt_config.history.len(), 1);
    }

    #[test]
    fn billing_config_response_handles_null_config() {
        let json = serde_json::json!({"config": null});
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        assert!(resp.config.is_none());
    }

    #[test]
    fn billing_config_response_handles_empty_history() {
        let json = serde_json::json!({
            "config": {
                "monthlyLimit": {"val": 1000},
                "used": {"val": 0}
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.monthly_limit.unwrap().val, 1000);
        assert!(config.history.is_empty());
    }

    #[test]
    fn billing_config_serializes_camel_case() {
        let config = BillingConfig {
            monthly_limit: Some(Cent { val: 100 }),
            ..empty_config()
        };
        let json = serde_json::to_value(&config).unwrap();
        assert!(json.get("monthlyLimit").is_some());
        // Fields with None are skipped
        assert!(json.get("creditUsagePercent").is_none());
        assert!(json.get("currentPeriod").is_none());
        assert!(json.get("used").is_none());
        assert!(json.get("onDemandCap").is_none());
        assert!(json.get("onDemandUsed").is_none());
        assert!(json.get("prepaidBalance").is_none());
        assert!(json.get("billingPeriodStart").is_none());
        // Empty history / productUsage are skipped
        assert!(json.get("history").is_none());
        assert!(json.get("productUsage").is_none());
    }

    #[test]
    fn billing_config_deserializes_credits_config_shape() {
        // Newer `GetGrokCreditsConfig` response: percentage-based usage, a typed current period, and history keyed by `period`
        let json = serde_json::json!({
            "config": {
                "creditUsagePercent": 42.5,
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "start": "2026-06-01T00:00:00Z",
                    "end": "2026-06-08T00:00:00Z"
                },
                "onDemandCap": {"val": 5000},
                "onDemandUsed": {"val": 300},
                "prepaidBalance": {"val": 1250},
                "isUnifiedBillingUser": true,
                "productUsage": [
                    {"product": "PRODUCT_GROK_BUILD", "usagePercent": 61.2}
                ],
                "history": [
                    {
                        "period": {
                            "type": "USAGE_PERIOD_TYPE_WEEKLY",
                            "start": "2026-05-25T00:00:00Z",
                            "end": "2026-06-01T00:00:00Z"
                        },
                        "onDemandUsed": {"val": 120}
                    }
                ]
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.credit_usage_percent, Some(42.5));
        let period = config.current_period.as_ref().unwrap();
        assert_eq!(
            period.period_type.as_deref(),
            Some("USAGE_PERIOD_TYPE_WEEKLY")
        );
        assert_eq!(period.end.as_deref(), Some("2026-06-08T00:00:00Z"));
        // Deprecated fields are absent in the credits shape.
        assert!(config.monthly_limit.is_none());
        assert!(config.billing_period_end.is_none());
        assert_eq!(config.on_demand_cap.unwrap().val, 5000);
        assert_eq!(config.on_demand_used.unwrap().val, 300);
        assert_eq!(config.prepaid_balance.unwrap().val, 1250);
        assert_eq!(config.is_unified_billing_user, Some(true));
        // The CLI billing code does not read `productUsage` yet
        assert_eq!(config.history.len(), 1);
        let Some(history) = config.history.first() else {
            panic!("expected one history period");
        };
        assert_eq!(history.on_demand_used.as_ref().unwrap().val, 120);
    }

    #[test]
    fn cent_serializes_as_val_field() {
        let c = Cent { val: 4299 };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json, serde_json::json!({"val": 4299}));
    }
}
