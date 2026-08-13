//! Pure ranking among SuperGrok login identities when
//! `[auth] auto_use_included_limits` is enabled (not a `preferred_method` value).
//!
//! Prefer **included** SuperGrok limits before dollar extras / console $.
//! Among identities with included headroom, sooner reset is a ranking heuristic.
//! Meters stay distinct: personal included ≠ Business included.
//!
//! After every SuperGrok included pool is exhausted: if any principal still has
//! SuperGrok **$ extras** (`prepaid_balance_cents > 0`) and a **live** session
//! JWT, keep that SuperGrok session primary and put console keys only as
//! failover (after-burner / SuperGrok $ extras before console). Hard-expired
//! JWTs are never after-burner primary. When extras are 0 or unknown (`None`),
//! console leads as before.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// SuperGrok account role (not a `preferred_method` value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupergrokAccountRole {
    Personal,
    Business,
}

/// Snapshot of included-allowance headroom for one SuperGrok identity.
///
/// Pure input for ranking; no network, no secret store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupergrokIdentityHeadroom {
    /// Stable id for tests / multi-session store (user_id, team_id, scope key).
    pub identity_id: String,
    pub role: SupergrokAccountRole,
    /// Remaining included units (or any positive scale). `0` = exhausted.
    pub included_remaining: u64,
    /// When this identity's included allowance resets. Earlier wins when both
    /// still have headroom. `None` sorts after any known reset (unknown last).
    pub reset_at: Option<DateTime<Utc>>,
}

impl SupergrokIdentityHeadroom {
    pub fn has_included_headroom(&self) -> bool {
        self.included_remaining > 0
    }
}

/// Billing fields for one SuperGrok principal from credits poll.
///
/// Ranking uses **included** first (`usage_pct` / `reset_at`). After included is
/// exhausted, [`order_credentials_for_preferred_auto`] uses `prepaid_balance_cents`
/// (SuperGrok **Extra Usage Credits** / `GetGrokCreditsConfig.prepaidBalance`) as
/// the after-burner before console. Never console team prepaid.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludedBillingFields {
    /// Included usage percent (0.0–100.0+). `None` = billing did not provide it.
    pub usage_pct: Option<f64>,
    /// When this principal's included pool resets (UTC). `None` = unknown.
    pub reset_at: Option<DateTime<Utc>>,
    /// Billing period type proto name when known (e.g. `USAGE_PERIOD_TYPE_WEEKLY`).
    /// Used by `/limits` sibling rows so copy can say "weekly" / "monthly"
    /// instead of a bare "Included allowance". Ranking ignores this field.
    pub period_type: Option<String>,
    /// SuperGrok session Extra Usage Credits remaining (USD cents) when the
    /// credits poll returned `prepaidBalance`. `None` = not observed on this
    /// principal (honest absence). After included exhaust, `Some(cents > 0)`
    /// keeps SuperGrok session primary (after-burner); `None` / `Some(0)` yield
    /// console primary.
    pub prepaid_balance_cents: Option<i64>,
    /// Grok Build `productUsage` % when observed on a credits poll for this
    /// principal. `None` = not on wire / not remembered (honest absence).
    /// Ranking ignores this field; dual `/limits` surfaces it on sibling rows.
    pub grok_build_usage_pct: Option<f64>,
}

/// Map included usage % to ranking headroom units.
///
/// - `usage_pct >= 100` → `0` (included exhausted)
/// - else → at least `1`, or floored remaining percent when known
///
/// Honest absence: callers that lack a usage reading should not invent a
/// percent — leave candidate remaining at the memo 0|1 default instead.
pub fn included_remaining_from_usage_pct(usage_pct: f64) -> u64 {
    if usage_pct >= 100.0 {
        0
    } else {
        let rem = (100.0 - usage_pct).floor() as i64;
        rem.max(1) as u64
    }
}

/// Parse a billing period-end timestamp into UTC for ranking.
///
/// Accepts RFC 3339 (preferred). Returns `None` on empty or unparseable input
/// (honest absence — do not invent a reset).
pub fn reset_at_from_period_end(period_end: &str) -> Option<DateTime<Utc>> {
    let s = period_end.trim();
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Apply included-billing fields onto one headroom row.
///
/// Live billing `usage_pct` is authoritative for included headroom:
/// - When `usage_pct` is present, remaining always comes from
///   [`included_remaining_from_usage_pct`] (period reset / recovery wins over a
///   stale out-of-allowance memo so SuperGrok becomes primary again and console
///   is omitted from the hop chain while free period used percent is below 100).
/// - When `usage_pct` is absent and `memo_exhausted`, force remaining `0`
///   (pre-request skip without a fresh poll).
/// - When `reset_at` is present, it replaces the previous value; missing
///   leaves the existing field (often `None`).
pub fn apply_included_billing_to_headroom(
    headroom: &mut SupergrokIdentityHeadroom,
    fields: &IncludedBillingFields,
    memo_exhausted: bool,
) {
    if let Some(pct) = fields.usage_pct {
        // Live included % wins over memo (period reset must put SuperGrok back).
        headroom.included_remaining = included_remaining_from_usage_pct(pct);
    } else if memo_exhausted {
        headroom.included_remaining = 0;
    }
    if let Some(reset) = fields.reset_at {
        headroom.reset_at = Some(reset);
    }
}

/// Whether live billing usage shows free SuperGrok period headroom (used &lt; 100%).
///
/// Used to clear stale credit-exhaust memos after period reset so prefer_live
/// and mid-turn hops do not stick on console while included still has room.
pub fn usage_pct_has_included_headroom(usage_pct: Option<f64>) -> bool {
    match usage_pct {
        Some(pct) => pct < 100.0,
        None => false,
    }
}

/// Enrich session candidates from a map of identity_id → included billing.
///
/// Identities missing from the map keep their prior remaining / `reset_at`
/// (typically memo 0|1 and `None`). Copies SuperGrok $ extras when the map
/// has a prepaid reading. Pure: no process cache, no I/O.
///
/// Returns access tokens whose memo should be cleared because live billing
/// reports free SuperGrok period headroom (used percent below 100). Callers
/// that own the exhaust memo should clear those fingerprints so silent
/// prefer_live / network re-resolve do not stick on console.
pub fn enrich_candidates_with_included_billing(
    candidates: &mut [SupergrokSessionCandidate],
    by_identity: &std::collections::BTreeMap<String, IncludedBillingFields>,
    memo_exhausted: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut clear_memo_tokens = Vec::new();
    for c in candidates.iter_mut() {
        let exhausted = memo_exhausted(&c.access_token);
        if let Some(fields) = by_identity.get(&c.headroom.identity_id) {
            apply_included_billing_to_headroom(&mut c.headroom, fields, exhausted);
            if fields.prepaid_balance_cents.is_some() {
                c.prepaid_balance_cents = fields.prepaid_balance_cents;
            }
            // Period reset / recovery: live included headroom must retire the
            // out-of-allowance memo so ranking and prefer_live agree.
            if exhausted && usage_pct_has_included_headroom(fields.usage_pct) {
                clear_memo_tokens.push(c.access_token.clone());
            }
        } else if exhausted {
            c.headroom.included_remaining = 0;
        }
    }
    clear_memo_tokens
}

/// True when SuperGrok session Extra Usage Credits remain (after-burner fuel).
///
/// `None` (not observed) and `Some(0)` / non-positive are **not** after-burner
/// headroom — console may lead after included exhaust.
pub fn has_positive_supergrok_dollar_extras(prepaid_balance_cents: Option<i64>) -> bool {
    prepaid_balance_cents.map(|c| c > 0).unwrap_or(false)
}

/// Plain role label for `/limits` / footer ("personal" | "business").
pub fn role_label(role: SupergrokAccountRole) -> &'static str {
    match role {
        SupergrokAccountRole::Personal => "personal",
        SupergrokAccountRole::Business => "business",
    }
}

