//! Leave SuperGrok when included weekly/monthly allowance is full (billing %)
//! **and** SuperGrok $ extras are gone or unknown.
//!
//! When included SuperGrok usage reports fully used, a console API key failover
//! path exists, and SuperGrok Extra Usage Credits are not a positive after-burner
//! (0 or never observed), mark the session JWT fingerprint out of allowance so
//! the sampler prefers the console key **before** the next request — without
//! waiting for HTTP 402.
//!
//! With `[auth] auto_use_included_limits` and known **positive** SuperGrok $
//! extras, do **not** mark **when every distinct included SuperGrok period
//! pool is exhausted** (after-burner / SuperGrok $ extras before console):
//! ranking keeps SuperGrok session primary and console only as failover.
//! If a sibling SuperGrok login still has included remaining, mark the full
//! identity so prefer_live / rank can hop. Next plan's included beats this
//! plan's never-expiring extras.
//!
//! Also holds a process-local map of **included** headroom + `reset_at` + extras
//! per SuperGrok identity (from billing polls). `load_supergrok_session_candidates`
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

/// Last SuperGrok billing poll result for one principal (process-local).
///
/// No tokens, secrets, or full HTTP bodies. Used so dual `/limits`, doctor, and
/// rank can tell **which** SuperGrok login polled OK vs auth-failed vs other
/// fail, without inventing a successful remember for a dead JWT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupergrokBillingPollOutcomeKind {
    /// Credits poll succeeded for this identity this process.
    Ok,
    /// Auth-class failure (expired JWT, no auth context, 401, …).
    AuthFailed,
    /// Non-auth failure (network, 5xx, parse, …).
    OtherFailed,
    /// Never polled this process (or cleared).
    Never,
}

/// Short process-local poll record (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupergrokBillingPollOutcome {
    pub kind: SupergrokBillingPollOutcomeKind,
    /// Short fail class for notes / doctor (`auth`, `network`, `other`).
    /// `None` when [`SupergrokBillingPollOutcomeKind::Ok`] or `Never`.
    pub error_class: Option<&'static str>,
}

impl SupergrokBillingPollOutcome {
    pub fn never() -> Self {
        Self {
            kind: SupergrokBillingPollOutcomeKind::Never,
            error_class: None,
        }
    }

    pub fn ok() -> Self {
        Self {
            kind: SupergrokBillingPollOutcomeKind::Ok,
            error_class: None,
        }
    }

    pub fn auth_failed() -> Self {
        Self {
            kind: SupergrokBillingPollOutcomeKind::AuthFailed,
            error_class: Some("auth"),
        }
    }

    pub fn other_failed(error_class: &'static str) -> Self {
        Self {
            kind: SupergrokBillingPollOutcomeKind::OtherFailed,
            error_class: Some(error_class),
        }
    }

    pub fn is_ok(self) -> bool {
        self.kind == SupergrokBillingPollOutcomeKind::Ok
    }

    pub fn is_auth_failed(self) -> bool {
        self.kind == SupergrokBillingPollOutcomeKind::AuthFailed
    }
}

/// Process-local last billing poll outcome per SuperGrok identity_id.
///
/// Never stores tokens. Cleared with [`clear_included_billing_cache`].
static POLL_OUTCOME_BY_IDENTITY: Mutex<BTreeMap<String, SupergrokBillingPollOutcome>> =
    Mutex::new(BTreeMap::new());

/// Consecutive auth-class billing poll fails per SuperGrok identity_id.
///
/// Used to demote a sibling from automatic re-poll after
/// [`SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD`] failures without deleting
/// `auth.json` secrets. Cleared with [`clear_included_billing_cache`].
/// Reset to zero on a successful poll for that identity.
static AUTH_FAIL_STREAK_BY_IDENTITY: Mutex<BTreeMap<String, u32>> = Mutex::new(BTreeMap::new());

/// After this many consecutive auth-class SuperGrok billing poll fails for one
/// identity, automatic sibling polls skip that principal until a successful
/// poll resets the streak (or the process cache is cleared).
///
/// Does **not** delete stored OIDC secrets. Active billing path still polls so
/// `/limits` and doctor can surface re-login; only the non-active sibling list
/// is filtered.
pub const SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD: u32 = 3;

/// Classify a billing poll error string into auth vs other (no secrets).
///
/// Auth class: expired / no auth context / unauthorized / 401-style messages
/// from the SuperGrok credits path. Everything else is other (network, 5xx, …).
pub fn classify_supergrok_billing_poll_error(err: &str) -> SupergrokBillingPollOutcomeKind {
    let e = err.to_ascii_lowercase();
    if e.contains("no auth context")
        || e.contains("invalid or expired")
        || e.contains("expired credential")
        || e.contains("unauthorized")
        || e.contains("authentication")
        || e.contains(" 401")
        || e.contains("status: 401")
        || e.contains("http 401")
        || e.contains("status code 401")
        || e.contains("(401)")
    {
        SupergrokBillingPollOutcomeKind::AuthFailed
    } else {
        SupergrokBillingPollOutcomeKind::OtherFailed
    }
}

/// Remember a successful SuperGrok credits poll for `identity_id`.
///
/// Call only after a real Ok response for that principal's JWT. Does not
/// invent meters; pair with [`remember_supergrok_included_billing`] when %.
/// Resets the consecutive auth-fail streak so sibling polling may resume.
pub fn remember_supergrok_billing_poll_ok(identity_id: &str) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    {
        let mut map = POLL_OUTCOME_BY_IDENTITY
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        map.insert(id.to_owned(), SupergrokBillingPollOutcome::ok());
    }
    reset_auth_fail_streak(id);
}

/// Remember a failed SuperGrok credits poll and demote stale included cache on
/// auth-class fails.
///
/// Auth fail: clear that identity's process-cache free-period `usage_pct` so
/// rank does not treat it as **fresh** headroom; increment the consecutive
/// auth-fail streak (sibling re-poll skips after threshold). Does **not**
/// delete `auth.json` secrets. Other fail: record outcome only (keep prior
/// cache and do not change the auth-fail streak).
pub fn remember_supergrok_billing_poll_failed(identity_id: &str, err: &str) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let kind = classify_supergrok_billing_poll_error(err);
    let outcome = match kind {
        SupergrokBillingPollOutcomeKind::AuthFailed => SupergrokBillingPollOutcome::auth_failed(),
        SupergrokBillingPollOutcomeKind::OtherFailed => {
            // Prefer network when message looks like transport; else other.
            let e = err.to_ascii_lowercase();
            let class = if e.contains("timeout")
                || e.contains("connection")
                || e.contains("dns")
                || e.contains("network")
                || e.contains("connect")
            {
                "network"
            } else {
                "other"
            };
            SupergrokBillingPollOutcome::other_failed(class)
        }
        SupergrokBillingPollOutcomeKind::Ok | SupergrokBillingPollOutcomeKind::Never => {
            SupergrokBillingPollOutcome::other_failed("other")
        }
    };
    {
        let mut map = POLL_OUTCOME_BY_IDENTITY
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        map.insert(id.to_owned(), outcome);
    }
    if kind == SupergrokBillingPollOutcomeKind::AuthFailed {
        demote_included_billing_on_auth_fail(id);
        bump_auth_fail_streak(id);
    }
}

