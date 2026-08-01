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
    /// Dual SuperGrok OIDC logins share one consumer SuperGrok included pool
    /// (billing `is_unified_billing_user`, and/or both rows show the same
    /// included % + reset). Not a client "mirror paint" of one slot onto the
    /// other — credentials are polled per slot; the credits API returns one
    /// pool. Also not console.x.ai Grok Business license seat/message usage.
    pub shared_unified_supergrok_pool: bool,
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
                // Default gap = missing management key (most common dogfood miss);
                // wire real gap via [`Self::with_console_prepaid_gap`].
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
            },
            // Single SuperGrok section: no dual-login shared-pool note.
            shared_unified_supergrok_pool: false,
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
        let shared_unified_supergrok_pool =
            principals.len() >= 2 && dual_principals_share_unified_supergrok_pool(principals);
        Self {
            live_identity,
            live_principal_label,
            primary,
            extra_principals: slots,
            console: ConsoleMeter {
                is_live: live_identity.is_console(),
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
            },
            shared_unified_supergrok_pool,
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

/// True when dual SuperGrok principal rows should explain a shared consumer pool.
///
/// - Any principal with `is_unified_billing_user == Some(true)`, or
/// - Two+ known included readings with the same floored % and same reset display.
///
/// Distinct included % / reset stay independent (no shared-pool note).
fn dual_principals_share_unified_supergrok_pool(principals: &[PrincipalLimitsInput]) -> bool {
    if principals.len() < 2 {
        return false;
    }
    if principals
        .iter()
        .any(|p| p.balance.as_ref().and_then(|b| b.is_unified_billing_user) == Some(true))
    {
        return true;
    }
    let known: Vec<&CreditBalance> = principals
        .iter()
        .filter_map(|p| p.balance.as_ref())
        .collect();
    if known.len() < 2 {
        return false;
    }
    let first = known[0];
    known.iter().all(|b| {
        b.usage_pct.floor() as i64 == first.usage_pct.floor() as i64
            && b.period_end_display == first.period_end_display
    })
}

/// Multi-line `/limits` body. Pure; hermetic fixtures only.
pub fn format_limits_detail(snap: &LimitsSnapshot) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("Limits".to_string());
    lines.push(snap.live_sampling_line());
    if snap.shared_unified_supergrok_pool {
        // Dogfood: dual rows both at e.g. 62% looked like a client mirror bug.
        // Live GetGrokCreditsConfig returns one unified SuperGrok pool for both
        // OIDC principals; console.x.ai Grok Business license usage is elsewhere.
        lines.push(
            "Note: SuperGrok included weekly is one shared consumer pool for this account \
(personal and business SuperGrok logins share it under unified billing). \
It is not console.x.ai Grok Business license seat/message usage."
                .to_string(),
        );
    }
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
            // period_label "Included" means unknown cycle — do not emit
            // "Included included allowance". Prefer "Included weekly/monthly
            // allowance" when the billing period type is known.
            let allowance_line = match inc.period_label {
                "Included" => format!("  Included allowance: {used}% used · {rem}% remaining"),
                other => format!(
                    "  Included {} allowance: {used}% used · {rem}% remaining",
                    other.to_lowercase()
                ),
            };
            lines.push(allowance_line);
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
            out.contains("Balance: no management key"),
            "honest console missing key: {out}"
        );
        assert!(
            !out.contains("no management key/team id"),
            "mushy combined gap retired: {out}"
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
            out.contains("Balance: no management key"),
            "no fake console $: {out}"
        );
        assert!(
            !out.contains("no management key/team id"),
            "mushy combined gap retired: {out}"
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
            !out.contains("no $ meter yet")
                && !out.contains("no management key/team id")
                && !out.contains("no management key")
                && !out.contains("no management team id"),
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
        assert!(out.contains("Balance: no management key"), "{out}");
        assert!(!out.contains("no management key/team id"), "{out}");
        assert!(!out.contains("no $ meter yet"), "{out}");
    }

    #[test]
    fn format_console_section_distinguishes_missing_key_team_loading_unavailable() {
        // Named contract: five operator-visible console prepaid states stay distinct.
        let missing_key =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey);
        let out_k = format_limits_detail(&missing_key);
        assert!(
            out_k.contains("Balance: no management key"),
            "default missing key: {out_k}"
        );
        assert!(
            !out_k.contains("no management key/team id"),
            "mushy combined retired: {out_k}"
        );

        let missing_team =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey)
                .with_console_prepaid_gap(ConsoleTeamPrepaidGap::MissingTeamId);
        let out_t = format_limits_detail(&missing_team);
        assert!(
            out_t.contains("Balance: no management team id"),
            "missing team: {out_t}"
        );
        assert!(
            !out_t.contains("Balance: no management key\n")
                && !out_t.contains("Balance: no management key"),
            "must not mash missing team into missing key: {out_t}"
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

        let with_dollars =
            LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey)
                .with_console_balance_cents(Some(2500));
        let out_d = format_limits_detail(&with_dollars);
        assert!(
            out_d.contains("Balance (console team prepaid): $25"),
            "{out_d}"
        );
        assert!(!out_d.contains("no management key"), "{out_d}");
        assert!(!out_d.contains("no management team id"), "{out_d}");
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
        assert!(out.contains("Balance: no management key"), "{out}");
        assert!(!out.contains("no management key/team id"), "{out}");
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
        // Process cache remembered included % + weekly period (no prepaid).
        let sibling = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(CreditBalance {
                period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                period_end_display: Some("July 28, 00:00".into()),
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
            !out.contains("Included included allowance"),
            "must never double the word included: {out}"
        );
        assert!(
            out.contains("Included weekly allowance: 40% used · 60% remaining"),
            "sibling included from process cache uses weekly when period known: {out}"
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

    /// Named contract: unknown period type → plain "Included allowance", never
    /// "Included included allowance" (the double-word dogfood bug).
    #[test]
    fn format_unknown_period_does_not_double_included_word() {
        let slot = PrincipalLimitsSlot {
            label: "SuperGrok (personal)".into(),
            included: Some(IncludedAllowanceMeter {
                period_label: "Included",
                used_pct: 62.0,
                next_reset_display: Some("Aug 3, 19:25".into()),
            }),
            dollar_extras: None,
            dollar_extras_observed: false,
        };
        let snap = LimitsSnapshot {
            live_identity: SamplingIdentityKind::SuperGrokSession,
            live_principal_label: Some("business".into()),
            primary: PrincipalLimitsSlot {
                label: "SuperGrok (business)".into(),
                included: Some(IncludedAllowanceMeter {
                    period_label: "Weekly",
                    used_pct: 62.0,
                    next_reset_display: Some("August 3, 19:25".into()),
                }),
                dollar_extras: Some(DollarExtrasMeter {
                    balance_cents: 10029,
                    auto_topup: Some(AutoTopupLine::Disabled),
                }),
                dollar_extras_observed: true,
            },
            extra_principals: vec![slot],
            console: ConsoleMeter {
                is_live: false,
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
            },
            // This fixture exercises double-"included" copy only; shared-pool
            // note is covered by dedicated dual-unified tests.
            shared_unified_supergrok_pool: false,
        };
        let out = format_limits_detail(&snap);
        assert!(
            !out.contains("Included included allowance"),
            "copy bug: double 'included' is forbidden: {out}"
        );
        assert!(
            out.contains("Included weekly allowance: 62% used · 38% remaining"),
            "business weekly line: {out}"
        );
        assert!(
            out.contains("Included allowance: 62% used · 38% remaining"),
            "personal unknown-period still honest, not doubled: {out}"
        );
        let personal = out.split("SuperGrok (personal):").nth(1).unwrap_or("");
        assert!(
            personal.contains("SuperGrok dollar extras: no data yet"),
            "sibling extras honest absence: {out}"
        );
    }

    /// Named contract: dual SuperGrok rows keep **per-slot** included % — never
    /// mirror the active principal's meter onto the sibling row.
    #[test]
    fn format_dual_principals_keep_distinct_included_pct() {
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", Some(10029))),
            autotopup: Some(AutoTopupInfo {
                enabled: false,
                topup_amount_cents: None,
                max_amount_cents: None,
            }),
            included_billing_only: false,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            // Distinct sibling pool: 15% used, different reset.
            balance: Some(weekly(15.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(
            !snap.shared_unified_supergrok_pool,
            "distinct included % must not claim a shared unified pool"
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Included weekly allowance: 62% used · 38% remaining"),
            "business slot own %: {out}"
        );
        assert!(
            out.contains("Included weekly allowance: 15% used · 85% remaining"),
            "personal slot own % (must not reuse business 62%): {out}"
        );
        // Count exact 62% lines — only business should carry it.
        let sixty_two = out.matches("62% used").count();
        assert_eq!(
            sixty_two, 1,
            "62% must appear once (business only), not mirrored: {out}"
        );
        let business_sec = out.split("SuperGrok (business):").nth(1).unwrap_or("");
        let personal_sec = out.split("SuperGrok (personal):").nth(1).unwrap_or("");
        // Truncate each section to its body (before next blank+header is fine).
        assert!(
            business_sec.contains("SuperGrok dollar extras: $100.29"),
            "business extras: {out}"
        );
        assert!(
            personal_sec.contains("SuperGrok dollar extras: no data yet"),
            "personal included-only: no invented dollars: {out}"
        );
        assert!(!out.contains("Included included allowance"), "{out}");
        assert!(
            !out.contains("shared consumer pool"),
            "distinct pools: no unified-share note: {out}"
        );
    }

    /// Named contract (dogfood 62%/62%): when both SuperGrok OIDC slots report
    /// the same included % + reset under unified billing, /limits must say they
    /// share one SuperGrok consumer pool — not look like a silent client mirror —
    /// and must name that this is not console Grok Business license usage.
    #[test]
    fn format_dual_unified_same_included_explains_shared_pool_not_console_business() {
        let mut business_bal = weekly(62.0, "August 3, 19:25", Some(10029));
        business_bal.is_unified_billing_user = Some(true);
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(business_bal),
            autotopup: Some(AutoTopupInfo {
                enabled: false,
                topup_amount_cents: None,
                max_amount_cents: None,
            }),
            included_billing_only: false,
        };
        // Sibling poll: same included % + reset the credits API returns for the
        // personal OIDC token under unified billing (distinct token, same pool).
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(
            snap.shared_unified_supergrok_pool,
            "unified flag must mark shared SuperGrok pool"
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("shared consumer pool"),
            "must explain shared SuperGrok pool: {out}"
        );
        assert!(
            out.contains("unified billing"),
            "must name unified billing: {out}"
        );
        assert!(
            out.contains("not console.x.ai Grok Business license"),
            "must distinguish SuperGrok included from console Business usage: {out}"
        );
        // Both rows still show their (same) per-slot readings — not collapsed.
        assert_eq!(
            out.matches("62% used").count(),
            2,
            "both slots keep their own 62% reading from each poll: {out}"
        );
        assert!(out.contains("SuperGrok (business):"), "{out}");
        assert!(out.contains("SuperGrok (personal):"), "{out}");
    }

    /// Same 62%/62% without the unified flag still gets the shared-pool note
    /// (identical included % + reset is the dogfood signal).
    #[test]
    fn format_dual_identical_included_without_flag_still_explains_shared_pool() {
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", Some(10029))),
            autotopup: None,
            included_billing_only: false,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(snap.shared_unified_supergrok_pool);
        let out = format_limits_detail(&snap);
        assert!(out.contains("shared consumer pool"), "{out}");
        assert!(
            out.contains("not console.x.ai Grok Business license"),
            "{out}"
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
