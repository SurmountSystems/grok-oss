//! SuperGrok / console spend meters for `/limits` detail view.
//!
//! Single source-of-truth **view-model** for the limits panel. Pure formatters
//! take a [`LimitsSnapshot`] (or build one from cached billing) — no network.
//!
//! Meters stay distinct in all copy:
//! - SuperGrok **included** weekly/monthly allowance (percent)
//! - SuperGrok **dollar extras** (prepaid session balance)
//! - **Console team prepaid** (Management API balance when configured; else
//!   honest not-configured / loading / unavailable copy — never a soft
//!   "feature unfinished" placeholder)
//!
//! Footer / credit bar stays one-line; `/limits` is the multi-line detail.
//! Dual SuperGrok principals use [`LimitsSnapshot::extra_principals`] (stacked
//! sections) with role labels when two OIDC principals exist.

use super::credit_bar::{
    AutoTopupInfo, ConsoleTeamPrepaidGap, CreditBalance, SamplingIdentityKind,
};

/// One SuperGrok principal's meters (personal and/or business).
#[derive(Debug, Clone, PartialEq)]
pub struct PrincipalLimitsSlot {
    /// Short plain label, e.g. `"SuperGrok"` or `"SuperGrok (business)"`.
    pub label: String,
    /// Included allowance (weekly/monthly %), if known.
    pub included: Option<IncludedAllowanceMeter>,
    /// SuperGrok prepaid dollar extras, if known and positive.
    pub dollar_extras: Option<DollarExtrasMeter>,
    /// When false, dollar extras were never observed for this principal
    /// (sibling included-only poll). Format as honest absence, not "none on file".
    pub dollar_extras_observed: bool,
}

/// SuperGrok included allowance (not dollar extras, not console).
#[derive(Debug, Clone, PartialEq)]
pub struct IncludedAllowanceMeter {
    /// `"Weekly"`, `"Monthly"`, or `"Included"` when period type unknown.
    pub period_label: &'static str,
    /// Usage percent of the included pool (0.0–100.0+).
    pub used_pct: f64,
    /// Wall-clock next reset when billing provided it (local display string).
    pub next_reset_display: Option<String>,
}

impl IncludedAllowanceMeter {
    /// Floored used % (matches SpendingLimiter / `/usage` summary).
    pub fn used_pct_floored(&self) -> i64 {
        self.used_pct.floor() as i64
    }

    /// Remaining % as floor-complement of used (99.994% → 1% left).
    pub fn remaining_pct_floored(&self) -> i64 {
        (100 - self.used_pct_floored()).max(0)
    }
}

/// SuperGrok prepaid dollar extras (session billing path only).
#[derive(Debug, Clone, PartialEq)]
pub struct DollarExtrasMeter {
    /// Absolute USD cents (billing may store negative accounting cents).
    pub balance_cents: i64,
    /// Auto top-up summary line without the leading label, or `None` when unknown.
    pub auto_topup: Option<AutoTopupLine>,
}

/// How auto top-up is described under SuperGrok dollar extras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoTopupLine {
    Disabled,
    /// Enabled with optional per-trigger and monthly max (absolute cents).
    Enabled {
        topup_cents: Option<i64>,
        max_monthly_cents: Option<i64>,
    },
}

/// Console / Business API key path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleMeter {
    /// True when live sampling is on a console key.
    pub is_live: bool,
    /// Console **team prepaid** remaining USD cents from the Management API
    /// (`GET …/billing/teams/{team_id}/prepaid/balance`). `None` = use
    /// [`Self::prepaid_gap`] for honest copy. Never SuperGrok session extras.
    pub balance_cents: Option<i64>,
    /// Why dollars are absent when [`Self::balance_cents`] is `None`.
    pub prepaid_gap: ConsoleTeamPrepaidGap,
}