/// How many consecutive auth-class billing poll fails this process has recorded
/// for `identity_id` (0 when unknown / never / after Ok).
pub fn consecutive_auth_fail_streak(identity_id: &str) -> u32 {
    let id = identity_id.trim();
    if id.is_empty() {
        return 0;
    }
    AUTH_FAIL_STREAK_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .copied()
        .unwrap_or(0)
}

/// Whether automatic **sibling** SuperGrok billing polls should skip this
/// identity after too many consecutive auth-class fails.
///
/// Process map only. Never deletes secrets. Successful poll resets the streak.
pub fn should_skip_supergrok_billing_poll_for_auth_streak(identity_id: &str) -> bool {
    consecutive_auth_fail_streak(identity_id) >= SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD
}

fn bump_auth_fail_streak(identity_id: &str) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let mut map = AUTH_FAIL_STREAK_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(id.to_owned()).or_insert(0);
    *entry = entry.saturating_add(1);
}

fn reset_auth_fail_streak(identity_id: &str) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let mut map = AUTH_FAIL_STREAK_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.remove(id);
}

/// Clear free SuperGrok period `usage_pct` (and reset) for an identity after
/// auth-class poll fail so enrich/rank do not treat stale cache as fresh.
///
/// Leaves SuperGrok $ extras and Build product % alone (not free-period
/// headroom). No secrets; process map only.
pub fn demote_included_billing_on_auth_fail(identity_id: &str) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    let mut map = INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(entry) = map.get_mut(id) {
        entry.usage_pct = None;
        entry.reset_at = None;
        // period_type left: cosmetic for /limits if a later path re-fills
    }
}

/// Last poll outcome for one SuperGrok identity (`Never` when unrecorded).
pub fn supergrok_billing_poll_outcome(identity_id: &str) -> SupergrokBillingPollOutcome {
    let id = identity_id.trim();
    if id.is_empty() {
        return SupergrokBillingPollOutcome::never();
    }
    POLL_OUTCOME_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .cloned()
        .unwrap_or_else(SupergrokBillingPollOutcome::never)
}

/// Snapshot of all known poll outcomes (tests / doctor / limits honesty).
pub fn supergrok_billing_poll_outcomes_snapshot() -> BTreeMap<String, SupergrokBillingPollOutcome> {
    POLL_OUTCOME_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// True when this identity's last poll was auth-class fail.
pub fn supergrok_identity_last_poll_auth_failed(identity_id: &str) -> bool {
    supergrok_billing_poll_outcome(identity_id).is_auth_failed()
}

/// True when this identity's last poll succeeded.
pub fn supergrok_identity_last_poll_ok(identity_id: &str) -> bool {
    supergrok_billing_poll_outcome(identity_id).is_ok()
}

/// Plain English fail note for dual SuperGrok billing (CLI / doctor / limits).
///
/// Role is primary; fingerprint is secondary; always includes re-login CTA.
/// Does **not** use a bare 12-char identity id as the only label.
pub fn format_supergrok_billing_fail_note(
    role_label: &str,
    fingerprint: &str,
    err: &str,
) -> String {
    let role = role_label.trim();
    let role = if role.is_empty() { "unknown" } else { role };
    let fp = fingerprint.trim();
    let fp_short = if fp.len() > 12 { &fp[..12] } else { fp };
    let err = err.trim();
    let err = if err.is_empty() {
        "billing poll failed"
    } else {
        err
    };
    format!(
        "SuperGrok ({role}) billing failed (fingerprint {fp_short}): {err}. \
Re-login that SuperGrok account with: grok login"
    )
}

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
        grok_build_usage_pct: None,
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
        grok_build_usage_pct: None,
    });
    entry.prepaid_balance_cents = Some(prepaid_balance_cents);
}

/// Remember Grok Build `productUsage` % for one SuperGrok principal.
///
/// Process cache only. Does not invent: call only when wire had the field.
/// Dual `/limits` sibling rows read this so Build % is not hard-coded None.
pub fn remember_supergrok_build_usage(identity_id: &str, grok_build_usage_pct: f64) {
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
        grok_build_usage_pct: None,
    });
    entry.grok_build_usage_pct = Some(grok_build_usage_pct);
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
    // Active path credits success → poll OK for this identity (status/rank).
    remember_supergrok_billing_poll_ok(&identity_id);
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
///
/// Skips siblings that have hit
/// [`SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD`] consecutive auth-class poll
/// fails this process (re-login needed; secrets stay on disk).
pub fn load_non_active_supergrok_billing_poll_targets(
    grok_home: &Path,
) -> Vec<SupergrokBillingPollTarget> {
    let active = active_supergrok_identity_id(grok_home);
    load_supergrok_billing_poll_targets(grok_home)
        .into_iter()
        .filter(|t| active.as_deref() != Some(t.identity_id.as_str()))
        .filter(|t| !should_skip_supergrok_billing_poll_for_auth_streak(&t.identity_id))
        .collect()
}

/// Whether a stored SuperGrok session should OIDC-refresh before a billing poll.
///
/// True only when the access token is past the early-invalidation buffer **and**
/// the entry still has refresh_token + issuer + client_id (OIDC-refreshable).
/// Does not delete secrets. External-only or incomplete entries return false
/// (caller may still poll with the stored access token).
pub fn session_needs_oidc_refresh_before_billing_poll(auth: &super::model::GrokAuth) -> bool {
    use super::model::{is_expired, is_supergrok_session_mode};

    if !is_supergrok_session_mode(auth.auth_mode) {
        return false;
    }
    if auth.auth_mode != AuthMode::Oidc {
        // External binary path is not refreshed via OIDC token exchange here.
        return false;
    }
    let has_rt = auth
        .refresh_token
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    let has_issuer = auth
        .oidc_issuer
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    let has_client = auth
        .oidc_client_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    has_rt && has_issuer && has_client && is_expired(auth)
}

/// Prefer multi-slot store entry for `identity_id` (same policy as poll targets).
///
/// Returns `(auth.json scope key, GrokAuth)`. Used so sibling OIDC refresh can
/// write back the multi-slot without clobbering the active base of another
/// principal.
pub fn find_supergrok_auth_entry_for_billing(
    grok_home: &Path,
    identity_id: &str,
) -> Option<(String, super::model::GrokAuth)> {
    use super::model::{is_supergrok_session_mode, supergrok_identity_id_from_auth};

    let want = identity_id.trim();
    if want.is_empty() {
        return None;
    }
    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok()?;
    let mut best: Option<(bool, String, super::model::GrokAuth)> = None;
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
        if id != want {
            continue;
        }
        let is_multi = scope.contains("::personal") || scope.contains("::team::");
        match &best {
            None => best = Some((is_multi, scope.clone(), auth.clone())),
            Some((prev_multi, _, _)) => {
                if is_multi && !*prev_multi {
                    best = Some((is_multi, scope.clone(), auth.clone()));
                }
            }
        }
    }
    best.map(|(_, scope, auth)| (scope, auth))
}

