//! SuperGrok / console spend meters for `/limits` detail view.
//!
//! Single source-of-truth **view-model** for the limits panel. Pure formatters
//! take a [`LimitsSnapshot`] (or build one from cached billing) — no network.
//!
//! Meters stay distinct in all copy:
//! - SuperGrok **included** weekly/monthly allowance (percent)
//! - SuperGrok **dollar extras** (prepaid session balance)
//! - **Console team prepaid** (Management API balance when configured; else
//!   honest not-configured / loading / unavailable copy, never a soft
//!   "feature unfinished" placeholder)
//! - **Console team postpaid** OAuth vs API class (invoice preview; distinct
//!   from prepaid remaining and SuperGrok $ extras)
//!
//! Footer / credit bar stays one-line; `/limits` is the multi-line detail.
//! Dual SuperGrok principals use [`LimitsSnapshot::extra_principals`] (stacked
//! sections) with role labels when two OIDC principals exist.

use super::credit_bar::{
    AutoTopupInfo, ConsoleTeamPrepaidGap, CreditBalance, SamplingIdentityKind,
};

/// Where a SuperGrok free-period included % reading came from.
///
/// Keeps dual unified fill honest: a filled row is not a successful poll of
/// that principal's JWT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IncludedSource {
    /// Not known (cold single-login / no reading).
    #[default]
    Unknown,
    /// Credits poll succeeded for this principal this run / active cache.
    LivePoll,
    /// Process included-billing cache (sibling remember), not a fill copy.
    ProcessCache,
    /// Copied from sibling under unified SuperGrok pool fill.
    SharedPoolFill,
}

impl IncludedSource {
    /// Wire value for `limits --json` (`includedSource`).
    pub fn as_wire(self) -> Option<&'static str> {
        match self {
            Self::Unknown => None,
            Self::LivePoll => Some("live_poll"),
            Self::ProcessCache => Some("process_cache"),
            Self::SharedPoolFill => Some("shared_pool_fill"),
        }
    }
}

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
    /// Grok Build `productUsage` % when observed on a credits poll for this
    /// principal. `None` when not on wire / sibling cache-only.
    pub grok_build_usage_pct: Option<f64>,
    /// True when this principal's JWT polled credits successfully this run
    /// (or active path known OK). False when poll failed or slot was only
    /// filled from the shared pool / cold.
    pub poll_succeeded: bool,
    /// Provenance for the free SuperGrok period included % on this row.
    pub included_source: IncludedSource,
    /// Short poll fail class when known (`auth`, `network`, `other`).
    pub poll_error_class: Option<&'static str>,
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
    /// Absolute next reset (UTC) for live countdown; `None` when unknown.
    pub next_reset_at: Option<chrono::DateTime<chrono::Utc>>,
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

    /// Remaining fraction of included allowance (0.0 = empty, 1.0 = full).
    pub fn remaining_fraction(&self) -> f32 {
        (self.remaining_pct_floored() as f32 / 100.0).clamp(0.0, 1.0)
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

/// Console team postpaid invoice preview aggregates (Management API M3).
///
/// Distinct from [`ConsoleMeter::balance_cents`] (prepaid remaining) and from
/// SuperGrok $ extras. Amounts are non-negative USD cents for the current
/// invoice period.
///
/// [`Self::default_credits_cents`] is the dashboard-class **team default credits**
/// allotment (often ~$1500 on the wire). It is **not** the prepaid wallet,
/// **not** free SuperGrok period allowance, and **not** SuperGrok top-up dollars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleTeamPostpaidMeter {
    pub period_total_cents: i64,
    pub oauth_class_cents: i64,
    pub api_class_cents: i64,
    pub other_class_cents: i64,
    /// Team default credits (dashboard allotment) in USD cents when present.
    pub default_credits_cents: Option<i64>,
}

impl ConsoleTeamPostpaidMeter {
    /// True when OAuth class spend is strictly greater than API class and &gt; 0.
    pub fn oauth_class_dominates(&self) -> bool {
        self.oauth_class_cents > 0 && self.oauth_class_cents > self.api_class_cents
    }

    /// Build from a shell Management postpaid preview meter.
    pub fn from_preview(p: &xai_grok_shell::auth::ConsoleTeamPostpaidPreview) -> Self {
        Self {
            period_total_cents: p.period_total_cents,
            oauth_class_cents: p.oauth_class_cents,
            api_class_cents: p.api_class_cents,
            other_class_cents: p.other_class_cents,
            default_credits_cents: p.default_credits_cents,
        }
    }
}

/// One description row on the Management usage series (spend over a day window).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTeamUsageSeriesRow {
    pub label: String,
    /// Plain class label: `oauth_grok_build`, `api_key`, or `other`.
    pub class_wire: &'static str,
    pub total_usd: f64,
}

/// Management POST usage analytics summary for `/limits` (not prepaid, not SuperGrok).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTeamUsageSeriesSummary {
    pub start_time: String,
    pub end_time: String,
    pub timezone: String,
    pub oauth_class_usd: f64,
    pub api_class_usd: f64,
    pub other_class_usd: f64,
    /// Top description rows by total (already sorted descending in shell).
    pub top_rows: Vec<ConsoleTeamUsageSeriesRow>,
    pub limit_reached: bool,
}

impl ConsoleTeamUsageSeriesSummary {
    /// Build from a shell Management usage series meter (keeps top 5 rows).
    pub fn from_series(s: &xai_grok_shell::auth::ConsoleTeamUsageSeries) -> Self {
        let top_rows = s
            .rows
            .iter()
            .take(5)
            .map(|r| ConsoleTeamUsageSeriesRow {
                label: r.label.clone(),
                class_wire: match r.class {
                    xai_grok_shell::auth::PostpaidLineClass::Oauth => "oauth_grok_build",
                    xai_grok_shell::auth::PostpaidLineClass::Api => "api_key",
                    xai_grok_shell::auth::PostpaidLineClass::Other => "other",
                },
                total_usd: r.total_usd,
            })
            .collect();
        Self {
            start_time: s.start_time.clone(),
            end_time: s.end_time.clone(),
            timezone: s.timezone.clone(),
            oauth_class_usd: s.oauth_class_usd,
            api_class_usd: s.api_class_usd,
            other_class_usd: s.other_class_usd,
            top_rows,
            limit_reached: s.limit_reached,
        }
    }

    pub fn period_total_usd(&self) -> f64 {
        self.oauth_class_usd + self.api_class_usd + self.other_class_usd
    }
}

/// Why team postpaid dollars are absent (honest gap; never invent $).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleTeamPostpaidGap {
    /// No Management API key configured.
    #[default]
    MissingManagementKey,
    /// Key present but team id unknown.
    MissingTeamId,
    /// Key+team known but fetch failed / empty body.
    Unavailable,
}

impl ConsoleTeamPostpaidGap {
    /// Stable wire value for `limits --json`.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::MissingManagementKey => "no_management_key",
            Self::MissingTeamId => "no_management_team_id",
            Self::Unavailable => "team_postpaid_unavailable",
        }
    }

    /// Short human gap (console section).
    pub fn as_display_str(self) -> &'static str {
        match self {
            Self::MissingManagementKey => "needs management key",
            Self::MissingTeamId => "needs team id",
            Self::Unavailable => "team postpaid unavailable",
        }
    }

    /// Gap after a billing attempt when cents are still unknown.
    pub fn after_billing_fetch(has_mgmt_key: bool, has_mgmt_team: bool) -> Self {
        if !has_mgmt_key {
            Self::MissingManagementKey
        } else if !has_mgmt_team {
            Self::MissingTeamId
        } else {
            Self::Unavailable
        }
    }
}

