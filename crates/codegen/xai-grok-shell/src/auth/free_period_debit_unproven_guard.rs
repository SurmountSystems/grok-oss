//! Honesty + optional hard block when free SuperGrok period limits stay flat
//! (unproven debit) while still below 100% used.
//!
//! Client free-period-first path can be correct (session primary, console not
//! live, `activeDriver=supergrok_free_period`) while free SuperGrok period used
//! % never steps and team Grok Build / OAuth settlement still climbs (server
//! C4). Dogfood must not hard-stop every turn for that server gap.
//!
//! **Default: allow turns.** Loud honesty still surfaces on `/limits`, doctor
//! dual-auth status, multipoll, and a warn log at turn start when unproven.
//! **Opt-in hard block:** set
//! `[auth] allow_spend_when_free_period_debit_unproven = false` (or env
//! `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0`).
//!
//! Does **not** invent free SuperGrok period used %. Uses flat-poll history +
//! live included billing snapshot only.

use super::config::PreferredAuthMethod;
use super::included_poll_history::{
    flat_poll_unproven_debit_from_history, included_poll_history_for,
};
use super::supergrok_identity_rank::{
    preferred_is_console_primary, usage_pct_has_included_headroom,
};
use super::{included_billing_fields_snapshot, load_supergrok_session_candidates};

/// Env override for allow-spend under unproven free SuperGrok period debit.
/// When set: truthy → allow; falsy (`0` / `false` / `off` / `no` / empty after
/// trim) → block. When unset: config / default **true** (allow).
pub const ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN_ENV: &str =
    "GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN";

/// Re-export config default (true = allow under unproven free SuperGrok period debit).
pub use super::config::default_allow_spend_when_free_period_debit_unproven;

/// User-facing block copy when opt-in hard block is on (toast / acp error).
/// Complete thoughts; meters named; not framed as an Internal error.
pub fn free_period_unproven_spend_block_message() -> &'static str {
    "Blocked: free SuperGrok period limits are not debiting (flat poll) while still below 100% used. SuperGrok session traffic can still move team Grok Build / OAuth settlement dollars and SuperGrok dollar credits. Hard block is on because [auth] allow_spend_when_free_period_debit_unproven = false (or env GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0). Set that key to true (or unset the env) to allow turns under unproven free SuperGrok period debit. Run grok limits / multipoll; file the C4 xAI ticket if free SuperGrok period never steps."
}

/// Pure decision: whether to block a sampler turn.
///
/// Block when **all** of:
/// - operator has **not** allowed spend under unproven debit (`allow` false)
/// - preferred method is **not** console-primary (`api_key` pin)
/// - flat-poll marks free SuperGrok period debit **unproven**
/// - free SuperGrok period usage is **known** and still has **headroom** (used < 100%)
///
/// Does not invent usage %. Unknown usage → do not block (avoid trapping
/// cold processes without billing data).
pub fn should_block_spend_when_free_period_debit_unproven(
    allow_spend_when_unproven: bool,
    preferred_is_console: bool,
    free_period_usage_known: bool,
    free_period_has_headroom: bool,
    flat_poll_unproven: bool,
) -> bool {
    if allow_spend_when_unproven {
        return false;
    }
    if preferred_is_console {
        return false;
    }
    if !flat_poll_unproven {
        return false;
    }
    free_period_usage_known && free_period_has_headroom
}

/// Headroom snapshot for the pure guard (from billing cache and/or poll history).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FreePeriodHeadroomEvidence {
    /// At least one principal has a known free SuperGrok period used %.
    pub usage_known: bool,
    /// Any known principal still has free SuperGrok period room (used < 100%).
    pub has_headroom: bool,
}

/// Pure: combine optional billing usage readings into headroom evidence.
///
/// Empty slice → unknown (do not invent). Any `Some(pct < 100)` → headroom.
/// All known readings ≥ 100 → no headroom.
pub fn free_period_headroom_from_usage_readings(
    usage_pcts: &[Option<f64>],
) -> FreePeriodHeadroomEvidence {
    let mut usage_known = false;
    let mut has_headroom = false;
    for pct in usage_pcts {
        if pct.is_some() {
            usage_known = true;
        }
        if usage_pct_has_included_headroom(*pct) {
            has_headroom = true;
        }
    }
    FreePeriodHeadroomEvidence {
        usage_known,
        has_headroom,
    }
}

/// Live process evaluation for the product gate (I/O: config + history + cache).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreePeriodUnprovenSpendGuard {
    pub block: bool,
    pub allow_spend_when_unproven: bool,
    pub preferred_is_console: bool,
    pub free_period_usage_known: bool,
    pub free_period_has_headroom: bool,
    pub flat_poll_unproven: bool,
}