/// Persist a refreshed SuperGrok session into its existing `auth.json` scope.
///
/// Updates only `scope` (typically the multi-slot). Does **not** promote the
/// sibling onto the active base when the base holds a different principal.
/// Does **not** delete other principals or secrets.
pub fn persist_refreshed_supergrok_billing_auth(
    grok_home: &Path,
    scope: &str,
    new_auth: super::model::GrokAuth,
) -> std::io::Result<()> {
    use super::storage::write_auth_json;

    let path = grok_home.join("auth.json");
    let mut map = read_auth_json(&path)?;
    map.insert(scope.to_owned(), new_auth);
    write_auth_json(&path, &map)
}

/// Ensure the SuperGrok principal has a usable access token for a billing poll.
///
/// When the multi-slot (or base) JWT is past the early-invalidation buffer and
/// still has OIDC refresh credentials, exchange the refresh token and write the
/// new access token back to that `auth.json` scope **before** the credits HTTP
/// call. On refresh failure, returns the existing token so the poll can record
/// auth-fail honesty (and eventually N-fail demote). Never deletes secrets.
///
/// Returns `(access_token, user_id)`.
pub async fn ensure_fresh_access_token_for_supergrok_billing_poll(
    grok_home: &Path,
    identity_id: &str,
) -> Option<(String, String)> {
    let (scope, auth) = find_supergrok_auth_entry_for_billing(grok_home, identity_id)?;
    if !session_needs_oidc_refresh_before_billing_poll(&auth) {
        return Some((auth.key.clone(), auth.user_id.clone()));
    }
    tracing::debug!(
        identity_id = %identity_id,
        scope = %scope,
        "sibling SuperGrok billing: OIDC refresh before poll"
    );
    match super::oidc::oidc_token_exchange(&auth).await {
        super::oidc::OidcRefreshResult::Success(new_auth) => {
            let token = new_auth.key.clone();
            let user_id = new_auth.user_id.clone();
            if let Err(e) = persist_refreshed_supergrok_billing_auth(grok_home, &scope, *new_auth) {
                tracing::warn!(
                    identity_id = %identity_id,
                    error = %e,
                    "sibling SuperGrok billing: failed to persist refreshed multi-slot token"
                );
                // Still use the fresh token for this poll even if disk write failed.
            }
            Some((token, user_id))
        }
        super::oidc::OidcRefreshResult::TerminalError { reason } => {
            tracing::debug!(
                identity_id = %identity_id,
                ?reason,
                "sibling SuperGrok billing: OIDC refresh terminal; polling with stored token"
            );
            Some((auth.key.clone(), auth.user_id.clone()))
        }
        super::oidc::OidcRefreshResult::Failed { .. } => {
            tracing::debug!(
                identity_id = %identity_id,
                "sibling SuperGrok billing: OIDC refresh failed; polling with stored token"
            );
            Some((auth.key.clone(), auth.user_id.clone()))
        }
    }
}