/// Console / Business API key path.
///
/// `PartialEq` only: [`Self::usage_series`] carries USD floats (docs series
/// values), so `Eq` would be dishonest.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleMeter {
    /// True when live sampling is on a console key.
    pub is_live: bool,
    /// True when an **inference** console / Business API key exists (env or
    /// secret store). Distinct from live sampling and from the Management API
    /// key used for team prepaid balance.
    pub key_available: bool,
    /// Console **team prepaid** remaining USD cents from the Management API
    /// (`GET …/billing/teams/{team_id}/prepaid/balance`). `None` = use
    /// [`Self::prepaid_gap`] for honest copy. Never SuperGrok session extras.
    pub balance_cents: Option<i64>,
    /// Why dollars are absent when [`Self::balance_cents`] is `None`.
    pub prepaid_gap: ConsoleTeamPrepaidGap,
    /// Team postpaid invoice preview (OAuth vs API class). Distinct from
    /// prepaid remaining. `None` + [`Self::postpaid_gap`] when unknown.
    pub postpaid: Option<ConsoleTeamPostpaidMeter>,
    /// Why postpaid is absent when [`Self::postpaid`] is `None`.
    pub postpaid_gap: ConsoleTeamPostpaidGap,
    /// Optional Management usage spend series (POST analytics) for a day window.
    /// Distinct from period postpaid totals and from prepaid remaining.
    pub usage_series: Option<ConsoleTeamUsageSeriesSummary>,
}

impl ConsoleMeter {
    /// Console **chat/API key** status for `/limits` (not Management prepaid).
    ///
    /// Key presence is implicit when a request path is shown:
    /// - key on file + console serving → `Requests: console`
    /// - key on file + SuperGrok serving → `Requests: SuperGrok`
    /// - no console chat key → `no key`
    ///
    /// Management key absence belongs only on the Balance line.
    pub fn key_status_line(&self) -> &'static str {
        if self.is_live {
            // Live console burn implies the key is present and handling requests.
            "Requests: console"
        } else if self.key_available {
            "Requests: SuperGrok"
        } else {
            "no key"
        }
    }
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
    /// Console key + team prepaid (key status separate from Balance gap).
    pub console: ConsoleMeter,
    /// Dual SuperGrok OIDC logins share one consumer SuperGrok included pool
    /// (billing `is_unified_billing_user`, and/or both rows show the same
    /// included % + reset). Not a client "mirror paint" of one slot onto the
    /// other — credentials are polled per slot; the credits API returns one
    /// pool. Also not console.x.ai Grok Business license seat/message usage.
    pub shared_unified_supergrok_pool: bool,
    /// When true, emit the optional flat-poll honesty note (included debit
    /// unproven). Caller supplies evidence — do not invent inference counters.
    pub flat_poll_unproven_debit: bool,
    /// True when the flat window observed Grok Build product % (name it in the
    /// note only when true). Ignored when [`Self::flat_poll_unproven_debit`] is
    /// false.
    pub flat_poll_observed_build: bool,
    /// True when the flat window observed SuperGrok $ extras. Ignored when
    /// [`Self::flat_poll_unproven_debit`] is false.
    pub flat_poll_observed_extras: bool,
}