/// Full `/limits` view-model (single principal + console + optional extras).
#[derive(Debug, Clone, PartialEq)]
pub struct LimitsSnapshot {
    /// Which identity Build is actually burning right now.
    pub live_identity: SamplingIdentityKind,
    /// When live is SuperGrok and the principal role is known: `"personal"` or
    /// `"business"`. `None` = single-login / unknown (keep generic live line).
    pub live_principal_label: Option<String>,
    /// Primary SuperGrok principal (active / single-login first).
    pub primary: PrincipalLimitsSlot,
    /// Additional SuperGrok principals (e.g. Business when personal is primary).
    pub extra_principals: Vec<PrincipalLimitsSlot>,
    /// Console path status (always present so copy can say live / not live).
    pub console: ConsoleMeter,
}

/// One SuperGrok principal input for multi-principal `/limits` build.
#[derive(Debug, Clone)]
pub struct PrincipalLimitsInput {
    /// Section title, e.g. `"SuperGrok (personal)"`.
    pub label: String,
    /// Role tag for live line (`"personal"` / `"business"`), when known.
    pub role_label: Option<String>,
    /// Billing for this principal when known; `None` → honest "no data yet".
    pub balance: Option<CreditBalance>,
    /// Auto top-up (usually only on the principal that was polled).
    pub autotopup: Option<AutoTopupInfo>,
    /// When true, `balance` only carries included % / reset from the process
    /// included-billing cache (sibling poll). Prepaid / on-demand extras were
    /// not observed — do not render "none on file".
    pub included_billing_only: bool,
}

impl LimitsSnapshot {
    /// Build from cached billing + live sampling identity (hermetic; no I/O).
    ///
    /// `balance` / `autotopup` come from the pager cache (`CreditBalance` /
    /// `AutoTopupInfo`). Missing balance yields empty included/extras meters
    /// with honest "no data" formatting. Single SuperGrok section labeled
    /// `"SuperGrok"` (use [`Self::from_principals`] for dual rows).
    pub fn from_billing(
        balance: Option<&CreditBalance>,
        autotopup: Option<&AutoTopupInfo>,
        live_identity: SamplingIdentityKind,
    ) -> Self {
        let (included, dollar_extras) = match balance {
            Some(bal) => (
                Some(included_from_balance(bal)),
                dollar_extras_from_balance(bal, autotopup),
            ),
            None => (None, None),
        };
        Self {
            live_identity,
            live_principal_label: None,
            primary: PrincipalLimitsSlot {
                label: "SuperGrok".into(),
                included,
                dollar_extras,
                // Single-login path: full billing cache or cold "none on file".
                dollar_extras_observed: true,
            },
            extra_principals: Vec::new(),
            console: ConsoleMeter {
                is_live: live_identity.is_console(),
                // Callers attach Management prepaid via
                // [`Self::with_console_balance_cents`] when known.
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::NotConfigured,
            },
        }
    }

    /// Attach console team prepaid cents (Management API / process cache).
    ///
    /// `None` keeps the current [`ConsoleMeter::prepaid_gap`] (default
    /// not-configured). Does not touch SuperGrok meters.
    pub fn with_console_balance_cents(mut self, cents: Option<i64>) -> Self {
        self.console.balance_cents = cents;
        self
    }

    /// Set honest gap copy when cents are unknown (not-configured / loading /
    /// unavailable). Ignored for display when [`ConsoleMeter::balance_cents`]
    /// is `Some`.
    pub fn with_console_prepaid_gap(mut self, gap: ConsoleTeamPrepaidGap) -> Self {
        self.console.prepaid_gap = gap;
        self
    }