/// Snapshot of the process included-billing map (for tests / limits fill).
pub fn included_billing_fields_snapshot() -> BTreeMap<String, IncludedBillingFields> {
    INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Clear process included-billing cache, poll outcomes, and auth-fail streaks
/// (tests / reset). Does not touch `auth.json` secrets on disk.
pub fn clear_included_billing_cache() {
    INCLUDED_BILLING_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    POLL_OUTCOME_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    AUTH_FAIL_STREAK_BY_IDENTITY
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
///
/// Skips the legacy `https://accounts.x.ai/sign-in` scope: that key is only a
/// storage fallback for pre-OIDC auth.json, not a second SuperGrok principal.
/// Ranking it next to the current OAuth base scope (especially when both lack
/// `user_id` / `team_id` and identity falls back to the store key) let free
/// SuperGrok period align hot-swap the intentional new-scope primary onto the
/// legacy entry.
pub fn load_supergrok_session_candidates(
    grok_home: &Path,
) -> Vec<super::supergrok_identity_rank::SupergrokSessionCandidate> {
    use super::model::{
        GrokAuth, LEGACY_SCOPE, is_expired_with_buffer, is_supergrok_session_mode,
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
        // Legacy scope is lookup fallback only — never a dual-identity rank peer.
        if scope == LEGACY_SCOPE {
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
                prepaid_balance_cents: None,
                hard_expired,
            }
        })
        .collect();
    let billing = included_billing_fields_snapshot();
    if !billing.is_empty() {
        // Billing usage % must not resurrect a hard-expired multi-slot as
        // "included headroom" (personal % can still poll for a dead JWT).
        // Live free SuperGrok period headroom (used percent below 100) clears
        // a stale out-of-allowance memo so prefer_live and network re-resolve
        // put SuperGrok back instead of sticking on console.
        let clear_tokens =
            enrich_candidates_with_included_billing(&mut candidates, &billing, |tok| {
                let t = tok.trim();
                // Hard-expired stays "exhausted" for enrich force-zero when no
                // usage is present; live usage < 100 still applies remaining
                // only for non-hard-expired (hard-expired filtered below).
                xai_grok_sampler::is_credential_exhausted(t) || hard_expired_tokens.contains(t)
            });
        for tok in clear_tokens {
            let t = tok.trim();
            if hard_expired_tokens.contains(t) {
                // Never clear memo via billing for a JWT the wire would reject.
                continue;
            }
            if xai_grok_sampler::is_credential_exhausted(t) {
                xai_grok_sampler::clear_exhausted(&grok_rate_limit::fingerprint_secret(t));
            }
        }
        // After memo clear, re-zero hard-expired rows that enrich may have
        // set from usage % (sibling poll can still report % for a dead JWT).
        for c in &mut candidates {
            if hard_expired_tokens.contains(c.access_token.trim()) {
                c.headroom.included_remaining = 0;
            }
        }
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
    let status = collect_dual_auth_status(grok_home);
    // Prefer config.toml under the same home apply is evaluating (hermetic tests
    // + multi-home hosts) before process-global effective config.
    let auto_use =
        auto_use_included_limits_for_home(grok_home).unwrap_or(status.auto_use_included_limits);
    apply_billing_usage_to_session_exhaust_inner(
        usage_pct,
        grok_home,
        period_end_rfc3339,
        auto_use,
        status.dual_auth_ready(),
    )
}

/// Pure after-burner memo gate: skip marking SuperGrok out of allowance when
/// included is full but SuperGrok $ extras remain under auto_use + dual-auth.
///
/// When true, callers clear any prior mark and leave SuperGrok live so prefer_live
/// does not hop to console before after-burner spend.
pub fn afterburner_skips_allowance_mark(
    usage_pct: f64,
    auto_use_included_limits: bool,
    dual_auth_ready: bool,
    prepaid_balance_cents: Option<i64>,
) -> bool {
    afterburner_skips_allowance_mark_with_sibling(
        usage_pct,
        auto_use_included_limits,
        dual_auth_ready,
        prepaid_balance_cents,
        false,
    )
}

/// After-burner skip with sibling included SuperGrok period limits.
///
/// SuperGrok dollar credits skip the out-of-allowance mark only when every
/// distinct included pool is exhausted. A sibling with included remaining
/// must not skip: mark the full identity so prefer_live / rank can hop.
pub fn afterburner_skips_allowance_mark_with_sibling(
    usage_pct: f64,
    auto_use_included_limits: bool,
    dual_auth_ready: bool,
    prepaid_balance_cents: Option<i64>,
    sibling_has_distinct_included_remaining: bool,
) -> bool {
    usage_pct >= xai_grok_sampler::INCLUDED_ALLOWANCE_EXHAUST_PCT
        && auto_use_included_limits
        && dual_auth_ready
        && super::supergrok_identity_rank::has_positive_supergrok_dollar_extras(
            prepaid_balance_cents,
        )
        && !sibling_has_distinct_included_remaining
}

/// True when another stored SuperGrok login still has included SuperGrok
/// period limits remaining. Used so after-burner extras do not skip the
/// out-of-allowance mark while a sibling pool can still be spent.
pub fn any_sibling_has_included_remaining(grok_home: &Path, active_identity_id: &str) -> bool {
    load_supergrok_session_candidates(grok_home)
        .iter()
        .any(|c| c.headroom.identity_id != active_identity_id && c.headroom.has_included_headroom())
}

/// Read `[auth] auto_use_included_limits` (or aliases) from `$home/config.toml`.
///
/// `None` = file missing / key absent (caller falls back to process config).
fn auto_use_included_limits_for_home(grok_home: &Path) -> Option<bool> {
    let path = grok_home.join("config.toml");
    let raw = std::fs::read_to_string(&path).ok()?;
    let value: toml::Value = toml::from_str(&raw).ok()?;
    let table_bool = |section: &str, key: &str| -> Option<bool> {
        value
            .get(section)
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_bool())
    };
    table_bool("auth", "auto_use_included_limits")
        .or_else(|| table_bool("grok_com_config", "auto_use_included_limits"))
        .or_else(|| table_bool("auth", "prefer_sooner_reset"))
        .or_else(|| table_bool("grok_com_config", "prefer_sooner_reset"))
}

fn apply_billing_usage_to_session_exhaust_inner(
    usage_pct: f64,
    grok_home: &Path,
    period_end_rfc3339: Option<&str>,
    auto_use_included_limits: bool,
    dual_auth_ready: bool,
) -> xai_grok_sampler::AllowanceExhaustAction {
    // Feed ranking even when dual-auth is not ready (multi SuperGrok alone).
    // Period type unknown on this path (usage-only callers); leave prior or None.
    remember_active_supergrok_included_billing(grok_home, usage_pct, period_end_rfc3339, None);

    // After-burner: with auto_use_included_limits and known positive SuperGrok
    // $ extras, keep SuperGrok session live — do not mark out of allowance so
    // prefer_live does not hop to console before extras burn.
    if dual_auth_ready {
        if let Some(identity_id) = active_supergrok_identity_id(grok_home) {
            let extras = included_billing_fields_snapshot()
                .get(&identity_id)
                .and_then(|f| f.prepaid_balance_cents);
            // Sibling included remaining only gates the 100% after-burner skip.
            // `load_supergrok_session_candidates` also clears exhaust memos when
            // live used percent is below 100. Calling that before sync would
            // swallow period-reset `Cleared` (sync would then see no memo).
            let sibling_included = usage_pct >= xai_grok_sampler::INCLUDED_ALLOWANCE_EXHAUST_PCT
                && any_sibling_has_included_remaining(grok_home, &identity_id);
            if afterburner_skips_allowance_mark_with_sibling(
                usage_pct,
                auto_use_included_limits,
                dual_auth_ready,
                extras,
                sibling_included,
            ) {
                let Some(token) = load_session_access_token(grok_home) else {
                    return xai_grok_sampler::AllowanceExhaustAction::None;
                };
                // Clear any prior mark so prefer_live does not hop to console.
                // Pass usage under the floor so sync clears without re-marking;
                // included is still full — ranking uses extras after-burner.
                let action = xai_grok_sampler::sync_allowance_exhaust_from_usage(
                    0.0,
                    Some(token.as_str()),
                    true,
                );
                if matches!(action, xai_grok_sampler::AllowanceExhaustAction::Cleared) {
                    tracing::info!(
                        target: "xai_grok_shell::auth",
                        usage_pct,
                        prepaid_balance_cents = extras,
                        "SuperGrok included full but $ extras remain; cleared allowance memo for after-burner"
                    );
                } else {
                    tracing::debug!(
                        target: "xai_grok_shell::auth",
                        usage_pct,
                        prepaid_balance_cents = extras,
                        "SuperGrok included full but $ extras remain; not marking out of allowance (after-burner)"
                    );
                }
                return action;
            }
        }
    }

    if !dual_auth_ready {
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
            "SuperGrok included usage printout full; fail-open does not remember SuperGrok out of allowance from this printout"
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
    use crate::auth::storage::{read_auth_json, write_auth_json};
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

    /// Named contract: auth-class billing fail demotes free-period process cache
    /// so rank does not treat stale usage % as fresh headroom.
    #[test]
    #[serial_test::serial]
    fn auth_failed_poll_demotes_included_usage_pct_not_fresh_headroom() {
        clear_included_billing_cache();
        remember_supergrok_included_billing(
            "user-dead",
            6.0,
            Some("2026-08-10T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_dollar_extras("user-dead", 500);
        assert_eq!(
            included_billing_fields_snapshot()
                .get("user-dead")
                .and_then(|f| f.usage_pct),
            Some(6.0)
        );
        remember_supergrok_billing_poll_failed(
            "user-dead",
            "Billing service error: no auth context",
        );
        let outcome = supergrok_billing_poll_outcome("user-dead");
        assert_eq!(outcome.kind, SupergrokBillingPollOutcomeKind::AuthFailed);
        assert_eq!(outcome.error_class, Some("auth"));
        let fields = included_billing_fields_snapshot()
            .get("user-dead")
            .cloned()
            .expect("entry remains for extras");
        assert_eq!(
            fields.usage_pct, None,
            "auth fail must demote free-period usage_pct"
        );
        assert_eq!(
            fields.prepaid_balance_cents,
            Some(500),
            "auth fail must not wipe SuperGrok $ extras memory"
        );
        clear_included_billing_cache();
    }

    /// Named contract: OIDC multi-slot needs refresh only when expired and still
    /// has refresh_token + issuer + client_id.
    #[test]
    fn session_needs_oidc_refresh_when_expired_with_refresh_credentials() {
        use chrono::{Duration, Utc};

        let fresh = GrokAuth {
            key: "fresh-at".into(),
            auth_mode: AuthMode::Oidc,
            user_id: "u1".into(),
            refresh_token: Some("rt".into()),
            oidc_issuer: Some("https://auth.x.ai".into()),
            oidc_client_id: Some("client".into()),
            expires_at: Some(Utc::now() + Duration::hours(2)),
            create_time: Utc::now(),
            ..Default::default()
        };
        assert!(
            !session_needs_oidc_refresh_before_billing_poll(&fresh),
            "live JWT must not force refresh"
        );

        let expired = GrokAuth {
            expires_at: Some(Utc::now() - Duration::hours(1)),
            ..fresh.clone()
        };
        assert!(
            session_needs_oidc_refresh_before_billing_poll(&expired),
            "expired OIDC with RT must need refresh before sibling poll"
        );

        let no_rt = GrokAuth {
            refresh_token: None,
            expires_at: Some(Utc::now() - Duration::hours(1)),
            ..fresh.clone()
        };
        assert!(
            !session_needs_oidc_refresh_before_billing_poll(&no_rt),
            "without refresh_token cannot OIDC-refresh"
        );

        let external = GrokAuth {
            auth_mode: AuthMode::External,
            expires_at: Some(Utc::now() - Duration::hours(1)),
            refresh_token: Some("rt".into()),
            oidc_issuer: Some("https://auth.x.ai".into()),
            oidc_client_id: Some("client".into()),
            ..fresh
        };
        assert!(
            !session_needs_oidc_refresh_before_billing_poll(&external),
            "External mode is not OIDC token-exchange refresh here"
        );
    }

    /// Named contract: prefer multi-slot store entry for billing identity lookup;
    /// persist refreshed auth only to that scope (sibling multi-slot stays
    /// multi-slot; does not wipe active base of another principal).
    #[test]
    #[serial_test::serial]
    fn find_and_persist_refreshed_multi_slot_for_billing_without_clobbering_base() {
        use crate::auth::model::upsert_supergrok_session;
        use chrono::{Duration, Utc};

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::multi-slot-refresh";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-personal-stale".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-ms".into(),
                refresh_token: Some("rt-p".into()),
                oidc_issuer: Some("https://auth.x.ai".into()),
                oidc_client_id: Some("cli".into()),
                expires_at: Some(Utc::now() - Duration::hours(2)),
                create_time: Utc::now() - Duration::hours(3),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-business-active".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-ms".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-ms".into()),
                refresh_token: Some("rt-b".into()),
                oidc_issuer: Some("https://auth.x.ai".into()),
                oidc_client_id: Some("cli".into()),
                expires_at: Some(Utc::now() + Duration::hours(2)),
                create_time: Utc::now(),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let personal_id = {
            let non = load_non_active_supergrok_billing_poll_targets(dir.path());
            assert_eq!(non.len(), 1, "personal sibling: {non:?}");
            non[0].identity_id.clone()
        };

        let (scope, auth) =
            find_supergrok_auth_entry_for_billing(dir.path(), &personal_id).expect("entry");
        assert!(
            scope.contains("::personal"),
            "must prefer multi-slot scope, got {scope}"
        );
        assert!(
            session_needs_oidc_refresh_before_billing_poll(&auth),
            "stale multi-slot needs refresh"
        );
        assert_eq!(auth.key, "tok-personal-stale");

        let refreshed = GrokAuth {
            key: "tok-personal-fresh".into(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            create_time: Utc::now(),
            ..auth
        };
        persist_refreshed_supergrok_billing_auth(dir.path(), &scope, refreshed).unwrap();

        let reread = read_auth_json(&dir.path().join("auth.json")).unwrap();
        assert_eq!(
            reread.get(&scope).map(|a| a.key.as_str()),
            Some("tok-personal-fresh"),
            "multi-slot updated"
        );
        // Active base is business (last upsert) — must still be business token.
        assert_eq!(
            reread.get(base).map(|a| a.key.as_str()),
            Some("tok-business-active"),
            "must not clobber active base of other principal"
        );
        let (_, after) =
            find_supergrok_auth_entry_for_billing(dir.path(), &personal_id).expect("after");
        assert_eq!(after.key, "tok-personal-fresh");
        assert!(!session_needs_oidc_refresh_before_billing_poll(&after));
        clear_included_billing_cache();
    }

    /// Named contract: ensure_fresh runs OIDC exchange for expired multi-slot
    /// and returns the new access token (hermetic mock IdP).
    #[tokio::test]
    #[serial_test::serial]
    async fn ensure_fresh_refreshes_expired_multi_slot_via_oidc_before_billing() {
        use crate::auth::model::upsert_supergrok_session;
        use chrono::{Duration, Utc};

        clear_included_billing_cache();
        let (issuer, _handle) = start_mock_oidc_for_billing_refresh().await;

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::ensure-fresh-billing";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "stale-sibling-at".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-sib-fresh".into(),
                refresh_token: Some("rt-sib".into()),
                oidc_issuer: Some(issuer.clone()),
                oidc_client_id: Some("billing-refresh-client".into()),
                expires_at: Some(Utc::now() - Duration::minutes(30)),
                create_time: Utc::now() - Duration::hours(2),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "active-business-at".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-act-fresh".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-ef".into()),
                refresh_token: Some("rt-act".into()),
                oidc_issuer: Some(issuer),
                oidc_client_id: Some("billing-refresh-client".into()),
                expires_at: Some(Utc::now() + Duration::hours(2)),
                create_time: Utc::now(),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let sibling_id = load_non_active_supergrok_billing_poll_targets(dir.path())
            .into_iter()
            .next()
            .expect("sibling")
            .identity_id;

        let (token, user_id) =
            ensure_fresh_access_token_for_supergrok_billing_poll(dir.path(), &sibling_id)
                .await
                .expect("token");
        assert_eq!(user_id, "user-sib-fresh");
        assert_eq!(
            token, "mock-billing-access-token",
            "must use OIDC-refreshed access token, not stale multi-slot JWT"
        );
        // Persisted multi-slot must hold the new key.
        let (_, stored) =
            find_supergrok_auth_entry_for_billing(dir.path(), &sibling_id).expect("stored");
        assert_eq!(stored.key, "mock-billing-access-token");
        assert!(!session_needs_oidc_refresh_before_billing_poll(&stored));
        clear_included_billing_cache();
    }

    /// Minimal mock IdP: discovery + token endpoint returning a fixed access token.
    async fn start_mock_oidc_for_billing_refresh() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let issuer_disc = issuer.clone();
        let app = axum::Router::new()
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
                    axum::Json(serde_json::json!({
                        "access_token": "mock-billing-access-token",
                        "refresh_token": "mock-billing-refresh-token",
                        "expires_in": 3600,
                        "token_type": "Bearer",
                    }))
                }),
            );
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        // Tiny settle so bind is ready.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        (issuer, handle)
    }

    /// Named contract: after N consecutive auth-class billing poll fails, skip
    /// automatic sibling re-poll for that SuperGrok identity. Do not delete
    /// secrets. Ok resets the streak. Network fails do not bump the streak.
    #[test]
    #[serial_test::serial]
    fn sibling_poll_skips_after_n_consecutive_auth_fails_without_secret_delete() {
        use crate::auth::model::upsert_supergrok_session;

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::auth-streak-skip";
        let mut map = AuthStore::default();
        // First = personal (sibling when business is last upsert / active).
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-personal-streak".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-p-streak".into(),
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "tok-business-streak".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-b-streak".into(),
                principal_type: Some("Team".into()),
                team_id: Some("team-streak".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let non_active_before = load_non_active_supergrok_billing_poll_targets(dir.path());
        assert_eq!(
            non_active_before.len(),
            1,
            "one sibling before auth streak: {non_active_before:?}"
        );
        let sibling_id = non_active_before[0].identity_id.clone();
        assert!(
            !should_skip_supergrok_billing_poll_for_auth_streak(&sibling_id),
            "fresh sibling must not be skipped"
        );

        // Network fails do not count toward the auth streak.
        remember_supergrok_billing_poll_failed(
            &sibling_id,
            "Failed to fetch billing data: timeout",
        );
        assert_eq!(consecutive_auth_fail_streak(&sibling_id), 0);
        assert!(!should_skip_supergrok_billing_poll_for_auth_streak(
            &sibling_id
        ));

        for i in 1..=SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD {
            remember_supergrok_billing_poll_failed(
                &sibling_id,
                "Billing service error: no auth context",
            );
            assert_eq!(
                consecutive_auth_fail_streak(&sibling_id),
                i,
                "auth fail streak after {i}"
            );
            if i < SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD {
                assert!(
                    !should_skip_supergrok_billing_poll_for_auth_streak(&sibling_id),
                    "must still poll before threshold; i={i}"
                );
                let still = load_non_active_supergrok_billing_poll_targets(dir.path());
                assert_eq!(
                    still.len(),
                    1,
                    "sibling still on poll list before threshold"
                );
            }
        }
        assert!(
            should_skip_supergrok_billing_poll_for_auth_streak(&sibling_id),
            "at threshold must skip automatic sibling poll"
        );
        let skipped = load_non_active_supergrok_billing_poll_targets(dir.path());
        assert!(
            skipped.is_empty(),
            "sibling demoted from poll list after N auth fails; got {skipped:?}"
        );

        // Secrets must still be on disk (no auto-delete).
        let reread =
            read_auth_json(&dir.path().join("auth.json")).expect("auth.json still present");
        assert!(
            reread
                .values()
                .any(|a| a.key.contains("tok-personal-streak"))
                || reread
                    .values()
                    .any(|a| a.key.contains("tok-business-streak")),
            "auth.json secrets must not be auto-deleted after N auth fails"
        );
        // Full target loader still lists both principals (skip is sibling list only).
        let all = load_supergrok_billing_poll_targets(dir.path());
        assert_eq!(all.len(), 2, "all targets still load secrets from disk");

        // Successful poll resets streak and restores sibling to the poll list.
        remember_supergrok_billing_poll_ok(&sibling_id);
        assert_eq!(consecutive_auth_fail_streak(&sibling_id), 0);
        assert!(!should_skip_supergrok_billing_poll_for_auth_streak(
            &sibling_id
        ));
        let restored = load_non_active_supergrok_billing_poll_targets(dir.path());
        assert_eq!(
            restored.len(),
            1,
            "sibling back on poll list after Ok: {restored:?}"
        );
        assert_eq!(restored[0].identity_id, sibling_id);

        clear_included_billing_cache();
    }

    /// Named contract: fail note names role + re-login CTA (not only 12-char id).
    #[test]
    fn billing_fail_note_names_role_fingerprint_and_relogin() {
        let note = format_supergrok_billing_fail_note(
            "personal",
            "abcdef0123456789ffff",
            "Invalid or expired credentials",
        );
        assert!(
            note.contains("SuperGrok (personal)"),
            "role primary: {note}"
        );
        assert!(
            note.contains("fingerprint abcdef012345"),
            "fingerprint secondary: {note}"
        );
        assert!(
            note.to_ascii_lowercase().contains("grok login"),
            "re-login CTA: {note}"
        );
        assert!(
            note.contains("Invalid or expired credentials"),
            "error text: {note}"
        );
        // Must not be the old soft shape that only used a bare short identity id.
        assert!(
            !note.starts_with("SuperGrok billing poll failed for "),
            "must not be short-id-only note: {note}"
        );
    }

    /// Named contract: successful poll records Ok; never invents fail.
    #[test]
    #[serial_test::serial]
    fn remember_poll_ok_sets_outcome_ok() {
        clear_included_billing_cache();
        remember_supergrok_billing_poll_ok("team-live");
        assert!(supergrok_identity_last_poll_ok("team-live"));
        assert!(!supergrok_identity_last_poll_auth_failed("team-live"));
        clear_included_billing_cache();
    }

    /// Named contract: sibling/full credits poll can remember SuperGrok Extra
    /// Usage Credits (`prepaidBalance`) without inventing console team $.
    #[test]
    #[serial_test::serial]
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

    /// Named contract (Issue 4): sibling credits poll can remember Grok Build
    /// productUsage % for dual `/limits` (not hard-coded None).
    #[test]
    #[serial_test::serial]
    fn remember_build_usage_stores_product_pct_for_limits_fill() {
        clear_included_billing_cache();
        remember_supergrok_included_billing(
            "team-sibling",
            65.0,
            Some("2026-08-04T01:25:32Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_build_usage("team-sibling", 54.0);
        let snap = included_billing_fields_snapshot();
        let fields = snap.get("team-sibling").expect("remembered identity");
        assert_eq!(fields.grok_build_usage_pct, Some(54.0));
        // Included re-remember must not wipe Build %.
        remember_supergrok_included_billing("team-sibling", 66.0, None, None);
        let snap2 = included_billing_fields_snapshot();
        assert_eq!(
            snap2.get("team-sibling").unwrap().grok_build_usage_pct,
            Some(54.0),
            "included re-remember must keep Build productUsage %"
        );
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
                AllowanceExhaustAction::None,
                "fail-open: client 100% printout must not Mark"
            );
            assert!(!xai_grok_sampler::is_credential_exhausted(session));
            // HTTP 402 rotate still marks; period-reset clear still works.
            xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(session));
            assert!(xai_grok_sampler::is_credential_exhausted(session));
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

    fn write_auto_use_config(home: &Path, enabled: bool) {
        let body = format!(
            "[auth]\nauto_use_included_limits = {}\n",
            if enabled { "true" } else { "false" }
        );
        std::fs::write(home.join("config.toml"), body).expect("write config.toml");
    }

    /// Named contract: personal included SuperGrok period limits full + SuperGrok
    /// dollar credits on that login must still mark the full identity when a
    /// distinct sibling still has included remaining, so prefer_live / rank can
    /// hop. After-burner skip is only for every distinct included pool exhausted.
    #[test]
    fn afterburner_does_not_skip_mark_when_sibling_has_included_remaining() {
        assert!(
            !afterburner_skips_allowance_mark_with_sibling(100.0, true, true, Some(10_029), true,),
            "sibling included remaining must not skip the out-of-allowance mark"
        );
        assert!(
            afterburner_skips_allowance_mark_with_sibling(100.0, true, true, Some(10_029), false,),
            "single-identity extras after-burner still skips when no sibling included remains"
        );
    }

    /// Apply path: sticky personal at 100% with extras + Business included
    /// remaining must Mark the personal JWT (not after-burner skip).
    #[test]
    #[serial_test::serial]
    fn apply_billing_marks_personal_full_when_business_sibling_has_included() {
        use crate::auth::model::upsert_supergrok_session;

        with_isolated_home(|home| {
            clear_included_billing_cache();
            let personal = "tok-personal-full-extras";
            let business = "tok-business-included-remaining";
            let base = "https://auth.x.ai::test-client";
            let mut map = AuthStore::default();
            upsert_supergrok_session(
                &mut map,
                base,
                GrokAuth {
                    key: business.into(),
                    auth_mode: AuthMode::Oidc,
                    user_id: "user-b".into(),
                    principal_type: Some("Team".into()),
                    team_id: Some("team-biz".into()),
                    ..Default::default()
                },
            );
            // Last upsert is the sticky base (personal).
            upsert_supergrok_session(
                &mut map,
                base,
                GrokAuth {
                    key: personal.into(),
                    auth_mode: AuthMode::Oidc,
                    user_id: "user-p".into(),
                    ..Default::default()
                },
            );
            write_auth_json(&home.join("auth.json"), &map).unwrap();
            write_auto_use_config(home, true);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            remember_supergrok_dollar_extras("user-p", 10_029);
            remember_supergrok_included_billing(
                "team-biz",
                40.0,
                Some("2026-08-20T00:00:00Z"),
                Some("USAGE_PERIOD_TYPE_WEEKLY"),
            );

            let action = apply_billing_usage_to_session_exhaust(100.0, home);
            assert_eq!(
                action,
                AllowanceExhaustAction::None,
                "fail-open: personal 100% printout must not Mark even when Business included remains; got {action:?}"
            );
            assert!(
                !xai_grok_sampler::is_credential_exhausted(personal),
                "personal JWT must stay unmarked from a client printout"
            );
            assert!(
                !xai_grok_sampler::is_credential_exhausted(business),
                "Business sibling must stay live"
            );
            clear_included_billing_cache();
        });
    }

    /// Pure gate: after-burner skips mark only when auto_use + dual-auth + extras > 0.
    #[test]
    fn afterburner_skips_allowance_mark_pure_policy() {
        assert!(afterburner_skips_allowance_mark(
            100.0,
            true,
            true,
            Some(10_029)
        ));
        assert!(
            !afterburner_skips_allowance_mark(100.0, true, true, Some(0)),
            "extras 0 must still mark"
        );
        assert!(
            !afterburner_skips_allowance_mark(100.0, true, true, None),
            "unknown extras must still mark"
        );
        assert!(
            !afterburner_skips_allowance_mark(100.0, false, true, Some(10_029)),
            "auto_use off → mark"
        );
        assert!(
            !afterburner_skips_allowance_mark(100.0, true, false, Some(10_029)),
            "no dual-auth → no after-burner skip"
        );
        assert!(
            !afterburner_skips_allowance_mark(99.0, true, true, Some(10_029)),
            "included not full → not this gate"
        );
    }

    /// Named contract (Issue 1): dual-auth + auto_use + positive extras + 100%
    /// included → do not mark SuperGrok out of allowance (after-burner).
    #[test]
    #[serial_test::serial]
    fn apply_billing_100_pct_with_positive_extras_and_auto_use_does_not_mark() {
        with_isolated_home(|home| {
            clear_included_billing_cache();
            let session = "session-jwt-afterburner-no-mark";
            write_oidc(home, session);
            write_auto_use_config(home, true);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            // write_oidc user_id is "user-1" → active identity_id.
            remember_supergrok_dollar_extras("user-1", 10_029);

            let action = apply_billing_usage_to_session_exhaust(100.0, home);
            assert_eq!(
                action,
                AllowanceExhaustAction::None,
                "after-burner must not Mark when extras remain; got {action:?}"
            );
            assert!(
                !xai_grok_sampler::is_credential_exhausted(session),
                "session must stay live for SuperGrok $ extras after-burner"
            );
            clear_included_billing_cache();
        });
    }

    /// Named contract (Issue 1): prior mark + auto_use + positive extras → Cleared.
    #[test]
    #[serial_test::serial]
    fn apply_billing_100_pct_with_positive_extras_clears_prior_mark() {
        with_isolated_home(|home| {
            clear_included_billing_cache();
            let session = "session-jwt-afterburner-clear";
            write_oidc(home, session);
            write_auto_use_config(home, true);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            // Prior HTTP 402 mark, not a client 100% printout.
            xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(session));
            assert!(xai_grok_sampler::is_credential_exhausted(session));

            remember_supergrok_dollar_extras("user-1", 5_000);
            let action = apply_billing_usage_to_session_exhaust(100.0, home);
            assert_eq!(
                action,
                AllowanceExhaustAction::Cleared,
                "after-burner must clear prior mark so prefer_live does not hop"
            );
            assert!(
                !xai_grok_sampler::is_credential_exhausted(session),
                "session must be live after after-burner clear"
            );
            clear_included_billing_cache();
        });
    }

    /// Fail-open: SuperGrok dollar credits 0 or unknown plus a 100% printout
    /// must not Mark. Identifier keeps the old catalog name.
    #[test]
    #[serial_test::serial]
    fn apply_billing_100_pct_auto_use_marks_when_extras_gone_or_unknown() {
        with_isolated_home(|home| {
            clear_included_billing_cache();
            let session = "session-jwt-afterburner-mark-no-extras";
            write_oidc(home, session);
            write_auto_use_config(home, true);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            let action_none = apply_billing_usage_to_session_exhaust(100.0, home);
            assert_eq!(
                action_none,
                AllowanceExhaustAction::None,
                "unknown SuperGrok dollar credits must not Mark from a 100% printout"
            );
            assert!(!xai_grok_sampler::is_credential_exhausted(session));

            remember_supergrok_dollar_extras("user-1", 0);
            let action_zero = apply_billing_usage_to_session_exhaust(100.0, home);
            assert_eq!(
                action_zero,
                AllowanceExhaustAction::None,
                "SuperGrok dollar credits 0 must not Mark from a 100% printout"
            );
            assert!(!xai_grok_sampler::is_credential_exhausted(session));
            clear_included_billing_cache();
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

    /// Named contract (period reset → SuperGrok again): after free SuperGrok
    /// period allowance was memoized full, a later billing read with used
    /// percent below 100% must clear the memo, rank SuperGrok primary, and
    /// omit console from the hop chain (limits before credits; network
    /// re-resolve must not stick on console credits).
    #[test]
    #[serial_test::serial]
    fn period_reset_clears_memo_and_ranks_supergrok_primary_without_console() {
        use crate::auth::supergrok_identity_rank::order_credentials_for_preferred_auto;
        use xai_grok_sampler::{
            SamplerConfig, clear_all_including_durable, prefer_live_identity_after_credit_exhaust,
        };

        with_isolated_home(|home| {
            let session = "session-jwt-period-reset-primary";
            write_oidc(home, session);
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-team-prepaid-key").unwrap());
            write_auto_use_config(home, true);

            // Prior HTTP 402 mark (not a client 100% printout).
            xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(session));
            assert!(xai_grok_sampler::is_credential_exhausted(session));
            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::None,
                "fail-open printout must not re-Mark"
            );
            assert!(xai_grok_sampler::is_credential_exhausted(session));

            // Period reset: free SuperGrok period used percent drops.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(7.0, home),
                AllowanceExhaustAction::Cleared
            );
            assert!(
                !xai_grok_sampler::is_credential_exhausted(session),
                "period reset must clear exhaust memo"
            );

            let candidates = load_supergrok_session_candidates(home);
            assert!(
                candidates
                    .iter()
                    .any(|c| c.access_token == session && c.headroom.has_included_headroom()),
                "load after reset must show SuperGrok included headroom: {:?}",
                candidates
                    .iter()
                    .map(|c| (c.access_token.as_str(), c.headroom.included_remaining))
                    .collect::<Vec<_>>()
            );

            let order = order_credentials_for_preferred_auto(
                &candidates,
                &["console-team-prepaid-key".into()],
            );
            assert_eq!(
                order.primary.as_deref(),
                Some(session),
                "auto rank primary must be SuperGrok session after period reset"
            );
            assert!(
                !order
                    .failover
                    .iter()
                    .any(|k| k == "console-team-prepaid-key"),
                "console omitted while free SuperGrok period headroom remains: {:?}",
                order.failover
            );

            // prefer_live must not hop to console after clear.
            let mut cfg = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: order.failover.clone(),
                session_identity_key: Some(session.into()),
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut cfg).is_none(),
                "prefer_live must leave SuperGrok after period-reset clear"
            );
            assert_eq!(cfg.api_key.as_deref(), Some(session));

            clear_all_including_durable();
            clear_included_billing_cache();
        });
    }

    /// Named contract (stale memo + remembered headroom only): even when
    /// `apply_billing` was not re-run, enrich on load with free SuperGrok
    /// period used percent below 100 must clear the memo and restore SuperGrok
    /// primary (covers shell remember-only path + process restart with durable
    /// memo + fresh billing cache).
    #[test]
    #[serial_test::serial]
    fn load_candidates_period_reset_billing_clears_stale_memo_without_apply() {
        use crate::auth::supergrok_identity_rank::order_credentials_for_preferred_auto;
        use xai_grok_sampler::clear_all_including_durable;

        with_isolated_home(|home| {
            let session = "session-jwt-stale-memo-only";
            write_oidc(home, session);
            // Prior HTTP 402 mark without going through apply.
            xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(session));
            assert!(xai_grok_sampler::is_credential_exhausted(session));

            // Billing cache only (no apply_billing): free SuperGrok period low %.
            remember_active_supergrok_included_billing(home, 3.0, None, None);

            let candidates = load_supergrok_session_candidates(home);
            assert!(
                !xai_grok_sampler::is_credential_exhausted(session),
                "load enrich must clear stale exhaust memo when free SuperGrok period used percent is below 100"
            );
            let order = order_credentials_for_preferred_auto(&candidates, &["console-key".into()]);
            assert_eq!(order.primary.as_deref(), Some(session));
            assert!(
                !order.failover.iter().any(|k| k == "console-key"),
                "console must not be in hop chain after period-reset enrich: {:?}",
                order.failover
            );

            clear_all_including_durable();
            clear_included_billing_cache();
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
        use xai_grok_sampler::clear_all_including_durable;

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::heavy-client";
        let stale = "tok-stale-exhausted-multi-slot";
        let live = "tok-live-supergrok-heavy-base";
        // Prior HTTP 402 mark on the stale SuperGrok fingerprint.
        xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(stale));

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
        use xai_grok_sampler::clear_all_including_durable;

        clear_all_including_durable();
        clear_included_billing_cache();

        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::biz-heavy";
        let stale = "tok-biz-stale-exhausted";
        let live = "tok-biz-live-heavy";
        xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(stale));
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

    /// Named contract: legacy sign-in scope is storage fallback only. When both
    /// OAuth base and legacy hold SuperGrok-session-mode tokens, free SuperGrok
    /// period ranking must not treat them as two principals (align would
    /// hot-swap the intentional new-scope primary onto legacy).
    #[test]
    #[serial_test::serial]
    fn load_supergrok_candidates_skips_legacy_scope_when_oauth_base_present() {
        use crate::auth::model::LEGACY_SCOPE;
        use chrono::Utc;

        clear_included_billing_cache();
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::rank-client";
        let mut map = AuthStore::default();
        map.insert(
            LEGACY_SCOPE.to_string(),
            GrokAuth {
                key: "legacy-key".into(),
                auth_mode: AuthMode::External,
                user_id: String::new(),
                expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
                ..Default::default()
            },
        );
        map.insert(
            base.to_string(),
            GrokAuth {
                key: "new-key".into(),
                auth_mode: AuthMode::External,
                user_id: String::new(),
                expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let candidates = load_supergrok_session_candidates(dir.path());
        assert_eq!(
            candidates.len(),
            1,
            "legacy scope must not be a free SuperGrok period dual-identity peer; got {:?}",
            candidates
                .iter()
                .map(|c| c.access_token.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(candidates[0].access_token.as_str(), "new-key");
        clear_included_billing_cache();
    }

    /// Hermetic: two SuperGrok principals in auth.json load as two rank candidates
    /// (deduped; not doubled by base + multi-slot).
    #[test]
    #[serial_test::serial]
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
            xai_grok_sampler::mark_exhausted(&grok_rate_limit::fingerprint_secret(session));
            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::None,
                "fail-open printout must not Mark; HTTP 402 mark is the seed"
            );
            assert!(
                supergrok_out_of_allowance_with_console_ready(home),
                "after HTTP 402 mark + dual-auth, meter must treat SuperGrok as out"
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
    #[serial_test::serial]
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
    #[serial_test::serial]
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