/// Limits panel section title for a SuperGrok principal role.
pub fn principal_limits_label(role: SupergrokAccountRole) -> String {
    match role {
        SupergrokAccountRole::Personal => "SuperGrok (personal)".into(),
        SupergrokAccountRole::Business => "SuperGrok (business)".into(),
    }
}

/// Result of ranking SuperGrok identities for `auto_use_included_limits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickSupergrokForAuto {
    /// Use this identity (has included headroom).
    Use {
        identity_id: String,
        role: SupergrokAccountRole,
    },
    /// Every known SuperGrok identity is out of included headroom.
    /// Caller may fail over to dollar extras / console if policy allows.
    ExhaustedAll,
    /// No SuperGrok identities were provided.
    NoIdentities,
}

/// Pick which SuperGrok identity to use under `auto_use_included_limits`.
///
/// Rules (included headroom only):
/// 1. Prefer identities with `included_remaining > 0`.
/// 2. Among those, earlier `reset_at` wins (sooner reset preferred).
/// 3. Missing `reset_at` sorts after known times.
/// 4. Tie-break: `identity_id` lexicographic (stable, not "Business first").
/// 5. If none have headroom but some identities exist → [`PickSupergrokForAuto::ExhaustedAll`].
pub fn pick_supergrok_identity_for_auto(
    identities: &[SupergrokIdentityHeadroom],
) -> PickSupergrokForAuto {
    if identities.is_empty() {
        return PickSupergrokForAuto::NoIdentities;
    }

    let mut with_headroom: Vec<&SupergrokIdentityHeadroom> = identities
        .iter()
        .filter(|i| i.has_included_headroom())
        .collect();

    if with_headroom.is_empty() {
        return PickSupergrokForAuto::ExhaustedAll;
    }

    with_headroom.sort_by(|a, b| {
        // Earlier reset first; None last.
        match (a.reset_at, b.reset_at) {
            (Some(ra), Some(rb)) => ra.cmp(&rb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| a.identity_id.cmp(&b.identity_id))
    });

    let best = with_headroom[0];
    PickSupergrokForAuto::Use {
        identity_id: best.identity_id.clone(),
        role: best.role,
    }
}

/// Role inferred from auth session fields (team principal → Business).
pub fn role_from_session_fields(
    principal_type: Option<&str>,
    team_id: Option<&str>,
) -> SupergrokAccountRole {
    use super::model::TEAM_PRINCIPAL_TYPE;
    if principal_type == Some(TEAM_PRINCIPAL_TYPE) && team_id.is_some() {
        SupergrokAccountRole::Business
    } else {
        SupergrokAccountRole::Personal
    }
}

/// One SuperGrok principal slot for multi-identity fixtures / future store.
///
/// Does not invent login UX. Tests build these from hand-written maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupergrokPrincipalSlot {
    pub identity_id: String,
    pub role: SupergrokAccountRole,
    /// Auth.json scope key (or other store key) for this session.
    pub store_scope: String,
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

/// List SuperGrok (OIDC/session) principal slots from an auth store map.
///
/// Skips plain API-key scopes. Used for unit fixtures until multi-login UX lands.
pub fn list_supergrok_principal_slots(
    entries: &[(String, SupergrokPrincipalSlotInput)],
) -> Vec<SupergrokPrincipalSlot> {
    entries
        .iter()
        .filter(|(_, input)| input.is_supergrok_session)
        .map(|(scope, input)| SupergrokPrincipalSlot {
            identity_id: input.identity_id.clone().unwrap_or_else(|| scope.clone()),
            role: role_from_session_fields(
                input.principal_type.as_deref(),
                input.team_id.as_deref(),
            ),
            store_scope: scope.clone(),
            user_id: input.user_id.clone(),
            team_id: input.team_id.clone(),
        })
        .collect()
}

/// Minimal input for [`list_supergrok_principal_slots`] (test/fixture friendly).
#[derive(Debug, Clone)]
pub struct SupergrokPrincipalSlotInput {
    pub is_supergrok_session: bool,
    pub user_id: String,
    pub principal_type: Option<String>,
    pub team_id: Option<String>,
    pub identity_id: Option<String>,
}

/// SuperGrok session with access token + included headroom for auto ordering.
///
/// Token is opaque (tests use fake strings; never log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupergrokSessionCandidate {
    pub headroom: SupergrokIdentityHeadroom,
    pub access_token: String,
    /// SuperGrok Extra Usage Credits remaining (USD cents) when known.
    /// Used only after included is exhausted (after-burner before console).
    /// `None` = not observed; never invent dollars.
    pub prepaid_balance_cents: Option<i64>,
    /// Wall-clock JWT hard-expired (wire would reject). Never after-burner
    /// primary even when prepaid still looks positive on a stale cache.
    pub hard_expired: bool,
}

