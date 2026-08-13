//! `x.ai/billing` extension handler.
//!
//! Fetches the authenticated user's Grok Build billing configuration
//! (credit limit, usage, on-demand cap, billing period, history) from
//! the backend. Used by the pager/desktop to display credits and usage.

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;

/// Billing period cycle identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCycle {
    pub year: i32,
    pub month: i32,
}

/// Cent value from the billing API (USD cents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cent {
    /// proto3 JSON omits zero-valued scalars, so a `$0` Cent arrives as `{}`;
    /// default to 0 rather than failing the whole parse.
    #[serde(default)]
    pub val: i64,
}

/// A usage period (weekly or monthly) from the newer credits config.
///
/// `start`/`end` are RFC 3339 timestamps. `period_type` is the proto enum name
/// (e.g. `USAGE_PERIOD_TYPE_WEEKLY`); kept so callers can distinguish weekly
/// vs monthly cycles.
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

/// Per-product included usage from SuperGrok credits config (`productUsage`).
///
/// Wire examples use proto enum names such as `PRODUCT_GROK_BUILD` with an
/// independent `usagePercent`. Top-level `creditUsagePercent` may stay flat
/// while a product entry moves (or the reverse); keep them distinct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductUsageEntry {
    /// Product id from the wire (e.g. `PRODUCT_GROK_BUILD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Included usage percent for this product when present (0.0–100.0+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_percent: Option<f64>,
}

/// Wire product id for Grok Build (CLI / coding surface).
pub const PRODUCT_GROK_BUILD: &str = "PRODUCT_GROK_BUILD";

/// Current billing configuration for Grok Build coding credits.
///
/// Carries both the newer credits-config fields (`credit_usage_percent`,
/// `current_period`) and the deprecated `GrokBuildBillingConfig` fields
/// (`monthly_limit`, `used`, `billing_period_*`). Consumers should prefer the
/// new fields and fall back to the deprecated ones, so the same struct works
/// against both the new `GetGrokCreditsConfig` and the legacy
/// `GetGrokBuildBillingConfig` backend responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingConfig {
    /// Included credit usage as a percentage of the allowance (0.0–100.0).
    /// Preferred over deriving from `monthly_limit`/`used`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_usage_percent: Option<f64>,
    /// Current usage period (weekly or monthly). Preferred over
    /// `billing_period_start`/`billing_period_end`.
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
    /// Remaining prepaid (purchased) credit balance, positive — the "bought
    /// credits" the user has topped up. Populated from the credits config
    /// (`GetGrokCreditsConfig.prepaid_balance`); absent in the legacy billing
    /// shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepaid_balance: Option<Cent>,
    /// Whether this user is on unified usage billing (shared weekly/monthly
    /// pool). From `GrokCreditsConfig.is_unified_billing_user`, which billing
    /// sets from remote settings `unified_consumer_billing_enabled`. `None` when
    /// absent (legacy `GetGrokBuildBillingConfig` shape or older servers).
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

/// Included usage percent for [`PRODUCT_GROK_BUILD`] when present on wire.
pub fn grok_build_usage_percent(config: &BillingConfig) -> Option<f64> {
    config.product_usage.iter().find_map(|entry| {
        let product = entry.product.as_deref()?;
        if product == PRODUCT_GROK_BUILD {
            entry.usage_percent
        } else {
            None
        }
    })
}

/// Top-level response (primarily from `GET /rest/grok/credits` + auto-topup-rule).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfigResponse {
    pub config: Option<BillingConfig>,
    /// Whether on-demand credit usage is enabled. When `false`, the pager
    /// should hide on-demand controls. Populated from `RemoteSettings`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    /// User-friendly subscription tier name (e.g. "SuperGrok Heavy").
    /// Populated from `RemoteSettings` so the pager can update its cached
    /// tier on every billing fetch without an extra request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_tier: Option<String>,
}

