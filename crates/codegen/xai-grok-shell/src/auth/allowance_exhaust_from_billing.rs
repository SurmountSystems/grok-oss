//! Leave SuperGrok when included weekly/monthly allowance is full (billing %).
//!
//! When included SuperGrok usage reports fully used and a console API key
//! failover path exists, mark the session JWT fingerprint out of allowance so
//! the sampler prefers the console key **before** the next request — without
//! waiting for HTTP 402 (extras would still succeed on SuperGrok and burn paid
//! balance).
//!
//! Also holds a process-local map of **included** headroom + `reset_at` per
//! SuperGrok identity (from billing polls). `load_supergrok_session_candidates`
//! merges that into ranking when present. Honest absence when never polled.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use super::dual_auth_status::collect_dual_auth_status;
use super::model::{API_KEY_SCOPE, AuthMode};
use super::storage::read_auth_json;
use super::supergrok_identity_rank::{
    IncludedBillingFields, enrich_candidates_with_included_billing, reset_at_from_period_end,
};

/// Process-local included-billing snapshots keyed by SuperGrok identity_id.
///
/// Filled when billing returns usage % / period end for a known principal.
/// Not durable across process restarts (next poll re-fills). Never stores tokens.
static INCLUDED_BILLING_BY_IDENTITY: Mutex<BTreeMap<String, IncludedBillingFields>> =
    Mutex::new(BTreeMap::new());

/// Remember included usage + optional reset for one SuperGrok principal.
///
/// Pure-ish side effect on process cache only. `period_end_rfc3339` is parsed
/// when present; unparseable / empty → leave prior `reset_at` or `None`.
/// `period_type` is the billing proto name (`USAGE_PERIOD_TYPE_WEEKLY`, …)
/// when known; empty/None leaves any prior value.
/// Does **not** clear a prior `prepaid_balance_cents` (use
/// [`remember_supergrok_dollar_extras`] after a full credits config parse).
pub fn remember_supergrok_included_billing(
    identity_id: &str,
    usage_pct: f64,
    period_end_rfc3339: Option<&str>,
    period_type: Option<&str>,
) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let reset_at = period_end_rfc3339.and_then(reset_at_from_period_end);
    let mut map = INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(id.to_owned()).or_insert(IncludedBillingFields {
        usage_pct: None,
        reset_at: None,
        period_type: None,
        prepaid_balance_cents: None,
    });
    entry.usage_pct = Some(usage_pct);
    if let Some(r) = reset_at {
        entry.reset_at = Some(r);
    }
    if let Some(pt) = period_type.map(str::trim).filter(|s| !s.is_empty()) {
        entry.period_type = Some(pt.to_owned());
    }
}

/// Remember SuperGrok Extra Usage Credits (`prepaidBalance`) for one principal.
///
/// Process cache only. Signed cents as returned by billing (UI takes abs).
/// Distinct from console team prepaid. Call after a successful credits poll
/// so dual `/limits` can show sibling dollar extras without inventing $.
pub fn remember_supergrok_dollar_extras(identity_id: &str, prepaid_balance_cents: i64) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let mut map = INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(id.to_owned()).or_insert(IncludedBillingFields {
        usage_pct: None,
        reset_at: None,
        period_type: None,
        prepaid_balance_cents: None,
    });
    entry.prepaid_balance_cents = Some(prepaid_balance_cents);
}

/// Remember included billing for the **active** SuperGrok session (base token).
///
/// Resolves identity_id from `auth.json` for the first SuperGrok session token.
/// No-op when no session is stored. Used from billing fetch so ranking can see
/// live `usage_pct` + `reset_at` for the principal that was polled.
pub fn remember_active_supergrok_included_billing(
    grok_home: &Path,
    usage_pct: f64,
    period_end_rfc3339: Option<&str>,
    period_type: Option<&str>,
) {
    let Some(identity_id) = active_supergrok_identity_id(grok_home) else {
        return;
    };
    remember_supergrok_included_billing(&identity_id, usage_pct, period_end_rfc3339, period_type);
}

/// Identity id of the first SuperGrok session in `auth.json` (active/base first).
pub fn active_supergrok_identity_id(grok_home: &Path) -> Option<String> {
    use super::model::{is_supergrok_session_mode, supergrok_identity_id_from_auth};

    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok()?;
    // Prefer multi-slot / base sessions already ordered via candidates load.
    // First non-empty SuperGrok token's identity (map iteration is BTree by scope).
    // Prefer the base-active scope when present: scopes without `::personal` /
    // `::team::` suffix are the AuthManager primary.
    let mut base: Option<String> = None;
    let mut any: Option<String> = None;
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        if !is_supergrok_session_mode(auth.auth_mode) {
            continue;
        }
        if auth.key.trim().is_empty() {
            continue;
        }
        let id = supergrok_identity_id_from_auth(auth, scope);
        let is_multi = scope.contains("::personal") || scope.contains("::team::");
        if !is_multi {
            base = Some(id);
            break;
        }
        if any.is_none() {
            any = Some(id);
        }
    }
    base.or(any)
}

/// Session credentials needed to poll included SuperGrok billing for one principal.
///
/// Used when dual SuperGrok principals exist so the **non-active** identity can
/// be polled on the same included-safe credits endpoint as the active session.
/// Never log `access_token` (custom [`Debug`] redacts it).
#[derive(Clone)]
pub struct SupergrokBillingPollTarget {
    pub identity_id: String,
    /// Access token for `Authorization: Bearer` (secret — do not log).
    pub access_token: String,
    /// `x-userid` header value from the stored session.
    pub user_id: String,
}