impl FreePeriodUnprovenSpendGuard {
    pub fn block_message(&self) -> Option<&'static str> {
        if self.block {
            Some(free_period_unproven_spend_block_message())
        } else {
            None
        }
    }

    /// True when free SuperGrok period debit is unproven with headroom and
    /// turns are still allowed (default dogfood path). Callers use this for
    /// warn logs / status honesty without blocking.
    pub fn honesty_unproven_allowed(&self) -> bool {
        self.flat_poll_unproven
            && self.free_period_usage_known
            && self.free_period_has_headroom
            && !self.block
            && !self.preferred_is_console
    }
}

/// Env truthy / falsy (same shape as other auth env flags).
fn env_flag_enabled(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

/// Resolve allow-spend from env (if set) then config table bools.
///
/// Env when **set** wins: truthy → true, falsy → false. Unset → config /
/// default true.
pub fn allow_spend_when_free_period_debit_unproven_from_config() -> bool {
    if let Ok(v) = std::env::var(ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN_ENV) {
        return env_flag_enabled(&v);
    }
    let Ok(value) = crate::config::load_effective_config_disk_only() else {
        return default_allow_spend_when_free_period_debit_unproven();
    };
    let table_bool = |section: &str, key: &str| -> Option<bool> {
        value
            .get(section)
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_bool())
    };
    table_bool("auth", "allow_spend_when_free_period_debit_unproven")
        .or_else(|| {
            table_bool(
                "grok_com_config",
                "allow_spend_when_free_period_debit_unproven",
            )
        })
        .unwrap_or_else(default_allow_spend_when_free_period_debit_unproven)
}

fn preferred_method_from_config() -> Option<PreferredAuthMethod> {
    let Ok(value) = crate::config::load_effective_config_disk_only() else {
        return None;
    };
    let table_str = |section: &str, key: &str| -> Option<&str> {
        value
            .get(section)
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_str())
    };
    let s = table_str("auth", "preferred_method")
        .or_else(|| table_str("grok_com_config", "preferred_method"))?;
    match s.trim().to_ascii_lowercase().as_str() {
        "api_key" => Some(PreferredAuthMethod::ApiKey),
        "oidc" => Some(PreferredAuthMethod::Oidc),
        _ => None,
    }
}

/// Free SuperGrok period headroom from live billing cache, else poll history %.
pub fn free_period_headroom_evidence_live() -> FreePeriodHeadroomEvidence {
    let billing = included_billing_fields_snapshot();
    let readings: Vec<Option<f64>> = billing.values().map(|f| f.usage_pct).collect();
    if readings.iter().any(|p| p.is_some()) {
        return free_period_headroom_from_usage_readings(&readings);
    }
    // Fall back to durable/process poll history (same ring as flat-poll).
    let home = crate::util::grok_home::grok_home();
    let candidates = load_supergrok_session_candidates(&home);
    let mut hist_readings: Vec<Option<f64>> = Vec::new();
    for c in &candidates {
        let samples = included_poll_history_for(&c.headroom.identity_id);
        hist_readings.push(samples.last().map(|s| s.credit_usage_percent));
    }
    free_period_headroom_from_usage_readings(&hist_readings)
}