/// Auto top-up configuration (from GetAutoTopupRule).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTopupRule {
    /// proto3 JSON omits `false`, so a disabled rule arrives without this field;
    /// default to `false` rather than failing the parse (which would otherwise
    /// keep a stale cached rule in the pager).
    #[serde(default)]
    pub enabled: bool,
    pub min_before_hitting_sl: Option<Cent>,
    pub topup_amount: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount_per_month: Option<Cent>,
}

/// Wrapper for the auto top-up rule response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutoTopupRuleResponse {
    #[serde(default)]
    pub rule: Option<AutoTopupRule>,
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/billing" => {
            tracing::info!("handling billing config request");
            handle_get_billing(agent).await
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

/// Fetch `GetGrokCreditsConfig` for one SuperGrok session token (included-safe).
///
/// Same CLI proxy path as the active `x.ai/billing` handler:
/// `GET {proxy}/billing?format=credits`. Does not burn SuperGrok dollar extras
/// (not an inference call). Used for non-active dual-principal polls.
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

/// Poll non-active SuperGrok principals and remember their included fields.
///
/// Best-effort: a failed sibling poll does not fail the active billing path.
/// No-op when fewer than two SuperGrok principals are stored.
pub async fn poll_and_remember_non_active_supergrok_included_billing(
    grok_home: &std::path::Path,
    proxy_base: &str,
) {
    let targets = crate::auth::load_non_active_supergrok_billing_poll_targets(grok_home);
    if targets.is_empty() {
        return;
    }
    for target in targets {
        match fetch_credits_config_with_session(proxy_base, &target.access_token, &target.user_id)
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
                    crate::auth::remember_supergrok_dollar_extras(&target.identity_id, prepaid.val);
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
                tracing::debug!(
                    identity_id = %target.identity_id,
                    usage_pct = pct,
                    prepaid = config.prepaid_balance.as_ref().map(|c| c.val),
                    "remembered non-active SuperGrok included + dollar extras billing"
                );
            }
            Err(e) => {
                tracing::debug!(
                    identity_id = %target.identity_id,
                    error = %e,
                    "sibling SuperGrok billing poll failed (active path unchanged)"
                );
            }
        }
    }
}

/// Structured context for unified-log entries from a successful billing fetch.
///
/// Keeps history to a count + the most recent period so `~/.grok/logs/unified.jsonl`
/// stays useful without dumping unbounded period arrays.
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
        // Drop full history array; surface length + latest entry instead.
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