/// Ordered SuperGrok tokens that still have included headroom (best first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoSupergrokOrder {
    pub live_tokens: Vec<String>,
    pub live_identity_ids: Vec<String>,
    /// True when candidates were non-empty but none had included headroom.
    pub exhausted_all_included: bool,
}

/// Primary + failover for `auto_use_included_limits`.
///
/// SuperGrok identities with included headroom are the only sampling chain while
/// any live SuperGrok still has included remaining: **console keys are omitted**
/// from primary and failover so a silent 429/credit hop cannot burn console
/// Grok Build $ while included weekly headroom remains.
///
/// When all SuperGrok included pools are exhausted:
/// - If any **live** SuperGrok still has **$ extras** (`prepaid_balance_cents > 0`
///   and not hard-expired): keep that SuperGrok session primary and queue console
///   keys as failover only (after-burner / SuperGrok $ extras before console).
/// - If extras are 0 or unknown, or every extras session is hard-expired: console
///   becomes primary; SuperGrok tokens are omitted (do not invent an after-burner
///   without a positive prepaid reading on a live JWT).
///
/// `primary_is_supergrok_included` is true whenever primary is a SuperGrok session
/// JWT (included headroom **or** $ extras after-burner) so auth type / host stay
/// on the SuperGrok proxy path. On after-burner both this flag and
/// `exhausted_all_supergrok_included` are true — the name means "primary is
/// SuperGrok session", not "included pool still has room".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoCredentialOrder {
    pub primary: Option<String>,
    pub failover: Vec<String>,
    /// True when primary is a SuperGrok session JWT (included **or** after-burner).
    pub primary_is_supergrok_included: bool,
    pub exhausted_all_supergrok_included: bool,
    /// Session JWT used for hop/session-host detection (first live SuperGrok,
    /// or `None` when only console remains).
    pub session_identity_key: Option<String>,
}