/// Evaluate the live product gate (config + history + billing).
pub fn evaluate_free_period_unproven_spend_guard() -> FreePeriodUnprovenSpendGuard {
    let allow = allow_spend_when_free_period_debit_unproven_from_config();
    let preferred = preferred_method_from_config();
    let preferred_is_console = preferred_is_console_primary(preferred);
    let flat_poll_unproven = flat_poll_unproven_debit_from_history();
    let head = free_period_headroom_evidence_live();
    let block = should_block_spend_when_free_period_debit_unproven(
        allow,
        preferred_is_console,
        head.usage_known,
        head.has_headroom,
        flat_poll_unproven,
    );
    FreePeriodUnprovenSpendGuard {
        block,
        allow_spend_when_unproven: allow,
        preferred_is_console,
        free_period_usage_known: head.usage_known,
        free_period_has_headroom: head.has_headroom,
        flat_poll_unproven,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_allow_spend_is_true_dogfood() {
        assert!(
            default_allow_spend_when_free_period_debit_unproven(),
            "default must allow turns under unproven free SuperGrok period debit"
        );
    }

    #[test]
    fn pure_blocks_when_unproven_and_headroom_and_not_allowed() {
        assert!(should_block_spend_when_free_period_debit_unproven(
            false, false, true, true, true
        ));
    }

    #[test]
    fn pure_allows_when_operator_allows_default() {
        assert!(!should_block_spend_when_free_period_debit_unproven(
            true, false, true, true, true
        ));
    }

    #[test]
    fn pure_allows_console_primary_pin() {
        assert!(!should_block_spend_when_free_period_debit_unproven(
            false, true, true, true, true
        ));
    }

    #[test]
    fn pure_allows_when_poll_not_unproven() {
        assert!(!should_block_spend_when_free_period_debit_unproven(
            false, false, true, true, false
        ));
    }

    #[test]
    fn pure_allows_when_free_period_full_no_headroom() {
        // After-burner / credits path: free SuperGrok period at 100% is not
        // this guard's job.
        assert!(!should_block_spend_when_free_period_debit_unproven(
            false, false, true, false, true
        ));
    }

    #[test]
    fn pure_allows_when_usage_unknown() {
        assert!(!should_block_spend_when_free_period_debit_unproven(
            false, false, false, false, true
        ));
    }

    #[test]
    fn headroom_from_readings_six_percent() {
        let ev = free_period_headroom_from_usage_readings(&[Some(6.0), Some(6.0)]);
        assert!(ev.usage_known);
        assert!(ev.has_headroom);
    }

    #[test]
    fn headroom_from_readings_full() {
        let ev = free_period_headroom_from_usage_readings(&[Some(100.0)]);
        assert!(ev.usage_known);
        assert!(!ev.has_headroom);
    }

    #[test]
    fn headroom_from_readings_empty_unknown() {
        let ev = free_period_headroom_from_usage_readings(&[]);
        assert!(!ev.usage_known);
        assert!(!ev.has_headroom);
    }

    #[test]
    fn headroom_mixed_one_with_room() {
        let ev = free_period_headroom_from_usage_readings(&[Some(100.0), Some(6.0)]);
        assert!(ev.usage_known);
        assert!(ev.has_headroom);
    }

    #[test]
    fn block_message_names_meters_and_opt_in_block() {
        let msg = free_period_unproven_spend_block_message();
        assert!(msg.contains("free SuperGrok period"));
        assert!(msg.contains("allow_spend_when_free_period_debit_unproven"));
        assert!(msg.contains("team Grok Build") || msg.contains("OAuth"));
        assert!(msg.contains("SuperGrok dollar credits"));
        assert!(
            msg.contains("limits") || msg.contains("not debiting"),
            "must speak of free SuperGrok period limits / debit"
        );
        assert!(
            msg.contains("false") || msg.contains("Hard block"),
            "must name opt-in hard block path"
        );
        assert!(
            !msg.to_ascii_lowercase().contains("internal error"),
            "must not frame intentional gate as Internal error"
        );
    }

    #[test]
    fn evaluate_does_not_block_on_cold_empty_history() {
        // Without flat-poll evidence, guard must not fire.
        let g = FreePeriodUnprovenSpendGuard {
            block: should_block_spend_when_free_period_debit_unproven(
                false, false, true, true, false,
            ),
            allow_spend_when_unproven: false,
            preferred_is_console: false,
            free_period_usage_known: true,
            free_period_has_headroom: true,
            flat_poll_unproven: false,
        };
        assert!(!g.block);
        assert!(g.block_message().is_none());
    }

    /// Named contract (2026-08-08 dogfood): free SuperGrok period 6% flat +
    /// unproven + default allow → does **not** block sampler turns.
    #[test]
    fn multipoll_six_percent_flat_unproven_does_not_block_by_default() {
        let allow = default_allow_spend_when_free_period_debit_unproven();
        let block = should_block_spend_when_free_period_debit_unproven(
            /* allow */ allow, /* console pin */ false, /* usage known */ true,
            /* headroom at 6% */ true, /* flat unproven */ true,
        );
        assert!(
            !block,
            "multipoll flat unproven must not hard-stop turns by default (dogfood)"
        );
    }

    /// Opt-in hard block: same multipoll evidence with allow=false → block.
    #[test]
    fn multipoll_six_percent_flat_unproven_blocks_when_allow_false() {
        let block = should_block_spend_when_free_period_debit_unproven(
            /* allow */ false, /* console pin */ false, /* usage known */ true,
            /* headroom at 6% */ true, /* flat unproven */ true,
        );
        assert!(
            block,
            "opt-in hard block (allow=false) must stop turns under flat unproven + headroom"
        );
    }

    #[test]
    fn honesty_unproven_allowed_when_default_allow() {
        let g = FreePeriodUnprovenSpendGuard {
            block: false,
            allow_spend_when_unproven: true,
            preferred_is_console: false,
            free_period_usage_known: true,
            free_period_has_headroom: true,
            flat_poll_unproven: true,
        };
        assert!(g.honesty_unproven_allowed());
        assert!(g.block_message().is_none());
    }

    #[test]
    fn honesty_unproven_allowed_false_when_blocked() {
        let g = FreePeriodUnprovenSpendGuard {
            block: true,
            allow_spend_when_unproven: false,
            preferred_is_console: false,
            free_period_usage_known: true,
            free_period_has_headroom: true,
            flat_poll_unproven: true,
        };
        assert!(!g.honesty_unproven_allowed());
        assert!(g.block_message().is_some());
    }
}