async fn handle_get_billing(agent: &MvpAgent) -> ExtResult {
    let auth = super::auth_gate::require_xai_auth(
        &agent.auth_manager,
        "Authentication required to fetch billing data",
        "Billing data requires auth with grok.com. Run `grok login` to authenticate.",
    )?;

    let proxy_base = agent.cli_chat_proxy_base_url();
    let base = proxy_base.trim_end_matches('/');

    // Credits balance / usage (new billing system) via the CLI proxy, which
    // forwards to the backend `GetGrokCreditsConfig`. Shared with non-active
    // dual-principal polls via [`fetch_credits_config_with_session`].
    let mut billing = match fetch_credits_config_with_session(base, &auth.key, &auth.user_id).await
    {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "billing: upstream request failed");
            xai_grok_telemetry::unified_log::warn(
                "billing: upstream request failed",
                None,
                Some(serde_json::json!({ "error": e })),
            );
            return Err(acp::Error::internal_error().data(e));
        }
    };

    // Enrich with fields from remote settings.
    let rs = agent.cfg.borrow().remote_settings.clone();
    billing.on_demand_enabled = rs.as_ref().and_then(|rs| rs.on_demand_enabled);
    billing.subscription_tier = rs.as_ref().and_then(|rs| {
        rs.subscription_tier_display
            .clone()
            .or_else(|| rs.subscription_tier.clone())
    });

    // Feed active principal into process included-billing cache (ranking + dual
    // /limits). Pager still remembers too; idempotent same values.
    let grok_home = crate::util::grok_home::grok_home();
    if let Some(ref config) = billing.config {
        let (usage_pct, period_end) = included_usage_and_period_end(config);
        if let Some(pct) = usage_pct {
            let period_type = config
                .current_period
                .as_ref()
                .and_then(|p| p.period_type.as_deref());
            crate::auth::remember_active_supergrok_included_billing(
                &grok_home,
                pct,
                period_end.as_deref(),
                period_type,
            );
        }
        // Active principal Extra Usage Credits into process cache (sibling
        // dual-/limits fill + ranking path share one remember map). Prefer the
        // credential that just polled when disk active id is missing.
        if let Some(prepaid) = config.prepaid_balance.as_ref() {
            let id = crate::auth::active_supergrok_identity_id(&grok_home)
                .unwrap_or_else(|| billing_log_identity_from_auth(&auth).0);
            crate::auth::remember_supergrok_dollar_extras(&id, prepaid.val);
        }
    }
    // Dual SuperGrok: also poll non-active principal(s) on the same
    // included-safe credits endpoint so sibling /limits rows fill honestly.
    // Best-effort; failures leave sibling as "no data yet".
    poll_and_remember_non_active_supergrok_included_billing(&grok_home, base).await;

    // Every prompt / /usage / poll path hits `x.ai/billing`; log the fetched
    // credits snapshot so support can correlate limit UX with real balances.
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

    // Auto top-up rule via the CLI proxy, which forwards to the backend
    // `GetAutoTopupRule`.
    let url = format!("{}/auto-topup-rule", base);
    let response = crate::http::shared_client()
        .get(&url)
        .header("Authorization", format!("Bearer {}", &auth.key))
        .header(
            "X-XAI-Token-Auth",
            crate::auth::GrokComConfig::default().token_header,
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

    // Return the upstream response body verbatim (as a JSON value) so /usage
    // can print the exact data from this request unformatted.
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
        // proto3 JSON omits `false` / `0`, so a disabled rule arrives without
        // `enabled` (and zero Cents as `{}`). It must still deserialize (as
        // disabled) rather than erroring — otherwise the pager keeps a stale
        // cached rule.
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
        let period = &config.history[0];
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
        assert_eq!(ctx["onDemandEnabled"], true);
        assert_eq!(ctx["subscriptionTier"], "SuperGrok");
        let config = ctx["config"].as_object().expect("config object");
        assert!(
            config.get("history").is_none(),
            "full history must be collapsed"
        );
        assert_eq!(config["historyLen"], 2);
        assert_eq!(
            config["latestHistory"]["billingCycle"]["month"], 3,
            "latest history period retained"
        );
        assert_eq!(config["creditUsagePercent"], 42.5);
        assert_eq!(config["prepaidBalance"]["val"], 100);
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
        // Newer `GetGrokCreditsConfig` response: percentage-based usage,
        // a typed current period, productUsage, and history keyed by `period`.
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
        assert_eq!(config.on_demand_cap.as_ref().unwrap().val, 5000);
        assert_eq!(config.on_demand_used.as_ref().unwrap().val, 300);
        // Bought (prepaid) credit balance is parsed from the credits config.
        assert_eq!(config.prepaid_balance.as_ref().unwrap().val, 1250);
        assert_eq!(config.is_unified_billing_user, Some(true));
        // productUsage is retained for observability (log / limits surfaces).
        assert_eq!(config.product_usage.len(), 1);
        assert_eq!(
            config.product_usage[0].product.as_deref(),
            Some(PRODUCT_GROK_BUILD)
        );
        assert_eq!(config.product_usage[0].usage_percent, Some(61.2));
        assert_eq!(grok_build_usage_percent(&config), Some(61.2));
        assert_eq!(config.history.len(), 1);
        assert_eq!(config.history[0].on_demand_used.as_ref().unwrap().val, 120);
    }

    #[test]
    fn cent_serializes_as_val_field() {
        let c = Cent { val: 4299 };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json, serde_json::json!({"val": 4299}));
    }
}