/// Rank SuperGrok candidates with included headroom (sooner reset first).
pub fn order_live_supergrok_for_auto(
    candidates: &[SupergrokSessionCandidate],
) -> AutoSupergrokOrder {
    if candidates.is_empty() {
        return AutoSupergrokOrder {
            live_tokens: Vec::new(),
            live_identity_ids: Vec::new(),
            exhausted_all_included: false,
        };
    }

    let headrooms: Vec<SupergrokIdentityHeadroom> =
        candidates.iter().map(|c| c.headroom.clone()).collect();
    match pick_supergrok_identity_for_auto(&headrooms) {
        PickSupergrokForAuto::NoIdentities => AutoSupergrokOrder {
            live_tokens: Vec::new(),
            live_identity_ids: Vec::new(),
            exhausted_all_included: false,
        },
        PickSupergrokForAuto::ExhaustedAll => AutoSupergrokOrder {
            live_tokens: Vec::new(),
            live_identity_ids: Vec::new(),
            exhausted_all_included: true,
        },
        PickSupergrokForAuto::Use { .. } => {
            let mut live: Vec<&SupergrokSessionCandidate> = candidates
                .iter()
                .filter(|c| c.headroom.has_included_headroom())
                .collect();
            live.sort_by(|a, b| {
                match (a.headroom.reset_at, b.headroom.reset_at) {
                    (Some(ra), Some(rb)) => ra.cmp(&rb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
                .then_with(|| a.headroom.identity_id.cmp(&b.headroom.identity_id))
            });
            AutoSupergrokOrder {
                live_tokens: live.iter().map(|c| c.access_token.clone()).collect(),
                live_identity_ids: live
                    .iter()
                    .map(|c| c.headroom.identity_id.clone())
                    .collect(),
                exhausted_all_included: false,
            }
        }
    }
}

/// Build primary/failover for `auto_use_included_limits`.
///
/// Order while any SuperGrok has included headroom: ranked SuperGrok only
/// (console keys **omitted** from the chain — limits-before-credits).
/// After every SuperGrok included pool is exhausted: SuperGrok $ extras (when
/// known positive) stay primary with console as failover; otherwise console
/// keys lead.
pub fn order_credentials_for_preferred_auto(
    sessions: &[SupergrokSessionCandidate],
    console_keys: &[String],
) -> AutoCredentialOrder {
    let ranked = order_live_supergrok_for_auto(sessions);
    let mut console: Vec<String> = console_keys
        .iter()
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
        .collect();
    // Drop console duplicates of SuperGrok tokens (should not happen, but safe).
    console.retain(|k| !ranked.live_tokens.iter().any(|t| t.trim() == k));

    if !ranked.live_tokens.is_empty() {
        let mut live = ranked.live_tokens;
        let primary = live.remove(0);
        let session_key = primary.clone();
        // Limits-before-credits: do not queue console while included headroom
        // remains. Silent failover (rate-limit / mid-turn hop) must not burn
        // console Grok Build $ until included is exhausted (after-burner or
        // console primary).
        let failover = live;
        return AutoCredentialOrder {
            primary: Some(primary),
            failover,
            primary_is_supergrok_included: true,
            exhausted_all_supergrok_included: false,
            session_identity_key: Some(session_key),
        };
    }

    // No live SuperGrok included headroom.
    let exhausted_all = ranked.exhausted_all_included || !sessions.is_empty();

    // After-burner: SuperGrok $ extras before console when known positive on a
    // live (not hard-expired) session JWT.
    let mut with_extras: Vec<&SupergrokSessionCandidate> = sessions
        .iter()
        .filter(|c| {
            !c.hard_expired && has_positive_supergrok_dollar_extras(c.prepaid_balance_cents)
        })
        .collect();
    if !with_extras.is_empty() {
        // Prefer larger remaining extras; stable id tie-break.
        with_extras.sort_by(|a, b| {
            b.prepaid_balance_cents
                .cmp(&a.prepaid_balance_cents)
                .then_with(|| a.headroom.identity_id.cmp(&b.headroom.identity_id))
        });
        let mut tokens: Vec<String> = with_extras.iter().map(|c| c.access_token.clone()).collect();
        // Drop SuperGrok tokens that collide with console key strings.
        tokens.retain(|t| !console.iter().any(|k| k == t));
        if !tokens.is_empty() {
            let primary = tokens.remove(0);
            let session_key = primary.clone();
            let mut failover = tokens;
            // Console only after SuperGrok extras chain (failover on true 402).
            console.retain(|k| k != &primary && !failover.iter().any(|t| t == k));
            failover.extend(console);
            return AutoCredentialOrder {
                primary: Some(primary),
                failover,
                // SuperGrok session primary (proxy / SessionToken); included is
                // exhausted — see struct docs for both flags true together.
                primary_is_supergrok_included: true,
                exhausted_all_supergrok_included: exhausted_all,
                session_identity_key: Some(session_key),
            };
        }
    }

    // Included full and extras 0/None → console primary.
    let mut keys = console;
    let primary = if keys.is_empty() {
        None
    } else {
        Some(keys.remove(0))
    };
    AutoCredentialOrder {
        primary,
        failover: keys,
        primary_is_supergrok_included: false,
        exhausted_all_supergrok_included: exhausted_all,
        session_identity_key: None,
    }
}

/// Re-order after one SuperGrok identity's included pool is exhausted.
///
/// Marks that identity as zero remaining, then re-runs auto order so the other
/// SuperGrok (if any headroom) is preferred before console.
pub fn order_after_supergrok_included_exhaust(
    sessions: &[SupergrokSessionCandidate],
    exhausted_identity_id: &str,
    console_keys: &[String],
) -> AutoCredentialOrder {
    let adjusted: Vec<SupergrokSessionCandidate> = sessions
        .iter()
        .map(|c| {
            let mut c = c.clone();
            if c.headroom.identity_id == exhausted_identity_id {
                c.headroom.included_remaining = 0;
            }
            c
        })
        .collect();
    order_credentials_for_preferred_auto(&adjusted, console_keys)
}

/// Whether resolve should treat preferred method as SuperGrok-session-first
/// dual-auth (oauth/oidc/unset) vs console-first (`api_key`).
pub fn preferred_is_console_primary(preferred: Option<super::config::PreferredAuthMethod>) -> bool {
    matches!(preferred, Some(super::config::PreferredAuthMethod::ApiKey))
}

/// Whether SuperGrok multi-identity ranking (included headroom first) applies.
///
/// Keys off `[auth] auto_use_included_limits`, not `preferred_method`.
/// Console-primary pin (`preferred_method = api_key`) still wins over ranking.
pub fn preferred_uses_supergrok_auto_rank(
    auto_use_included_limits: bool,
    preferred: Option<super::config::PreferredAuthMethod>,
) -> bool {
    auto_use_included_limits && !preferred_is_console_primary(preferred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).single().expect("valid ts")
    }

    fn id(
        identity_id: &str,
        role: SupergrokAccountRole,
        remaining: u64,
        reset: Option<i64>,
    ) -> SupergrokIdentityHeadroom {
        SupergrokIdentityHeadroom {
            identity_id: identity_id.into(),
            role,
            included_remaining: remaining,
            reset_at: reset.map(ts),
        }
    }

    #[test]
    fn both_have_headroom_earlier_reset_wins() {
        let personal = id(
            "personal-1",
            SupergrokAccountRole::Personal,
            100,
            Some(2_000),
        );
        let business = id(
            "business-1",
            SupergrokAccountRole::Business,
            100,
            Some(1_000),
        );
        // Business resets sooner → pick Business even though not "first" by role.
        let pick = pick_supergrok_identity_for_auto(&[personal, business]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "business-1".into(),
                role: SupergrokAccountRole::Business,
            }
        );
    }

    #[test]
    fn both_have_headroom_personal_resets_sooner() {
        let personal = id("personal-1", SupergrokAccountRole::Personal, 50, Some(500));
        let business = id(
            "business-1",
            SupergrokAccountRole::Business,
            999,
            Some(5_000),
        );
        let pick = pick_supergrok_identity_for_auto(&[business.clone(), personal.clone()]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "personal-1".into(),
                role: SupergrokAccountRole::Personal,
            }
        );
    }

    #[test]
    fn one_exhausted_other_with_headroom() {
        let personal = id("personal-1", SupergrokAccountRole::Personal, 0, Some(100));
        let business = id(
            "business-1",
            SupergrokAccountRole::Business,
            10,
            Some(9_999),
        );
        let pick = pick_supergrok_identity_for_auto(&[personal, business]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "business-1".into(),
                role: SupergrokAccountRole::Business,
            }
        );
    }

    #[test]
    fn personal_exhausted_business_used() {
        let a = id("p", SupergrokAccountRole::Personal, 0, Some(1));
        let b = id("b", SupergrokAccountRole::Business, 1, Some(100));
        assert_eq!(
            pick_supergrok_identity_for_auto(&[a, b]),
            PickSupergrokForAuto::Use {
                identity_id: "b".into(),
                role: SupergrokAccountRole::Business,
            }
        );
    }

    #[test]
    fn business_exhausted_personal_used() {
        let a = id("p", SupergrokAccountRole::Personal, 5, Some(100));
        let b = id("b", SupergrokAccountRole::Business, 0, Some(1));
        assert_eq!(
            pick_supergrok_identity_for_auto(&[a, b]),
            PickSupergrokForAuto::Use {
                identity_id: "p".into(),
                role: SupergrokAccountRole::Personal,
            }
        );
    }

    #[test]
    fn both_exhausted_signals_need_console_or_dollars() {
        let a = id("p", SupergrokAccountRole::Personal, 0, Some(1));
        let b = id("b", SupergrokAccountRole::Business, 0, Some(2));
        assert_eq!(
            pick_supergrok_identity_for_auto(&[a, b]),
            PickSupergrokForAuto::ExhaustedAll
        );
    }

    #[test]
    fn empty_is_no_identities() {
        assert_eq!(
            pick_supergrok_identity_for_auto(&[]),
            PickSupergrokForAuto::NoIdentities
        );
    }

    #[test]
    fn unknown_reset_sorts_after_known() {
        let known = id("early", SupergrokAccountRole::Personal, 1, Some(10));
        let unknown = id("late", SupergrokAccountRole::Business, 1, None);
        let pick = pick_supergrok_identity_for_auto(&[unknown, known]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "early".into(),
                role: SupergrokAccountRole::Personal,
            }
        );
    }

    #[test]
    fn equal_reset_tiebreak_by_identity_id_not_business_first() {
        let business = id("zzz-biz", SupergrokAccountRole::Business, 1, Some(100));
        let personal = id("aaa-per", SupergrokAccountRole::Personal, 1, Some(100));
        let pick = pick_supergrok_identity_for_auto(&[business, personal]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "aaa-per".into(),
                role: SupergrokAccountRole::Personal,
            },
            "must not hardcode Business first; equal reset → id order"
        );
    }

    #[test]
    fn single_with_headroom() {
        let only = id("solo", SupergrokAccountRole::Personal, 3, Some(1));
        assert_eq!(
            pick_supergrok_identity_for_auto(&[only]),
            PickSupergrokForAuto::Use {
                identity_id: "solo".into(),
                role: SupergrokAccountRole::Personal,
            }
        );
    }

    #[test]
    fn role_from_team_principal_is_business() {
        assert_eq!(
            role_from_session_fields(Some("Team"), Some("team-uuid")),
            SupergrokAccountRole::Business
        );
        assert_eq!(
            role_from_session_fields(None, None),
            SupergrokAccountRole::Personal
        );
        assert_eq!(
            role_from_session_fields(Some("Team"), None),
            SupergrokAccountRole::Personal
        );
    }

    #[test]
    fn list_slots_keeps_two_principals_skips_api_key() {
        let entries = vec![
            (
                "https://auth.x.ai::client-personal".into(),
                SupergrokPrincipalSlotInput {
                    is_supergrok_session: true,
                    user_id: "u-personal".into(),
                    principal_type: None,
                    team_id: None,
                    identity_id: Some("personal-u".into()),
                },
            ),
            (
                "https://auth.x.ai::client-team".into(),
                SupergrokPrincipalSlotInput {
                    is_supergrok_session: true,
                    user_id: "u-biz".into(),
                    principal_type: Some("Team".into()),
                    team_id: Some("team-1".into()),
                    identity_id: Some("biz-team-1".into()),
                },
            ),
            (
                "xai::api_key".into(),
                SupergrokPrincipalSlotInput {
                    is_supergrok_session: false,
                    user_id: "key".into(),
                    principal_type: None,
                    team_id: None,
                    identity_id: None,
                },
            ),
        ];
        let slots = list_supergrok_principal_slots(&entries);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].role, SupergrokAccountRole::Personal);
        assert_eq!(slots[1].role, SupergrokAccountRole::Business);
        assert_eq!(slots[1].team_id.as_deref(), Some("team-1"));
    }

    fn cand(
        identity_id: &str,
        role: SupergrokAccountRole,
        remaining: u64,
        reset: Option<i64>,
        token: &str,
    ) -> SupergrokSessionCandidate {
        cand_with_extras(identity_id, role, remaining, reset, token, None)
    }

    fn cand_with_extras(
        identity_id: &str,
        role: SupergrokAccountRole,
        remaining: u64,
        reset: Option<i64>,
        token: &str,
        prepaid_balance_cents: Option<i64>,
    ) -> SupergrokSessionCandidate {
        cand_full(
            identity_id,
            role,
            remaining,
            reset,
            token,
            prepaid_balance_cents,
            false,
        )
    }

    fn cand_full(
        identity_id: &str,
        role: SupergrokAccountRole,
        remaining: u64,
        reset: Option<i64>,
        token: &str,
        prepaid_balance_cents: Option<i64>,
        hard_expired: bool,
    ) -> SupergrokSessionCandidate {
        SupergrokSessionCandidate {
            headroom: id(identity_id, role, remaining, reset),
            access_token: token.into(),
            prepaid_balance_cents,
            hard_expired,
        }
    }

    // ── Slice 3 wire: credential order + hop (fixture dual SuperGrok) ──

    #[test]
    fn auto_order_both_headroom_earlier_reset_primary_console_last() {
        let personal = cand(
            "personal-1",
            SupergrokAccountRole::Personal,
            50,
            Some(2_000),
            "tok-personal",
        );
        let business = cand(
            "business-1",
            SupergrokAccountRole::Business,
            50,
            Some(1_000),
            "tok-business",
        );
        let order =
            order_credentials_for_preferred_auto(&[personal, business], &["console-key-1".into()]);
        assert_eq!(order.primary.as_deref(), Some("tok-business"));
        assert!(order.primary_is_supergrok_included);
        assert_eq!(
            order.failover,
            vec!["tok-personal".to_string()],
            "other SuperGrok only; console omitted while included headroom remains"
        );
        assert!(
            !order.failover.iter().any(|k| k == "console-key-1"),
            "console must not sit in failover while SuperGrok included has room"
        );
        assert_eq!(order.session_identity_key.as_deref(), Some("tok-business"));
        assert!(!order.exhausted_all_supergrok_included);
    }

    #[test]
    fn auto_order_not_business_first_when_personal_resets_sooner() {
        let personal = cand(
            "personal-1",
            SupergrokAccountRole::Personal,
            10,
            Some(100),
            "tok-p",
        );
        let business = cand(
            "business-1",
            SupergrokAccountRole::Business,
            999,
            Some(9_000),
            "tok-b",
        );
        let order = order_credentials_for_preferred_auto(&[business, personal], &["ck".into()]);
        assert_eq!(order.primary.as_deref(), Some("tok-p"));
        assert_eq!(order.failover[0], "tok-b");
    }

    #[test]
    fn auto_hop_after_one_supergrok_exhaust_uses_other_before_console() {
        let personal = cand(
            "personal-1",
            SupergrokAccountRole::Personal,
            10,
            Some(100),
            "tok-p",
        );
        let business = cand(
            "business-1",
            SupergrokAccountRole::Business,
            20,
            Some(200),
            "tok-b",
        );
        // personal was primary; its included is now exhausted → hop to business.
        let order = order_after_supergrok_included_exhaust(
            &[personal, business],
            "personal-1",
            &["console-a".into()],
        );
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-b"),
            "other SuperGrok with headroom before console $"
        );
        assert!(order.primary_is_supergrok_included);
        assert!(
            order.failover.is_empty(),
            "console omitted while sibling SuperGrok still has included headroom: {:?}",
            order.failover
        );
        assert!(!order.failover.iter().any(|k| k == "tok-p"));
        assert!(!order.failover.iter().any(|k| k == "console-a"));
    }

    #[test]
    fn auto_both_included_exhausted_console_primary_no_supergrok_primary() {
        let personal = cand(
            "personal-1",
            SupergrokAccountRole::Personal,
            0,
            Some(100),
            "tok-p",
        );
        let business = cand(
            "business-1",
            SupergrokAccountRole::Business,
            0,
            Some(200),
            "tok-b",
        );
        let order = order_credentials_for_preferred_auto(
            &[personal, business],
            &["console-1".into(), "console-2".into()],
        );
        assert_eq!(order.primary.as_deref(), Some("console-1"));
        assert!(!order.primary_is_supergrok_included);
        assert!(order.exhausted_all_supergrok_included);
        assert_eq!(order.failover, vec!["console-2".to_string()]);
        assert!(
            order.session_identity_key.is_none(),
            "do not keep SuperGrok session as hop identity when both included exhausted"
        );
        // Exhausted SuperGrok tokens without positive extras must not lead.
        assert_ne!(order.primary.as_deref(), Some("tok-p"));
        assert_ne!(order.primary.as_deref(), Some("tok-b"));
    }

    #[test]
    fn auto_single_session_headroom_omits_console_from_chain() {
        let only = cand(
            "solo",
            SupergrokAccountRole::Personal,
            5,
            Some(1),
            "tok-solo",
        );
        let order = order_credentials_for_preferred_auto(&[only], &["ck".into()]);
        assert_eq!(order.primary.as_deref(), Some("tok-solo"));
        assert!(
            order.failover.is_empty(),
            "limits-before-credits: console not in failover while included headroom remains"
        );
        assert!(!order.failover.iter().any(|k| k == "ck"));
        assert!(order.primary_is_supergrok_included);
        assert!(!order.exhausted_all_supergrok_included);
    }

    /// Named contract (Design A): while any live SuperGrok principal still has
    /// included remaining, the auto credential chain must not include console
    /// API keys as primary or silent failover.
    #[test]
    fn auto_order_omits_console_while_any_supergrok_included_headroom() {
        let live = cand(
            "team-live",
            SupergrokAccountRole::Business,
            35, // usage ~65% → remaining 35
            Some(1_000),
            "tok-team-live",
        );
        let order = order_credentials_for_preferred_auto(
            &[live],
            &["console-env-key".into(), "console-store-key".into()],
        );
        assert_eq!(order.primary.as_deref(), Some("tok-team-live"));
        assert!(order.primary_is_supergrok_included);
        assert_ne!(order.primary.as_deref(), Some("console-env-key"));
        assert_ne!(order.primary.as_deref(), Some("console-store-key"));
        assert!(
            !order
                .failover
                .iter()
                .any(|k| k == "console-env-key" || k == "console-store-key"),
            "console keys must not be in failover while included headroom: {:?}",
            order.failover
        );
        // ExhaustedAll without extras flips console primary (sibling tests cover
        // after-burner when extras remain).
        let exhausted = cand(
            "team-live",
            SupergrokAccountRole::Business,
            0,
            Some(1_000),
            "tok-team-live",
        );
        let after = order_credentials_for_preferred_auto(
            &[exhausted],
            &["console-env-key".into(), "console-store-key".into()],
        );
        assert_eq!(after.primary.as_deref(), Some("console-env-key"));
        assert!(after.exhausted_all_supergrok_included);
        assert!(!after.primary_is_supergrok_included);
    }

    /// Design A regression alias: included headroom still omits console.
    #[test]
    fn auto_with_included_headroom_still_omits_console() {
        let live = cand(
            "solo",
            SupergrokAccountRole::Personal,
            12,
            Some(500),
            "tok-headroom",
        );
        let order = order_credentials_for_preferred_auto(&[live], &["console-k".into()]);
        assert_eq!(order.primary.as_deref(), Some("tok-headroom"));
        assert!(
            order.failover.is_empty(),
            "Design A: console omitted while included has room: {:?}",
            order.failover
        );
        assert!(!order.exhausted_all_supergrok_included);
    }

    /// Included full but SuperGrok $ extras remain → stay on SuperGrok session;
    /// console only as failover (after-burner).
    #[test]
    fn auto_order_keeps_supergrok_when_included_full_but_extras_remain() {
        let session = cand_with_extras(
            "team-live",
            SupergrokAccountRole::Business,
            0, // included exhausted
            Some(1_000),
            "tok-supergrok-extras",
            Some(10_029), // $100.29 SuperGrok $ extras (cents)
        );
        let order = order_credentials_for_preferred_auto(
            &[session],
            &["console-env-key".into(), "console-store-key".into()],
        );
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-supergrok-extras"),
            "after-burner: SuperGrok session primary while $ extras remain"
        );
        assert!(
            order.primary_is_supergrok_included,
            "primary is SuperGrok session (proxy host / SessionToken)"
        );
        assert!(
            order.exhausted_all_supergrok_included,
            "included pools are exhausted; after-burner is $ extras"
        );
        assert_eq!(
            order.failover,
            vec![
                "console-env-key".to_string(),
                "console-store-key".to_string()
            ],
            "console only as failover after SuperGrok $ extras: {:?}",
            order.failover
        );
        assert_eq!(
            order.session_identity_key.as_deref(),
            Some("tok-supergrok-extras")
        );
        // Both flags true on after-burner: SuperGrok session primary + included done.
        assert!(order.primary_is_supergrok_included && order.exhausted_all_supergrok_included);
    }

    /// Hard-expired SuperGrok JWT must not lead after-burner even with positive
    /// prepaid on a stale cache — prefer console (avoid a guaranteed wire fail).
    #[test]
    fn auto_afterburner_skips_hard_expired_session_prefers_console() {
        let dead = cand_full(
            "team-dead",
            SupergrokAccountRole::Business,
            0,
            Some(1_000),
            "tok-hard-expired",
            Some(10_029),
            true, // hard_expired
        );
        let order = order_credentials_for_preferred_auto(&[dead], &["console-live-key".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some("console-live-key"),
            "hard-expired SuperGrok must not be after-burner primary: {:?}",
            order
        );
        assert!(!order.primary_is_supergrok_included);
        assert!(
            !order.failover.iter().any(|k| k == "tok-hard-expired"),
            "hard-expired SuperGrok must not queue after-burner hop: {:?}",
            order.failover
        );
    }

    /// Live SuperGrok with extras wins after-burner over a hard-expired sibling.
    #[test]
    fn auto_afterburner_prefers_live_extras_over_hard_expired_sibling() {
        let dead = cand_full(
            "dead",
            SupergrokAccountRole::Personal,
            0,
            None,
            "tok-dead",
            Some(50_000),
            true,
        );
        let live = cand_full(
            "live",
            SupergrokAccountRole::Business,
            0,
            None,
            "tok-live",
            Some(1_000),
            false,
        );
        let order = order_credentials_for_preferred_auto(&[dead, live], &["console-k".into()]);
        assert_eq!(order.primary.as_deref(), Some("tok-live"));
        assert!(order.primary_is_supergrok_included);
        assert!(
            !order.failover.iter().any(|k| k == "tok-dead"),
            "hard-expired sibling must not sit in after-burner failover: {:?}",
            order.failover
        );
        assert!(order.failover.iter().any(|k| k == "console-k"));
    }

    /// Included full and extras 0 or unknown → console primary.
    #[test]
    fn auto_after_included_and_extras_gone_console_primary() {
        let zero_extras = cand_with_extras(
            "team-live",
            SupergrokAccountRole::Business,
            0,
            Some(1_000),
            "tok-supergrok",
            Some(0),
        );
        let order_zero =
            order_credentials_for_preferred_auto(&[zero_extras], &["console-key".into()]);
        assert_eq!(order_zero.primary.as_deref(), Some("console-key"));
        assert!(!order_zero.primary_is_supergrok_included);
        assert!(order_zero.exhausted_all_supergrok_included);
        assert!(
            !order_zero.failover.iter().any(|k| k == "tok-supergrok"),
            "no SuperGrok silent extras hop when prepaid is 0: {:?}",
            order_zero.failover
        );

        let unknown_extras = cand(
            "team-live",
            SupergrokAccountRole::Business,
            0,
            Some(1_000),
            "tok-supergrok",
        );
        assert!(unknown_extras.prepaid_balance_cents.is_none());
        let order_none =
            order_credentials_for_preferred_auto(&[unknown_extras], &["console-key".into()]);
        assert_eq!(
            order_none.primary.as_deref(),
            Some("console-key"),
            "honest absence of extras → console primary (do not invent after-burner)"
        );
        assert!(!order_none.primary_is_supergrok_included);
    }

    #[test]
    fn auto_use_included_limits_flag_vs_api_key_pin() {
        use crate::auth::PreferredAuthMethod;
        assert!(preferred_uses_supergrok_auto_rank(true, None));
        assert!(preferred_uses_supergrok_auto_rank(
            true,
            Some(PreferredAuthMethod::Oidc)
        ));
        assert!(
            !preferred_uses_supergrok_auto_rank(true, Some(PreferredAuthMethod::ApiKey)),
            "api_key pin still console-primary; ranking does not override"
        );
        assert!(!preferred_uses_supergrok_auto_rank(
            false,
            Some(PreferredAuthMethod::Oidc)
        ));
        assert!(!preferred_uses_supergrok_auto_rank(false, None));
        assert!(preferred_is_console_primary(Some(
            PreferredAuthMethod::ApiKey
        )));
        assert!(!preferred_is_console_primary(Some(
            PreferredAuthMethod::Oidc
        )));
    }

    // ── Live billing fields → ranking (Track B1) ──

    #[test]
    fn included_remaining_from_usage_pct_policy() {
        assert_eq!(included_remaining_from_usage_pct(100.0), 0);
        assert_eq!(included_remaining_from_usage_pct(100.1), 0);
        assert_eq!(included_remaining_from_usage_pct(0.0), 100);
        assert_eq!(included_remaining_from_usage_pct(24.0), 76);
        assert_eq!(included_remaining_from_usage_pct(99.994), 1);
        // Floor of remaining is 0 at 99.0% used → still at least 1 (not exhausted).
        assert_eq!(included_remaining_from_usage_pct(99.0), 1);
    }

    #[test]
    fn reset_at_from_period_end_parses_rfc3339() {
        let raw = "2026-07-30T12:00:00Z";
        let dt = reset_at_from_period_end(raw).expect("parse");
        let expected = DateTime::parse_from_rfc3339(raw)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(dt, expected);
        assert!(reset_at_from_period_end("").is_none());
        assert!(reset_at_from_period_end("not-a-date").is_none());
        // Display strings are not timestamps — honest absence.
        assert!(reset_at_from_period_end("Jul 30, 12:00").is_none());
    }

    #[test]
    fn two_principals_billing_fields_sooner_reset_ranks_first() {
        // Hermetic: personal resets later with more headroom; business sooner.
        // Policy: earlier reset wins among included pools (not more-remaining).
        let personal = id("user-p", SupergrokAccountRole::Personal, 76, Some(2_000));
        let business = id("team-biz", SupergrokAccountRole::Business, 20, Some(1_000));
        let pick = pick_supergrok_identity_for_auto(&[personal, business]);
        assert_eq!(
            pick,
            PickSupergrokForAuto::Use {
                identity_id: "team-biz".into(),
                role: SupergrokAccountRole::Business,
            },
            "sooner reset + headroom beats later reset even with more remaining"
        );

        let order = order_credentials_for_preferred_auto(
            &[
                cand(
                    "user-p",
                    SupergrokAccountRole::Personal,
                    76,
                    Some(2_000),
                    "tok-p",
                ),
                cand(
                    "team-biz",
                    SupergrokAccountRole::Business,
                    20,
                    Some(1_000),
                    "tok-b",
                ),
            ],
            &["console-k".into()],
        );
        assert_eq!(order.primary.as_deref(), Some("tok-b"));
        assert_eq!(order.failover, vec!["tok-p".to_string()]);
        assert!(
            !order.failover.iter().any(|k| k == "console-k"),
            "console omitted while dual SuperGrok still have included headroom"
        );
        assert!(order.primary_is_supergrok_included);
    }

    #[test]
    fn enrich_candidates_applies_usage_and_reset_at() {
        use std::collections::BTreeMap;

        let mut candidates = vec![
            cand("user-p", SupergrokAccountRole::Personal, 1, None, "tok-p"),
            cand("team-biz", SupergrokAccountRole::Business, 1, None, "tok-b"),
        ];
        let mut fields = BTreeMap::new();
        fields.insert(
            "user-p".into(),
            IncludedBillingFields {
                usage_pct: Some(40.0),
                reset_at: Some(ts(5_000)),
                period_type: None,
                prepaid_balance_cents: None,
                grok_build_usage_pct: None,
            },
        );
        fields.insert(
            "team-biz".into(),
            IncludedBillingFields {
                usage_pct: Some(10.0),
                reset_at: Some(ts(1_000)),
                period_type: None,
                prepaid_balance_cents: None,
                grok_build_usage_pct: None,
            },
        );
        let _ = enrich_candidates_with_included_billing(&mut candidates, &fields, |_| false);

        assert_eq!(candidates[0].headroom.included_remaining, 60);
        assert_eq!(candidates[0].headroom.reset_at, Some(ts(5_000)));
        assert_eq!(candidates[1].headroom.included_remaining, 90);
        assert_eq!(candidates[1].headroom.reset_at, Some(ts(1_000)));

        let order = order_credentials_for_preferred_auto(&candidates, &["ck".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-b"),
            "business sooner reset after enrich"
        );
    }

    #[test]
    fn enrich_full_usage_exhausts_identity_memo_without_usage_forces_zero() {
        use std::collections::BTreeMap;

        let mut candidates = vec![cand(
            "user-p",
            SupergrokAccountRole::Personal,
            1,
            Some(100),
            "tok-p",
        )];
        let mut fields = BTreeMap::new();
        fields.insert(
            "user-p".into(),
            IncludedBillingFields {
                usage_pct: Some(100.0),
                reset_at: Some(ts(9_999)),
                period_type: None,
                prepaid_balance_cents: None,
                grok_build_usage_pct: None,
            },
        );
        let _ = enrich_candidates_with_included_billing(&mut candidates, &fields, |_| false);
        assert_eq!(candidates[0].headroom.included_remaining, 0);

        // Memo without a live usage reading still forces zero.
        fields.insert(
            "user-p".into(),
            IncludedBillingFields {
                usage_pct: None,
                reset_at: None,
                period_type: None,
                prepaid_balance_cents: None,
                grok_build_usage_pct: None,
            },
        );
        candidates[0].headroom.included_remaining = 1;
        let clear =
            enrich_candidates_with_included_billing(&mut candidates, &fields, |tok| tok == "tok-p");
        assert_eq!(
            candidates[0].headroom.included_remaining, 0,
            "memo-exhausted without usage_pct forces zero remaining"
        );
        assert!(
            clear.is_empty(),
            "no clear list without live free-period headroom"
        );
    }

    /// Named contract (period reset): prior exhaust memo + live free SuperGrok
    /// period used percent below 100% → SuperGrok has headroom again; console
    /// omitted from failover (limits before credits / network hop safe).
    #[test]
    fn enrich_period_reset_billing_headroom_beats_stale_exhaust_memo() {
        use std::collections::BTreeMap;

        let mut candidates = vec![cand(
            "user-p",
            SupergrokAccountRole::Personal,
            0, // memo left remaining at 0
            Some(100),
            "tok-session",
        )];
        let mut fields = BTreeMap::new();
        fields.insert(
            "user-p".into(),
            IncludedBillingFields {
                usage_pct: Some(8.0), // period reset / new period
                reset_at: Some(ts(9_999)),
                period_type: None,
                prepaid_balance_cents: None,
                grok_build_usage_pct: None,
            },
        );
        let clear = enrich_candidates_with_included_billing(&mut candidates, &fields, |tok| {
            tok == "tok-session"
        });
        assert_eq!(
            candidates[0].headroom.included_remaining, 92,
            "live free SuperGrok period used percent below 100 must restore headroom"
        );
        assert_eq!(
            clear,
            vec!["tok-session".to_string()],
            "caller must clear stale exhaust memo for this token"
        );

        let order = order_credentials_for_preferred_auto(&candidates, &["console-k".into()]);
        assert_eq!(
            order.primary.as_deref(),
            Some("tok-session"),
            "period reset → SuperGrok session primary again"
        );
        assert!(
            !order.failover.iter().any(|k| k == "console-k"),
            "console must stay omitted while free SuperGrok period headroom remains \
             (network / rate-limit hop must not burn console): {:?}",
            order.failover
        );
        assert!(order.primary_is_supergrok_included);
        assert!(!order.exhausted_all_supergrok_included);
    }

    /// Named contract (network economics): mid-turn hop chain must not include
    /// console while free SuperGrok period used percent is below 100 under
    /// auto_use ranking (connection failures retry same SuperGrok; identity
    /// rotate only walks SuperGrok→SuperGrok if multi).
    #[test]
    fn auto_order_with_included_headroom_omits_console_from_hop_chain() {
        let order = order_credentials_for_preferred_auto(
            &[cand(
                "user-p",
                SupergrokAccountRole::Personal,
                50,
                Some(1_000),
                "sg-live",
            )],
            &["console-team-key".into()],
        );
        assert_eq!(order.primary.as_deref(), Some("sg-live"));
        assert!(
            order.failover.is_empty() || !order.failover.iter().any(|k| k.contains("console")),
            "no console in failover while included headroom remains: {:?}",
            order.failover
        );
    }

    #[test]
    fn principal_limits_label_distinguishes_roles() {
        assert_eq!(
            principal_limits_label(SupergrokAccountRole::Personal),
            "SuperGrok (personal)"
        );
        assert_eq!(
            principal_limits_label(SupergrokAccountRole::Business),
            "SuperGrok (business)"
        );
        assert_eq!(role_label(SupergrokAccountRole::Personal), "personal");
        assert_eq!(role_label(SupergrokAccountRole::Business), "business");
    }
}