    /// Build dual (or multi) SuperGrok principal rows from hermetic inputs.
    ///
    /// First input is primary; the rest become [`Self::extra_principals`].
    /// Empty inputs fall back to a single empty SuperGrok section.
    /// `live_principal_role` is `"personal"` / `"business"` when known and
    /// live sampling is SuperGrok (shown on the Live sampling line).
    pub fn from_principals(
        principals: &[PrincipalLimitsInput],
        live_identity: SamplingIdentityKind,
        live_principal_role: Option<&str>,
    ) -> Self {
        if principals.is_empty() {
            return Self::from_billing(None, None, live_identity);
        }
        let mut slots: Vec<PrincipalLimitsSlot> = principals
            .iter()
            .map(|p| {
                let (included, dollar_extras, dollar_extras_observed) = match p.balance.as_ref() {
                    Some(bal) if p.included_billing_only => {
                        // Sibling included-only remember: fill included, leave
                        // extras as unobserved (not "none on file").
                        (Some(included_from_balance(bal)), None, false)
                    }
                    Some(bal) => (
                        Some(included_from_balance(bal)),
                        dollar_extras_from_balance(bal, p.autotopup.as_ref()),
                        true,
                    ),
                    // included_billing_only with no % yet still means extras unobserved.
                    None if p.included_billing_only => (None, None, false),
                    None => (None, None, true),
                };
                PrincipalLimitsSlot {
                    label: p.label.clone(),
                    included,
                    dollar_extras,
                    dollar_extras_observed,
                }
            })
            .collect();
        let primary = slots.remove(0);
        let live_principal_label = if live_identity.is_console() {
            None
        } else {
            live_principal_role
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .or_else(|| principals.first().and_then(|p| p.role_label.clone()))
        };
        Self {
            live_identity,
            live_principal_label,
            primary,
            extra_principals: slots,
            console: ConsoleMeter {
                is_live: live_identity.is_console(),
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::NotConfigured,
            },
        }
    }

    /// Live sampling line text (includes principal role when known).
    pub fn live_sampling_line(&self) -> String {
        match (self.live_identity, self.live_principal_label.as_deref()) {
            (SamplingIdentityKind::SuperGrokSession, Some(role)) => {
                format!("Live sampling: SuperGrok session ({role})")
            }
            (SamplingIdentityKind::SuperGrokSession, None) => {
                "Live sampling: SuperGrok session".into()
            }
            (SamplingIdentityKind::ConsoleKey, _) => "Live sampling: console key".into(),
        }
    }
}

fn included_from_balance(bal: &CreditBalance) -> IncludedAllowanceMeter {
    let period_label = match bal.period_type.as_deref() {
        Some(t) if t.contains("WEEKLY") => "Weekly",
        Some(t) if t.contains("MONTHLY") => "Monthly",
        _ => "Included",
    };
    IncludedAllowanceMeter {
        period_label,
        used_pct: bal.usage_pct,
        next_reset_display: bal.period_end_display.clone(),
    }
}

fn dollar_extras_from_balance(
    bal: &CreditBalance,
    autotopup: Option<&AutoTopupInfo>,
) -> Option<DollarExtrasMeter> {
    let balance_cents = bal.prepaid_balance_cents.map(i64::abs).filter(|c| *c > 0)?;
    let auto_topup = autotopup.map(|at| {
        if at.enabled {
            AutoTopupLine::Enabled {
                topup_cents: at.topup_amount_cents.map(i64::abs),
                max_monthly_cents: at.max_amount_cents.map(i64::abs),
            }
        } else {
            AutoTopupLine::Disabled
        }
    });
    Some(DollarExtrasMeter {
        balance_cents,
        auto_topup,
    })
}

/// Format cents as `$N` or `$N.NN` (absolute value).
fn fmt_dollars(cents: i64) -> String {
    let dollars = cents.abs() as f64 / 100.0;
    if dollars.fract() == 0.0 {
        format!("${dollars:.0}")
    } else {
        format!("${dollars:.2}")
    }
}

/// Multi-line `/limits` body. Pure; hermetic fixtures only.
pub fn format_limits_detail(snap: &LimitsSnapshot) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("Limits".to_string());
    lines.push(snap.live_sampling_line());
    lines.push(String::new());

    format_principal(&mut lines, &snap.primary);

    for extra in &snap.extra_principals {
        lines.push(String::new());
        format_principal(&mut lines, extra);
    }

    lines.push(String::new());
    format_console(&mut lines, &snap.console);

    lines.join("\n")
}