impl std::fmt::Debug for SupergrokBillingPollTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Redact the raw JWT / session token so `:?` / panic formatting cannot
        // leak credentials into logs. Fingerprint first/last 4 of non-empty.
        let token_dbg = if self.access_token.is_empty() {
            "<empty>".to_owned()
        } else if self.access_token.len() <= 8 {
            "***".to_owned()
        } else {
            let t = &self.access_token;
            format!("{}...{} (len={})", &t[..4], &t[t.len() - 4..], t.len())
        };
        f.debug_struct("SupergrokBillingPollTarget")
            .field("identity_id", &self.identity_id)
            .field("access_token", &token_dbg)
            .field("user_id", &self.user_id)
            .finish()
    }
}

/// All SuperGrok principals that can be billed (deduped by identity_id).
///
/// Prefer multi-slot scopes over a duplicate base for the same identity.
/// Empty when no SuperGrok session tokens are stored.
pub fn load_supergrok_billing_poll_targets(grok_home: &Path) -> Vec<SupergrokBillingPollTarget> {
    use super::model::{is_supergrok_session_mode, supergrok_identity_id_from_auth};

    let path = grok_home.join("auth.json");
    let Ok(map) = read_auth_json(&path) else {
        return Vec::new();
    };
    // identity_id → (is_multi, target)
    let mut by_id: BTreeMap<String, (bool, SupergrokBillingPollTarget)> = BTreeMap::new();
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        if !is_supergrok_session_mode(auth.auth_mode) {
            continue;
        }
        let token = auth.key.trim();
        if token.is_empty() {
            continue;
        }
        let identity_id = supergrok_identity_id_from_auth(auth, scope);
        let is_multi = scope.contains("::personal") || scope.contains("::team::");
        let target = SupergrokBillingPollTarget {
            identity_id: identity_id.clone(),
            access_token: token.to_owned(),
            user_id: auth.user_id.clone(),
        };
        match by_id.get(&identity_id) {
            None => {
                by_id.insert(identity_id, (is_multi, target));
            }
            Some((prev_multi, _)) => {
                if is_multi && !*prev_multi {
                    by_id.insert(identity_id, (is_multi, target));
                }
            }
        }
    }
    by_id.into_values().map(|(_, t)| t).collect()
}

/// SuperGrok principals other than the active base identity.
///
/// When only one principal exists, returns empty (active path already polls).
/// When two (or more) exist, returns the sibling(s) so billing refresh can
/// remember their included % + reset without inventing scrape pipelines.
pub fn load_non_active_supergrok_billing_poll_targets(
    grok_home: &Path,
) -> Vec<SupergrokBillingPollTarget> {
    let active = active_supergrok_identity_id(grok_home);
    load_supergrok_billing_poll_targets(grok_home)
        .into_iter()
        .filter(|t| active.as_deref() != Some(t.identity_id.as_str()))
        .collect()
}