/// One SuperGrok principal input for multi-principal `/limits` build.
#[derive(Debug, Clone, Default)]
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
    /// Live credits poll succeeded for this principal this collect. `None` =
    /// unknown (legacy callers); `Some(false)` = failed; `Some(true)` = OK.
    pub poll_succeeded: Option<bool>,
    /// Short fail class when poll failed (`auth`, `network`, `other`).
    pub poll_error_class: Option<&'static str>,
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
        let included_source = if included.is_some() {
            IncludedSource::LivePoll
        } else {
            IncludedSource::Unknown
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
                grok_build_usage_pct: balance.and_then(|b| b.grok_build_usage_pct),
                poll_succeeded: balance.is_some(),
                included_source,
                poll_error_class: None,
            },
            extra_principals: Vec::new(),
            console: ConsoleMeter {
                is_live: live_identity.is_console(),
                // Default unknown; wire via [`Self::with_console_key_available`].
                key_available: live_identity.is_console(),
                // Callers attach Management prepaid via
                // [`Self::with_console_balance_cents`] when known.
                // Default gap = missing management key (most common dogfood miss);
                // wire real gap via [`Self::with_console_prepaid_gap`].
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
                postpaid: None,
                postpaid_gap: ConsoleTeamPostpaidGap::MissingManagementKey,
                usage_series: None,
            },
            // Single SuperGrok section: no dual-login shared-pool note.
            shared_unified_supergrok_pool: false,
            flat_poll_unproven_debit: false,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
        }
    }

    /// Mark optional flat-poll honesty (included debit unproven under load).
    ///
    /// Only set when product has poll evidence. Default false (no invented
    /// inference counters). Does **not** invent Build/extras observed flags;
    /// use [`Self::with_flat_poll_observed_meters`] when history saw those
    /// fields.
    pub fn with_flat_poll_unproven_debit(mut self, flat: bool) -> Self {
        self.flat_poll_unproven_debit = flat;
        if !flat {
            self.flat_poll_observed_build = false;
            self.flat_poll_observed_extras = false;
        }
        self
    }

    /// Which optional meters were observed flat in the poll window (Issue 1).
    ///
    /// Only names Build / SuperGrok $ extras in honesty copy when the matching
    /// flag is true. Safe default for both is false.
    pub fn with_flat_poll_observed_meters(mut self, build: bool, extras: bool) -> Self {
        self.flat_poll_observed_build = build;
        self.flat_poll_observed_extras = extras;
        self
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

    /// Whether an inference console / Business API key is available (store or
    /// env). Live sampling still uses [`ConsoleMeter::is_live`].
    pub fn with_console_key_available(mut self, available: bool) -> Self {
        self.console.key_available = available || self.console.is_live;
        self
    }

    /// Attach console team postpaid invoice preview (Management API M3).
    ///
    /// `None` keeps the current [`ConsoleMeter::postpaid_gap`]. Does not touch
    /// prepaid remaining or SuperGrok meters.
    pub fn with_console_postpaid(mut self, postpaid: Option<ConsoleTeamPostpaidMeter>) -> Self {
        self.console.postpaid = postpaid;
        self
    }

    /// Set honest postpaid gap when preview is unknown.
    pub fn with_console_postpaid_gap(mut self, gap: ConsoleTeamPostpaidGap) -> Self {
        self.console.postpaid_gap = gap;
        self
    }

    /// Attach Management usage spend series (POST analytics window summary).
    ///
    /// Does not touch prepaid remaining, postpaid period totals, or SuperGrok.
    pub fn with_console_usage_series(
        mut self,
        series: Option<ConsoleTeamUsageSeriesSummary>,
    ) -> Self {
        self.console.usage_series = series;
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
                let (included, dollar_extras, dollar_extras_observed, included_source) = match p
                    .balance
                    .as_ref()
                {
                    Some(bal) if p.included_billing_only => {
                        // Sibling process-cache path: included from remember.
                        // Dollar extras only when prepaidBalance was observed
                        // on this principal's credits poll (not invented).
                        let extras = dollar_extras_from_balance(bal, p.autotopup.as_ref());
                        let extras_observed = bal.prepaid_balance_cents.is_some();
                        (
                            Some(included_from_balance(bal)),
                            extras,
                            extras_observed,
                            IncludedSource::ProcessCache,
                        )
                    }
                    Some(bal) => (
                        Some(included_from_balance(bal)),
                        dollar_extras_from_balance(bal, p.autotopup.as_ref()),
                        true,
                        IncludedSource::LivePoll,
                    ),
                    // included_billing_only with no % yet still means extras unobserved.
                    None if p.included_billing_only => (None, None, false, IncludedSource::Unknown),
                    None => (None, None, true, IncludedSource::Unknown),
                };
                let poll_succeeded = p.poll_succeeded.unwrap_or_else(|| {
                    // Legacy callers: balance present and not process-cache-only
                    // implies live poll; included_billing_only without explicit
                    // flag stays false (cache path).
                    p.balance.is_some() && !p.included_billing_only
                });
                PrincipalLimitsSlot {
                    label: p.label.clone(),
                    included,
                    dollar_extras,
                    dollar_extras_observed,
                    grok_build_usage_pct: p.balance.as_ref().and_then(|b| b.grok_build_usage_pct),
                    poll_succeeded,
                    included_source,
                    poll_error_class: p.poll_error_class,
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
        // Shared pool: wire unified flag and/or matching live_poll successes.
        // Do not invent "shared pool" solely from fill onto a failed principal
        // without any successful live reading (dual_principals helper already
        // requires known balances or wire flag).
        let shared_unified_supergrok_pool =
            principals.len() >= 2 && dual_principals_share_unified_supergrok_pool(principals);
        // Unified pool + sibling never polled: show the same included reading
        // (honest same pool), not forever-empty personal/business rows. Dollar
        // extras stay unobserved until that principal's full billing is seen.
        // Filled slots get included_source = SharedPoolFill (not live_poll).
        let (primary, slots) = if shared_unified_supergrok_pool {
            let (primary, slots) = fill_unified_included_on_empty_slots(primary, slots);
            // Same Extra Usage Credits pool under unified billing: when any
            // principal observed prepaidBalance, show it on all SuperGrok rows
            // (not "no data yet" that implies missing SuperGrok $).
            fill_unified_dollar_extras_on_empty_slots(primary, slots)
        } else {
            (primary, slots)
        };
        Self {
            live_identity,
            live_principal_label,
            primary,
            extra_principals: slots,
            console: ConsoleMeter {
                is_live: live_identity.is_console(),
                key_available: live_identity.is_console(),
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
                postpaid: None,
                postpaid_gap: ConsoleTeamPostpaidGap::MissingManagementKey,
                usage_series: None,
            },
            shared_unified_supergrok_pool,
            flat_poll_unproven_debit: false,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
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

/// **Active:** line for human `/limits` (same Design A driver as status chrome).
///
/// Names free SuperGrok period, SuperGrok extras after-burner, or console key.
/// Does not name team Grok Build settlement or console team prepaid as the
/// active driver (those stay distinct meters below).
pub fn active_driver_line_for_snapshot(snap: &LimitsSnapshot) -> String {
    use super::credit_bar::active_spend_driver;

    let included_known = snap.primary.included.is_some();
    let included_pct = snap
        .primary
        .included
        .as_ref()
        .map(|i| i.used_pct)
        .unwrap_or(0.0);
    let extras_cents = snap.primary.dollar_extras.as_ref().map(|d| d.balance_cents);
    let driver = active_spend_driver(
        snap.live_identity,
        included_known,
        included_pct,
        extras_cents,
    );
    format!("Active: {}", driver.as_human())
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
        next_reset_at: bal.period_end_at,
    }
}

/// Live countdown to next reset: `Xd Yh Zm Ws` (or `0d 0h 0m 0s` when past).
///
/// Pure; tests inject `now`. Days can be zero. Always includes all four units
/// so the modal can tick in place without layout jump.
pub fn format_reset_countdown(
    now: chrono::DateTime<chrono::Utc>,
    reset_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let remaining = reset_at.signed_duration_since(now);
    let total_secs = remaining.num_seconds().max(0) as u64;
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let mins = (total_secs % 3_600) / 60;
    let secs = total_secs % 60;
    format!("{days}d {hours}h {mins}m {secs}s")
}

/// True when the live countdown has reached zero (reset time is now or past).
pub fn countdown_is_zero(
    now: chrono::DateTime<chrono::Utc>,
    reset_at: chrono::DateTime<chrono::Utc>,
) -> bool {
    reset_at.signed_duration_since(now).num_seconds() <= 0
}

/// Earliest absolute next-reset across all SuperGrok principal rows, if any.
pub fn earliest_reset_at(snap: &LimitsSnapshot) -> Option<chrono::DateTime<chrono::Utc>> {
    let mut times: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    if let Some(t) = snap.primary.included.as_ref().and_then(|i| i.next_reset_at) {
        times.push(t);
    }
    for extra in &snap.extra_principals {
        if let Some(t) = extra.included.as_ref().and_then(|i| i.next_reset_at) {
            times.push(t);
        }
    }
    times.into_iter().min()
}

/// Meter fill color role by **used** % (success < 80, warn 80–99, danger 100+).
///
/// Returns a plain role name for theme mapping (not a color invent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowanceMeterTone {
    Success,
    Warning,
    Danger,
}

impl AllowanceMeterTone {
    pub fn from_used_pct(used_pct: f64) -> Self {
        if used_pct >= 100.0 {
            Self::Danger
        } else if used_pct >= 80.0 {
            Self::Warning
        } else {
            Self::Success
        }
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

/// When dual SuperGrok rows share one included pool and a slot has no included
/// reading yet, copy the known included meter so personal/business does not
/// look forever empty under unified billing. Dollar extras are handled by
/// [`fill_unified_dollar_extras_on_empty_slots`] (same Extra Usage Credits pool).
fn fill_unified_included_on_empty_slots(
    mut primary: PrincipalLimitsSlot,
    mut extras: Vec<PrincipalLimitsSlot>,
) -> (PrincipalLimitsSlot, Vec<PrincipalLimitsSlot>) {
    let template = primary
        .included
        .clone()
        .or_else(|| extras.iter().find_map(|s| s.included.clone()));
    let Some(inc) = template else {
        return (primary, extras);
    };
    if primary.included.is_none() {
        primary.included = Some(inc.clone());
        primary.included_source = IncludedSource::SharedPoolFill;
        // Fill is not a successful poll of this JWT.
        primary.poll_succeeded = false;
        // Included fill alone does not observe dollar extras.
        if primary.dollar_extras.is_none() {
            primary.dollar_extras_observed = false;
        }
    }
    for slot in &mut extras {
        if slot.included.is_none() {
            slot.included = Some(inc.clone());
            slot.included_source = IncludedSource::SharedPoolFill;
            slot.poll_succeeded = false;
            if slot.dollar_extras.is_none() {
                slot.dollar_extras_observed = false;
            }
        }
    }
    (primary, extras)
}

/// Under unified SuperGrok billing, Extra Usage Credits (`prepaidBalance`) is
/// one account pool. When any dual row observed a positive/zero prepaid, copy
/// that meter onto rows that still show unobserved extras so `/limits` does not
/// look like half the SuperGrok $ is missing. Never invent cents when no row
/// observed prepaid.
fn fill_unified_dollar_extras_on_empty_slots(
    mut primary: PrincipalLimitsSlot,
    mut extras: Vec<PrincipalLimitsSlot>,
) -> (PrincipalLimitsSlot, Vec<PrincipalLimitsSlot>) {
    let template = primary
        .dollar_extras
        .clone()
        .filter(|_| primary.dollar_extras_observed)
        .or_else(|| {
            extras.iter().find_map(|s| {
                if s.dollar_extras_observed {
                    s.dollar_extras.clone()
                } else {
                    None
                }
            })
        });
    // Also accept "observed empty" (none on file) from a row that polled full.
    let observed_empty = primary.dollar_extras_observed && primary.dollar_extras.is_none()
        || extras
            .iter()
            .any(|s| s.dollar_extras_observed && s.dollar_extras.is_none());
    let Some(dollars) = template else {
        if observed_empty {
            if !primary.dollar_extras_observed {
                primary.dollar_extras_observed = true;
                primary.dollar_extras = None;
            }
            for slot in &mut extras {
                if !slot.dollar_extras_observed {
                    slot.dollar_extras_observed = true;
                    slot.dollar_extras = None;
                }
            }
        }
        return (primary, extras);
    };
    if !primary.dollar_extras_observed || primary.dollar_extras.is_none() {
        primary.dollar_extras = Some(dollars.clone());
        primary.dollar_extras_observed = true;
    }
    for slot in &mut extras {
        if !slot.dollar_extras_observed || slot.dollar_extras.is_none() {
            slot.dollar_extras = Some(dollars.clone());
            slot.dollar_extras_observed = true;
        }
    }
    (primary, extras)
}

/// Multi-line `/limits` body. Pure; hermetic fixtures only.
///
/// No body title: modal chrome already shows **Limits** (double title was a
/// dogfood pain). First line is live sampling; second is **Active:** driver
/// (free SuperGrok period | SuperGrok extras | console key).
pub fn format_limits_detail(snap: &LimitsSnapshot) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(snap.live_sampling_line());
    lines.push(active_driver_line_for_snapshot(snap));
    if snap.shared_unified_supergrok_pool {
        // Dogfood: dual rows both at e.g. 62% looked like a client mirror bug.
        // One short line only — no lecture wall. Extra Usage Credits (dollar
        // extras) are the same prepaidBalance pool under unified billing.
        lines.push(
            "Note: personal + business share one SuperGrok weekly pool and \
Extra Usage Credits (not console team prepaid)."
                .to_string(),
        );
    }
    // Dual poll honesty: name which principal failed and which rows are
    // shared-pool fill (not live_poll). Keep near the top so operators see
    // trust caveats on the meters they are about to read. Not debug-only.
    for note in dual_poll_honesty_notes(snap) {
        lines.push(note);
    }
    lines.push(String::new());

    let console_live = snap.live_identity.is_console();
    format_principal(&mut lines, &snap.primary, console_live);

    for extra in &snap.extra_principals {
        lines.push(String::new());
        format_principal(&mut lines, extra, console_live);
    }

    lines.push(String::new());
    format_console(&mut lines, &snap.console);

    // Longer honesty notes after meters (not before). Always-on license-page
    // and poll-reading notes wrap to many TUI rows; putting them first buried
    // SuperGrok included % + remaining bar under the fold on typical heights.
    // CLI `grok limits` keeps the same order: meters first, caveats second.
    for note in honesty_notes_for_snapshot(snap) {
        lines.push(String::new());
        lines.push(note.to_string());
    }

    // Double-entry spend summary (local vs Management); full view is /spend.
    lines.push(String::new());
    lines.push(format_limits_double_entry_section(snap));

    lines.join("\n")
}

/// Compact double-entry block for `/limits` (local book + remote honesty).
fn format_limits_double_entry_section(snap: &LimitsSnapshot) -> String {
    let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
    let mut remote = xai_grok_shell::token_economy::RemoteBookSummary::default();
    let has_mgmt = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    if !has_mgmt || !cfg.reconcile_management_usage {
        remote.remote_unavailable = true;
        if !has_mgmt {
            remote.remote_setup_note =
                Some("No management key on file for console team remote book.".into());
        }
    } else {
        // Snapshot meters (prepaid / postpaid) + latest series sample in grok_oss.db.
        if let Some(cents) = snap.console.balance_cents {
            remote.prepaid_remaining_cents = Some(cents);
        }
        if let Some(pp) = &snap.console.postpaid {
            remote.postpaid_api_class_cents = Some(pp.api_class_cents);
            remote.postpaid_oauth_class_cents = Some(pp.oauth_class_cents);
        }
        if let Some(store) = xai_grok_shell::grok_oss::try_open_from_token_economy_config(&cfg)
            && let Ok(Some(sample)) = xai_grok_shell::token_economy::latest_remote_sample(
                &store,
                "management_usage_series",
            )
        {
            remote.api_class_usd = sample.payload.get("api_class_usd").and_then(|v| v.as_f64());
            remote.oauth_class_usd = sample
                .payload
                .get("oauth_class_usd")
                .and_then(|v| v.as_f64());
            if let (Some(s), Some(e)) = (sample.window_start, sample.window_end) {
                remote.window_label = Some(format!("{s} → {e}"));
            }
        }
    }

    let mut supergrok = xai_grok_shell::token_economy::SuperGrokPeriodContext::default();
    if let Some(inc) = &snap.primary.included {
        supergrok.usage_pct = Some(inc.used_pct);
        supergrok.period_label = Some(inc.period_label.to_lowercase());
        supergrok.pacing_sentence = format_principal_pacing(inc, snap.live_identity.is_console());
    }

    let report = xai_grok_shell::token_economy::build_double_entry_report(&cfg, remote, supergrok);
    xai_grok_shell::token_economy::format_limits_spend_section(&report)
}

/// Honesty notes for a snapshot (limits modal / human `grok limits` body).
pub fn honesty_notes_for_snapshot(snap: &LimitsSnapshot) -> Vec<String> {
    use super::limits_honesty::{LimitsHonestyInput, honesty_notes_for_limits};

    let has_included = snap.primary.included.is_some()
        || snap.extra_principals.iter().any(|p| p.included.is_some());
    let oauth_postpaid_dominates = snap
        .console
        .postpaid
        .as_ref()
        .is_some_and(|p| p.oauth_class_dominates());
    let has_team_default_credits = snap
        .console
        .postpaid
        .as_ref()
        .and_then(|p| p.default_credits_cents)
        .is_some();
    honesty_notes_for_limits(LimitsHonestyInput {
        live: snap.live_identity,
        has_included_reading: has_included,
        flat_poll_unproven_debit: snap.flat_poll_unproven_debit,
        flat_poll_observed_build: snap.flat_poll_observed_build,
        flat_poll_observed_extras: snap.flat_poll_observed_extras,
        oauth_postpaid_dominates,
        has_console_team_prepaid_reading: snap.console.balance_cents.is_some(),
        has_team_default_credits_reading: has_team_default_credits,
    })
}

/// Dual SuperGrok poll-fail + shared-pool fill notes (human `/limits` body
/// and `limits --json` notes).
///
/// Role primary. Not debug-only. Fail note only when a poll error class is
/// known; fill note when included % is shared-pool fill.
pub fn dual_poll_honesty_notes_for_snapshot(snap: &LimitsSnapshot) -> Vec<String> {
    dual_poll_honesty_notes(snap)
}

fn dual_poll_honesty_notes(snap: &LimitsSnapshot) -> Vec<String> {
    use super::limits_honesty::{
        note_dual_principal_billing_failed, note_shared_pool_fill_not_live_poll,
    };

    let mut notes = Vec::new();
    let mut seen_fail = std::collections::BTreeSet::new();
    let mut seen_fill = std::collections::BTreeSet::new();
    for slot in std::iter::once(&snap.primary).chain(snap.extra_principals.iter()) {
        let role = role_from_principal_label(&slot.label);
        if !slot.poll_succeeded
            && slot.poll_error_class.is_some()
            && seen_fail.insert(role.to_owned())
        {
            notes.push(note_dual_principal_billing_failed(role));
        }
        if slot.included_source == IncludedSource::SharedPoolFill
            && seen_fill.insert(role.to_owned())
        {
            notes.push(note_shared_pool_fill_not_live_poll(role));
        }
    }
    notes
}

fn role_from_principal_label(label: &str) -> &str {
    label
        .strip_prefix("SuperGrok (")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(label)
}

/// Free SuperGrok period linear-burn sentence for a principal included meter.
///
/// Uses next reset as period end and period_label for weekly/monthly start
/// derivation. Omit when bounds or config say so. Never dollars.
fn format_principal_pacing(inc: &IncludedAllowanceMeter, console_live: bool) -> Option<String> {
    let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
    if !cfg.show_period_pacing {
        return None;
    }
    let end = inc.next_reset_at?;
    let period_type = match inc.period_label {
        "Weekly" => Some("USAGE_PERIOD_TYPE_WEEKLY"),
        "Monthly" => Some("USAGE_PERIOD_TYPE_MONTHLY"),
        _ => None,
    };
    let start = xai_grok_shell::token_economy::resolve_period_start(None, Some(end), period_type)?;
    let p = xai_grok_shell::token_economy::compute_period_pacing(
        inc.used_pct,
        start,
        end,
        chrono::Utc::now(),
    )?;
    Some(if console_live {
        p.full_sentence_console_live()
    } else {
        p.full_sentence()
    })
}

fn format_principal(lines: &mut Vec<String>, p: &PrincipalLimitsSlot, console_live: bool) {
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
            // Free SuperGrok period linear-burn pacing (omit when bounds missing).
            if let Some(pacing) = format_principal_pacing(inc, console_live) {
                lines.push(format!("  {pacing}"));
            }
        }
        None => {
            lines.push("  Included allowance: no data yet".to_string());
            lines.push("  Next reset: not known yet".to_string());
        }
    }

    // Branch 2b: always surface Grok Build productUsage % when wire has it.
    // Distinct from top-level included allowance %; never invent when None.
    // Shared phrase with `/usage` (Issue 5).
    if let Some(build_pct) = p.grok_build_usage_pct {
        lines.push(format!(
            "  {}",
            super::limits_honesty::format_grok_build_product_usage_line(build_pct)
        ));
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
    lines.push(format!("  {}", c.key_status_line()));
    // P1 prominence: team postpaid OAuth / Grok Build class near top of Console
    // when known and positive (dogfood settlement proof; distinct from prepaid).
    if let Some(p) = &c.postpaid
        && p.oauth_class_cents > 0
    {
        lines.push(format!(
            "  Team postpaid OAuth / Grok Build class: {}",
            fmt_dollars(p.oauth_class_cents)
        ));
    }
    match c.balance_cents {
        // Short Balance line — dollars are console team prepaid (never SuperGrok extras).
        Some(cents) => lines.push(format!("  Balance: {}", fmt_dollars(cents))),
        None => {
            // Short Balance gap only. No Management Key lecture wall — operators
            // rejected that framing for chat-key honesty (dogfood).
            lines.push(format!("  Balance: {}", c.prepaid_gap.as_display_str()));
        }
    }
    // Postpaid period + API class (OAuth / Grok Build already above when > 0).
    match &c.postpaid {
        Some(p) => {
            lines.push(format!(
                "  Team postpaid (period): {}",
                fmt_dollars(p.period_total_cents)
            ));
            // Zero OAuth class still listed here so operators see the split.
            if p.oauth_class_cents <= 0 {
                lines.push(format!(
                    "  Team postpaid OAuth / Grok Build class: {}",
                    fmt_dollars(p.oauth_class_cents)
                ));
            }
            lines.push(format!(
                "  Team postpaid API class: {}",
                fmt_dollars(p.api_class_cents)
            ));
            // Team default credits: dashboard allotment, own line (not prepaid).
            if let Some(dc) = p.default_credits_cents {
                lines.push(format!(
                    "  Team default credits (dashboard allotment; not the prepaid wallet): {}",
                    fmt_dollars(dc)
                ));
            }
        }
        None => {
            lines.push(format!(
                "  Team postpaid: {}",
                c.postpaid_gap.as_display_str()
            ));
        }
    }
    // Optional Management usage series (POST analytics; spend over a day window).
    if let Some(series) = &c.usage_series {
        lines.push(format!(
            "  Team usage series ({} .. {}, {}):",
            series.start_time, series.end_time, series.timezone
        ));
        lines.push(format!(
            "    OAuth / Grok Build class: {}",
            fmt_usd_float(series.oauth_class_usd)
        ));
        lines.push(format!(
            "    API-key class: {}",
            fmt_usd_float(series.api_class_usd)
        ));
        if series.other_class_usd.abs() > f64::EPSILON {
            lines.push(format!(
                "    Other class: {}",
                fmt_usd_float(series.other_class_usd)
            ));
        }
        for row in series.top_rows.iter().take(3) {
            lines.push(format!(
                "    · {}: {}",
                truncate_series_label(&row.label, 48),
                fmt_usd_float(row.total_usd)
            ));
        }
        if series.limit_reached {
            lines.push(
                "    (Management reported limitReached: only a subset of groups returned)"
                    .to_string(),
            );
        }
    }
}

/// Format a USD float for series lines (`$N` or `$N.NN`).
fn fmt_usd_float(usd: f64) -> String {
    let a = usd.abs();
    if (a - a.round()).abs() < 1e-9 {
        format!("${:.0}", usd)
    } else {
        format!("${usd:.2}")
    }
}

fn truncate_series_label(label: &str, max: usize) -> String {
    let t = label.trim();
    if t.chars().count() <= max {
        t.to_owned()
    } else {
        // ASCII three-dot ellipsis (product prose: no unicode ellipsis).
        let keep = max.saturating_sub(3);
        let mut s: String = t.chars().take(keep).collect();
        s.push_str("...");
        s
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
            period_end_at: None,
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: None,
            period_type: None,
            is_unified_billing_user: None,
            grok_build_usage_pct: None,
            included_usage_known: true,
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
            next_reset_at: None,
        };
        assert_eq!(inc.used_pct_floored(), 99);
        assert_eq!(inc.remaining_pct_floored(), 1);
        let full = IncludedAllowanceMeter {
            period_label: "Weekly",
            used_pct: 100.0,
            next_reset_display: None,
            next_reset_at: None,
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
        // Chrome owns the "Limits" title — body starts with live sampling.
        assert!(
            out.starts_with("Live sampling:"),
            "no body double-title; live first: {out}"
        );
        assert!(
            !out.starts_with("Limits\n"),
            "body must not repeat chrome title: {out}"
        );
        assert!(
            out.contains("Live sampling: SuperGrok session"),
            "live identity: {out}"
        );
        assert!(
            out.contains("Active: free SuperGrok period"),
            "active driver with free-period headroom: {out}"
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
        assert!(
            out.contains("no key"),
            "no inference console key → no key: {out}"
        );
        assert!(
            !out.contains("Requests:"),
            "must not claim a request path when no console key: {out}"
        );
        assert!(
            !out.contains("Path:"),
            "Path: wording retired (read as key missing): {out}"
        );
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
        assert!(
            out.contains("Requests: console"),
            "console live key status: {out}"
        );
        assert!(
            !out.contains("saved"),
            "omit saved; presence is implicit: {out}"
        );
        assert!(!out.contains("Path:"), "Path: wording retired: {out}");
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
        assert!(
            out.contains("Requests: console"),
            "console live key status: {out}"
        );
        assert!(
            !out.contains("saved"),
            "omit saved; presence is implicit: {out}"
        );
        assert!(
            out.contains("Balance: $125"),
            "real management prepaid dollars (short Balance): {out}"
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
            out_d.contains("Balance: $25"),
            "short Balance line: {out_d}"
        );
        assert!(
            !out_d.contains("Balance (console team prepaid)"),
            "long Balance label retired: {out_d}"
        );
        assert!(!out_d.contains("no management key"), "{out_d}");
        assert!(!out_d.contains("no management team id"), "{out_d}");
        // Soft polish: when prepaid $ is shown, name process-cache lag,
        // app last-good that can outlive TTL, + force path.
        assert!(
            out_d.contains("console team prepaid process cache may lag"),
            "prepaid lag honesty when dollars shown: {out_d}"
        );
        assert!(
            out_d.to_ascii_lowercase().contains("last successful"),
            "must name app last-good: {out_d}"
        );
        assert!(
            out_d.contains("grok limits"),
            "must name CLI force-refresh path: {out_d}"
        );
        assert!(
            out_d.contains("/limits"),
            "must name TUI force-refresh path: {out_d}"
        );
    }

    /// Named contract: SuperGrok live (`console.isLive=false`) + Management
    /// prepaid fixture → Console API team block still shows Balance $N.
    #[test]
    fn format_supergrok_live_with_management_prepaid_shows_team_balance() {
        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_key_available(true)
                .with_console_balance_cents(Some(12_500))
                .with_console_prepaid_gap(ConsoleTeamPrepaidGap::Loading);
        assert!(!snap.console.is_live, "fixture: SuperGrok live");
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Live sampling: SuperGrok session"),
            "live: {out}"
        );
        assert!(
            out.contains("Requests: SuperGrok"),
            "console key on file but SuperGrok serving: {out}"
        );
        assert!(
            out.contains("Balance: $125"),
            "team prepaid must show even when console.isLive=false: {out}"
        );
        assert!(
            out.contains("Console API:"),
            "team Management section must not be omitted: {out}"
        );
        assert!(
            !out.contains("no management key"),
            "must not claim missing key when cents present: {out}"
        );
        // SuperGrok meters stay SuperGrok-labeled; team $ is not SuperGrok extras.
        assert!(
            out.contains("Included weekly allowance: 65% used"),
            "SuperGrok included still shown: {out}"
        );
        assert!(
            !out.contains("SuperGrok dollar extras: $125"),
            "must not mash team prepaid into SuperGrok extras: {out}"
        );
    }

    /// Named contract: SuperGrok live + no management key → Console team block
    /// still present with honest Balance gap (not silent omit of whole team section).
    #[test]
    fn format_supergrok_live_without_mgmt_key_keeps_honest_team_block() {
        let bal = weekly(65.0, "Aug 4, 12:00", None);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_key_available(true);
        assert!(!snap.console.is_live);
        let out = format_limits_detail(&snap);
        assert!(out.contains("Console API:"), "team section present: {out}");
        assert!(
            out.contains("Balance: no management key"),
            "honest team gap, not silent omit: {out}"
        );
        assert!(
            !out.contains("no management key/team id"),
            "mushy combined retired: {out}"
        );
        assert!(!out.contains("no $ meter yet"), "{out}");
    }

    /// Named contract: /limits honesty must not claim Grok Business license
    /// messages/conversations as a product meter; one plain note that the
    /// license page is not SuperGrok or team Management.
    #[test]
    fn format_limits_honesty_distinguishes_license_page_from_product_meters() {
        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_balance_cents(Some(12_500));
        let out = format_limits_detail(&snap);
        let lower = out.to_ascii_lowercase();
        // Must not present license seat message/conversation counts as product meters.
        assert!(
            !lower.contains("license messages")
                && !lower.contains("license conversations")
                && !lower.contains("seat message usage"),
            "must not claim license messages/conversations as product meter: {out}"
        );
        // One plain note: Platforms → Grok Business licenses page ≠ SuperGrok / team Management.
        assert!(
            lower.contains("license")
                && (lower.contains("not") || lower.contains("≠") || lower.contains("different")),
            "must note license page is not SuperGrok/team Management: {out}"
        );
        assert!(
            lower.contains("management")
                || lower.contains("team prepaid")
                || lower.contains("supergrok")
                || lower.contains("team usage"),
            "license note must name product meters it is not: {out}"
        );
        // P0 sharper: team Usage / zeros expected.
        assert!(
            lower.contains("team usage") || lower.contains("grok build"),
            "must name team Usage / Grok Build settlement: {out}"
        );
        assert!(
            lower.contains("zeros") && lower.contains("expected"),
            "must say license zeros are expected: {out}"
        );
    }

    /// Named contract (P2): when usage series is known (process cache / collect),
    /// Console format surfaces OAuth / Grok Build class USD and does **not**
    /// mash it into team prepaid Balance or free SuperGrok period %.
    #[test]
    fn format_console_surfaces_usage_series_oauth_class_when_known() {
        let bal = weekly(6.0, "Aug 7, 12:00", None);
        let series = ConsoleTeamUsageSeriesSummary {
            start_time: "2026-08-01 00:00:00".into(),
            end_time: "2026-08-08 00:00:00".into(),
            timezone: "Etc/GMT".into(),
            oauth_class_usd: 823.71,
            api_class_usd: 1.0,
            other_class_usd: 0.0,
            top_rows: vec![ConsoleTeamUsageSeriesRow {
                label: "Grok Build OAuth grok-4.5-build".into(),
                class_wire: "oauth_grok_build",
                total_usd: 823.71,
            }],
            limit_reached: false,
        };
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_key_available(true)
                .with_console_balance_cents(Some(34_000))
                .with_console_usage_series(Some(series));
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Team usage series"),
            "must name usage series when known: {out}"
        );
        assert!(
            out.contains("OAuth / Grok Build class: $823.71")
                || out.contains("OAuth / Grok Build class: $823"),
            "series OAuth / Grok Build class must appear: {out}"
        );
        assert!(
            out.contains("Balance: $340"),
            "team prepaid Balance stays its own meter: {out}"
        );
        assert!(
            out.contains("6%") || out.contains("6.0"),
            "free SuperGrok period % stays on SuperGrok rows: {out}"
        );
        // No mash: prepaid Balance must not show series window dollars.
        assert!(
            !out.contains("Balance: $823") && !out.contains("Balance: $823.71"),
            "must not fold series into prepaid Balance: {out}"
        );
        // Free SuperGrok period line is percent, not series USD.
        let free_period_line = out
            .lines()
            .find(|l| {
                l.contains("included") || l.contains("free SuperGrok") || l.contains("% used")
            })
            .unwrap_or("");
        assert!(
            !free_period_line.contains("823"),
            "must not mash series USD into free SuperGrok period line: {free_period_line}"
        );
    }

    /// Named contract (P1): when postpaid OAuth / Grok Build class is known,
    /// Console block shows it prominently (before Balance / early in section),
    /// labeled distinctly from team prepaid Balance.
    #[test]
    fn format_console_surfaces_grok_build_class_prominently() {
        let bal = weekly(65.0, "Aug 4, 12:00", None);
        let postpaid = ConsoleTeamPostpaidMeter {
            period_total_cents: 82_500,
            oauth_class_cents: 82_371,
            api_class_cents: 129,
            other_class_cents: 0,
            default_credits_cents: None,
        };
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_console_key_available(true)
                .with_console_balance_cents(Some(34_000))
                .with_console_postpaid(Some(postpaid));
        let out = format_limits_detail(&snap);
        let console_idx = out.find("Console API:").expect("console section");
        let oauth_idx = out
            .find("Team postpaid OAuth / Grok Build class:")
            .expect("Grok Build class line");
        let balance_idx = out.find("Balance: $340").expect("prepaid Balance");
        assert!(
            oauth_idx > console_idx && oauth_idx < balance_idx,
            "Grok Build class must appear near top of Console, before prepaid Balance:\n{out}"
        );
        assert!(
            out.contains("$823.71"),
            "must show OAuth / Grok Build class dollars: {out}"
        );
        assert!(
            out.contains("Balance: $340"),
            "prepaid Balance stays separate: {out}"
        );
        // Must not mash into one credits line.
        assert!(
            !out.contains("credits: $823.71") && !out.contains("Balance: $823.71"),
            "must not fold class into prepaid Balance: {out}"
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
                next_reset_at: None,
            }),
            dollar_extras: None,
            dollar_extras_observed: true,
            grok_build_usage_pct: None,
            poll_succeeded: true,
            included_source: IncludedSource::Unknown,
            poll_error_class: None,
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(weekly(10.0, "Jul 30, 12:00", None)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: None, // never polled
            autotopup: None,
            // Unpolled sibling: included-only absence (not "none on file").
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
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
        let business_sec = out.split("SuperGrok (business):").nth(1).unwrap_or("");
        assert!(
            business_sec.contains("SuperGrok dollar extras: no data yet"),
            "unpolled sibling must not claim none-on-file extras: {out}"
        );
        assert!(
            !business_sec.contains("none on file"),
            "unpolled sibling extras: {out}"
        );
    }

    /// Named contract: unified fill labels filled principal as shared_pool_fill
    /// (not live_poll); successful poll principal stays live_poll; human text
    /// names role on fill.
    #[test]
    fn dual_fill_provenance_not_live_poll_and_names_role() {
        let mut business_bal = weekly(6.0, "August 10, 00:00", Some(500));
        business_bal.is_unified_billing_user = Some(true);
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(business_bal),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: None,
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: Some(false),
            poll_error_class: Some("auth"),
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(snap.shared_unified_supergrok_pool);
        assert!(
            snap.primary.poll_succeeded,
            "business live poll must succeed"
        );
        assert_eq!(snap.primary.included_source, IncludedSource::LivePoll);
        let personal_slot = snap
            .extra_principals
            .iter()
            .find(|p| p.label.contains("personal"))
            .expect("personal slot");
        assert!(
            !personal_slot.poll_succeeded,
            "filled personal must not claim pollSucceeded"
        );
        assert_eq!(
            personal_slot.included_source,
            IncludedSource::SharedPoolFill,
            "fill must be shared_pool_fill not live_poll"
        );
        assert_eq!(
            personal_slot.included.as_ref().map(|i| i.used_pct),
            Some(6.0)
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("personal")
                && (out.contains("shared SuperGrok pool") || out.contains("billing poll failed")),
            "human text must name personal fail/fill: {out}"
        );
        // JSON path
        let report = crate::limits_cmd::report_from_snapshot(&snap, vec![]);
        let personal_json = report
            .supergrok
            .principals
            .iter()
            .find(|p| p.label.contains("personal"))
            .expect("personal json");
        assert!(!personal_json.poll_succeeded);
        assert_eq!(personal_json.included_source, Some("shared_pool_fill"));
        assert_eq!(personal_json.poll_error_class, Some("auth"));
        let business_json = report
            .supergrok
            .principals
            .iter()
            .find(|p| p.label.contains("business"))
            .expect("business json");
        assert!(business_json.poll_succeeded);
        assert_eq!(business_json.included_source, Some("live_poll"));
    }

    /// Unified billing + cold sibling: paint the shared included pool on the
    /// empty personal/business row (not forever "no data yet").
    #[test]
    fn format_unified_fills_cold_sibling_included_from_known_pool() {
        let mut business_bal = weekly(65.0, "August 3, 19:25", Some(10029));
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: None,
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(snap.shared_unified_supergrok_pool);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("share one SuperGrok weekly pool")
                || out.contains("shared") && out.contains("pool"),
            "shared pool note: {out}"
        );
        // Both rows show the same included reading (one pool).
        assert_eq!(
            out.matches("65% used").count(),
            2,
            "cold personal must show shared included, not empty: {out}"
        );
        let personal_sec = out.split("SuperGrok (personal):").nth(1).unwrap_or("");
        assert!(
            personal_sec.contains("65% used"),
            "personal must show shared included %: {out}"
        );
        // Unified Extra Usage Credits pool: business observed $100.29 → personal
        // must show the same (not "no data yet" half-balance).
        assert!(
            personal_sec.contains("SuperGrok dollar extras: $100.29"),
            "unified cold personal must share observed Extra Usage Credits: {out}"
        );
        assert_eq!(
            out.matches("SuperGrok dollar extras: $100.29").count(),
            2,
            "both SuperGrok rows show the shared prepaidBalance: {out}"
        );
        assert!(!personal_sec.contains("none on file"), "{out}");
        assert!(
            !personal_sec.contains("Included allowance: no data yet"),
            "unified cold personal must not stay empty: {out}"
        );
    }

    /// Named contract: under unified billing, Extra Usage Credits observed on
    /// one principal fill the other dual row (full SuperGrok $ the API returned).
    #[test]
    fn format_unified_shares_observed_dollar_extras_across_principals() {
        let mut business_bal = weekly(65.0, "August 3, 19:25", Some(10029));
        business_bal.is_unified_billing_user = Some(true);
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(business_bal),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        // Sibling process cache: included only, no prepaid on this slot yet.
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(CreditBalance {
                period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                period_end_display: Some("August 3, 19:25".into()),
                prepaid_balance_cents: None,
                is_unified_billing_user: Some(true),
                ..bal(65.0)
            }),
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Extra Usage Credits") || out.contains("share one SuperGrok weekly pool"),
            "shared-pool note mentions Extra Usage Credits: {out}"
        );
        assert_eq!(
            out.matches("SuperGrok dollar extras: $100.29").count(),
            2,
            "full SuperGrok $ extras on both dual rows: {out}"
        );
        assert!(
            !out.contains("SuperGrok dollar extras: no data yet"),
            "must not look like SuperGrok $ is only half-observed: {out}"
        );
    }

    /// Sibling included-only poll fills included % but must not claim dollar
    /// extras are known empty ("none on file") when not unified / no template.
    #[test]
    fn format_sibling_included_only_extras_honest_absence() {
        let active = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(24.0, "Jul 30, 12:00", Some(1250))),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
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
            poll_succeeded: None,
            poll_error_class: None,
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
                next_reset_at: None,
            }),
            dollar_extras: None,
            dollar_extras_observed: false,
            grok_build_usage_pct: None,
            poll_succeeded: true,
            included_source: IncludedSource::Unknown,
            poll_error_class: None,
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
                    next_reset_at: None,
                }),
                dollar_extras: Some(DollarExtrasMeter {
                    balance_cents: 10029,
                    auto_topup: Some(AutoTopupLine::Disabled),
                }),
                dollar_extras_observed: true,
                grok_build_usage_pct: None,
                poll_succeeded: true,
                included_source: IncludedSource::Unknown,
                poll_error_class: None,
            },
            extra_principals: vec![slot],
            console: ConsoleMeter {
                is_live: false,
                key_available: false,
                balance_cents: None,
                prepaid_gap: ConsoleTeamPrepaidGap::MissingManagementKey,
                postpaid: None,
                postpaid_gap: ConsoleTeamPostpaidGap::MissingManagementKey,
                usage_series: None,
            },
            // This fixture exercises double-"included" copy only; shared-pool
            // note is covered by dedicated dual-unified tests.
            shared_unified_supergrok_pool: false,
            flat_poll_unproven_debit: false,
            flat_poll_observed_build: false,
            flat_poll_observed_extras: false,
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            // Distinct sibling pool: 15% used, different reset.
            balance: Some(weekly(15.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
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
            !out.contains("share one SuperGrok weekly pool")
                && !out.contains("shared consumer pool"),
            "distinct pools: no unified-share note: {out}"
        );
    }

    /// Named contract (dogfood 62%/62%): when both SuperGrok OIDC slots report
    /// the same included % + reset under unified billing, /limits must say they
    /// share one SuperGrok weekly pool — not look like a silent client mirror —
    /// and briefly note that is not console Grok Business.
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        // Sibling poll: same included % + reset the credits API returns for the
        // personal OIDC token under unified billing (distinct token, same pool).
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
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
            out.contains("share one SuperGrok weekly pool"),
            "must explain shared SuperGrok pool (short): {out}"
        );
        // Keep copy scannable — no long unified-billing lecture.
        assert!(
            !out.contains("unified billing") && !out.contains("shared consumer pool"),
            "long shared-pool lecture retired: {out}"
        );
        assert!(
            out.to_ascii_lowercase().contains("not console")
                && (out.to_ascii_lowercase().contains("team prepaid")
                    || out.to_ascii_lowercase().contains("grok business")),
            "must distinguish SuperGrok pool from console team prepaid: {out}"
        );
        // Both rows still show their (same) per-slot readings — not collapsed.
        assert_eq!(
            out.matches("62% used").count(),
            2,
            "both slots keep their own 62% reading from each poll: {out}"
        );
        // Shared Extra Usage Credits fill under unified pool.
        assert_eq!(
            out.matches("SuperGrok dollar extras: $100.29").count(),
            2,
            "shared prepaidBalance on both dual rows: {out}"
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
            poll_succeeded: None,
            poll_error_class: None,
        };
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(weekly(62.0, "August 3, 19:25", None)),
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let snap = LimitsSnapshot::from_principals(
            &[business, personal],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        assert!(snap.shared_unified_supergrok_pool);
        let out = format_limits_detail(&snap);
        assert!(out.contains("share one SuperGrok weekly pool"), "{out}");
        assert!(
            out.to_ascii_lowercase().contains("not console")
                && (out.to_ascii_lowercase().contains("team prepaid")
                    || out.to_ascii_lowercase().contains("grok business")),
            "{out}"
        );
        assert_eq!(
            out.matches("SuperGrok dollar extras: $100.29").count(),
            2,
            "identical-included path also shares Extra Usage Credits: {out}"
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
            poll_succeeded: None,
            poll_error_class: None,
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

    #[test]
    fn format_reset_countdown_includes_days_hours_minutes_seconds() {
        let reset = chrono::DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        // 2d 7h 25m 0s
        assert_eq!(format_reset_countdown(now, reset), "2d 7h 25m 0s");
        let almost = chrono::DateTime::parse_from_rfc3339("2026-08-03T19:24:59Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(format_reset_countdown(almost, reset), "0d 0h 0m 1s");
        let past = chrono::DateTime::parse_from_rfc3339("2026-08-03T19:25:01Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(format_reset_countdown(past, reset), "0d 0h 0m 0s");
        assert!(countdown_is_zero(past, reset));
        assert!(!countdown_is_zero(almost, reset));
    }

    /// Named contract (a): console chat key on file, SuperGrok handling
    /// requests → show request path only (presence implicit), never missing.
    #[test]
    fn console_key_on_file_requests_supergrok_is_not_missing() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession)
            .with_console_key_available(true);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Requests: SuperGrok"),
            "stored console key must show SuperGrok requests: {out}"
        );
        assert!(
            !out.contains("no key"),
            "key on file must not say no key: {out}"
        );
        assert!(
            !out.contains("saved"),
            "omit saved; presence is implicit: {out}"
        );
        assert!(!out.contains("Path:"), "Path: wording retired: {out}");
        assert!(
            !out.contains("not live") && !out.contains("not sampling"),
            "ban not-live / not-sampling path jargon when key exists: {out}"
        );
        // Balance gap may still mention Management API; request-path line must not.
        let requests_line = out
            .lines()
            .find(|l| l.trim_start().starts_with("Requests:"))
            .expect("requests status line");
        assert!(
            !requests_line.to_ascii_lowercase().contains("management"),
            "Requests line must not mention management: {requests_line}"
        );
    }

    /// Named contract (b): console chat key present and handling requests.
    #[test]
    fn console_key_on_file_requests_console_when_live() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::ConsoleKey)
            .with_console_key_available(true);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Requests: console"),
            "live console key must show console requests: {out}"
        );
        assert!(
            !out.contains("no key"),
            "key on file must not say no key: {out}"
        );
        assert!(
            !out.contains("saved"),
            "omit saved; presence is implicit: {out}"
        );
        assert!(!out.contains("Path:"), "Path: wording retired: {out}");
    }

    /// Named contract (c): no console chat key in store/env.
    #[test]
    fn console_key_no_key_when_absent() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(out.contains("no key"), "no console key → no key: {out}");
        assert!(
            !out.contains("Requests:"),
            "must not claim a request path when no console key: {out}"
        );
        assert!(!out.contains("Path:"), "Path: wording retired: {out}");
    }

    #[test]
    fn allowance_meter_tone_from_used_pct() {
        assert_eq!(
            AllowanceMeterTone::from_used_pct(0.0),
            AllowanceMeterTone::Success
        );
        assert_eq!(
            AllowanceMeterTone::from_used_pct(79.9),
            AllowanceMeterTone::Success
        );
        assert_eq!(
            AllowanceMeterTone::from_used_pct(80.0),
            AllowanceMeterTone::Warning
        );
        assert_eq!(
            AllowanceMeterTone::from_used_pct(100.0),
            AllowanceMeterTone::Danger
        );
    }

    #[test]
    fn earliest_reset_at_picks_soonest_principal() {
        let early = chrono::DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let late = chrono::DateTime::parse_from_rfc3339("2026-08-10T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let mut bal = weekly(50.0, "August 3, 19:25", None);
        bal.period_end_at = Some(early);
        let mut bal2 = weekly(10.0, "August 10, 00:00", None);
        bal2.period_end_at = Some(late);
        let snap = LimitsSnapshot::from_principals(
            &[
                PrincipalLimitsInput {
                    label: "SuperGrok (personal)".into(),
                    role_label: Some("personal".into()),
                    balance: Some(bal),
                    autotopup: None,
                    included_billing_only: false,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
                PrincipalLimitsInput {
                    label: "SuperGrok (business)".into(),
                    role_label: Some("business".into()),
                    balance: Some(bal2),
                    autotopup: None,
                    included_billing_only: true,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
            ],
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
        );
        assert_eq!(earliest_reset_at(&snap), Some(early));
    }

    /// Named contract (Slice 3): SuperGrok included % is a billing poll reading,
    /// not proof of included-limit burn. Body must not overclaim burn.
    #[test]
    fn format_supergrok_included_pct_not_presented_as_proven_burn() {
        use super::super::limits_honesty::{
            NOTE_INCLUDED_PCT_IS_BILLING_POLL, contains_forbidden_included_burn_claim,
        };

        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains(NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "must include poll-reading honesty note: {out}"
        );
        assert!(
            !contains_forbidden_included_burn_claim(&out),
            "must not claim using/burning SuperGrok included from flat %: {out}"
        );
        // Meter still shows the poll reading (65% used is the API field, not a
        // product "you are burning" claim).
        assert!(
            out.contains("Included weekly allowance: 65% used · 35% remaining"),
            "included meter still shown: {out}"
        );
        assert!(
            out.contains("Live sampling: SuperGrok session"),
            "session path proven: {out}"
        );
    }

    /// Named contract (Slice 3): optional flat-poll note when evidence flag set.
    #[test]
    fn format_flat_poll_note_when_snapshot_flags_unproven_debit() {
        use super::super::limits_honesty::flat_poll_unproven_debit_note;

        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        // Explicit observed extras (prepaid on fixture) so note can name them.
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession)
                .with_flat_poll_unproven_debit(true)
                .with_flat_poll_observed_meters(false, true);
        let out = format_limits_detail(&snap);
        let expected = flat_poll_unproven_debit_note(false, true);
        assert!(
            out.contains(&expected),
            "flat-poll honesty note required when flag set: {out}"
        );
        assert!(
            out.contains("included debit is unproven"),
            "must say debit unproven: {out}"
        );
        assert!(
            !out.contains("Grok Build product % stayed")
                && !out.contains("Grok Build product %, and"),
            "must not claim Build flat without observed flag: {out}"
        );
    }

    /// Named contract (branch 2b): Grok Build productUsage % surfaces in human
    /// `/limits` when present on the principal (never invent when None).
    #[test]
    fn format_surfaces_grok_build_product_usage_when_on_wire() {
        let mut bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        bal.grok_build_usage_pct = Some(54.0);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Grok Build product usage: 54% used"),
            "human limits must surface Build productUsage when wire has it: {out}"
        );
        // Cold path: no invent when absent.
        let cold = LimitsSnapshot::from_billing(
            Some(&weekly(65.0, "Aug 4, 12:00", Some(10029))),
            None,
            SamplingIdentityKind::SuperGrokSession,
        );
        let cold_out = format_limits_detail(&cold);
        assert!(
            !cold_out.contains("Grok Build product usage:"),
            "must not invent Build % when wire has none: {cold_out}"
        );
    }

    /// Named contract (Issue 4): dual principal human format shows sibling
    /// Build % when that principal's balance carries it (sibling process cache
    /// path sets CreditBalance.grok_build_usage_pct from IncludedBillingFields).
    #[test]
    fn format_dual_principal_surfaces_sibling_grok_build_usage() {
        let mut active = weekly(65.0, "August 4, 12:00", Some(10029));
        active.grok_build_usage_pct = Some(54.0);
        let mut sibling = weekly(65.0, "August 4, 12:00", Some(10029));
        sibling.grok_build_usage_pct = Some(61.0);
        let snap = LimitsSnapshot::from_principals(
            &[
                PrincipalLimitsInput {
                    label: "SuperGrok (business)".into(),
                    role_label: Some("business".into()),
                    balance: Some(active),
                    autotopup: None,
                    included_billing_only: false,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
                PrincipalLimitsInput {
                    label: "SuperGrok (personal)".into(),
                    role_label: Some("personal".into()),
                    balance: Some(sibling),
                    autotopup: None,
                    included_billing_only: false,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
            ],
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
        );
        let out = format_limits_detail(&snap);
        assert!(
            out.contains("Grok Build product usage: 54% used"),
            "active Build %: {out}"
        );
        assert!(
            out.contains("Grok Build product usage: 61% used"),
            "sibling Build % from process cache must surface: {out}"
        );
    }

    /// Console live: no SuperGrok burn honesty lecture (meters stay on SuperGrok
    /// rows without claiming live SuperGrok burn).
    #[test]
    fn format_console_live_skips_supergrok_burn_honesty_note() {
        use super::super::limits_honesty::NOTE_INCLUDED_PCT_IS_BILLING_POLL;

        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        let snap = LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::ConsoleKey);
        let out = format_limits_detail(&snap);
        assert!(
            !out.contains(NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "console live must not sell SuperGrok burn note: {out}"
        );
        assert!(
            out.contains("Live sampling: console key"),
            "console path: {out}"
        );
    }

    /// Console live + flat flag: still no SuperGrok honesty (flat note's
    /// "session path can still be live" contradicts console live sampling).
    #[test]
    fn format_console_live_with_flat_flag_skips_all_supergrok_honesty() {
        use super::super::limits_honesty::{
            NOTE_INCLUDED_PCT_IS_BILLING_POLL, flat_poll_unproven_debit_note,
        };

        let bal = weekly(65.0, "Aug 4, 12:00", Some(10029));
        let snap = LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::ConsoleKey)
            .with_flat_poll_unproven_debit(true)
            .with_flat_poll_observed_meters(true, true);
        let out = format_limits_detail(&snap);
        assert!(
            !out.contains(NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "console + flat: no base honesty: {out}"
        );
        let flat = flat_poll_unproven_debit_note(true, true);
        assert!(
            !out.contains(&flat),
            "console + flat: no flat-poll honesty: {out}"
        );
        assert!(
            !out.contains("session path can still be live"),
            "must not claim session path under console live: {out}"
        );
        assert!(
            out.contains("Live sampling: console key"),
            "console path: {out}"
        );
    }
}