fn format_principal(lines: &mut Vec<String>, p: &PrincipalLimitsSlot) {
    lines.push(format!("{}:", p.label));
    match &p.included {
        Some(inc) => {
            let used = inc.used_pct_floored();
            let rem = inc.remaining_pct_floored();
            lines.push(format!(
                "  Included {} allowance: {}% used · {}% remaining",
                inc.period_label.to_lowercase(),
                used,
                rem
            ));
            match &inc.next_reset_display {
                Some(reset) => lines.push(format!("  Next reset: {reset}")),
                None => lines.push("  Next reset: not known yet".to_string()),
            }
        }
        None => {
            lines.push("  Included allowance: no data yet".to_string());
            lines.push("  Next reset: not known yet".to_string());
        }
    }

    match &p.dollar_extras {
        Some(d) => {
            lines.push(format!(
                "  SuperGrok dollar extras: {}",
                fmt_dollars(d.balance_cents)
            ));
            match &d.auto_topup {
                None => lines.push("  Auto topup: unknown".to_string()),
                Some(AutoTopupLine::Disabled) => lines.push("  Auto topup: disabled".to_string()),
                Some(AutoTopupLine::Enabled {
                    topup_cents,
                    max_monthly_cents,
                }) => {
                    match topup_cents {
                        Some(c) => lines.push(format!("  Auto topup: {}", fmt_dollars(*c))),
                        None => lines.push("  Auto topup: enabled".to_string()),
                    }
                    if let Some(max) = max_monthly_cents {
                        lines.push(format!("  Max monthly topup: {}", fmt_dollars(*max)));
                    }
                }
            }
        }
        None if p.dollar_extras_observed => {
            lines.push("  SuperGrok dollar extras: none on file".to_string());
        }
        None => {
            // Included-only sibling poll (or other unobserved path): do not
            // claim extras are known empty.
            lines.push("  SuperGrok dollar extras: no data yet".to_string());
        }
    }
}