/// Snapshot of the process included-billing map (for tests / limits fill).
pub fn included_billing_fields_snapshot() -> BTreeMap<String, IncludedBillingFields> {
    INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Clear process included-billing cache (tests).
pub fn clear_included_billing_cache() {
    INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
}

/// Load the SuperGrok/session access token from `auth.json` (OIDC or External).
///
/// Skips API-key and legacy WebLogin scopes. Returns the first non-empty key.
/// Used only to fingerprint for the exhausted-identity memo — never logged.
pub fn load_session_access_token(grok_home: &Path) -> Option<String> {
    load_all_session_access_tokens(grok_home)
        .into_iter()
        .next()
        .map(|(_, token)| token)
}

/// All SuperGrok/session access tokens from `auth.json` (OIDC or External).
///
/// Each entry is `(scope_key, access_token)`. Skips API-key and WebLogin.
/// Order is map iteration order (BTreeMap by scope). Dogfood still usually has
/// one OIDC scope; multi-login can populate two.
pub fn load_all_session_access_tokens(grok_home: &Path) -> Vec<(String, String)> {
    let path = grok_home.join("auth.json");
    let Ok(map) = read_auth_json(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        match auth.auth_mode {
            AuthMode::Oidc | AuthMode::External => {
                let k = auth.key.trim();
                if !k.is_empty() {
                    out.push((scope.clone(), k.to_owned()));
                }
            }
            AuthMode::ApiKey | AuthMode::WebLogin => continue,
        }
    }
    out
}

/// Whether `next` should replace `prev` when both map to the same SuperGrok
/// identity_id (base active + multi-slot for one principal).
///
/// Preference (routing correctness under `auto_use_included_limits`):
/// 1. Live (not memoized out of allowance) over exhausted.
/// 2. Later `expires_at` (fresh refresh) over earlier.
/// 3. Later `create_time` when expiry is tied/unknown.
/// 4. Multi-slot over base only as a weak tie-break (store shape).
///
/// Dogfood bug: stale exhausted multi-slot won over a refreshed base SuperGrok
/// Heavy JWT → ranking treated SuperGrok as dead and stuck on console API.
fn prefer_supergrok_store_entry(
    prev: &super::model::GrokAuth,
    prev_is_multi: bool,
    next: &super::model::GrokAuth,
    next_is_multi: bool,
) -> bool {
    let prev_exh = xai_grok_sampler::is_credential_exhausted(prev.key.trim());
    let next_exh = xai_grok_sampler::is_credential_exhausted(next.key.trim());
    if prev_exh != next_exh {
        return !next_exh;
    }
    match (prev.expires_at, next.expires_at) {
        (Some(a), Some(b)) if a != b => return b > a,
        (None, Some(_)) => return true,
        (Some(_), None) => return false,
        _ => {}
    }
    if next.create_time != prev.create_time {
        return next.create_time > prev.create_time;
    }
    next_is_multi && !prev_is_multi
}

/// Build SuperGrok session candidates for auto ranking from `auth.json`.
///
/// Default remaining: `0` when the token fingerprint is memoized exhausted,
/// **or the JWT is hard-expired on the wall clock**, else `1` (unknown headroom
/// still treated as "try SuperGrok included first"). Hard-expired multi-slots
/// must not rank as live included headroom ahead of another live SuperGrok
/// principal (or silently queue a dead JWT as primary while console sits
/// ready). When billing has been remembered for an identity
/// ([`remember_supergrok_included_billing`]), remaining comes from usage % and
/// `reset_at` from the period end (honest `None` when never polled / unparseable).
///
/// Dedupes by `identity_id` so base active + multi-slot for the same principal
/// count once. When both exist, prefer the **live / fresher** token (not a
/// stale multi-slot that was left behind after base refresh).
pub fn load_supergrok_session_candidates(
    grok_home: &Path,
) -> Vec<super::supergrok_identity_rank::SupergrokSessionCandidate> {
    use super::model::{
        GrokAuth, is_expired_with_buffer, is_supergrok_session_mode,
        supergrok_identity_id_from_auth,
    };
    use super::supergrok_identity_rank::{
        SupergrokIdentityHeadroom, SupergrokSessionCandidate, role_from_session_fields,
    };
    use chrono::Duration;

    let path = grok_home.join("auth.json");
    let Ok(map) = read_auth_json(&path) else {
        return Vec::new();
    };
    // identity_id → (is_multi, auth chosen for that principal).
    let mut by_id: BTreeMap<String, (bool, GrokAuth)> = BTreeMap::new();
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        if !is_supergrok_session_mode(auth.auth_mode) {
            continue;
        }
        let token = auth.key.trim();
        if token.is_empty() {
            continue;
        }
        let identity_id = supergrok_identity_id_from_auth(auth, scope);
        let is_multi = scope.contains("::personal") || scope.contains("::team::");
        match by_id.get(&identity_id) {
            None => {
                by_id.insert(identity_id, (is_multi, auth.clone()));
            }
            Some((prev_multi, prev_auth)) => {
                if prefer_supergrok_store_entry(prev_auth, *prev_multi, auth, is_multi) {
                    by_id.insert(identity_id, (is_multi, auth.clone()));
                }
            }
        }
    }
    // Tokens hard-expired on wall clock (for billing enrich force-zero).
    let mut hard_expired_tokens: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut candidates: Vec<SupergrokSessionCandidate> = by_id
        .into_iter()
        .map(|(identity_id, (_is_multi, auth))| {
            let token = auth.key.trim();
            // Hard wall-clock expiry (no early-invalidation buffer): a JWT that
            // the wire would reject must not count as included headroom.
            let hard_expired = is_expired_with_buffer(&auth, Duration::zero());
            if hard_expired {
                hard_expired_tokens.insert(token.to_owned());
            }
            let remaining = if hard_expired || xai_grok_sampler::is_credential_exhausted(token) {
                0
            } else {
                1
            };
            let role =
                role_from_session_fields(auth.principal_type.as_deref(), auth.team_id.as_deref());
            SupergrokSessionCandidate {
                headroom: SupergrokIdentityHeadroom {
                    identity_id,
                    role,
                    included_remaining: remaining,
                    reset_at: None,
                },
                access_token: token.to_owned(),
            }
        })
        .collect();
    let billing = included_billing_fields_snapshot();
    if !billing.is_empty() {
        // Billing usage % must not resurrect a hard-expired multi-slot as
        // "included headroom" (personal % can still poll for a dead JWT).
        enrich_candidates_with_included_billing(&mut candidates, &billing, |tok| {
            let t = tok.trim();
            xai_grok_sampler::is_credential_exhausted(t) || hard_expired_tokens.contains(t)
        });
    }
    candidates
}

/// Apply billing `usage_pct` to the credit-exhausted memo when dual-auth is ready.
///
/// Also remembers included headroom for the active SuperGrok identity so
/// `auto_use_included_limits` ranking can use live usage (reset_at still
/// needs [`remember_active_supergrok_included_billing`] with a period end).
///
/// Safe no-op when session or console failover is missing. See
/// [`xai_grok_sampler::sync_allowance_exhaust_from_usage`].
pub fn apply_billing_usage_to_session_exhaust(
    usage_pct: f64,
    grok_home: &Path,
) -> xai_grok_sampler::AllowanceExhaustAction {
    apply_billing_usage_to_session_exhaust_with_period(usage_pct, grok_home, None)
}

/// Like [`apply_billing_usage_to_session_exhaust`] with optional period-end
/// RFC 3339 for ranking `reset_at`.
pub fn apply_billing_usage_to_session_exhaust_with_period(
    usage_pct: f64,
    grok_home: &Path,
    period_end_rfc3339: Option<&str>,
) -> xai_grok_sampler::AllowanceExhaustAction {
    // Feed ranking even when dual-auth is not ready (multi SuperGrok alone).
    // Period type unknown on this path (usage-only callers); leave prior or None.
    remember_active_supergrok_included_billing(grok_home, usage_pct, period_end_rfc3339, None);

    let status = collect_dual_auth_status(grok_home);
    if !status.dual_auth_ready() {
        // Still allow clear of a prior mark if usage dropped but console key
        // was removed — only when we can fingerprint the session.
        let Some(token) = load_session_access_token(grok_home) else {
            return xai_grok_sampler::AllowanceExhaustAction::None;
        };
        return xai_grok_sampler::sync_allowance_exhaust_from_usage(
            usage_pct,
            Some(token.as_str()),
            false,
        );
    }
    let token = load_session_access_token(grok_home);
    let action =
        xai_grok_sampler::sync_allowance_exhaust_from_usage(usage_pct, token.as_deref(), true);
    if matches!(action, xai_grok_sampler::AllowanceExhaustAction::Marked) {
        tracing::info!(
            target: "xai_grok_shell::auth",
            usage_pct,
            "SuperGrok included usage full; remembering session out of allowance so next request uses console key"
        );
    }
    action
}