fn format_console(lines: &mut Vec<String>, c: &ConsoleMeter) {
    lines.push("Console API:".to_string());
    let live = if c.is_live { "live" } else { "not live" };
    lines.push(format!("  Path: {live}"));
    match c.balance_cents {
        // Plain console team prepaid label — never "SuperGrok extras".
        Some(cents) => lines.push(format!(
            "  Balance (console team prepaid): {}",
            fmt_dollars(cents)
        )),
        None => lines.push(format!("  Balance: {}", c.prepaid_gap.as_display_str())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::credit_bar::CreditBalance;

    fn bal(pct: f64) -> CreditBalance {
        CreditBalance {
            usage_pct: pct,
            effective_usage_pct: pct,
            period_end_display: None,
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: None,
            period_type: None,
            is_unified_billing_user: None,
        }
    }

    fn weekly(pct: f64, reset: &str, prepaid: Option<i64>) -> CreditBalance {
        CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            period_end_display: Some(reset.into()),
            prepaid_balance_cents: prepaid,
            ..bal(pct)
        }
    }

    #[test]
    fn included_floors_used_and_remaining() {
        let inc = IncludedAllowanceMeter {
            period_label: "Weekly",
            used_pct: 99.994,
            next_reset_display: None,
        };
        assert_eq!(inc.used_pct_floored(), 99);
        assert_eq!(inc.remaining_pct_floored(), 1);
        let full = IncludedAllowanceMeter {
            period_label: "Weekly",
            used_pct: 100.0,
            next_reset_display: None,
        };
        assert_eq!(full.used_pct_floored(), 100);
        assert_eq!(full.remaining_pct_floored(), 0);
    }

    #[test]
    fn format_supergrok_session_with_weekly_and_extras() {
        let bal = weekly(24.0, "Jul 30, 12:00", Some(1250));
        let topup = AutoTopupInfo {
            enabled: true,
            topup_amount_cents: Some(2000),
            max_amount_cents: Some(10000),
        };
        let snap = LimitsSnapshot::from_billing(
            Some(&bal),
            Some(&topup),
            SamplingIdentityKind::SuperGrokSession,
        );
        let out = format_limits_detail(&snap);
        assert!(out.starts_with("Limits\n"), "header: {out}");
        assert!(
            out.contains("Live sampling: SuperGrok session"),
            "live identity: {out}"
        );
        assert!(
            out.contains("Included weekly allowance: 24% used · 76% remaining"),
            "included meter: {out}"
        );
        assert!(out.contains("Next reset: Jul 30, 12:00"), "reset: {out}");
        assert!(
            out.contains("SuperGrok dollar extras: $12.50"),
            "extras separate from included: {out}"
        );
        assert!(out.contains("Auto topup: $20"), "topup: {out}");
        assert!(out.contains("Max monthly topup: $100"), "max: {out}");
        assert!(out.contains("Console API:"), "console section: {out}");
        assert!(out.contains("Path: not live"), "console not live: {out}");
        assert!(
            out.contains("Balance: no management key/team id"),
            "honest console not-configured: {out}"
        );
        assert!(
            !out.contains("no $ meter yet"),
            "soft placeholder retired: {out}"
        );
        // Meters never mashed into one generic "credits" line.
        assert!(
            !out.to_lowercase().contains("credits left:"),
            "must not use generic credits mash: {out}"
        );
    }

    #[test]
    fn format_console_live_honest_no_dollar_meter() {
        let bal = weekly(100.0, "Jul 30, 12:00", Some(500));
        let snap = LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::ConsoleKey);
        let out = format_limits_detail(&snap);
        assert!(out.contains("Live sampling: console key"), "live: {out}");
        assert!(out.contains("Path: live"), "console live: {out}");
        assert!(
            out.contains("Balance: no management key/team id"),
            "no fake console $: {out}"
        );
        assert!(
            !out.contains("no $ meter yet"),
            "soft placeholder retired: {out}"
        );
        // SuperGrok meters still shown as separate (not claimed as live burn).
        assert!(
            out.contains("Included weekly allowance: 100% used · 0% remaining"),
            "included still visible: {out}"
        );
        assert!(
            out.contains("SuperGrok dollar extras: $5"),
            "extras still labeled SuperGrok: {out}"
        );
        assert!(out.contains("Auto topup: unknown"), "unknown topup: {out}");
    }

    /// Named contract: Management prepaid fixture on console live → real dollars
    /// under plain **console team prepaid** copy (never SuperGrok extras).
    #[test]
    fn console_live_with_management_fixture_shows_prepaid_balance() {
        let bal = weekly(100.0, "Jul 30, 12:00", Some(996));
        let snap = LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::ConsoleKey)
            .with_console_balance_cents(Some(12_500));
        let out = format_limits_detail(&snap);
        assert!(out.contains("Live sampling: console key"), "live: {out}");
        assert!(out.contains("Path: live"), "console live: {out}");
        assert!(
            out.contains("Balance (console team prepaid): $125"),
            "real management prepaid dollars: {out}"
        );
        assert!(
            !out.contains("no $ meter yet") && !out.contains("no management key/team id"),
            "must not claim absence when cents present: {out}"
        );
        // SuperGrok personal extras stay SuperGrok-labeled, not sold as console $.
        assert!(
            out.contains("SuperGrok dollar extras: $9.96"),
            "SuperGrok extras still labeled SuperGrok: {out}"
        );
        assert!(
            !out.contains("SuperGrok dollar extras: $125"),
            "must not mash console prepaid into SuperGrok extras: {out}"
        );
        assert!(!out.to_lowercase().contains("credits left:"), "{out}");
    }

    #[test]
    fn format_no_billing_data() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(out.contains("Included allowance: no data yet"), "{out}");
        assert!(out.contains("Next reset: not known yet"), "{out}");
        assert!(
            out.contains("SuperGrok dollar extras: none on file"),
            "{out}"
        );
        assert!(out.contains("Balance: no management key/team id"), "{out}");
        assert!(!out.contains("no $ meter yet"), "{out}");
    }

    #[test]
    fn format_console_section_distinguishes_unconfigured_from_unavailable() {
        let unconfigured =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey);
        let out_u = format_limits_detail(&unconfigured);
        assert!(
            out_u.contains("Balance: no management key/team id"),
            "{out_u}"
        );

        let unavailable =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey)
                .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Unavailable);
        let out_a = format_limits_detail(&unavailable);
        assert!(
            out_a.contains("Balance: team prepaid unavailable"),
            "{out_a}"
        );
        assert!(!out_a.contains("no $ meter yet"), "{out_a}");

        let loading = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey)
            .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Loading);
        let out_l = format_limits_detail(&loading);
        assert!(
            out_l.contains("Balance: loading team prepaid..."),
            "{out_l}"
        );
    }

    #[test]
    fn format_zero_prepaid_omits_extras_amount() {
        let bal = weekly(10.0, "Aug 1, 00:00", Some(0));
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("SuperGrok dollar extras: none on file"),
            "zero prepaid is not a positive extras meter: {out}"
        );
        assert!(!out.contains("$0"), "must not show $0 extras: {out}");
    }

    #[test]
    fn format_negative_billing_cents_as_positive_dollars() {
        let bal = weekly(50.0, "Aug 1, 00:00", Some(-750));
        let topup = AutoTopupInfo {
            enabled: false,
            topup_amount_cents: None,
            max_amount_cents: None,
        };
        let snap = LimitsSnapshot::from_billing(
            Some(&bal),
            Some(&topup),
            SamplingIdentityKind::SuperGrokSession,
        );
        let out = format_limits_detail(&snap);
        assert!(out.contains("SuperGrok dollar extras: $7.50"), "{out}");
        assert!(out.contains("Auto topup: disabled"), "{out}");
    }

    #[test]
    fn format_monthly_period_label() {
        let bal = CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_MONTHLY".into()),
            period_end_display: Some("Aug 31, 12:00".into()),
            ..bal(40.0)
        };
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Included monthly allowance: 40% used · 60% remaining"),
            "{out}"
        );
    }

    #[test]
    fn extra_principals_hook_renders_when_present() {
        let mut snap =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession);
        snap.extra_principals.push(PrincipalLimitsSlot {
            label: "SuperGrok Business".into(),
            included: Some(IncludedAllowanceMeter {
                period_label: "Weekly",
                used_pct: 5.0,
                next_reset_display: Some("soon".into()),
            }),
            dollar_extras: None,
            dollar_extras_observed: true,
        });
        let out = format_limits_detail(&snap);
        assert!(out.contains("SuperGrok Business:"), "{out}");
        assert!(
            out.contains("Included weekly allowance: 5% used · 95% remaining"),
            "{out}"
        );
    }

    #[test]
    fn from_billing_sets_console_live_from_identity() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey);
        assert!(snap.console.is_live);
        assert!(snap.console.balance_cents.is_none());
        assert!(snap.extra_principals.is_empty());
        assert!(snap.live_principal_label.is_none());
    }

    /// Dual SuperGrok principals: both pools stacked with own included % + reset.
    #[test]
    fn format_dual_principals_shows_both_pools_and_live_role() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(40.0, "Aug 1, 00:00", Some(500))),
            autotopup: None,
            included_billing_only: false,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(weekly(10.0, "Jul 30, 12:00", None)),
            autotopup: None,
            included_billing_only: false,
        };
        let snap = LimitsSnapshot::from_principals(
            &[personal, business],
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Live sampling: SuperGrok session (personal)"),
            "live principal when known: {out}"
        );
        assert!(out.contains("SuperGrok (personal):"), "{out}");
        assert!(out.contains("SuperGrok (business):"), "{out}");
        assert!(
            out.contains("Included weekly allowance: 40% used · 60% remaining"),
            "personal included: {out}"
        );
        assert!(
            out.contains("Included weekly allowance: 10% used · 90% remaining"),
            "business included: {out}"
        );
        assert!(
            out.contains("Next reset: Aug 1, 00:00"),
            "personal reset: {out}"
        );
        assert!(
            out.contains("Next reset: Jul 30, 12:00"),
            "business reset: {out}"
        );
        // Dollar extras only on personal (business had none / zero).
        assert!(
            out.contains("SuperGrok dollar extras: $5"),
            "personal extras stay SuperGrok-labeled: {out}"
        );
        // Console separate.
        assert!(out.contains("Balance: no management key/team id"), "{out}");
        assert!(!out.contains("no $ meter yet"), "{out}");
        // Must not mash meters.
        assert!(!out.to_lowercase().contains("credits left:"), "{out}");
    }

    #[test]
    fn format_dual_principals_honest_absence_for_unknown_pool() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(24.0, "Jul 30, 12:00", None)),
            autotopup: None,
            included_billing_only: false,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: None, // never polled
            autotopup: None,
            included_billing_only: false,
        };
        let snap = LimitsSnapshot::from_principals(
            &[personal, business],
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("SuperGrok (business):"),
            "second principal still listed: {out}"
        );
        assert!(
            out.contains("Included allowance: no data yet"),
            "honest absence for unpolled business: {out}"
        );
        assert!(
            out.matches("Next reset: not known yet").count() >= 1,
            "{out}"
        );
    }

    /// Sibling included-only poll fills included % but must not claim dollar
    /// extras are known empty ("none on file").
    #[test]
    fn format_sibling_included_only_extras_honest_absence() {
        let active = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(24.0, "Jul 30, 12:00", Some(1250))),
            autotopup: None,
            included_billing_only: false,
        };
        // Process cache remembered included % only (no prepaid fields).
        let sibling = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(CreditBalance {
                period_type: None,
                period_end_display: Some("Jul 28, 00:00".into()),
                prepaid_balance_cents: None,
                ..bal(40.0)
            }),
            autotopup: None,
            included_billing_only: true,
        };
        let snap = LimitsSnapshot::from_principals(
            &[active, sibling],
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Included included allowance: 40% used · 60% remaining")
                || out.contains("Included allowance: 40% used · 60% remaining")
                || out.contains("40% used · 60% remaining"),
            "sibling included from process cache: {out}"
        );
        // Active still shows real extras.
        assert!(
            out.contains("SuperGrok dollar extras: $12.50"),
            "active extras unchanged: {out}"
        );
        // Sibling must not overclaim "none on file".
        let business_section = out.split("SuperGrok (business):").nth(1).unwrap_or("");
        assert!(
            business_section.contains("SuperGrok dollar extras: no data yet"),
            "included-only sibling extras must be honest absence, not none-on-file: {out}"
        );
        assert!(
            !business_section.contains("none on file"),
            "must not claim unobserved extras empty: {out}"
        );
    }

    #[test]
    fn live_console_omits_principal_role_on_sampling_line() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(100.0, "Jul 30, 12:00", Some(100))),
            autotopup: None,
            included_billing_only: false,
        };
        let snap = LimitsSnapshot::from_principals(
            &[personal],
            SamplingIdentityKind::ConsoleKey,
            Some("personal"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Live sampling: console key"),
            "console live stays plain: {out}"
        );
        assert!(
            !out.contains("console key (personal)"),
            "do not attach SuperGrok role to console live: {out}"
        );
    }
}