/// True when SuperGrok session is memoized out of allowance and a console
/// failover path is ready (dual-auth).
///
/// Used by the pager meter so silent sticky prefer_live (console live without
/// hop toast) does not keep showing SuperGrok prepaid extras as the spend pool.
pub fn supergrok_out_of_allowance_with_console_ready(grok_home: &Path) -> bool {
    let status = collect_dual_auth_status(grok_home);
    if !status.dual_auth_ready() {
        return false;
    }
    let Some(token) = load_session_access_token(grok_home) else {
        return false;
    };
    xai_grok_sampler::is_credential_exhausted(&token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::credentials_store::{CredentialsStore, FORCE_FILE_ENV};
    use crate::auth::model::{AuthStore, GrokAuth};
    use crate::auth::storage::write_auth_json;
    use crate::auth::xai_console::add_console_api_key;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use xai_grok_sampler::{AllowanceExhaustAction, clear_all_including_durable};
    use xai_grok_test_support::EnvGuard;

    /// Serialize tests that touch the process-global exhausted memo + env.
    fn with_isolated_home<R>(f: impl FnOnce(&Path) -> R) -> R {
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().expect("temp GROK_HOME");
        let _home = EnvGuard::set("GROK_HOME", dir.path());
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        // Operator host may have XAI_API_KEY set (env wins) — isolate tests.
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        clear_all_including_durable();
        let out = f(dir.path());
        clear_all_including_durable();
        out
    }

    /// Named contract: sibling/full credits poll can remember SuperGrok Extra
    /// Usage Credits (`prepaidBalance`) without inventing console team $.
    #[test]
    fn remember_dollar_extras_stores_prepaid_cents_for_limits_fill() {
        clear_included_billing_cache();
        remember_supergrok_included_billing(
            "team-surmount",
            65.0,
            Some("2026-08-04T01:25:32Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_dollar_extras("team-surmount", 10029);
        let snap = included_billing_fields_snapshot();
        let fields = snap.get("team-surmount").expect("remembered identity");
        assert_eq!(fields.usage_pct, Some(65.0));
        assert_eq!(fields.prepaid_balance_cents, Some(10029));
        // Second remember of included must not wipe prepaid.
        remember_supergrok_included_billing("team-surmount", 70.0, None, None);
        let snap2 = included_billing_fields_snapshot();
        assert_eq!(
            snap2.get("team-surmount").unwrap().prepaid_balance_cents,
            Some(10029),
            "included re-remember must keep prepaidBalance"
        );
        assert_eq!(snap2.get("team-surmount").unwrap().usage_pct, Some(70.0));
        clear_included_billing_cache();
    }

    fn write_oidc(home: &Path, key: &str) {
        let path = home.join("auth.json");
        let mut map = AuthStore::default();
        map.insert(
            "https://auth.x.ai::test-client".to_owned(),
            GrokAuth {
                key: key.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-1".into(),
                ..Default::default()
            },
        );
        write_auth_json(&path, &map).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn apply_billing_100_pct_marks_session_when_dual_auth_ready() {
        with_isolated_home(|home| {
            let session = "session-jwt-for-allowance-exhaust";
            write_oidc(home, session);
            // File-backed store (same path dual_auth_status probes under FORCE_FILE).
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::Marked
            );
            // Cleared only fires when a prior mark existed → proves mark.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(12.0, home),
                AllowanceExhaustAction::Cleared
            );
            // Second clear is a no-op.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(5.0, home),
                AllowanceExhaustAction::None
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn apply_billing_session_only_does_not_mark() {
        with_isolated_home(|home| {
            let session = "session-only-no-console";
            write_oidc(home, session);
            // No console key + env cleared → do not mark SuperGrok out.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::None
            );
        });
    }

    #[test]
    fn load_session_skips_api_key_scope() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        let mut map = AuthStore::default();
        map.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "console-as-api-key-scope".into(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        write_auth_json(&path, &map).unwrap();
        assert!(load_session_access_token(dir.path()).is_none());
    }

    /// Named contract (Business / SuperGrok Heavy routing): when base holds a
    /// **live** SuperGrok JWT and the multi-slot for the same principal still
    /// has a stale token memoized out of allowance, ranking must use the live
    /// base token — not treat SuperGrok as exhausted and hand primary to console.
    ///
    /// Dogfood: refreshed SuperGrok Heavy (tier 5) on base + exhausted multi-slot
    /// + `auto_use_included_limits` silently stayed on console Business API.
    #[test]
    #[serial_test::serial]
    fn load_candidates_prefers_live_base_over_stale_exhausted_multi_slot() {
        use crate::auth::model::{SUPERGROK_PERSONAL_MULTI_SLOT, multi_slot_scope_for_auth};
        use crate::auth::supergrok_identity_rank::order_credentials_for_preferred_auto;
        use chrono::{Duration, Utc};
        use xai_grok_sampler::{clear_all_including_durable, sync_allowance_exhaust_from_usage};

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::heavy-client";
        let stale = "tok-stale-exhausted-multi-slot";
        let live = "tok-live-supergrok-heavy-base";
        // dual-auth ready + 100% → mark stale SuperGrok fingerprint out of allowance
        let _ = sync_allowance_exhaust_from_usage(100.0, Some(stale), true);

        let now = Utc::now();
        let mut map = AuthStore::default();
        // Stale multi-slot (older expiry, exhausted memo) + fresher base active.
        // Manual inserts on purpose: upsert would keep them in lockstep.
        let multi_key = format!("{base}::{SUPERGROK_PERSONAL_MULTI_SLOT}");
        map.insert(
            multi_key,
            GrokAuth {
                key: stale.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-heavy".into(),
                principal_type: Some("User".into()),
                team_id: Some("team-workplace".into()),
                create_time: now - Duration::hours(6),
                expires_at: Some(now - Duration::minutes(5)),
                ..Default::default()
            },
        );
        map.insert(
            base.to_owned(),
            GrokAuth {
                key: live.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-heavy".into(),
                principal_type: Some("User".into()),
                team_id: Some("team-workplace".into()),
                create_time: now,
                expires_at: Some(now + Duration::hours(6)),
                ..Default::default()
            },
        );
        // Sanity: multi-slot helper names match store shape.
        let _ = multi_slot_scope_for_auth(base, map.get(base).expect("base"));
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(
            candidates.len(),
            1,
            "same identity once; got {:?}",
            candidates
                .iter()
                .map(|c| (&c.headroom.identity_id, c.access_token.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            candidates[0].access_token.as_str(),
            live,
            "must pick live SuperGrok Heavy base JWT, not exhausted multi-slot"
        );
        assert!(
            candidates[0].headroom.included_remaining > 0,
            "live token must not inherit exhausted remaining=0"
        );

        let order = order_credentials_for_preferred_auto(&candidates, &["console-biz-key".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some(live),
            "auto rank primary must be SuperGrok Heavy session, not console"
        );
        assert!(
            order.primary_is_supergrok_included,
            "SuperGrok included primary (not console Business API)"
        );
        assert!(
            !order.failover.iter().any(|k| k == "console-biz-key"),
            "console must be omitted from failover while SuperGrok included headroom remains: {:?}",
            order.failover
        );

        clear_all_including_durable();
        clear_included_billing_cache();
    }

    /// Business SuperGrok (Team principal) live base wins over exhausted multi-slot.
    #[test]
    #[serial_test::serial]
    fn load_candidates_prefers_live_business_base_over_exhausted_team_multi_slot() {
        use chrono::{Duration, Utc};
        use xai_grok_sampler::{clear_all_including_durable, sync_allowance_exhaust_from_usage};

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::biz-heavy";
        let stale = "tok-biz-stale-exhausted";
        let live = "tok-biz-live-heavy";
        let _ = sync_allowance_exhaust_from_usage(100.0, Some(stale), true);
        let now = Utc::now();
        let mut map = AuthStore::default();
        let team_id = "team-surmount-biz";
        map.insert(
            format!("{base}::team::{team_id}"),
            GrokAuth {
                key: stale.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b".into(),
                principal_type: Some("Team".into()),
                team_id: Some(team_id.into()),
                create_time: now - Duration::hours(3),
                expires_at: Some(now - Duration::minutes(1)),
                ..Default::default()
            },
        );
        map.insert(
            base.to_owned(),
            GrokAuth {
                key: live.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b".into(),
                principal_type: Some("Team".into()),
                team_id: Some(team_id.into()),
                create_time: now,
                expires_at: Some(now + Duration::hours(5)),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].access_token.as_str(), live);
        assert_eq!(
            candidates[0].headroom.role,
            crate::auth::SupergrokAccountRole::Business
        );
        assert!(candidates[0].headroom.included_remaining > 0);

        clear_all_including_durable();
        clear_included_billing_cache();
    }

    /// Dogfood (2026-08-01): live Team SuperGrok + hard-expired personal multi-slot
    /// + console key + included usage well below 100% must keep SuperGrok session
    /// primary (not console). Expired personal must not rank as included headroom.
    ///
    /// Named contract: while SuperGrok included headroom remains on a live
    /// session JWT, `auto_use_included_limits` must not prefer the console key
    /// just because a second SuperGrok multi-slot is dead on the wall clock.
    #[test]
    #[serial_test::serial]
    fn load_candidates_expired_personal_does_not_push_live_team_to_console() {
        use crate::auth::supergrok_identity_rank::order_credentials_for_preferred_auto;
        use chrono::{Duration, Utc};
        use xai_grok_sampler::clear_all_including_durable;

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let now = Utc::now();
        let team_id = "team-surmount-live";
        let personal_teamish = "personal-default-team";
        let live_team = "tok-live-team-supergrok";
        let dead_personal = "tok-expired-personal-multi";
        let base = "https://auth.x.ai::dogfood-client";

        let mut map = AuthStore::default();
        // Live Team SuperGrok (base + team multi-slot same token).
        map.insert(
            base.to_owned(),
            GrokAuth {
                key: live_team.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-dogfood".into(),
                principal_type: Some("Team".into()),
                team_id: Some(team_id.into()),
                team_name: Some("Surmount".into()),
                create_time: now,
                expires_at: Some(now + Duration::hours(6)),
                ..Default::default()
            },
        );
        map.insert(
            format!("{base}::team::{team_id}"),
            GrokAuth {
                key: live_team.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-dogfood".into(),
                principal_type: Some("Team".into()),
                team_id: Some(team_id.into()),
                team_name: Some("Surmount".into()),
                create_time: now,
                expires_at: Some(now + Duration::hours(6)),
                ..Default::default()
            },
        );
        // Hard-expired personal multi-slot (different identity_id via team_id).
        map.insert(
            format!("{base}::personal"),
            GrokAuth {
                key: dead_personal.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-dogfood".into(),
                principal_type: Some("User".into()),
                team_id: Some(personal_teamish.into()),
                create_time: now - Duration::hours(20),
                expires_at: Some(now - Duration::hours(12)),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        // Billing says 65% used (included headroom remains) for both identities.
        remember_supergrok_included_billing(team_id, 65.0, None, Some("USAGE_PERIOD_TYPE_WEEKLY"));
        remember_supergrok_included_billing(
            personal_teamish,
            65.0,
            None,
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(
            candidates.len(),
            2,
            "Team + personal are distinct SuperGrok identities"
        );
        let team = candidates
            .iter()
            .find(|c| c.headroom.identity_id == team_id)
            .expect("live Team SuperGrok candidate");
        let personal = candidates
            .iter()
            .find(|c| c.headroom.identity_id == personal_teamish)
            .expect("expired personal candidate still listed");
        assert_eq!(team.access_token.as_str(), live_team);
        assert!(
            team.headroom.included_remaining > 0,
            "live Team with usage 65% must keep included headroom"
        );
        assert_eq!(
            personal.headroom.included_remaining, 0,
            "hard-expired personal multi-slot must not rank as included headroom \
             even when billing still reports 65% for that pool"
        );

        let order = order_credentials_for_preferred_auto(&candidates, &["console-team-key".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some(live_team),
            "primary must stay live SuperGrok session, not console; got {:?}",
            order.primary
        );
        assert!(
            order.primary_is_supergrok_included,
            "must not ExhaustedAll → console while live SuperGrok has headroom"
        );
        assert!(
            !order.failover.iter().any(|k| k == dead_personal),
            "expired personal JWT must not sit in failover: {:?}",
            order.failover
        );
        assert!(
            !order.failover.iter().any(|k| k == "console-team-key"),
            "console must be omitted from failover while live SuperGrok has included headroom: {:?}",
            order.failover
        );

        clear_all_including_durable();
        clear_included_billing_cache();
    }

    /// Hermetic: two SuperGrok principals in auth.json load as two rank candidates
    /// (deduped; not doubled by base + multi-slot).
    #[test]
    fn load_supergrok_candidates_two_principals_deduped() {
        use crate::auth::model::upsert_supergrok_session;
        use crate::auth::supergrok_identity_rank::{
            SupergrokAccountRole, order_credentials_for_preferred_auto,
            pick_supergrok_identity_for_auto,
        };

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::rank-client";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-personal-included".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-business-included".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-biz".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(
            candidates.len(),
            2,
            "personal + business once each (not base+slot duplicates); got {:?}",
            candidates
                .iter()
                .map(|c| c.headroom.identity_id.as_str())
                .collect::<Vec<_>>()
        );
        let roles: Vec<_> = candidates.iter().map(|c| c.headroom.role).collect();
        assert!(roles.contains(&SupergrokAccountRole::Personal));
        assert!(roles.contains(&SupergrokAccountRole::Business));

        // Both have headroom (1); ranking picks stable identity_id order among live
        // (reset_at both None → identity_id lex).
        let headrooms: Vec<_> = candidates.iter().map(|c| c.headroom.clone()).collect();
        let pick = pick_supergrok_identity_for_auto(&headrooms);
        assert!(
            matches!(pick, crate::auth::PickSupergrokForAuto::Use { .. }),
            "{pick:?}"
        );

        let order = order_credentials_for_preferred_auto(&candidates, &["console-k".into()]);
        assert!(
            order.primary_is_supergrok_included,
            "included SuperGrok before console"
        );
        assert_ne!(order.primary.as_deref(), Some("console-k"));
        assert!(
            !order.failover.iter().any(|k| k == "console-k"),
            "console omitted while SuperGrok included headroom remains: {:?}",
            order.failover
        );
    }

    /// Remembered billing headroom + reset_at flow into load → rank order.
    #[test]
    #[serial_test::serial]
    fn load_candidates_picks_up_remembered_billing_sooner_reset() {
        use crate::auth::model::upsert_supergrok_session;
        use crate::auth::supergrok_identity_rank::{
            SupergrokAccountRole, order_credentials_for_preferred_auto,
        };
        use xai_grok_sampler::clear_all_including_durable;

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::billing-rank";
        let mut map = AuthStore::default();
        // Personal (user-p) resets later; business (team-biz) sooner.
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-p-bill".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-b-bill".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-biz".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        // identity_id for personal = user-p; business = team-biz
        remember_supergrok_included_billing(
            "user-p",
            40.0,
            Some("2026-08-01T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_included_billing(
            "team-biz",
            80.0,
            Some("2026-07-30T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(candidates.len(), 2);
        let by_id: std::collections::BTreeMap<_, _> = candidates
            .iter()
            .map(|c| (c.headroom.identity_id.as_str(), c))
            .collect();
        let personal = by_id.get("user-p").expect("personal");
        let business = by_id.get("team-biz").expect("business");
        assert!(personal.headroom.included_remaining > 0);
        assert!(business.headroom.included_remaining > 0);
        assert!(personal.headroom.reset_at.is_some());
        assert!(business.headroom.reset_at.is_some());
        assert!(
            business.headroom.reset_at.unwrap() < personal.headroom.reset_at.unwrap(),
            "fixture: business resets sooner"
        );

        let order = order_credentials_for_preferred_auto(&candidates, &["console-k".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-b-bill"),
            "sooner-reset business before personal; got {:?}",
            order
        );
        assert!(order.primary_is_supergrok_included);

        clear_included_billing_cache();
        clear_all_including_durable();
    }

    #[test]
    #[serial_test::serial]
    fn out_of_allowance_helper_true_when_marked_and_dual_auth() {
        with_isolated_home(|home| {
            let session = "session-jwt-for-meter-honesty";
            write_oidc(home, session);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-for-meter").unwrap());
            assert!(
                !supergrok_out_of_allowance_with_console_ready(home),
                "not marked yet"
            );
            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::Marked
            );
            assert!(
                supergrok_out_of_allowance_with_console_ready(home),
                "after mark + dual-auth, meter must treat SuperGrok as out"
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn out_of_allowance_helper_false_without_console() {
        with_isolated_home(|home| {
            write_oidc(home, "session-only-meter");
            // No console key → dual_auth_ready false even if we somehow marked.
            assert!(
                !supergrok_out_of_allowance_with_console_ready(home),
                "session-only must not claim console-ready exhaust for meter"
            );
        });
    }

    /// Debug must not print the raw access token (security hygiene).
    #[test]
    fn supergrok_billing_poll_target_debug_redacts_access_token() {
        let target = SupergrokBillingPollTarget {
            identity_id: "user-p".into(),
            access_token: "super-secret-jwt-token-value-xyz".into(),
            user_id: "uid-1".into(),
        };
        let dbg = format!("{target:?}");
        assert!(
            !dbg.contains("super-secret-jwt-token-value-xyz"),
            "raw token must not appear in Debug: {dbg}"
        );
        assert!(dbg.contains("user-p"), "identity_id still visible: {dbg}");
        assert!(
            dbg.contains("access_token") && (dbg.contains("***") || dbg.contains("...")),
            "redacted token field expected: {dbg}"
        );
        // Short tokens fully masked.
        let short = SupergrokBillingPollTarget {
            identity_id: "x".into(),
            access_token: "abcd".into(),
            user_id: "u".into(),
        };
        let short_dbg = format!("{short:?}");
        assert!(
            !short_dbg.contains("abcd") || short_dbg.contains("***"),
            "short token redacted: {short_dbg}"
        );
        assert!(!short_dbg.contains("\"abcd\""), "{short_dbg}");
    }

    /// One SuperGrok principal → no non-active billing poll targets.
    #[test]
    fn non_active_poll_targets_empty_when_single_principal() {
        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        write_oidc(dir.path(), "solo-tok");
        let targets = load_non_active_supergrok_billing_poll_targets(dir.path());
        assert!(
            targets.is_empty(),
            "single principal is only the active poll path; got {targets:?}"
        );
        let all = load_supergrok_billing_poll_targets(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].access_token, "solo-tok");
    }

    /// Two SuperGrok principals → non-active list is the sibling (not active base).
    #[test]
    fn non_active_poll_targets_returns_sibling_when_dual_principals() {
        use crate::auth::model::upsert_supergrok_session;

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::sibling-poll";
        let mut map = AuthStore::default();
        // First upsert = personal; second = business (active base = last upsert).
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-personal-sibling".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-sib".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-business-sibling".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-sib".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-sib".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let all = load_supergrok_billing_poll_targets(dir.path());
        assert_eq!(all.len(), 2, "personal + business poll targets");

        let active = active_supergrok_identity_id(dir.path()).expect("active id");
        let non_active = load_non_active_supergrok_billing_poll_targets(dir.path());
        assert_eq!(
            non_active.len(),
            1,
            "exactly one sibling; active={active}; non_active={non_active:?}"
        );
        assert_ne!(
            non_active[0].identity_id, active,
            "sibling must not be the active identity"
        );
        // Sibling must carry a real token + user_id for the credits request.
        assert!(!non_active[0].access_token.is_empty());
        assert!(!non_active[0].user_id.is_empty());
        // Tokens are one of the two we wrote.
        assert!(
            non_active[0].access_token == "tok-personal-sibling"
                || non_active[0].access_token == "tok-business-sibling"
        );
        // Named contract: dual SuperGrok poll targets must not share one JWT.
        // Same token for both slots would paint one principal's credits on both
        // /limits rows (the 62%/62% "mirror" failure mode when wiring is wrong).
        let tokens: Vec<&str> = all.iter().map(|t| t.access_token.as_str()).collect();
        assert_eq!(tokens.len(), 2);
        assert_ne!(
            tokens[0], tokens[1],
            "personal and business must poll with distinct access tokens; got {tokens:?}"
        );
        let personal = all
            .iter()
            .find(|t| t.access_token == "tok-personal-sibling")
            .expect("personal target");
        let business = all
            .iter()
            .find(|t| t.access_token == "tok-business-sibling")
            .expect("business target");
        assert_ne!(
            personal.identity_id, business.identity_id,
            "identity_ids must differ so process cache keys stay per-slot"
        );
    }

    /// Hermetic: remember included billing for both principals → load enriches both.
    #[test]
    #[serial_test::serial]
    fn remember_both_principals_enriches_dual_candidates() {
        use crate::auth::model::upsert_supergrok_session;

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::dual-remember";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-p-rem".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-rem".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-b-rem".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-rem".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-rem".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        // Simulate active poll + non-active sibling poll both remembering.
        remember_supergrok_included_billing(
            "user-p-rem",
            25.0,
            Some("2026-08-10T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_included_billing(
            "team-rem",
            70.0,
            Some("2026-08-01T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );

        let snap = included_billing_fields_snapshot();
        assert!(
            snap.contains_key("user-p-rem") && snap.contains_key("team-rem"),
            "both identities remembered: {snap:?}"
        );
        assert_eq!(snap["user-p-rem"].usage_pct, Some(25.0));
        assert_eq!(snap["team-rem"].usage_pct, Some(70.0));
        assert!(snap["user-p-rem"].reset_at.is_some());
        assert!(snap["team-rem"].reset_at.is_some());

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(candidates.len(), 2);
        let by_id: BTreeMap<_, _> = candidates
            .iter()
            .map(|c| (c.headroom.identity_id.as_str(), c))
            .collect();
        let personal = by_id.get("user-p-rem").expect("personal candidate");
        let business = by_id.get("team-rem").expect("business candidate");
        assert!(
            personal.headroom.included_remaining > 0,
            "personal headroom from 25% usage"
        );
        assert!(
            business.headroom.included_remaining > 0,
            "business headroom from 70% usage"
        );
        assert!(
            business.headroom.reset_at.unwrap() < personal.headroom.reset_at.unwrap(),
            "business resets sooner"
        );

        clear_included_billing_cache();
    }

    /// Hermetic HTTP: dual principals → non-active poll remembers sibling included %.
    ///
    /// Serial: touches process-global `INCLUDED_BILLING_BY_IDENTITY`.
    #[tokio::test]
    #[serial_test::serial]
    async fn poll_non_active_remembers_sibling_included_billing() {
        use crate::auth::model::upsert_supergrok_session;
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_included_billing_cache();

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_h = hits.clone();
        let app = Router::new().route(
            "/billing",
            get(move |req: axum::http::Request<axum::body::Body>| {
                let hits = hits_h.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let auth = req
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    // Sibling personal token gets 40% / early reset; anything else 90%.
                    let (pct, end) = if auth.contains("tok-personal-poll") {
                        (40.0, "2026-07-30T00:00:00Z")
                    } else {
                        (90.0, "2026-08-15T00:00:00Z")
                    };
                    axum::Json(serde_json::json!({
                        "config": {
                            "creditUsagePercent": pct,
                            "currentPeriod": {
                                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                                "end": end
                            }
                        }
                    }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let dir = TempDir::new().unwrap();
        let base_scope = "https://auth.x.ai::poll-e2e";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base_scope,
            GrokAuth {
                key: "tok-personal-poll".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-poll".into(),
                ..Default::default()
            },
        );
        // Second upsert becomes active base (business).
        upsert_supergrok_session(
            &mut map,
            base_scope,
            GrokAuth {
                key: "tok-business-poll".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-poll".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-poll".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let proxy = format!("http://{addr}");
        crate::extensions::billing::poll_and_remember_non_active_supergrok_included_billing(
            dir.path(),
            &proxy,
        )
        .await;

        assert!(
            hits.load(Ordering::SeqCst) >= 1,
            "must hit credits endpoint for sibling"
        );

        let snap = included_billing_fields_snapshot();
        // Active is business (team-poll); non-active is personal (user-p-poll).
        assert!(
            snap.contains_key("user-p-poll"),
            "sibling personal must be remembered; snap={snap:?}"
        );
        assert_eq!(snap["user-p-poll"].usage_pct, Some(40.0));
        assert!(snap["user-p-poll"].reset_at.is_some());
        // Active is not polled by this helper (pager / active path owns it).
        assert!(
            !snap.contains_key("team-poll"),
            "non-active poll must not invent active remember: {snap:?}"
        );

        // Simulate active poll remembering business, then load both.
        remember_supergrok_included_billing(
            "team-poll",
            90.0,
            Some("2026-08-15T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(candidates.len(), 2);
        let by_id: BTreeMap<_, _> = candidates
            .iter()
            .map(|c| {
                (
                    c.headroom.identity_id.as_str(),
                    c.headroom.included_remaining,
                )
            })
            .collect();
        assert!(
            *by_id.get("user-p-poll").unwrap_or(&0) > 0,
            "personal enriched from sibling poll"
        );
        assert!(
            *by_id.get("team-poll").unwrap_or(&0) > 0,
            "business enriched from active remember"
        );
        // Sibling poll must retain period_type so /limits can say "weekly".
        assert_eq!(
            snap["user-p-poll"].period_type.as_deref(),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
            "sibling remember keeps period type for limits copy"
        );

        clear_included_billing_cache();
        server.abort();
    }

    /// Named contract: mock returns **different** included % per Bearer token.
    /// Active (business) + sibling (personal) remember only their own reading —
    /// never paint personal's % onto the business identity (or vice versa).
    #[tokio::test]
    #[serial_test::serial]
    async fn dual_poll_remembers_distinct_pct_per_token_never_cross_paints() {
        use crate::auth::model::upsert_supergrok_session;
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_included_billing_cache();

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_h = hits.clone();
        let app = Router::new().route(
            "/billing",
            get(move |req: axum::http::Request<axum::body::Body>| {
                let hits = hits_h.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let auth = req
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    // Distinct mock results: personal 62%, business 5%.
                    let (pct, end) = if auth.contains("tok-personal-distinct") {
                        (62.0, "2026-08-03T19:25:00Z")
                    } else if auth.contains("tok-business-distinct") {
                        (5.0, "2026-08-10T00:00:00Z")
                    } else {
                        (99.0, "2026-09-01T00:00:00Z")
                    };
                    axum::Json(serde_json::json!({
                        "config": {
                            "creditUsagePercent": pct,
                            "isUnifiedBillingUser": false,
                            "currentPeriod": {
                                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                                "end": end
                            }
                        }
                    }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let dir = TempDir::new().unwrap();
        let base_scope = "https://auth.x.ai::distinct-pct";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base_scope,
            GrokAuth {
                key: "tok-personal-distinct".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-distinct".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base_scope,
            GrokAuth {
                key: "tok-business-distinct".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-distinct".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-distinct".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let proxy = format!("http://{addr}");
        // Sibling (personal) path — uses personal JWT only.
        crate::extensions::billing::poll_and_remember_non_active_supergrok_included_billing(
            dir.path(),
            &proxy,
        )
        .await;
        // Active (business) path: poll with business JWT, remember under active
        // identity_id from auth.json (team-distinct), matching production wire.
        let biz_resp = crate::extensions::billing::fetch_credits_config_with_session(
            &proxy,
            "tok-business-distinct",
            "user-b-distinct",
        )
        .await
        .expect("business credits fetch");
        let biz_cfg = biz_resp.config.as_ref().expect("config");
        let (biz_pct, biz_end) = crate::extensions::billing::included_usage_and_period_end(biz_cfg);
        let biz_pct = biz_pct.expect("business usage %");
        let biz_period = biz_cfg
            .current_period
            .as_ref()
            .and_then(|p| p.period_type.as_deref());
        remember_active_supergrok_included_billing(
            dir.path(),
            biz_pct,
            biz_end.as_deref(),
            biz_period,
        );

        assert!(
            hits.load(Ordering::SeqCst) >= 2,
            "must hit credits for sibling and active"
        );
        assert_eq!(biz_pct, 5.0, "business token → 5%");

        let snap = included_billing_fields_snapshot();
        assert_eq!(
            snap.get("user-p-distinct").and_then(|f| f.usage_pct),
            Some(62.0),
            "personal identity keeps 62% from personal token; snap={snap:?}"
        );
        assert_eq!(
            snap.get("team-distinct").and_then(|f| f.usage_pct),
            Some(5.0),
            "business identity keeps 5% from business token; snap={snap:?}"
        );
        // Cross-paint guard: personal must not hold business %, and vice versa.
        assert_ne!(
            snap["user-p-distinct"].usage_pct,
            snap["team-distinct"].usage_pct
        );

        clear_included_billing_cache();
        server.abort();
    }
}
