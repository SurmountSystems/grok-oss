//! Credit balance indicator for the agent status bar.
//!
//! Shows the user's coding credit usage as a compact status bar item.
//! Fetches real data from the `x.ai/billing` agent extension.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::context_bar::fmt_pct5;
use super::progress_bar::progress_bar_spans;
use crate::theme::Theme;

/// Credit balance state from the billing API.
#[derive(Debug, Clone)]
pub struct CreditBalance {
    /// Usage as a percentage of the allowance (0.0 to 100.0).
    pub usage_pct: f64,
    /// Usage as a percentage of total budget (free and on-demand when enabled).
    pub effective_usage_pct: f64,
    /// Billing period end as a formatted local wall-clock string (no zone label), e.g. "Mar 31, 12:00".
    pub period_end_display: Option<String>,
    /// Absolute period end (UTC) when billing provided an RFC 3339 end.
    /// Used by `/limits` live countdown; display string stays local format.
    /// With [`Self::period_type`], also drives free SuperGrok period linear-burn
    /// pacing (start derived when wire start is absent).
    pub period_end_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether pay-as-you-go (on-demand) billing is enabled.
    pub pay_as_you_go: bool,
    /// On-demand spending cap in USD cents (e.g. 500 is $5.00).
    pub on_demand_cap_cents: Option<i64>,
    /// On-demand usage this period in USD cents.
    pub on_demand_used_cents: Option<i64>,
    /// Remaining prepaid ("bought") credit balance in USD cents.
    pub prepaid_balance_cents: Option<i64>,
    /// Usage period type from the billing response (the proto enum name, e.g. `USAGE_PERIOD_TYPE_WEEKLY`).
    /// Drives the "Weekly/Monthly limit" label.
    pub period_type: Option<String>,
    /// From credits config `is_unified_billing_user` (`None` if absent).
    /// `Some(true)` means the unified pool and buy-credits UX; `Some(false)` means the legacy on-demand (PAYG) UX.
    pub is_unified_billing_user: Option<bool>,
    /// Grok Build product usage % from wire `productUsage` when present.
    /// Distinct from top-level included `usage_pct`. `None` when not on wire
    /// or not observed (sibling process-cache-only path).
    pub grok_build_usage_pct: Option<f64>,
    /// Whether [`Self::usage_pct`] is a real included-allowance reading.
    ///
    /// `false` when the billing config had neither `credit_usage_percent` nor
    /// a usable monthly limit/used pair (honest absence — same rule as shell
    /// [`xai_grok_shell::extensions::billing::included_usage_and_period_end`]).
    /// True zero (`usage_pct == 0.0` with this flag true) is allowed.
    pub included_usage_known: bool,
}

/// OpenRouter account credits remaining (USD cents), from `GET /api/v1/credits`.
///
/// Separate from xAI [`CreditBalance`] so switching models can show the right
/// provider balance without overwriting Build prepaid / usage state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenRouterCreditBalance {
    /// Remaining account balance in USD cents (can be zero or negative).
    pub balance_cents: i64,
}

/// Which identity is live for sampling (drives meter honesty in the prompt footer).
///
/// After SuperGrok included allowance is full, Build can stay on a **console**
/// API key while SuperGrok billing still reports personal SuperGrok dollar credits.
/// The footer must not present those SuperGrok dollar credits as what Build is burning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SamplingIdentityKind {
    /// Live sampling uses the SuperGrok OAuth session (default when unknown).
    #[default]
    SuperGrokSession,
    /// Live sampling uses a console / Business API key (`api.x.ai`).
    ConsoleKey,
}

impl SamplingIdentityKind {
    /// Plain-language label for status / meter copy (no secrets).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SuperGrokSession => "SuperGrok session",
            Self::ConsoleKey => "console key",
        }
    }

    /// True when live sampling is on a console / Business API key.
    pub fn is_console(self) -> bool {
        matches!(self, Self::ConsoleKey)
    }
}

/// Why console team prepaid dollars are not shown (honest states, not a soft
/// "feature unfinished" placeholder).
///
/// When Management GET balance succeeds, surfaces show real `$N` instead.
/// Missing key and missing team id are **distinct** plain copy so the operator
/// knows which credential to add (never one mushy "key/team id" line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleTeamPrepaidGap {
    /// No management API key (config or keyring). Team id may still be set.
    #[default]
    MissingManagementKey,
    /// Management key present; `[endpoints] management_team_id` unset/blank.
    MissingTeamId,
    /// Key + team id set; balance not in cache yet (fetch may be in flight).
    Loading,
    /// Key + team id set; balance still unknown (fetch failed or never succeeded).
    Unavailable,
}

impl ConsoleTeamPrepaidGap {
    /// Short honest phrase for footer / `/usage` / `/limits` (ASCII `...` only).
    pub fn as_display_str(self) -> &'static str {
        match self {
            Self::MissingManagementKey => "no management key",
            Self::MissingTeamId => "no management team id",
            Self::Loading => "loading team prepaid...",
            Self::Unavailable => "team prepaid unavailable",
        }
    }

    /// Credits tab phrase when cents are still unknown.
    ///
    /// A console inference key is not a management key. That row says
    /// `not available` instead of `no management key`.
    pub fn credits_tab_unknown_phrase(self, inference_key: bool) -> &'static str {
        match self {
            Self::MissingManagementKey if inference_key => "not available",
            other => other.as_display_str(),
        }
    }

    /// From whether Management key and team id are each present (pre-fetch).
    ///
    /// | key | team | gap |
    /// |-----|------|-----|
    /// | no  | *    | [`Self::MissingManagementKey`] (key is the first blocker) |
    /// | yes | no   | [`Self::Loading`] (key validation may discover team id) |
    /// | yes | yes  | [`Self::Loading`] (cold / fetch may be in flight) |
    ///
    /// There is no process-wide "last fetch failed" bit yet, so surfaces that
    /// just finished a billing fetch and still have no cents should pass
    /// [`Self::after_billing_fetch`] (key without discoverable team →
    /// [`Self::MissingTeamId`]; key+team fetch miss → [`Self::Unavailable`]).
    pub fn from_management_config(has_management_key: bool, has_management_team_id: bool) -> Self {
        match (has_management_key, has_management_team_id) {
            (false, _) => Self::MissingManagementKey,
            // Key alone is enough to attempt discovery + prepaid fetch.
            (true, _) => Self::Loading,
        }
    }

    /// Gap after a completed billing fetch when cents are still unknown.
    ///
    /// | key | team (after discovery) | gap |
    /// |-----|------------------------|-----|
    /// | no  | * | [`Self::MissingManagementKey`] |
    /// | yes | no | [`Self::MissingTeamId`] (validation / pin still needed) |
    /// | yes | yes | [`Self::Unavailable`] (fetch ran; still no balance) |
    pub fn after_billing_fetch(has_management_key: bool, has_management_team_id: bool) -> Self {
        match (has_management_key, has_management_team_id) {
            (false, _) => Self::MissingManagementKey,
            (true, false) => Self::MissingTeamId,
            (true, true) => Self::Unavailable,
        }
    }
}

/// Resolve honest gap from the process Management key + team_id config.
///
/// Configured + cold → [`ConsoleTeamPrepaidGap::Loading`] (footer / pre-fetch).
/// Post-fetch `/usage` should use [`resolve_console_team_prepaid_gap_after_billing_fetch`].
pub fn resolve_console_team_prepaid_gap_default() -> ConsoleTeamPrepaidGap {
    ConsoleTeamPrepaidGap::from_management_config(
        xai_grok_shell::auth::resolve_management_api_key_default().is_some(),
        xai_grok_shell::auth::resolve_management_team_id_default().is_some(),
    )
}

/// Team prepaid cents from the console inference key billing cache.
///
/// No management key. Does not invent a balance and does not call the network.
/// `None` when a management key is configured, when no console inference key
/// exists, or when that cache is cold.
pub fn team_prepaid_cents_from_inference_key_cache() -> Option<i64> {
    if xai_grok_shell::auth::resolve_management_api_key_default().is_some() {
        return None;
    }
    if !xai_grok_shell::auth::console_inference_key_present_default() {
        return None;
    }
    xai_grok_shell::auth::cached_console_team_prepaid_cents_any()
}

/// Gap after a billing fetch completed with cents still unknown.
pub fn resolve_console_team_prepaid_gap_after_billing_fetch() -> ConsoleTeamPrepaidGap {
    ConsoleTeamPrepaidGap::after_billing_fetch(
        xai_grok_shell::auth::resolve_management_api_key_default().is_some(),
        xai_grok_shell::auth::resolve_management_team_id_default().is_some(),
    )
}

/// Compact footer console prepaid cents + gap.
///
/// The live TUI used `AgentView.console_team_prepaid_cents` (never filled) plus
/// the cold [`from_management_config`] gap, so `console · loading team prepaid...`
/// stuck after Management cents were already in the process cache, and after a
/// finished fetch that still had no cents.
pub fn compact_footer_console_prepaid(
    agent_cents: Option<i64>,
    cached_cents: Option<i64>,
    billing_settled: bool,
    has_management_key: bool,
    has_management_team_id: bool,
) -> (Option<i64>, ConsoleTeamPrepaidGap) {
    let cents = agent_cents.or(cached_cents);
    let gap = if cents.is_some() {
        ConsoleTeamPrepaidGap::Loading
    } else if billing_settled {
        ConsoleTeamPrepaidGap::after_billing_fetch(has_management_key, has_management_team_id)
    } else {
        ConsoleTeamPrepaidGap::from_management_config(has_management_key, has_management_team_id)
    };
    (cents, gap)
}

/// Map a dual-auth hop status/toast reason to the **destination** identity.
///
/// Returns `None` when `reason` is not a known identity-switch string.
pub fn sampling_identity_from_hop_reason(reason: &str) -> Option<SamplingIdentityKind> {
    // Exact allow-list mirrors sampler hop copy (no loose substring match).
    match reason {
        "Switched SuperGrok session → console key (out of allowance)"
        | "Switched SuperGrok session → console key (rate limited)"
        | "Switched to next console key (out of allowance)"
        | "Switched to next console key (rate limited)" => Some(SamplingIdentityKind::ConsoleKey),
        "Switched console key → SuperGrok session (out of allowance)"
        | "Switched console key → SuperGrok session (rate limited)"
        | "Switched to next SuperGrok session (out of allowance)"
        | "Switched to next SuperGrok session (rate limited)" => {
            Some(SamplingIdentityKind::SuperGrokSession)
        }
        _ => None,
    }
}

/// Meter identity from tracked UI state plus SuperGrok out-of-allowance memo.
///
/// Silent sticky prefer_live (and restart while the memo lives) can leave
/// samples on the **console key** without hop toast chrome. Tracked state may
/// still default to SuperGrokSession. The footer must follow the **live spend
/// pool**, never SuperGrok dollar credits while console is what Build burns.
///
/// `supergrok_out_of_allowance_with_console_ready` is true when dual-auth can
/// use a console key and the SuperGrok session fingerprint is still memoized
/// out of allowance (process + durable `$GROK_HOME/exhausted_credits/`).
///
/// **Free SuperGrok period headroom wins for status paint:** when the live
/// free-period poll is known and used percent is below 100, use
/// [`status_sampling_identity_for_compact_meter`] instead so a stale exhaust
/// memo cannot paint `console · $N` while free period still has room.
pub fn meter_sampling_identity(
    tracked: SamplingIdentityKind,
    supergrok_out_of_allowance_with_console_ready: bool,
) -> SamplingIdentityKind {
    if tracked.is_console() {
        return SamplingIdentityKind::ConsoleKey;
    }
    if supergrok_out_of_allowance_with_console_ready {
        SamplingIdentityKind::ConsoleKey
    } else {
        tracked
    }
}

/// Which meter is the **active spend driver** for chrome and `/limits`.
///
/// Same order as Design A compact status and included-period-first token economy:
/// included SuperGrok period limits while they still have room, then SuperGrok
/// dollar credits (after-burner), then console key.
///
/// **Intent chrome, not settlement proof.** Team Grok Build / OAuth class and
/// console team prepaid remaining can still move under SuperGrok session while
/// this enum stays included SuperGrok period limits. Those settlement meters
/// are tracked separately (`teamPrepaidUsd`, team postpaid OAuth class); do not
/// read [`ActiveSpendDriver::SuperGrokFreePeriod`] as "team prepaid is not paying."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSpendDriver {
    /// Included SuperGrok period limits are the client spend-order driver (used
    /// % &lt; 100, or full with no SuperGrok dollar credits left so chrome shows the
    /// included-period form). Not proof those included SuperGrok period limits
    /// were debited or that team meters did not settle the work.
    SuperGrokFreePeriod,
    /// Included SuperGrok period limits full and SuperGrok dollar credits known
    /// positive (after-burner).
    SuperGrokDollarCredits,
    /// Console API key is the live sampling principal.
    ConsoleKey,
}

impl ActiveSpendDriver {
    /// Wire / JSON value (`activeDriver` on `limits --json`).
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::SuperGrokFreePeriod => "supergrok_free_period",
            Self::SuperGrokDollarCredits => "supergrok_extras",
            Self::ConsoleKey => "console_key",
        }
    }

    /// Human label for `/limits` **Active:** line (plain American English).
    pub fn as_human(self) -> &'static str {
        match self {
            Self::SuperGrokFreePeriod => "SuperGrok period",
            Self::SuperGrokDollarCredits => "SuperGrok dollar credits",
            Self::ConsoleKey => "console key",
        }
    }
}

/// Combined remaining included SuperGrok period limits from process cache
/// plus the active credit balance. Unknown identities do not add.
///
/// Does **not** copy the live JWT `is_unified_billing_user` onto sibling
/// SuperGrok cache rows. Wire `is_unified_billing_user == true` on a row
/// still counts that identity once. Combined remaining groups by SuperGrok
/// `identity_id`. Compact identity still uses
/// [`independent_included_from_active_and_process_cache`].
pub fn combined_included_from_active_and_process_cache(
    active: Option<&CreditBalance>,
) -> xai_grok_shell::auth::CombinedIncludedRemaining {
    xai_grok_shell::auth::combined_included_remaining(
        &included_pool_readings_from_active_and_process_cache(active),
    )
}

/// Distinct included SuperGrok period pools without a live JWT unified
/// flag on sibling cache rows.
///
/// Compact identity uses this count. A live `is_unified_billing_user` is
/// not proof two independently polled percents are one workspace.
pub fn independent_included_from_active_and_process_cache(
    active: Option<&CreditBalance>,
) -> xai_grok_shell::auth::CombinedIncludedRemaining {
    xai_grok_shell::auth::combined_included_remaining(
        &included_pool_readings_from_active_and_process_cache(active),
    )
}

fn included_pool_readings_from_active_and_process_cache(
    active: Option<&CreditBalance>,
) -> Vec<xai_grok_shell::auth::IncludedPoolReading> {
    use xai_grok_shell::auth::{IncludedPoolReading, included_billing_fields_snapshot};

    let snap = included_billing_fields_snapshot();
    let mut readings: Vec<IncludedPoolReading> = snap
        .into_iter()
        .map(|(identity_id, fields)| IncludedPoolReading {
            identity_id,
            usage_pct: fields.usage_pct,
            reset_at: fields.reset_at,
            // Sibling SuperGrok identities must not inherit the live JWT
            // unified-billing flag. Process cache does not store that flag
            // per identity; leave it unset.
            is_unified_billing_user: None,
        })
        .collect();
    if readings.is_empty()
        && let Some(bal) = active
        && bal.included_usage_known
    {
        readings.push(IncludedPoolReading {
            identity_id: "active".into(),
            usage_pct: Some(bal.usage_pct),
            reset_at: bal.period_end_at,
            // No sibling: this row's own wire flag still counts once.
            is_unified_billing_user: bal.is_unified_billing_user,
        });
    }
    readings
}

/// Active spend driver from live sampling identity, included SuperGrok period
/// limits, and SuperGrok dollar credits.
///
/// Matches Design A compact meter logic. SuperGrok dollar credits balance and team
/// prepaid on the account do **not** flip the driver while included SuperGrok
/// period limits still have room. Console live always returns [`ActiveSpendDriver::ConsoleKey`].
pub fn active_spend_driver(
    live: SamplingIdentityKind,
    included_usage_known: bool,
    included_usage_pct: f64,
    super_grok_dollar_credits_cents: Option<i64>,
) -> ActiveSpendDriver {
    if live.is_console() {
        return ActiveSpendDriver::ConsoleKey;
    }
    if included_usage_known && included_usage_pct >= 100.0 {
        if super_grok_dollar_credits_cents
            .map(i64::abs)
            .is_some_and(|c| c > 0)
        {
            return ActiveSpendDriver::SuperGrokDollarCredits;
        }
        return ActiveSpendDriver::SuperGrokFreePeriod;
    }
    ActiveSpendDriver::SuperGrokFreePeriod
}

/// Status compact-meter sampling identity under included-period-first chrome law.
///
/// Sticky exhaust memo (`memo_out_of_allowance_console_ready`) and a tracked
/// console sampling identity must **not** force console chrome when included
/// SuperGrok period limits are known and still have room (used percent below
/// 100). Compact names that included pool, not `console · $N`.
///
/// When included SuperGrok period limits are full (≥ 100%) or usage unknown,
/// sticky memo may still pin console (true after-full / cold sticky path).
/// Tracked console stays console only on that after-full or unknown path.
pub fn status_sampling_identity_for_compact_meter(
    tracked: SamplingIdentityKind,
    free_period_usage_known: bool,
    free_period_usage_pct: f64,
    memo_out_of_allowance_console_ready: bool,
) -> SamplingIdentityKind {
    // Included SuperGrok period limits with room win compact paint, even if
    // tracked sampling is console (sticky hop, fail-open, preferred_method).
    if free_period_usage_known && free_period_usage_pct < 100.0 {
        return SamplingIdentityKind::SuperGrokSession;
    }
    if tracked.is_console() {
        return SamplingIdentityKind::ConsoleKey;
    }
    if memo_out_of_allowance_console_ready {
        return SamplingIdentityKind::ConsoleKey;
    }
    tracked
}

/// Compact-meter / `/limits` identity from tracked hop state plus combined
/// included SuperGrok period remaining.
///
/// After a hop toast, `tracked` is the destination (`sampling_identity_from_hop_reason`).
/// Combined remaining still blocks a stale exhaust memo from painting console
/// while a sibling included pool has room.
pub fn compact_meter_identity(
    tracked: SamplingIdentityKind,
    balance: Option<&CreditBalance>,
) -> SamplingIdentityKind {
    let combined = combined_included_from_active_and_process_cache(balance);
    let (known, pct) = xai_grok_shell::auth::chrome_included_usage_from_combined(
        balance.is_some_and(|b| b.included_usage_known),
        balance.map(|b| b.usage_pct).unwrap_or(0.0),
        &combined,
    );
    status_sampling_identity_for_compact_meter(
        tracked,
        known,
        pct,
        xai_grok_shell::auth::supergrok_out_of_allowance_with_console_ready(
            &xai_grok_config::grok_home(),
        ),
    )
}

/// Tracked identity update after billing allowance-exhaust sync.
///
/// - `marked`: SuperGrok included full (or re-mark) → console is live next request
/// - `cleared`: period reset; SuperGrok available again **unless** console is
///   the auth primary (`preferred_method = api_key` / `is_api_key_auth`)
/// - neither: leave tracked identity unchanged (`None`)
///
/// `marked` wins if both flags are ever true.
pub fn sampling_identity_after_allowance_sync(
    marked: bool,
    cleared: bool,
    console_auth_primary: bool,
) -> Option<SamplingIdentityKind> {
    if marked {
        return Some(SamplingIdentityKind::ConsoleKey);
    }
    if cleared {
        return Some(if console_auth_primary {
            SamplingIdentityKind::ConsoleKey
        } else {
            SamplingIdentityKind::SuperGrokSession
        });
    }
    None
}

impl Default for CreditBalance {
    fn default() -> Self {
        Self {
            usage_pct: 0.0,
            effective_usage_pct: 0.0,
            period_end_display: None,
            period_end_at: None,
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: None,
            period_type: None,
            is_unified_billing_user: None,
            grok_build_usage_pct: None,
            included_usage_known: false,
        }
    }
}

impl CreditBalance {
    /// Label for the percentage allowance, chosen from the period type: "Weekly limit" / "Monthly limit", falling back to "Usage" when unknown.
    pub fn usage_label(&self) -> &'static str {
        match self.period_type.as_deref() {
            Some(t) if t.contains("WEEKLY") => "Weekly limit",
            Some(t) if t.contains("MONTHLY") => "Monthly limit",
            _ => "Usage",
        }
    }

    /// Free SuperGrok period linear-burn pacing when period end + type allow it.
    ///
    /// Uses free SuperGrok period **used percent** only (never dollars). Missing
    /// bounds → `None`. Respects `[token_economy] show_period_pacing`.
    pub fn period_pacing(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<xai_grok_shell::token_economy::PeriodPacing> {
        let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
        if !cfg.show_period_pacing {
            return None;
        }
        let end = self.period_end_at?;
        let start = xai_grok_shell::token_economy::resolve_period_start(
            None,
            Some(end),
            self.period_type.as_deref(),
        )?;
        xai_grok_shell::token_economy::compute_period_pacing(self.usage_pct, start, end, now)
    }

    /// Compact pacing chip for credit/status chrome, or `None` when omitted.
    pub fn pacing_chip(
        &self,
        live: SamplingIdentityKind,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<String> {
        let p = self.period_pacing(now)?;
        Some(if live.is_console() {
            p.compact_label_console_live()
        } else {
            p.compact_label()
        })
    }

    /// Full pacing sentence for `/usage` / `/limits`, or `None` when omitted.
    pub fn pacing_sentence(
        &self,
        live: SamplingIdentityKind,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<String> {
        let p = self.period_pacing(now)?;
        Some(if live.is_console() {
            p.full_sentence_console_live()
        } else {
            p.full_sentence()
        })
    }
}

/// Auto top-up rule data used by the `/usage` summary.
#[derive(Debug, Clone)]
pub struct AutoTopupInfo {
    /// Whether auto top-up is enabled.
    pub enabled: bool,
    /// Per-trigger top-up amount in USD cents.
    pub topup_amount_cents: Option<i64>,
    /// Optional maximum monthly top-up amount in USD cents.
    pub max_amount_cents: Option<i64>,
}

impl AutoTopupInfo {
    /// A known "auto top-up disabled" state, distinct from an unresolved `None`, which means the rule hasn't been fetched yet.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            topup_amount_cents: None,
            max_amount_cents: None,
        }
    }
}

/// Outcome of an auto top-up rule fetch, so a transient failure doesn't clear a previously cached rule.
#[derive(Debug, Clone)]
pub enum AutoTopupFetch {
    /// A definitive rule state (a real rule, or [`AutoTopupInfo::disabled`] when the backend reports none).
    /// Stored as the *known* auto top-up state.
    Resolved(AutoTopupInfo),
    /// Fetch failed; keep the cached value.
    /// A stored `None` therefore means "not yet known", not "no auto top-up".
    Unchanged,
    /// The rule is not applicable (no prepaid credits); reset the cache to "unknown" so a later credits period doesn't read a stale rule.
    Cleared,
}

/// Outcome of a SuperGrok included/credits billing fetch.
///
/// Mirrors [`AutoTopupFetch`]: a transport or parse failure must **not** wipe
/// last-known SuperGrok chrome when OpenRouter or console team prepaid still
/// succeed. Only a successful response with **no** `config` object clears the
/// SuperGrok cache (`Resolved(None)`).
#[derive(Debug, Clone)]
pub enum CreditBalanceFetch {
    /// SuperGrok path succeeded. `None` = response carried no billing config
    /// (clear SuperGrok app/agent cache). `Some` = apply that balance.
    Resolved(Option<CreditBalance>),
    /// SuperGrok transport/parse failed — keep last-known-good SuperGrok
    /// balance. Side meters (OpenRouter / console prepaid) may still update.
    Unchanged,
}

/// Pure SuperGrok cache policy (no network). Unit-tested so effects cannot
/// silently regress to "side meter success ⇒ wipe SuperGrok."
///
/// - `supergrok_ok = true` → [`CreditBalanceFetch::Resolved`]`(balance_when_ok)`
/// - `supergrok_ok = false` → [`CreditBalanceFetch::Unchanged`]
pub fn credit_balance_fetch_from_supergrok_path(
    supergrok_ok: bool,
    balance_when_ok: Option<CreditBalance>,
) -> CreditBalanceFetch {
    if supergrok_ok {
        CreditBalanceFetch::Resolved(balance_when_ok)
    } else {
        CreditBalanceFetch::Unchanged
    }
}

/// Whether a SuperGrok balance may feed ranking cache / allowance exhaust.
///
/// Placeholder `usage_pct: 0.0` with `included_usage_known: false` must not
/// poison ranking or clear a Marked exhaust memo as if free SuperGrok period
/// reset.
pub fn should_apply_included_usage_side_effects(bal: &CreditBalance) -> bool {
    bal.included_usage_known
}

/// Format `cents` as a dollar string: whole dollars as `$N`, otherwise `$N.NN`.
fn fmt_dollars(cents: i64) -> String {
    let dollars = cents as f64 / 100.0;
    if dollars.fract() == 0.0 {
        format!("${dollars:.0}")
    } else {
        format!("${dollars:.2}")
    }
}

/// Build the `/usage` summary block shown in scrollback. Always shows usage % and (when known) the
/// next reset time. The credits block is rendered only when the user has a positive prepaid
/// balance.
pub fn format_usage_summary(balance: &CreditBalance, autotopup: Option<&AutoTopupInfo>) -> String {
    // Floor to match the backend SpendingLimiter's `as u8` truncation (99.994% renders as 99%, never 100% until truly exhausted)
    let mut lines = vec![format!(
        "{}: {}%",
        balance.usage_label(),
        balance.usage_pct.floor() as i64
    )];
    if let Some(reset) = &balance.period_end_display {
        lines.push(format!("Next reset: {reset}"));
    }
    // Free SuperGrok period linear-burn pacing (omit when bounds missing).
    if balance.included_usage_known
        && let Some(pacing) = balance.pacing_sentence(live, now)
    {
        lines.push(pacing);
    }
    // Branch 2b: surface Grok Build productUsage % when observed (distinct
    // from top-level included %). Shared phrase with `/limits` (Issue 5).
    if let Some(build_pct) = balance.grok_build_usage_pct {
        lines.push(crate::views::limits_honesty::format_grok_build_product_usage_line(build_pct));
    }

    // Billing stores credit / top-up amounts as negative cents (accounting convention); display the absolute USD value, matching the web clients
    if let Some(prepaid) = balance
        .prepaid_balance_cents
        .map(i64::abs)
        .filter(|c| *c > 0)
    {
        lines.push(String::new());
        lines.push(format!(
            "SuperGrok dollar credits: {}",
            fmt_dollars(prepaid)
        ));
        match autotopup {
            Some(at) if at.enabled && at.topup_amount_cents.is_some() => {
                lines.push(format!(
                    "Auto topup: {}",
                    fmt_dollars(at.topup_amount_cents.unwrap().abs())
                ));
                if let Some(max) = at.max_amount_cents {
                    lines.push(format!("Max monthly topup: {}", fmt_dollars(max.abs())));
                }
            }
            _ => lines.push("Auto topup: disabled".to_string()),
        }
    }

    // Legacy on-demand (pay-as-you-go) billing, shown only when enabled, for users on the older monthly and on-demand model
    // Amounts always carry cents (e.g. `$50.00`), matching the web client.
    if balance.pay_as_you_go {
        let used = balance.on_demand_used_cents.unwrap_or(0).abs() as f64 / 100.0;
        let cap = balance.on_demand_cap_cents.unwrap_or(0).abs() as f64 / 100.0;
        lines.push(String::new());
        lines.push(format!("Pay-as-you-go: ${used:.2} used of ${cap:.2} limit"));
    }

    lines.join("\n")
}

/// `/usage` billing follow-up keyed by **live sampling identity**.
///
/// When live sampling is a **console key**, names **console team prepaid**
/// (Management API cents) or an honest gap ([`ConsoleTeamPrepaidGap`]). Does
/// **not** present SuperGrok session billing / SuperGrok dollar credits as the live
/// console spend (those are a different pool). SuperGrok-primary keeps
/// [`format_usage_summary`].
pub fn format_usage_summary_with_live_identity(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    sampling_identity: SamplingIdentityKind,
    console_team_prepaid_cents: Option<i64>,
) -> String {
    format_usage_summary_with_live_identity_and_gap(
        balance,
        autotopup,
        sampling_identity,
        console_team_prepaid_cents,
        // Callers that know config should pass an explicit gap; default is the
        // most common dogfood miss (no management key stored yet).
        ConsoleTeamPrepaidGap::MissingManagementKey,
    )
}

/// Like [`format_usage_summary_with_live_identity`] with an explicit gap reason
/// when cents are unknown.
pub fn format_usage_summary_with_live_identity_and_gap(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    sampling_identity: SamplingIdentityKind,
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: ConsoleTeamPrepaidGap,
) -> String {
    format_usage_summary_with_live_identity_gap_and_honesty(
        balance,
        autotopup,
        sampling_identity,
        console_team_prepaid_cents,
        console_team_prepaid_gap,
        false,
        false,
        false,
        false,
    )
}

/// Like [`format_usage_summary_with_live_identity_and_gap`] plus SuperGrok
/// honesty notes (branch 2b flat-poll + C6 OAuth Usage).
///
/// When live sampling is SuperGrok session, appends the same honesty stack as
/// `/limits` for the given flags. Console live never gets SuperGrok burn /
/// flat / C6 notes (meters stay distinct; no session-path parenthetical).
///
/// `has_included_reading` aligns with snapshot: a SuperGrok `CreditBalance`
/// always carries included usage % (same meter as `primary.included.is_some()`
/// on `/limits`), so `balance.is_some()` is the correct gate (Issue 7).
pub fn format_usage_summary_with_live_identity_gap_and_honesty(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    sampling_identity: SamplingIdentityKind,
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: ConsoleTeamPrepaidGap,
    flat_poll_unproven_debit: bool,
    flat_poll_observed_build: bool,
    flat_poll_observed_dollar_credits: bool,
    oauth_postpaid_dominates: bool,
) -> String {
    if sampling_identity.is_console() {
        let mut lines = vec![format!("Live sampling: {}", sampling_identity.as_str())];
        match console_team_prepaid_cents {
            Some(cents) => lines.push(format!(
                "Console team prepaid: {}",
                fmt_dollars(cents.abs())
            )),
            None => lines.push(format!(
                "Console team prepaid: {}",
                console_team_prepaid_gap.as_display_str()
            )),
        }
        // SuperGrok period pacing is context only (not live principal); never dollars.
        if let Some(bal) = balance
            && let Some(pacing) =
                bal.pacing_sentence(SamplingIdentityKind::ConsoleKey, chrono::Utc::now())
        {
            lines.push(pacing);
        }
        return lines.join("\n");
    }

    let mut body = match balance {
        Some(bal) => {
            format_usage_summary_with_live(bal, autotopup, sampling_identity, chrono::Utc::now())
        }
        None => "No billing data available.".to_string(),
    };
    // SuperGrok live still surfaces team Management prepaid when known (or an
    // honest gap when the Management path is active). Distinct from SuperGrok
    // dollar credits; never re-labels live sampling as console.
    match console_team_prepaid_cents {
        Some(cents) => {
            body.push('\n');
            body.push_str(&format!(
                "Console team prepaid: {}",
                fmt_dollars(cents.abs())
            ));
        }
        None => match console_team_prepaid_gap {
            ConsoleTeamPrepaidGap::MissingManagementKey => {
                // SuperGrok-only mid-period: keep /usage quiet on team gap.
                // (Footer high-usage path still appends the gap.)
            }
            gap => {
                body.push('\n');
                body.push_str(&format!("Console team prepaid: {}", gap.as_display_str()));
            }
        },
    }
    // Honest included reading only when the balance carries a known meter
    // (mirrors snapshot primary.included.is_some() for the single-cache path).
    let has_included_reading = balance.is_some_and(|b| b.included_usage_known);
    // Pure guard from flags already on this /usage path (no history re-scan).
    let turns_blocked = {
        use xai_grok_shell::auth::{
            allow_spend_when_free_period_debit_unproven_from_config,
            free_period_headroom_from_usage_readings,
            should_block_spend_when_free_period_debit_unproven,
        };
        let allow = allow_spend_when_free_period_debit_unproven_from_config();
        let reading = balance.and_then(|b| {
            if b.included_usage_known {
                Some(b.usage_pct)
            } else {
                None
            }
        });
        let head = free_period_headroom_from_usage_readings(&[reading]);
        should_block_spend_when_free_period_debit_unproven(
            allow,
            sampling_identity.is_console(),
            head.usage_known,
            head.has_headroom,
            flat_poll_unproven_debit,
        )
    };
    let notes = crate::views::limits_honesty::honesty_notes_for_limits(
        crate::views::limits_honesty::LimitsHonestyInput {
            live: sampling_identity,
            has_included_reading,
            flat_poll_unproven_debit,
            flat_poll_observed_build,
            flat_poll_observed_dollar_credits,
            oauth_postpaid_dominates,
            has_console_team_prepaid_reading: console_team_prepaid_cents.is_some(),
            // Default credits live on `/limits` postpaid preview, not `/usage`.
            has_team_default_credits_reading: false,
            turns_blocked_free_period_debit_unproven: turns_blocked,
            billing_credits_card: Default::default(),
        },
    );
    for note in notes {
        body.push('\n');
        body.push_str(&note);
    }
    body
}

/// Low-balance ($10) and pay-as-you-go critical ($5) warning thresholds, in cents.
const LOW_BALANCE_CENTS: i64 = 1000;
const PAY_AS_YOU_GO_CRITICAL_CENTS: i64 = 500;

/// The prompt's usage/credits warning as `(text, critical)`, or `None`. `critical` renders yellow,
/// else grey; team users with `usage_visible = false` never warn. Gateway light-frontend (`kind:
/// "chat"`) sessions must not show Build coding-credit warnings.
pub fn usage_warning(
    balance: &CreditBalance,
    autotopup: Option<&AutoTopupInfo>,
    usage_visible: bool,
) -> Option<(String, bool)> {
    usage_warning_for_session(balance, autotopup, usage_visible, false)
}

/// Like [`usage_warning`], but suppresses output for gateway/chat-kind sessions.
pub fn usage_warning_for_session(
    balance: &CreditBalance,
    autotopup: Option<&AutoTopupInfo>,
    usage_visible: bool,
    gateway_chat: bool,
) -> Option<(String, bool)> {
    usage_warning_for_session_with_openrouter(
        Some(balance),
        autotopup,
        None,
        usage_visible,
        gateway_chat,
        false,
    )
}

/// Prompt info-row warning, optionally preferring OpenRouter account credits
/// when the active model is OpenRouter-backed.
///
/// Defaults live sampling identity to SuperGrok session. Prefer
/// [`usage_warning_for_session_with_identity`] when the pager knows the live
/// primary (console key after stay-on-console, hop toast, etc.).
pub fn usage_warning_for_session_with_openrouter(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    openrouter: Option<&OpenRouterCreditBalance>,
    usage_visible: bool,
    gateway_chat: bool,
    openrouter_model: bool,
) -> Option<(String, bool)> {
    usage_warning_for_session_with_identity(
        balance,
        autotopup,
        openrouter,
        usage_visible,
        gateway_chat,
        openrouter_model,
        SamplingIdentityKind::SuperGrokSession,
    )
}

/// Like [`usage_warning_for_session_with_openrouter`], but labels the meter by
/// **live sampling identity**.
///
/// When `openrouter_model` is true and an OR balance is known, always shows
/// `OpenRouter credits left: $N` (yellow when ≤ $10). xAI SuperGrok billing is
/// ignored for that model so the footer matches the provider actually charged.
///
/// When live primary is a **console key**, never presents SuperGrok dollar
/// credits as the spend meter (personal SuperGrok $ is a different pool). Shows
/// console team prepaid dollars when Management API cents are known, else an
/// honest gap (`console key · no management key` / `no management team id` /
/// loading / unavailable).
///
/// When live primary is SuperGrok, prepaid is labeled **SuperGrok dollar credits
/// left**, never generic "Credits left".
pub fn usage_warning_for_session_with_identity(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    openrouter: Option<&OpenRouterCreditBalance>,
    usage_visible: bool,
    gateway_chat: bool,
    openrouter_model: bool,
    sampling_identity: SamplingIdentityKind,
) -> Option<(String, bool)> {
    usage_warning_for_session_with_identity_and_principal(
        balance,
        autotopup,
        openrouter,
        usage_visible,
        gateway_chat,
        openrouter_model,
        sampling_identity,
        None,
        None,
    )
}

/// Like [`usage_warning_for_session_with_identity`] with optional live SuperGrok
/// principal role (`"personal"` / `"business"`) for dual-login footers.
///
/// `console_team_prepaid_cents` is Management API team prepaid remaining
/// (absolute USD cents). Only used when live identity is console; never mixed
/// with SuperGrok dollar credits. When cents are `None`, uses
/// [`ConsoleTeamPrepaidGap::MissingManagementKey`] — prefer
/// [`usage_warning_for_session_with_identity_principal_and_gap`] when the
/// caller knows the real gap reason.
pub fn usage_warning_for_session_with_identity_and_principal(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    openrouter: Option<&OpenRouterCreditBalance>,
    usage_visible: bool,
    gateway_chat: bool,
    openrouter_model: bool,
    sampling_identity: SamplingIdentityKind,
    live_principal_role: Option<&str>,
    console_team_prepaid_cents: Option<i64>,
) -> Option<(String, bool)> {
    usage_warning_for_session_with_identity_principal_and_gap(
        balance,
        autotopup,
        openrouter,
        usage_visible,
        gateway_chat,
        openrouter_model,
        sampling_identity,
        live_principal_role,
        console_team_prepaid_cents,
        ConsoleTeamPrepaidGap::MissingManagementKey,
    )
}

/// Like [`usage_warning_for_session_with_identity_and_principal`] with an
/// explicit [`ConsoleTeamPrepaidGap`] when cents are unknown.
///
/// When live sampling is **SuperGrok session**, team Management prepaid is
/// still surfaced when known (or an honest gap when the Management path is
/// active / dual-auth dogfood expects a team section). SuperGrok % / SuperGrok
/// dollar credits stay the primary SuperGrok story; team dollars never re-label live sampling
/// as console. Does not surface team postpaid OAuth / Grok Build class; use
/// [`usage_warning_for_session_with_identity_principal_gap_and_postpaid`] when
/// that period class is known.
pub fn usage_warning_for_session_with_identity_principal_and_gap(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    openrouter: Option<&OpenRouterCreditBalance>,
    usage_visible: bool,
    gateway_chat: bool,
    openrouter_model: bool,
    sampling_identity: SamplingIdentityKind,
    live_principal_role: Option<&str>,
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: ConsoleTeamPrepaidGap,
) -> Option<(String, bool)> {
    usage_warning_for_session_with_identity_principal_gap_and_postpaid(
        balance,
        autotopup,
        openrouter,
        usage_visible,
        gateway_chat,
        openrouter_model,
        sampling_identity,
        live_principal_role,
        console_team_prepaid_cents,
        console_team_prepaid_gap,
        None,
    )
}

/// Like [`usage_warning_for_session_with_identity_principal_and_gap`] with
/// optional team **postpaid OAuth / Grok Build class** period cents.
///
/// SuperGrok live: included SuperGrok period limits / SuperGrok dollar credits
/// stay the SuperGrok story.
/// While free SuperGrok period still has room, the prompt footer stays quiet on
/// team wallets (no long "not the active spend path" team prepaid / Grok Build
/// class line next to model name). Compact status already names free SuperGrok
/// period; full team meters live on `/limits`. After free SuperGrok period is
/// full, secondary team prepaid / Grok Build class may attach under the
/// not-active-spend label. Never mashes postpaid class into prepaid or free
/// SuperGrok period %. Compact status bar (Design A free-period `%`) is a
/// different path.
pub fn usage_warning_for_session_with_identity_principal_gap_and_postpaid(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    openrouter: Option<&OpenRouterCreditBalance>,
    usage_visible: bool,
    gateway_chat: bool,
    openrouter_model: bool,
    sampling_identity: SamplingIdentityKind,
    live_principal_role: Option<&str>,
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: ConsoleTeamPrepaidGap,
    team_postpaid_oauth_class_cents: Option<i64>,
) -> Option<(String, bool)> {
    if gateway_chat || !usage_visible {
        return None;
    }

    if openrouter_model {
        let or = openrouter?;
        // Show remaining even at $0 so the user sees the balance was fetched.
        let text = format!(
            "OpenRouter credits left: {}",
            fmt_dollars(or.balance_cents.abs())
        );
        let critical = or.balance_cents.abs() <= LOW_BALANCE_CENTS || or.balance_cents <= 0;
        return Some((text, critical));
    }

    // Console / Business API key is live: do not show SuperGrok dollar credits
    // or included-% as if they were the pool Build is burning. When Management
    // prepaid cents are known, show plain console team prepaid dollars.
    // Honest gap still beats the wrong SuperGrok number. Optional Grok Build
    // class period $ attaches as a second chip (distinct from prepaid).
    if sampling_identity.is_console() {
        let label = sampling_identity.as_str();
        let mut chars = label.chars();
        let labeled = match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        };
        let mut text = if let Some(cents) = console_team_prepaid_cents {
            let remaining = cents.abs();
            format!("{labeled} · team prepaid: {}", fmt_dollars(remaining))
        } else {
            format!("{labeled} · {}", console_team_prepaid_gap.as_display_str())
        };
        let critical = console_team_prepaid_cents
            .map(|c| c.abs() <= LOW_BALANCE_CENTS)
            .unwrap_or(false);
        if let Some(chip) = team_grok_build_class_footer_chip(team_postpaid_oauth_class_cents) {
            text = format!("{text} · {chip}");
        }
        return Some((text, critical));
    }

    // SuperGrok session live path (below). Free SuperGrok period with room:
    // SuperGrok-only footer warnings (high free-period %, SuperGrok dollar
    // credits). Team Management prepaid / Grok Build class stay off the footer
    // until free SuperGrok period is full (then secondary not-active-spend).
    let free_period_has_room = free_supergrok_period_has_room(balance);
    let supergrok_warning =
        supergrok_session_usage_warning(balance, autotopup, live_principal_role);
    merge_supergrok_warning_with_team_meters(
        supergrok_warning,
        console_team_prepaid_cents,
        console_team_prepaid_gap,
        team_postpaid_oauth_class_cents,
        free_period_has_room,
    )
}

/// True when free SuperGrok period limits are known and still have room
/// (`usage_pct` below 100). That is the Design A primary spend path; the
/// prompt footer must not dominate with secondary team wallet copy.
fn free_supergrok_period_has_room(balance: Option<&CreditBalance>) -> bool {
    balance.is_some_and(|b| b.included_usage_known && b.usage_pct < 100.0)
}

/// Footer chip for team postpaid OAuth / Grok Build class period dollars.
///
/// Returns `None` when cents are unknown or zero (do not invent prominence).
/// Distinct from team prepaid remaining and free SuperGrok period %.
/// Console-live footers use this chip as-is. SuperGrok-live secondary team
/// footers use [`format_team_settlement_footer`] instead (same dollars,
/// labeled as not the active spend path).
pub fn team_grok_build_class_footer_chip(oauth_class_cents: Option<i64>) -> Option<String> {
    let cents = oauth_class_cents?.abs();
    if cents == 0 {
        return None;
    }
    Some(format!("team Grok Build class: {}", fmt_dollars(cents)))
}

/// Plain prefix for SuperGrok-live team prepaid / Grok Build class footer chips.
///
/// Under SuperGrok session these meters are **secondary team wallet readings**
/// (Management team prepaid remaining and/or Grok Build class period $). They
/// are **not** the Design A compact spend-order driver (free SuperGrok period %
/// / SuperGrok dollar credits / console key). The prefix must make that
/// unmissable: bare "Team settlement" failed (operators read it as "we are
/// paying team prepaid now" while free SuperGrok period still had room).
pub const TEAM_SECONDARY_METERS_LABEL: &str = "not the active spend path";

/// Deprecated alias for [`TEAM_SECONDARY_METERS_LABEL`] (older call sites / docs).
#[deprecated(note = "use TEAM_SECONDARY_METERS_LABEL; Team settlement was misread as active pay")]
pub const TEAM_SETTLEMENT_LABEL: &str = TEAM_SECONDARY_METERS_LABEL;

/// SuperGrok-live footer fragment for secondary team wallet meters.
///
/// Builds `not the active spend path: team prepaid remaining $N · Grok Build
/// class $M` (parts omitted when unknown). Call only after free SuperGrok
/// period is full (see [`merge_supergrok_warning_with_team_meters`]); while free
/// SuperGrok period has room the prompt footer stays quiet on team $ and this
/// helper is not used. Never invents dollars. Missing management key omits the
/// prepaid gap (that honesty lives on `/limits`); postpaid class may still
/// attach. Loading / unavailable gaps show when the Management path is active
/// after free SuperGrok period is full.
fn format_team_settlement_footer(
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: ConsoleTeamPrepaidGap,
    team_postpaid_oauth_class_cents: Option<i64>,
) -> Option<(String, bool)> {
    let mut parts: Vec<String> = Vec::new();
    let mut critical = false;

    match console_team_prepaid_cents {
        Some(cents) => {
            let remaining = cents.abs();
            // Same "remaining" vocabulary as `/limits` Team prepaid remaining.
            parts.push(format!("team prepaid remaining {}", fmt_dollars(remaining)));
            critical = remaining <= LOW_BALANCE_CENTS;
        }
        None => match console_team_prepaid_gap {
            // No management key: team honesty lives on `/limits` Console API
            // Balance. Do not invent a prepaid gap on SuperGrok-only footers.
            ConsoleTeamPrepaidGap::MissingManagementKey => {}
            gap => {
                // Key present (or post-fetch miss): Management path is active.
                parts.push(gap.as_display_str().to_string());
            }
        },
    }

    if let Some(cents) = team_postpaid_oauth_class_cents
        .map(i64::abs)
        .filter(|c| *c > 0)
    {
        parts.push(format!("Grok Build class {}", fmt_dollars(cents)));
    }

    if parts.is_empty() {
        return None;
    }
    let body = parts.join(" · ");
    Some((format!("{TEAM_SECONDARY_METERS_LABEL}: {body}"), critical))
}

/// SuperGrok-only footer warning (included % / SuperGrok dollar credits). No team
/// Management dollars here; caller merges via
/// [`merge_supergrok_warning_with_team_prepaid`].
fn supergrok_session_usage_warning(
    balance: Option<&CreditBalance>,
    autotopup: Option<&AutoTopupInfo>,
    live_principal_role: Option<&str>,
) -> Option<(String, bool)> {
    let balance = balance?;
    let role_suffix = live_principal_role
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({s})"))
        .unwrap_or_default();

    // A non-zero prepaid balance (stored as signed cents) means SuperGrok
    // dollar credits from the session billing path.
    let credits = balance
        .prepaid_balance_cents
        .map(i64::abs)
        .filter(|c| *c > 0);

    let Some(credits_cents) = credits else {
        // Pay-as-you-go (legacy on-demand): warn on dollars left in the cap once the included allowance is spent
        if balance.pay_as_you_go {
            if balance.usage_pct >= 100.0 {
                let cap = balance.on_demand_cap_cents.unwrap_or(0).abs();
                let used = balance.on_demand_used_cents.unwrap_or(0).abs();
                let remaining = (cap - used).max(0);
                if remaining <= LOW_BALANCE_CENTS {
                    let text = format!("Pay-as-you-go limit left: {}", fmt_dollars(remaining));
                    return Some((text, remaining <= PAY_AS_YOU_GO_CRITICAL_CENTS));
                }
            }
            return None;
        }

        let pct = balance.effective_usage_pct;
        if pct > 90.0 {
            // "Left" is the complement of floored usage, so it agrees with the floored summary: 99.994% shows "1% left", not "0%"
            let remaining = (100 - pct.floor() as i64).max(0);
            let label = balance.usage_label();
            return Some((
                format!("{label} left{role_suffix}: {remaining}%"),
                pct > 95.0,
            ));
        }
        return None;
    };

    // SuperGrok dollar credits are only drawn down at 100% included usage; don't
    // warn before then.
    if balance.usage_pct < 100.0 {
        return None;
    }

    let credits_warning = || {
        (
            format!(
                "SuperGrok dollar credits left{role_suffix}: {}",
                fmt_dollars(credits_cents)
            ),
            true,
        )
    };

    // Auto top-up gates the warning: unknown stays silent; disabled warns when low
    // Enabled without a max never warns; enabled with a max warns below one top-up amount
    match autotopup {
        None => None,
        Some(at) if !at.enabled => (credits_cents <= LOW_BALANCE_CENTS).then(credits_warning),
        Some(at) if at.max_amount_cents.is_none() => None,
        Some(at) => at
            .topup_amount_cents
            .map(i64::abs)
            .and_then(|amt| (credits_cents < amt).then(credits_warning)),
    }
}

/// Gateway light-frontend (`kind: "chat"`) sessions must not show Build coding credits. Remote
/// settings or a managed opt-in for chat entry can share the same gate later; for now it only
/// suppresses misleading local telemetry.
pub fn credit_bar_line(balance: &CreditBalance, hovered: bool, theme: &Theme) -> Line<'static> {
    credit_bar_line_for_session(balance, hovered, theme, false)
        .expect("non-chat credit_bar_line always renders")
}

/// Like [`credit_bar_line`], but returns `None` for gateway/chat-kind sessions so the status bar never implies Build sampler / coding-credit usage.
pub fn credit_bar_line_for_session(
    balance: &CreditBalance,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
) -> Option<Line<'static>> {
    credit_bar_line_for_session_with_workspace(balance, hovered, theme, gateway_chat, None)
}

/// Like [`credit_bar_line_for_session`] with an honest workspace word when
/// the compact percent is a single known pool (`personal` / `business`) or
/// combined remaining across distinct pools (`combined`).
pub fn credit_bar_line_for_session_with_workspace(
    balance: &CreditBalance,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
    live_principal_role: Option<&str>,
) -> Option<Line<'static>> {
    credit_bar_line_for_session_emphasizing_meter_source(
        balance,
        hovered,
        theme,
        gateway_chat,
        live_principal_role,
        None,
        ConsoleTeamPrepaidGap::MissingManagementKey,
        None,
    )
}

/// SuperGrok compact chip that can emphasize a `/limits meter` pin.
pub fn credit_bar_line_for_session_emphasizing_meter_source(
    balance: &CreditBalance,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
    live_principal_role: Option<&str>,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    meter_source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
) -> Option<Line<'static>> {
    if gateway_chat {
        return None;
    }
    let active_auth_failed = active_supergrok_poll_auth_failed_from_process();
    let pin_skips_included_loading = matches!(
        meter_source,
        Some(xai_grok_shell::auth::limits_pins::MeterSource::DollarCredits)
            | Some(xai_grok_shell::auth::limits_pins::MeterSource::Console)
    );
    if (!balance.included_usage_known || active_auth_failed) && !pin_skips_included_loading {
        return Some(credit_bar_loading_line(hovered, theme));
    }

    let combined = combined_included_from_active_and_process_cache(Some(balance));
    let (included_known, included_pct) = xai_grok_shell::auth::chrome_included_usage_from_combined(
        balance.included_usage_known,
        balance.usage_pct,
        &combined,
    );
    // Identity must not use the unified-collapsed pool count. Copying the live
    // JWT flag onto sibling rows can make distinct_pool_count 1 while the
    // painted percent is another workspace's poll.
    let independent = independent_included_from_active_and_process_cache(Some(balance));
    let meter = compact_meter_text_for_meter_source(
        meter_source,
        SamplingIdentityKind::SuperGrokSession,
        included_known,
        included_pct,
        console_prepaid_cents,
        console_gap,
        balance.prepaid_balance_cents,
        compact_included_workspace_qualifier_for_painted(
            independent.distinct_pool_count,
            live_principal_role,
            Some(balance.usage_pct),
            Some(included_pct),
        ),
    );

    // Included SuperGrok period limits % path may append linear-burn pacing.
    // SuperGrok dollar credits $ path does not (period is full; pacing is about
    // included burn).
    let on_dollar_credits = meter.contains("SuperGrok dollar credits");
    let on_console = meter.starts_with("console");
    let text = if on_dollar_credits || on_console {
        meter
    } else {
        match balance.pacing_chip(SamplingIdentityKind::SuperGrokSession, chrono::Utc::now()) {
            Some(chip) if chip.len() <= 28 => format!("{meter} · {chip}"),
            _ => meter,
        }
    };

    // Combined remaining is for multi-pool chrome (stay on included while
    // a sibling pool still has room). Color thresholds are on the live
    // reading when there is only one distinct pool so 79.9 stays success
    // and 80.0 is warning. Reconstructing used % from floored remaining
    // would turn 79.9 into 80 and paint the wrong color.
    let included_fill_pct = if combined.distinct_pool_count > 1 {
        included_pct
    } else {
        balance.usage_pct
    };
    let color = if on_dollar_credits {
        let cents = balance.prepaid_balance_cents.map(i64::abs).unwrap_or(0);
        if cents <= LOW_BALANCE_CENTS {
            theme.warning
        } else {
            theme.accent_success
        }
    } else if included_fill_pct >= 100.0 {
        theme.accent_error
    } else if included_fill_pct >= 80.0 {
        theme.warning
    } else {
        theme.accent_success
    };

    // In-place hover swap, same as the context-window chip: progress bar plus
    // fmt_pct5 at the default chip width. Only when this chip is naming
    // included SuperGrok period limits. SuperGrok dollar credits stay `$`.
    // CreditBalance has used %, not a remaining count. Do not invent 490/510.
    if hovered && !on_dollar_credits && !on_console {
        const PCT_WIDTH: u16 = 5;
        const BAR_PCT_GAP: u16 = 1;
        let min_width = BAR_PCT_GAP + PCT_WIDTH;
        let natural_width = text.chars().count() as u16;
        let total_width = natural_width.max(min_width);
        let bar_width = total_width.saturating_sub(min_width);
        let mut spans = progress_bar_spans(
            bar_width,
            (included_fill_pct / 100.0) as f32,
            color,
            theme.bg_highlight,
        );
        spans.push(Span::styled(" ", Style::default().bg(theme.bg_base)));
        spans.push(Span::styled(
            fmt_pct5(included_fill_pct),
            Style::default().fg(color).bg(theme.bg_base),
        ));
        return Some(Line::from(spans));
    }

    let style = Style::default().fg(color).bg(theme.bg_base);
    Some(Line::from(Span::styled(text, style)))
}

/// Status-bar placeholder when SuperGrok limits are in play but billing has
/// not warmed yet. Always visible and clickable (`ShowLimits`); never blank
/// until the first successful fetch.
///
/// Prefixed with `SuperGrok period ·` so cold chrome names the meter in short
/// human words (not secondary team prepaid, not a bare abstraction). SuperGrok
/// is paid; do not paint "free SuperGrok period". ASCII `...` only (no unicode
/// ellipsis). Dim so warm percent still reads as the primary signal once data
/// arrives.
pub fn credit_bar_loading_line(hovered: bool, theme: &Theme) -> Line<'static> {
    let text = included_supergrok_period_limits_compact_meter("...%", None);
    let mut style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
    if hovered {
        style = style.add_modifier(ratatui::style::Modifier::BOLD);
    }
    Line::from(Span::styled(text, style))
}

/// Compact status-bar name for included SuperGrok period limits (used %).
///
/// SuperGrok is a paid product. Compact status and `/limits` **Active:**
/// ([`ActiveSpendDriver::as_human`]) both paint short `SuperGrok period`.
/// Do not paint the long string `included SuperGrok period limits` in TUI chrome.
const INCLUDED_SUPERGROK_PERIOD_LIMITS_COMPACT: &str = "SuperGrok period";

/// Workspace word for compact included SuperGrok period limits chrome.
///
/// Named contract: chrome must not imply a workspace it did not poll.
/// Two or more distinct included pools paint `combined`, never personal or
/// business alone. One known pool paints `personal` or `business` when that
/// role is known. Unknown role stays unlabeled (`None`); do not invent a
/// workspace. SuperGrok Heavy is not this meter.
///
/// Prefer [`compact_included_workspace_qualifier_for_painted`] on the live
/// paint path so a unified-collapsed remaining percent cannot wear the live
/// JWT `personal` / `business` word.
pub fn compact_included_workspace_qualifier(
    distinct_pool_count: usize,
    live_principal_role: Option<&str>,
) -> Option<&'static str> {
    compact_included_workspace_qualifier_for_painted(
        distinct_pool_count,
        live_principal_role,
        None,
        None,
    )
}

/// Compact identity when remaining chrome may have collapsed pools.
///
/// `independent_distinct_pool_count` is the pool count **without** copying
/// the live JWT `is_unified_billing_user` onto sibling rows. If that count
/// is more than one, paint `combined`. If the painted used percent (floored)
/// is not the live JWT's own included used percent (floored), paint
/// `combined` even when collapsed remaining reports one pool. Unknown role
/// stays unlabeled only when the painted percent is that live poll. Do not
/// stamp `personal` or `business` on a percent that workspace did not
/// independently poll.
pub fn compact_included_workspace_qualifier_for_painted(
    independent_distinct_pool_count: usize,
    live_principal_role: Option<&str>,
    live_included_used_pct: Option<f64>,
    painted_included_used_pct: Option<f64>,
) -> Option<&'static str> {
    // Compact paints `{pct:.0}%`. Combined remaining reconstructs used
    // percent from remaining units (33.7 live can paint 34). Compare the
    // compact display unit, not raw floor, so one poll is not labeled
    // combined. 40 vs 5 is still a foreign percent.
    let painted_is_not_live_poll = match (live_included_used_pct, painted_included_used_pct) {
        (Some(live), Some(painted)) => format!("{live:.0}") != format!("{painted:.0}"),
        _ => false,
    };
    if independent_distinct_pool_count > 1 || painted_is_not_live_poll {
        return Some("combined");
    }
    match live_principal_role.map(str::trim) {
        Some("personal") => Some("personal"),
        Some("business") => Some("business"),
        _ => None,
    }
}

/// Compact status label for included SuperGrok period limits used percent.
///
/// Plain American English meter name, never bare "intent", never "free
/// SuperGrok period". `pct_display` is already formatted (e.g. `"24%"` or
/// `"...%"`). `workspace` is `personal` / `business` / `combined` when the
/// chrome can name the pool honestly.
fn included_supergrok_period_limits_compact_meter(
    pct_display: &str,
    workspace: Option<&str>,
) -> String {
    match workspace.map(str::trim).filter(|s| !s.is_empty()) {
        Some(w) => format!("{INCLUDED_SUPERGROK_PERIOD_LIMITS_COMPACT} · {w} · {pct_display}"),
        None => format!("{INCLUDED_SUPERGROK_PERIOD_LIMITS_COMPACT} · {pct_display}"),
    }
}

/// True when the **active** SuperGrok principal's last billing poll was
/// auth-class failed (process-local). Used so compact status never paints
/// free SuperGrok period success from sibling-only fill.
pub fn active_supergrok_poll_auth_failed_from_process() -> bool {
    let home = xai_grok_shell::util::grok_home::grok_home();
    let Some(id) = xai_grok_shell::auth::active_supergrok_identity_id(&home) else {
        return false;
    };
    xai_grok_shell::auth::supergrok_identity_last_poll_auth_failed(&id)
}

/// Live SuperGrok principal role for compact chrome (`personal` / `business`).
///
/// Reads the stored session listing for [`active_supergrok_identity_id`]. Does
/// not invent a workspace when the listing is missing. SuperGrok Heavy is not
/// this field.
pub fn compact_live_principal_role_from_process() -> Option<&'static str> {
    let home = xai_grok_shell::util::grok_home::grok_home();
    let id = xai_grok_shell::auth::active_supergrok_identity_id(&home)?;
    let map = xai_grok_shell::auth::read_auth_json(&home.join("auth.json")).ok()?;
    let listings = xai_grok_shell::auth::list_supergrok_principal_listings(&map);
    listings
        .iter()
        .find(|l| l.identity_id == id)
        .and_then(|l| match l.role_label {
            "personal" => Some("personal"),
            "business" => Some("business"),
            _ => None,
        })
}

/// Compact status meter text for the live sampling identity.
///
/// Design A (active meter only = spend-order chrome, not settlement proof):
/// - **Included SuperGrok period limits known with room** (`included < 100%`) →
///   `SuperGrok period · N%` (workspace word when known).
///   Console sampling identity and Management prepaid `$N` do not lead the
///   compact chip while that included pool is still the primary meter.
/// - **Console live** (included full or unknown) → console team prepaid `$N`
///   or honest gap. Never bare SuperGrok included-period `...%` / `N%` as if
///   included SuperGrok period limits drive the turn when they do not.
/// - **SuperGrok live + included period full** (`≥ 100%`) + positive SuperGrok
///   dollar credits → SuperGrok dollar credits `$` (not bare `100%` as if included
///   period still drives after-burner spend).
/// - **SuperGrok live + included period full + no SuperGrok dollar credits** →
///   `SuperGrok period · 100%` (included pool is empty; no
///   second meter).
/// - **SuperGrok live + cold included** → honest
///   `SuperGrok period · ...%`.
/// - **SuperGrok live + active poll auth-failed** → honest
///   `SuperGrok period · ...%` (never sibling-only success).
///
/// Team prepaid / Grok Build class never paint on this compact meter while free
/// SuperGrok period has room (team wallets stay on `/limits`; after free SuperGrok
/// period is full they may appear as footer **not the active spend path** chips).
///
/// `super_grok_dollar_credits_cents` is session billing prepaid (SuperGrok dollar credits),
/// never console team Management prepaid.
pub fn compact_meter_text_for_live_identity(
    live: SamplingIdentityKind,
    included_usage_known: bool,
    included_usage_pct: f64,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    super_grok_dollar_credits_cents: Option<i64>,
) -> String {
    compact_meter_text_for_live_identity_with_workspace(
        live,
        included_usage_known,
        included_usage_pct,
        console_prepaid_cents,
        console_gap,
        super_grok_dollar_credits_cents,
        None,
    )
}

/// Compact status meter with an honest workspace word (`personal` /
/// `business` / `combined`) when the chrome can name the pool it polled.
pub fn compact_meter_text_for_live_identity_with_workspace(
    live: SamplingIdentityKind,
    included_usage_known: bool,
    included_usage_pct: f64,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    super_grok_dollar_credits_cents: Option<i64>,
    workspace: Option<&str>,
) -> String {
    compact_meter_text_for_live_identity_with_active_poll(
        live,
        included_usage_known,
        included_usage_pct,
        console_prepaid_cents,
        console_gap,
        super_grok_dollar_credits_cents,
        false,
        workspace,
    )
}

/// Compact status meter with dual SuperGrok active-poll honesty.
///
/// When live sampling is SuperGrok and the **active** principal's last billing
/// poll was auth-failed, never paint a free SuperGrok period % as healthy
/// (sibling shared-pool fill must not look like active success). Honest cold
/// `...%` so the operator re-logins the live role.
pub fn compact_meter_text_for_live_identity_with_active_poll(
    live: SamplingIdentityKind,
    included_usage_known: bool,
    included_usage_pct: f64,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    super_grok_dollar_credits_cents: Option<i64>,
    active_supergrok_poll_auth_failed: bool,
    workspace: Option<&str>,
) -> String {
    if active_supergrok_poll_auth_failed && !live.is_console() {
        // Active JWT auth-failed: do not paint included-period success from
        // sibling fill or stale cache as if this login polled OK. Still name
        // the meter.
        included_supergrok_period_limits_compact_meter("...%", workspace)
    } else if included_usage_known && included_usage_pct < 100.0 {
        // Included SuperGrok period limits still have room: this is the
        // spend-order driver. Do not lead with Management prepaid `console · $N`.
        included_supergrok_period_limits_compact_meter(
            &format!("{included_usage_pct:.0}%"),
            workspace,
        )
    } else if live.is_console() {
        console_compact_meter_text(console_prepaid_cents, console_gap)
    } else if !included_usage_known {
        included_supergrok_period_limits_compact_meter("...%", workspace)
    } else if included_usage_pct >= 100.0 {
        // Included SuperGrok period limits full: after-burner spend is SuperGrok
        // $ credits when any remain. Do not paint bare included % as the live
        // driver.
        match super_grok_dollar_credits_cents
            .map(i64::abs)
            .filter(|c| *c > 0)
        {
            Some(cents) => format!("SuperGrok dollar credits · {}", fmt_dollars(cents)),
            None => included_supergrok_period_limits_compact_meter(
                &format!("{included_usage_pct:.0}%"),
                workspace,
            ),
        }
    } else {
        included_supergrok_period_limits_compact_meter("...%", workspace)
    }
}

/// Compact status / `/limits` chrome for a named [`MeterSource`] pin.
///
/// `None` is Design A spend-order chrome. A pin names the meter that drives
/// the compact line so the operator can tell included SuperGrok period limits
/// from SuperGrok dollar credits from console. Combined is only when remaining
/// is across distinct SuperGrok identities (`workspace == Some("combined")`).
/// grok-oss limits JSON is a client printout, not xAI billing truth.
pub fn compact_meter_text_for_meter_source(
    source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
    live: SamplingIdentityKind,
    included_usage_known: bool,
    included_usage_pct: f64,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    super_grok_dollar_credits_cents: Option<i64>,
    workspace: Option<&str>,
) -> String {
    use xai_grok_shell::auth::limits_pins::MeterSource;

    match source {
        None => compact_meter_text_for_live_identity_with_active_poll(
            live,
            included_usage_known,
            included_usage_pct,
            console_prepaid_cents,
            console_gap,
            super_grok_dollar_credits_cents,
            false,
            workspace,
        ),
        Some(MeterSource::Included) => {
            included_compact_pct(included_usage_known, included_usage_pct, workspace)
        }
        Some(MeterSource::DollarCredits) => match super_grok_dollar_credits_cents.map(i64::abs) {
            Some(cents) => format!("SuperGrok dollar credits · {}", fmt_dollars(cents)),
            None => "SuperGrok dollar credits · ...".to_string(),
        },
        Some(MeterSource::Console) => match console_prepaid_cents {
            Some(cents) => format!(
                "{} · {}",
                MeterSource::Console.as_human(),
                fmt_dollars(cents.abs())
            ),
            None => format!(
                "{} · {}",
                MeterSource::Console.as_human(),
                console_gap.as_display_str()
            ),
        },
        Some(MeterSource::Combined) => {
            if workspace == Some("combined") {
                included_compact_pct(included_usage_known, included_usage_pct, Some("combined"))
            } else {
                compact_meter_text_for_live_identity_with_active_poll(
                    live,
                    included_usage_known,
                    included_usage_pct,
                    console_prepaid_cents,
                    console_gap,
                    super_grok_dollar_credits_cents,
                    false,
                    workspace,
                )
            }
        }
    }
}

fn included_compact_pct(
    included_usage_known: bool,
    included_usage_pct: f64,
    workspace: Option<&str>,
) -> String {
    if included_usage_known {
        included_supergrok_period_limits_compact_meter(
            &format!("{included_usage_pct:.0}%"),
            workspace,
        )
    } else {
        included_supergrok_period_limits_compact_meter("...%", workspace)
    }
}

fn console_compact_meter_text(
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
) -> String {
    match console_prepaid_cents {
        Some(cents) => {
            let dollars = cents.abs() as f64 / 100.0;
            if dollars.fract() == 0.0 {
                format!("console · ${dollars:.0}")
            } else {
                format!("console · ${dollars:.2}")
            }
        }
        None => format!("console · {}", console_gap.as_display_str()),
    }
}

/// Status-bar compact meter for the live identity.
///
/// Always paints for Build sessions (`None` only for gateway chat). Cold
/// SuperGrok uses the loading placeholder so the chip stays clickable before
/// the first billing fetch. Uses
/// [`credit_bar_line_for_session`] / [`compact_meter_text_for_live_identity`]
/// / [`credit_bar_loading_line`]. Not a new meter.
pub fn credit_status_line_for_live_session(
    balance: Option<&CreditBalance>,
    live: SamplingIdentityKind,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
) -> Option<Line<'static>> {
    credit_status_line_for_live_session_with_workspace(
        balance,
        live,
        console_prepaid_cents,
        console_gap,
        hovered,
        theme,
        gateway_chat,
        None,
    )
}

/// Like [`credit_status_line_for_live_session`] with a workspace word when
/// SuperGrok included chrome can name the pool honestly.
pub fn credit_status_line_for_live_session_with_workspace(
    balance: Option<&CreditBalance>,
    live: SamplingIdentityKind,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
    live_principal_role: Option<&str>,
) -> Option<Line<'static>> {
    credit_status_line_for_live_session_emphasizing_meter_source(
        balance,
        live,
        console_prepaid_cents,
        console_gap,
        hovered,
        theme,
        gateway_chat,
        live_principal_role,
        None,
    )
}

/// Status compact chip that can emphasize a `/limits meter` pin.
pub fn credit_status_line_for_live_session_emphasizing_meter_source(
    balance: Option<&CreditBalance>,
    live: SamplingIdentityKind,
    console_prepaid_cents: Option<i64>,
    console_gap: ConsoleTeamPrepaidGap,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
    live_principal_role: Option<&str>,
    meter_source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
) -> Option<Line<'static>> {
    if gateway_chat {
        return None;
    }
    let included_has_room = balance.is_some_and(|b| b.included_usage_known && b.usage_pct < 100.0);
    if live.is_console() && meter_source.is_none() && !included_has_room {
        let text = compact_meter_text_for_live_identity(
            live,
            balance.is_some_and(|b| b.included_usage_known),
            balance.map(|b| b.usage_pct).unwrap_or(0.0),
            console_prepaid_cents,
            console_gap,
            balance.and_then(|b| b.prepaid_balance_cents),
        );
        let color = match console_prepaid_cents {
            Some(cents) if cents.abs() <= LOW_BALANCE_CENTS => theme.warning,
            Some(_) => theme.accent_success,
            None => theme.gray_dim,
        };
        let mut style = Style::default().fg(color).bg(theme.bg_base);
        if hovered {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        return Some(Line::from(Span::styled(text, style)));
    }
    match balance {
        Some(bal) => credit_bar_line_for_session_emphasizing_meter_source(
            bal,
            hovered,
            theme,
            false,
            live_principal_role,
            console_prepaid_cents,
            console_gap,
            meter_source,
        ),
        None => {
            let text = compact_meter_text_for_meter_source(
                meter_source,
                live,
                false,
                0.0,
                console_prepaid_cents,
                console_gap,
                None,
                None,
            );
            let mut style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
            if hovered {
                style = style.add_modifier(ratatui::style::Modifier::BOLD);
            }
            Some(Line::from(Span::styled(text, style)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn or_bal(cents: i64) -> OpenRouterCreditBalance {
        OpenRouterCreditBalance {
            balance_cents: cents,
        }
    }

    fn topup(enabled: bool, amount: Option<i64>, max: Option<i64>) -> AutoTopupInfo {
        AutoTopupInfo {
            enabled,
            topup_amount_cents: amount,
            max_amount_cents: max,
        }
    }

    #[test]
    fn summary_no_credits_omits_credits_block() {
        let b = CreditBalance {
            period_end_display: Some("June 14, 16:00".into()),
            prepaid_balance_cents: Some(0),
            ..bal(25.0)
        };
        // Even with an auto-topup rule present, zero prepaid omits the credits block
        let out = format_usage_summary(&b, Some(&topup(true, Some(2000), Some(10000))));
        assert_eq!(out, "Usage: 25%\nNext reset: June 14, 16:00");
    }

    #[test]
    fn summary_credits_without_autotopup_shows_disabled() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(10000),
            ..bal(25.0)
        };
        assert_eq!(
            format_usage_summary(&b, None),
            "Usage: 25%\n\nSuperGrok dollar credits: $100\nAuto topup: disabled"
        );
        // A disabled rule renders the same.
        assert_eq!(
            format_usage_summary(&b, Some(&topup(false, Some(2000), Some(10000)))),
            "Usage: 25%\n\nSuperGrok dollar credits: $100\nAuto topup: disabled"
        );
    }

    #[test]
    fn summary_autotopup_enabled_without_max_omits_max() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(10000),
            ..bal(25.0)
        };
        assert_eq!(
            format_usage_summary(&b, Some(&topup(true, Some(2000), None))),
            "Usage: 25%\n\nSuperGrok dollar credits: $100\nAuto topup: $20"
        );
    }

    #[test]
    fn summary_autotopup_enabled_with_max_renders_all() {
        let b = CreditBalance {
            period_end_display: Some("June 14, 16:00".into()),
            prepaid_balance_cents: Some(10000),
            ..bal(25.0)
        };
        assert_eq!(
            format_usage_summary(&b, Some(&topup(true, Some(2000), Some(10000)))),
            "Usage: 25%\nNext reset: June 14, 16:00\n\nSuperGrok dollar credits: $100\nAuto topup: $20\nMax monthly topup: $100"
        );
    }

    #[test]
    fn summary_formats_fractional_dollars() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(1250),
            ..bal(25.0)
        };
        assert_eq!(
            format_usage_summary(&b, Some(&topup(true, Some(550), None))),
            "Usage: 25%\n\nSuperGrok dollar credits: $12.50\nAuto topup: $5.50"
        );
    }

    #[test]
    fn summary_abs_negative_billing_amounts() {
        // Billing returns credit / top-up amounts as negative cents; the summary must render them as positive USD (matching the web)
        let b = CreditBalance {
            prepaid_balance_cents: Some(-500),
            ..bal(100.0)
        };
        assert_eq!(
            format_usage_summary(&b, Some(&topup(true, Some(-500), Some(-1000)))),
            "Usage: 100%\n\nSuperGrok dollar credits: $5\nAuto topup: $5\nMax monthly topup: $10"
        );
    }

    #[test]
    fn summary_pay_as_you_go_enabled_renders_used_of_limit() {
        let b = CreditBalance {
            pay_as_you_go: true,
            on_demand_used_cents: Some(355),
            on_demand_cap_cents: Some(5000),
            period_type: Some("USAGE_PERIOD_TYPE_MONTHLY".into()),
            period_end_display: Some("June 30, 16:00".into()),
            ..bal(91.0)
        };
        assert_eq!(
            format_usage_summary(&b, None),
            "Monthly limit: 91%\nNext reset: June 30, 16:00\n\nPay-as-you-go: $3.55 used of $50.00 limit"
        );
    }

    #[test]
    fn summary_pay_as_you_go_disabled_omits_line() {
        let b = CreditBalance {
            pay_as_you_go: false,
            period_type: Some("USAGE_PERIOD_TYPE_MONTHLY".into()),
            period_end_display: Some("June 30, 16:00".into()),
            ..bal(91.0)
        };
        assert_eq!(
            format_usage_summary(&b, None),
            "Monthly limit: 91%\nNext reset: June 30, 16:00"
        );
    }

    fn bal_period(pct: f64, period_type: &str) -> CreditBalance {
        CreditBalance {
            period_type: Some(period_type.to_string()),
            ..bal(pct)
        }
    }

    #[test]
    fn usage_label_from_period_type() {
        assert_eq!(
            bal_period(0.0, "USAGE_PERIOD_TYPE_WEEKLY").usage_label(),
            "Weekly limit"
        );
        assert_eq!(
            bal_period(0.0, "USAGE_PERIOD_TYPE_MONTHLY").usage_label(),
            "Monthly limit"
        );
        // Unknown / unspecified / absent falls back to "Usage"
        assert_eq!(
            bal_period(0.0, "USAGE_PERIOD_TYPE_UNSPECIFIED").usage_label(),
            "Usage"
        );
        assert_eq!(bal(0.0).usage_label(), "Usage");
    }

    #[test]
    fn summary_uses_period_label() {
        let weekly = bal_period(25.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(format_usage_summary(&weekly, None), "Weekly limit: 25%");
        let monthly = bal_period(25.0, "USAGE_PERIOD_TYPE_MONTHLY");
        assert_eq!(format_usage_summary(&monthly, None), "Monthly limit: 25%");
    }

    #[test]
    fn warning_uses_period_label() {
        let weekly = bal_period(92.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(
            usage_warning(&weekly, None, true),
            Some(("Weekly limit left: 8%".to_string(), false))
        );
    }

    #[test]
    fn summary_floors_usage_percent() {
        // Match the backend SpendingLimiter (`as u8` truncation): 99.994% must render as 99%, not round up to 100%
        let almost = bal_period(99.994, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(format_usage_summary(&almost, None), "Weekly limit: 99%");
        // A true 100% still shows 100%.
        let full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(format_usage_summary(&full, None), "Weekly limit: 100%");
    }

    #[test]
    fn warning_percent_left_is_floor_complement() {
        // 99.994% used floors to 99%, so it shows "1% left" (not "0% left"), and the warning and the floored summary always sum to 100
        let almost = bal_period(99.994, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(
            usage_warning(&almost, None, true),
            Some(("Weekly limit left: 1%".to_string(), true))
        );
        // A true 100% (no credits) shows "0% left"
        let full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(
            usage_warning(&full, None, true),
            Some(("Weekly limit left: 0%".to_string(), true))
        );
    }

    #[test]
    fn warning_usage_model_thresholds() {
        assert_eq!(usage_warning(&bal(50.0), None, true), None);
        assert_eq!(
            usage_warning(&bal(92.0), None, true),
            Some(("Usage left: 8%".to_string(), false))
        );
        assert_eq!(
            usage_warning(&bal(97.0), None, true),
            Some(("Usage left: 3%".to_string(), true))
        );
    }

    #[test]
    fn warning_hidden_for_team_users() {
        assert_eq!(usage_warning(&bal(99.0), None, false), None);
        let credits = CreditBalance {
            prepaid_balance_cents: Some(100),
            ..bal(0.0)
        };
        assert_eq!(usage_warning(&credits, None, false), None);
    }

    #[test]
    fn warning_credits_unknown_topup_is_suppressed() {
        // At 100% usage with prepaid credits but the rule not yet known (None), never warn; it resolves on the next billing fetch
        let b = CreditBalance {
            prepaid_balance_cents: Some(100),
            ..bal(100.0)
        };
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_credits_suppressed_below_full_usage() {
        // Low credits and no auto top-up, but the included allowance still has room (usage < 100%), so no warning; credits aren't being spent yet
        let disabled = topup(false, None, None);
        let low = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(0.0)
        };
        assert_eq!(usage_warning(&low, Some(&disabled), true), None);
        // The same balance warns once the allowance is exhausted
        let exhausted = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&exhausted, Some(&disabled), true),
            Some(("SuperGrok dollar credits left: $4.53".to_string(), true))
        );
    }

    #[test]
    fn warning_credits_no_topup_low_shows_dollars() {
        // "No auto top-up" is a known, disabled rule (not an unresolved None).
        let b = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(100.0)
        };
        let disabled = topup(false, None, None);
        assert_eq!(
            usage_warning(&b, Some(&disabled), true),
            Some(("SuperGrok dollar credits left: $4.53".to_string(), true))
        );
    }

    #[test]
    fn warning_credits_no_topup_above_threshold_silent() {
        let disabled = topup(false, None, None);
        let b = CreditBalance {
            prepaid_balance_cents: Some(1500),
            ..bal(100.0)
        };
        assert_eq!(usage_warning(&b, Some(&disabled), true), None);
        // Exactly $10 is still "low".
        let at_ten = CreditBalance {
            prepaid_balance_cents: Some(1000),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&at_ten, Some(&disabled), true),
            Some(("SuperGrok dollar credits left: $10".to_string(), true))
        );
    }

    #[test]
    fn warning_credits_topup_no_max_never_warns() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(1),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&b, Some(&topup(true, Some(2000), None)), true),
            None
        );
    }

    #[test]
    fn warning_credits_topup_with_max_below_topup_amount() {
        // $15 balance, $20 top-up amount, $100 max: below one top-up, so it warns
        let b = CreditBalance {
            prepaid_balance_cents: Some(1500),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&b, Some(&topup(true, Some(2000), Some(10000))), true),
            Some(("SuperGrok dollar credits left: $15".to_string(), true))
        );
        let plenty = CreditBalance {
            prepaid_balance_cents: Some(2500),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&plenty, Some(&topup(true, Some(2000), Some(10000))), true),
            None
        );
    }

    #[test]
    fn warning_credits_handles_negative_cents() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(-453),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&b, Some(&topup(true, Some(-2000), Some(-10000))), true),
            Some(("SuperGrok dollar credits left: $4.53".to_string(), true))
        );
    }

    #[test]
    fn warning_credits_take_precedence_over_usage() {
        // A credits user below 100% usage gets no warning: no usage-% warning, and credits aren't being spent yet
        // A non-credits user would see "Usage left: 1%" at 99%
        let b = CreditBalance {
            prepaid_balance_cents: Some(5000),
            ..bal(99.0)
        };
        assert_eq!(
            usage_warning(&b, Some(&topup(false, None, None)), true),
            None
        );
        // Zero prepaid falls back to the usage model.
        let zero = CreditBalance {
            prepaid_balance_cents: Some(0),
            ..bal(99.0)
        };
        assert_eq!(
            usage_warning(&zero, None, true),
            Some(("Usage left: 1%".to_string(), true))
        );
    }

    fn pay_as_you_go(usage_pct: f64, cap_cents: i64, used_cents: i64) -> CreditBalance {
        CreditBalance {
            pay_as_you_go: true,
            on_demand_cap_cents: Some(cap_cents),
            on_demand_used_cents: Some(used_cents),
            period_type: Some("USAGE_PERIOD_TYPE_MONTHLY".into()),
            ..bal(usage_pct)
        }
    }

    #[test]
    fn warning_pay_as_you_go_low_dollars_shows_remaining() {
        // $50 cap, $42 used leaves $8, shown grey (above $5)
        let grey = pay_as_you_go(100.0, 5000, 4200);
        assert_eq!(
            usage_warning(&grey, None, true),
            Some(("Pay-as-you-go limit left: $8".to_string(), false))
        );
        // $50 cap, $46 used leaves $4, critical (yellow)
        let yellow = pay_as_you_go(100.0, 5000, 4600);
        assert_eq!(
            usage_warning(&yellow, None, true),
            Some(("Pay-as-you-go limit left: $4".to_string(), true))
        );
    }

    #[test]
    fn warning_pay_as_you_go_boundaries() {
        // Exactly $10 left shows the warning, grey
        let at_ten = pay_as_you_go(100.0, 5000, 4000);
        assert_eq!(
            usage_warning(&at_ten, None, true),
            Some(("Pay-as-you-go limit left: $10".to_string(), false))
        );
        // Exactly $5 left is critical (yellow)
        let at_five = pay_as_you_go(100.0, 5000, 4500);
        assert_eq!(
            usage_warning(&at_five, None, true),
            Some(("Pay-as-you-go limit left: $5".to_string(), true))
        );
    }

    #[test]
    fn warning_pay_as_you_go_above_threshold_silent() {
        // $20 left (above the $10 threshold) gets no warning
        let b = pay_as_you_go(100.0, 5000, 3000);
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_pay_as_you_go_suppressed_below_full_usage() {
        // Pay-as-you-go users get no percentage warning before the included allowance is exhausted, even with low on-demand room remaining
        let b = pay_as_you_go(95.0, 5000, 4800);
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_pay_as_you_go_fractional_dollars() {
        // $50 cap, $46.50 used leaves $3.50: critical, fractional formatting
        let b = pay_as_you_go(100.0, 5000, 4650);
        assert_eq!(
            usage_warning(&b, None, true),
            Some(("Pay-as-you-go limit left: $3.50".to_string(), true))
        );
    }

    #[test]
    fn test_credit_bar_line_shows_percentage() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(24.0), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Compact status: free SuperGrok period used % (no "Credits used").
        assert_eq!(text, "free SuperGrok period · 24%");
        assert!(!text.contains("Credits"));
        assert!(
            !text.contains("intent ·") && !text.split_whitespace().any(|w| w == "intent"),
            "must not use bare intent as paying-path label: {text}"
        );
    }

    /// Named contract (2026-08-09): status compact meter names the real meter
    /// (free SuperGrok period), never the bare abstraction word "intent".
    #[test]
    fn compact_status_names_free_supergrok_period_not_bare_intent() {
        let warm = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            24.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
        );
        assert_eq!(warm, "free SuperGrok period · 24%");
        assert!(
            !warm.contains("intent ·") && !warm.split_whitespace().any(|w| w == "intent"),
            "paying-path label must not be bare intent: {warm}"
        );

        let cold = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            false,
            0.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
        );
        assert_eq!(cold, "free SuperGrok period · ...%");
        assert!(
            !cold.contains("intent ·") && !cold.split_whitespace().any(|w| w == "intent"),
            "cold chrome must not use bare intent: {cold}"
        );

        assert_eq!(
            ActiveSpendDriver::SuperGrokFreePeriod.as_human(),
            "free SuperGrok period",
            "compact prefix must stay aligned with ActiveSpendDriver human label"
        );
    }

    #[test]
    fn test_color_thresholds() {
        let theme = Theme::default();

        let low = credit_bar_line(&bal(50.0), false, &theme);
        assert_eq!(
            low.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_success)
        );

        let high = credit_bar_line(&bal(85.0), false, &theme);
        assert_eq!(
            high.spans.first().and_then(|s| s.style.fg),
            Some(theme.warning)
        );

        let over = credit_bar_line(&bal(100.0), false, &theme);
        assert_eq!(
            over.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_error)
        );
    }

    #[test]
    fn test_zero_percent() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(0.0), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "Credits used: 0%");
        assert_eq!(
            line.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_success)
        );
    }

    /// Named contract: unknown included meter must not paint a silent `0%`.
    #[test]
    fn unknown_included_usage_paints_loading_placeholder_not_zero() {
        let theme = Theme::default();
        // Exactly 80% renders yellow (warning)
        let at_80 = credit_bar_line(&bal(80.0), false, &theme);
        assert_eq!(
            at_80.spans.first().and_then(|s| s.style.fg),
            Some(theme.warning)
        );

        // Just below 80% renders green (success)
        let below_80 = credit_bar_line(&bal(79.9), false, &theme);
        assert_eq!(
            below_80.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_success)
        );
    }

    /// Named contract: true zero (known reading of 0%) stays free-period-labeled `0%`.
    #[test]
    fn true_zero_included_usage_paints_zero_percent() {
        let theme = Theme::default();
        // Exactly 100% renders red (error)
        let at_100 = credit_bar_line(&bal(100.0), false, &theme);
        assert_eq!(
            at_100.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_error)
        );

        // Just below 100% renders yellow (warning)
        let below_100 = credit_bar_line(&bal(99.9), false, &theme);
        assert_eq!(
            below_100.spans.first().and_then(|s| s.style.fg),
            Some(theme.warning)
        );
    }

    /// Named contract: SuperGrok live + free SuperGrok period still has room +
    /// Management prepaid known → prompt footer stays quiet (no long secondary
    /// team prepaid line). Team prepaid lives on `/limits`. Compact free SuperGrok
    /// period chrome is a separate path.
    #[test]
    fn test_over_100_percent() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(150.0), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "Credits used: 150%");
        assert_eq!(
            line.spans.first().and_then(|s| s.style.fg),
            Some(theme.accent_error)
        );
    }

    /// Named contract: free SuperGrok period full + Management prepaid known →
    /// secondary team prepaid under not-active-spend (not Team settlement jargon).
    #[test]
    fn footer_supergrok_live_after_free_period_full_shows_secondary_team_prepaid() {
        // Included SuperGrok period limits full with SuperGrok dollar credits
        // present and unknown autotopup → SuperGrok dollar credits warning is
        // silent; secondary team can show alone.
        let mut b = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        b.prepaid_balance_cents = Some(5_00); // SuperGrok dollar credits (not team)
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&b),
            None, // unknown autotopup → no SuperGrok dollar credits warning
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            Some(12_500),
            ConsoleTeamPrepaidGap::Loading,
        );
        let (text, critical) = w.expect("after free period full, secondary team prepaid may show");
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("not the active spend path") && lower.contains("team prepaid remaining"),
            "must label team prepaid as secondary (not active spend): {text}"
        );
        assert!(
            !lower.contains("team settlement"),
            "must not use Team settlement jargon (read as active pay): {text}"
        );
        assert!(text.contains("$125"), "management prepaid dollars: {text}");
        assert!(
            !lower.starts_with("console key"),
            "must not claim console live while SuperGrok session is live: {text}"
        );
        assert!(!critical, "$125 is above low-balance threshold");
        assert!(
            !text.contains("SuperGrok dollar credits left: $125"),
            "must not mash team prepaid into SuperGrok extras: {text}"
        );
    }

    /// Named contract (P1): SuperGrok live after free SuperGrok period is full +
    /// known postpaid OAuth / Grok Build class cents → footer surfaces secondary
    /// team Grok Build class $, distinct from team prepaid. Does not replace
    /// Design A compact free-period `%` while free SuperGrok period has room.
    #[test]
    fn footer_supergrok_live_surfaces_team_grok_build_class_when_known() {
        // Mid free-period with room: team $ must not dominate the footer.
        let mid = bal_period(65.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let quiet = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&mid),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(82_371),
        );
        assert!(
            quiet.is_none(),
            "free SuperGrok period with room must not paint team $ footer: {quiet:?}"
        );
        let compact = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            65.0,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert!(
            compact.starts_with("SuperGrok period ·")
                && compact.contains('%')
                && !compact.to_ascii_lowercase().contains("grok build"),
            "Design A compact stays free SuperGrok period %, not team $: {compact}"
        );

        // Free SuperGrok period full + SuperGrok $ credits (unknown autotopup quiet):
        // secondary team meters may attach without a SuperGrok % critical warning.
        let mut full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        full.prepaid_balance_cents = Some(5_00);
        let w = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&full),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            Some(34_000), // team prepaid remaining
            ConsoleTeamPrepaidGap::Loading,
            Some(82_371), // ~$823.71 Grok Build class period spend
        );
        let (text, _) = w.expect("must surface secondary team meters after free period full");
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("not the active spend path")
                && lower.contains("team prepaid remaining")
                && text.contains("$340"),
            "team prepaid remaining as secondary meter: {text}"
        );
        assert!(
            lower.contains("grok build class") && text.contains("$823.71"),
            "must surface team Grok Build class period $: {text}"
        );
        assert!(
            !text.contains("prepaid $823.71") && !text.contains("team prepaid: $823.71"),
            "must not label Grok Build class as prepaid: {text}"
        );
        assert!(
            !text.contains("SuperGrok dollar credits left: $823.71"),
            "must not mash class into SuperGrok extras: {text}"
        );
        // Pure chip helper contract (console-live form).
        let chip = team_grok_build_class_footer_chip(Some(82_371)).expect("chip");
        assert!(chip.contains("team Grok Build class"));
        assert!(chip.contains("$823.71"));
        assert!(team_grok_build_class_footer_chip(Some(0)).is_none());
        assert!(team_grok_build_class_footer_chip(None).is_none());
    }

    /// Named contract (P1): SuperGrok live after free SuperGrok period is full +
    /// only Grok Build class known (no prepaid) → standalone secondary class chip.
    #[test]
    fn footer_supergrok_live_standalone_grok_build_class_without_prepaid() {
        let mut b = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        b.prepaid_balance_cents = Some(5_00); // quiet SuperGrok dollar credits path (unknown autotopup)
        let w = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(20_176),
        );
        let (text, critical) = w.expect("standalone Grok Build class chip after free period full");
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("not the active spend path") && lower.contains("grok build class"),
            "must name Grok Build class as secondary (not active spend): {text}"
        );
        assert!(text.contains("$201.76"), "class dollars: {text}");
        assert!(
            !lower.contains("prepaid"),
            "must not invent prepaid when unknown: {text}"
        );
        assert!(!critical);
    }

    /// Named contract: SuperGrok live + high included % + no management key →
    /// footer keeps SuperGrok % only (team `no management key` honesty is the
    /// `/limits` Console API Balance line, not silent omit of that block).
    #[test]
    fn footer_supergrok_live_high_usage_without_mgmt_key_keeps_supergrok_only() {
        let b = bal_period(96.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
        );
        let (text, _) = w.expect("SuperGrok high usage warning");
        assert!(
            text.contains('%') || text.to_ascii_lowercase().contains("left"),
            "SuperGrok included remaining must still show: {text}"
        );
        // Missing management key is not appended to SuperGrok-only footer noise.
        assert!(
            !text.to_ascii_lowercase().contains("no management key"),
            "team missing-key gap belongs on /limits Balance, not SuperGrok footer: {text}"
        );
        assert!(!text.contains('$'), "must not invent team dollars: {text}");
    }

    /// Named contract: SuperGrok live + free SuperGrok period still has room +
    /// Management loading → do not paint loading team prepaid on the footer
    /// (team honesty stays on `/limits` while free SuperGrok period drives).
    #[test]
    fn footer_supergrok_live_mgmt_loading_quiet_while_free_period_has_room() {
        let b = bal_period(40.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::Loading,
        );
        assert!(
            w.is_none(),
            "free SuperGrok period with room must not paint loading team prepaid: {w:?}"
        );
    }

    /// Named contract: free SuperGrok period full + Management loading → honest
    /// secondary loading gap under not-active-spend (Management path active).
    #[test]
    fn footer_supergrok_live_mgmt_loading_after_free_period_full() {
        let mut b = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        b.prepaid_balance_cents = Some(5_00); // quiet SuperGrok dollar credits path
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::Loading,
        );
        let (text, critical) = w.expect("loading team prepaid after free period full");
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("not the active spend path") && lower.contains("loading team prepaid"),
            "honest loading gap under not-active-spend label: {text}"
        );
        assert!(
            !text.contains('$'),
            "must not invent dollars while loading: {text}"
        );
        assert!(!critical);
    }

    #[test]
    fn usage_summary_unknown_included_says_not_yet_available() {
        let mut b = bal(0.0);
        b.included_usage_known = false;
        let out = format_usage_summary(&b, None);
        assert!(
            out.contains("not yet available"),
            "unknown included must not print 0%, got {out}"
        );
        assert!(!out.contains("0%"), "got {out}");
    }

    #[test]
    fn test_boundary_at_80_percent() {
        let theme = Theme::default();
        // Exactly 80% should be warning (yellow).
        let at_80 = credit_bar_line(&bal(80.0), false, &theme);
        assert_eq!(at_80.spans[0].style.fg, Some(theme.warning));

        // Just below 80% should be success (green).
        let below_80 = credit_bar_line(&bal(79.9), false, &theme);
        assert_eq!(below_80.spans[0].style.fg, Some(theme.accent_success));
    }

    #[test]
    fn test_boundary_at_100_percent() {
        let theme = Theme::default();
        // Exactly 100% should be error (red).
        let at_100 = credit_bar_line(&bal(100.0), false, &theme);
        assert_eq!(at_100.spans[0].style.fg, Some(theme.accent_error));

        // Just below 100% should be warning (yellow).
        let below_100 = credit_bar_line(&bal(99.9), false, &theme);
        assert_eq!(below_100.spans[0].style.fg, Some(theme.warning));
    }

    #[test]
    fn test_over_100_percent() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(150.0), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "SuperGrok period · 150%");
        assert_eq!(line.spans[0].style.fg, Some(theme.accent_error));
    }

    #[test]
    fn test_fractional_percentage_rounds_display() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(33.7), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "SuperGrok period · 34%");
    }

    #[test]
    fn test_credit_balance_with_on_demand_fields() {
        let balance = CreditBalance {
            effective_usage_pct: 25.0,
            period_end_display: Some("Jun 1, 00:00".into()),
            pay_as_you_go: true,
            on_demand_cap_cents: Some(2000),
            on_demand_used_cents: Some(500),
            ..bal(50.0)
        };
        let theme = Theme::default();
        // The credit bar uses usage_pct (not effective_usage_pct).
        let line = credit_bar_line(&balance, false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "SuperGrok period · 50%");
    }

    #[test]
    fn gateway_chat_suppresses_credit_bar_and_usage_warning() {
        let theme = Theme::default();
        let b = bal(90.0);
        assert!(credit_bar_line_for_session(&b, false, &theme, true).is_none());
        assert!(usage_warning_for_session(&b, None, true, true).is_none());
        // Build path still renders.
        assert!(credit_bar_line_for_session(&b, false, &theme, false).is_some());
    }

    #[test]
    fn credit_bar_loading_line_is_honest_placeholder() {
        let theme = Theme::default();
        let line = credit_bar_loading_line(false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "SuperGrok period · ...%");
        assert!(!text.contains("Credits"));
        assert_eq!(line.spans[0].style.fg, Some(theme.gray_dim));
        // No unicode ellipsis.
        assert!(!text.contains('\u{2026}'));
    }

    /// Status-bar helper: always a chip for Build, never for gateway chat.
    #[test]
    fn credit_status_line_always_paints_build_never_gateway_chat() {
        let theme = Theme::default();
        let warm = credit_status_line_for_live_session(
            Some(&bal(24.0)),
            SamplingIdentityKind::SuperGrokSession,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            false,
            &theme,
            false,
        )
        .expect("Build session must paint the compact meter");
        let warm_text: String = warm.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            warm_text.contains("SuperGrok period") && warm_text.contains("24%"),
            "warm Build chip: {warm_text}"
        );
        let cold = credit_status_line_for_live_session(
            None,
            SamplingIdentityKind::SuperGrokSession,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            false,
            &theme,
            false,
        )
        .expect("cold Build session still paints a clickable placeholder");
        let cold_text: String = cold.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(cold_text, "SuperGrok period · ...%");
        assert!(
            credit_status_line_for_live_session(
                Some(&bal(24.0)),
                SamplingIdentityKind::SuperGrokSession,
                None,
                ConsoleTeamPrepaidGap::MissingManagementKey,
                false,
                &theme,
                true,
            )
            .is_none(),
            "gateway chat must not paint Build coding-credit chrome"
        );
    }

    /// Named contract: console-live compact status must name console (prepaid
    /// or honest gap), not bare SuperGrok `...%` as if SuperGrok drives the turn.
    #[test]
    fn compact_status_console_live_does_not_imply_supergrok_drives_turn() {
        let cold_console = compact_meter_text_for_live_identity(
            SamplingIdentityKind::ConsoleKey,
            false, // SuperGrok included unknown
            0.0,
            None,
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert!(
            cold_console.contains("console"),
            "console live must name console: {cold_console}"
        );
        assert_ne!(
            cold_console, "...%",
            "must not paint bare SuperGrok cold meter while console live"
        );
        assert!(
            !cold_console.starts_with("..."),
            "must not look like SuperGrok free period: {cold_console}"
        );

        let prepaid = compact_meter_text_for_live_identity(
            SamplingIdentityKind::ConsoleKey,
            false,
            0.0,
            Some(77_700),
            ConsoleTeamPrepaidGap::Loading,
            Some(9900), // SuperGrok dollar credits must not hijack console chrome
        );
        assert!(
            prepaid.contains("console") && prepaid.contains("777"),
            "console live with prepaid must show team $: {prepaid}"
        );
        assert!(
            !prepaid.contains('%'),
            "console prepaid is $, not SuperGrok %: {prepaid}"
        );
        assert!(
            !prepaid.to_ascii_lowercase().contains("extras"),
            "console live must not paint SuperGrok extras: {prepaid}"
        );

        // SuperGrok live + cold still uses honest free SuperGrok period · ...%.
        let sg_cold = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            false,
            0.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
        );
        assert_eq!(sg_cold, "SuperGrok period · ...%");
    }

    /// Named contract: hop destination console + SuperGrok dollar credits
    /// still on the account must not keep the SuperGrok dollar credits chip.
    #[test]
    fn active_driver_console_does_not_paint_supergrok_dollar_credits_chip() {
        let hop = "Switched SuperGrok session → console key (rate limited)";
        let tracked = SamplingIdentityKind::default();
        assert_eq!(tracked, SamplingIdentityKind::SuperGrokSession);
        let dest = sampling_identity_from_hop_reason(hop).unwrap_or(tracked);
        let bal = CreditBalance {
            prepaid_balance_cents: Some(26_264),
            included_usage_known: true,
            usage_pct: 100.0,
            effective_usage_pct: 100.0,
            ..CreditBalance::default()
        };
        let live = compact_meter_identity(dest, Some(&bal));
        assert_eq!(live, SamplingIdentityKind::ConsoleKey);
        assert_eq!(
            active_spend_driver(live, true, 100.0, Some(26_264)),
            ActiveSpendDriver::ConsoleKey
        );
        let theme = Theme::default();
        let line = credit_status_line_for_live_session(
            Some(&bal),
            live,
            Some(22_675),
            ConsoleTeamPrepaidGap::Loading,
            false,
            &theme,
            false,
        )
        .expect("Build session paints compact credits");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !text.contains("SuperGrok dollar credits")
                && !text.to_ascii_lowercase().contains("extras"),
            "hop-to-console compact must not keep SuperGrok dollar credits: {text}"
        );
    }

    /// Named contract: compact status names console team prepaid when console
    /// is the live key.
    #[test]
    fn compact_status_names_console_team_prepaid_when_console_is_live() {
        let hop = "Switched SuperGrok session → console key (out of allowance)";
        let dest = sampling_identity_from_hop_reason(hop)
            .unwrap_or(SamplingIdentityKind::SuperGrokSession);
        let bal = CreditBalance {
            prepaid_balance_cents: Some(26_264),
            included_usage_known: true,
            usage_pct: 100.0,
            effective_usage_pct: 100.0,
            ..CreditBalance::default()
        };
        let live = compact_meter_identity(dest, Some(&bal));
        let theme = Theme::default();
        let line = credit_status_line_for_live_session(
            Some(&bal),
            live,
            Some(22_675),
            ConsoleTeamPrepaidGap::Loading,
            false,
            &theme,
            false,
        )
        .expect("Build session paints compact credits");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("console") && text.contains("226.75"),
            "compact must name console team prepaid when console is live: {text}"
        );
        assert!(
            !text.contains("SuperGrok dollar credits")
                && !text.to_ascii_lowercase().contains("extras"),
            "console-live compact must not paint SuperGrok dollar credits: {text}"
        );
    }

    /// Named contract: active SuperGrok poll auth-failed must not paint
    /// free-period % (sibling fill / stale cache must not look like active OK).
    #[test]
    fn compact_status_active_auth_failed_not_sibling_free_period_pct() {
        let text = compact_meter_text_for_live_identity_with_active_poll(
            SamplingIdentityKind::SuperGrokSession,
            true,
            6.0, // sibling-looking free-period reading
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(500),
            true, // active poll AuthFailed
            None,
        );
        assert_eq!(
            text, "SuperGrok period · ...%",
            "active auth fail must be cold free-period chrome, not 6%: {text}"
        );
        assert!(
            !text.contains('6'),
            "must not paint sibling free-period success: {text}"
        );
    }

    /// Named contract (Design A): SuperGrok live + included SuperGrok period
    /// limits full + SuperGrok dollar credits known positive → compact meter
    /// shows SuperGrok dollar credits $, not included-period used % as if
    /// included still drives after-burner spend. Must not teach extras.
    #[test]
    fn compact_status_supergrok_on_dollar_credits_shows_dollars_not_free_period_pct() {
        let on_credits = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            100.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(453), // $4.53 SuperGrok dollar credits
        );
        assert!(
            on_credits.contains("SuperGrok dollar credits") && on_credits.contains("4.53"),
            "included period full + SuperGrok dollar credits must show SuperGrok dollar credits $: {on_credits}"
        );
        assert!(
            !on_credits.to_ascii_lowercase().contains("extras"),
            "compact meter must not teach extras as a nickname: {on_credits}"
        );
        assert!(
            !on_credits.contains('%'),
            "must not paint included-period % while SuperGrok dollar credits drive: {on_credits}"
        );

        // Line paint path must match the pure helper.
        let theme = Theme::default();
        let bal = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(100.0)
        };
        let line = credit_bar_line_for_session(&bal, false, &theme, false)
            .expect("SuperGrok dollar credits meter must paint");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, on_credits);
    }

    /// Named contract: SuperGrok live + included SuperGrok period limits have
    /// room → included-period %. SuperGrok dollar credits on the account are
    /// not the live driver yet.
    #[test]
    fn compact_status_supergrok_free_period_room_shows_pct_not_dollar_credits() {
        let mid = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            42.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(12_500),
        );
        assert_eq!(mid, "SuperGrok period · 42%");
        assert!(
            !mid.to_ascii_lowercase().contains("extras"),
            "free period with room must not paint extras as live: {mid}"
        );
    }

    /// Included SuperGrok period limits full with no SuperGrok dollar credits
    /// left → included-period-labeled 100% is honest (included empty; no
    /// second meter).
    #[test]
    fn compact_status_supergrok_full_without_dollar_credits_shows_100_pct() {
        let full = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            100.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
        );
        assert_eq!(full, "SuperGrok period · 100%");
        let zero_dollar_credits = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            100.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(0),
        );
        assert_eq!(zero_dollar_credits, "SuperGrok period · 100%");
    }

    /// Named contract (P1 smoking gun): SuperGrok live + free period 6% + exhaust
    /// memo claiming out of allowance + team prepaid known → compact meter is
    /// **`included SuperGrok period limits · 6%`**, never **`console · $340`**.
    /// Sticky pin blocked by live headroom.
    #[test]
    fn compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars() {
        let identity = status_sampling_identity_for_compact_meter(
            SamplingIdentityKind::SuperGrokSession,
            true,
            6.0,
            true, // memo claims out of allowance + console ready
        );
        assert_eq!(
            identity,
            SamplingIdentityKind::SuperGrokSession,
            "live free-period headroom must block sticky console pin"
        );
        let text = compact_meter_text_for_live_identity(
            identity,
            true,
            6.0,
            Some(34_000), // team prepaid $340 must not hijack free-period chrome
            ConsoleTeamPrepaidGap::Loading,
            Some(10_029), // SuperGrok dollar credits on account; not live driver
        );
        assert_eq!(text, "SuperGrok period · 6%");
        assert!(
            !text.to_ascii_lowercase().contains("console"),
            "must not paint console while free period has room: {text}"
        );
        assert!(
            !text.contains("340") && !text.contains('$'),
            "must not paint team prepaid $ while free period drives: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("extras"),
            "must not paint SuperGrok extras while free period has room: {text}"
        );
    }

    /// Named contract (operator shot 2026-08-21 11:41): compact footer
    /// `353K / 500K | console · $12.45`. SuperGrok is live. Included SuperGrok
    /// period limits are known and still have room. Compact must name included
    /// SuperGrok period limits, not lead with `console · $12.45`.
    ///
    /// `$12.45` is grok-oss printout of Management prepaid remaining, not
    /// console.x.ai Billing Credits. SuperGrok is paid. grok-oss compact chrome
    /// is a client printout, not xAI billing truth. Do not hide included
    /// SuperGrok period limits behind console dollars.
    #[test]
    fn compact_bar_must_not_lead_with_console_dollars_while_included_period_has_room() {
        let theme = Theme::default();
        let balance = bal(15.0);
        let management_prepaid_cents = Some(1_245);

        let assert_included_not_console_dollars = |live: SamplingIdentityKind, path: &str| {
            let line = credit_status_line_for_live_session(
                Some(&balance),
                live,
                management_prepaid_cents,
                ConsoleTeamPrepaidGap::Loading,
                false,
                &theme,
                false,
            )
            .expect("Build session paints compact credits");
            let text = line_text(&line);
            assert!(
                !text.starts_with("console · $12.45") && !text.starts_with("console · $"),
                "{path}: compact bar with SuperGrok live and included SuperGrok period limits known must not lead with console · $12.45: {text}"
            );
            assert!(
                text.contains("SuperGrok period") && text.contains("15%"),
                "{path}: compact must name included SuperGrok period limits: {text}"
            );
            assert!(
                !text.to_ascii_lowercase().contains("extras"),
                "{path}: must not teach extras as a nickname: {text}"
            );
        };

        assert_included_not_console_dollars(
            SamplingIdentityKind::SuperGrokSession,
            "SuperGrok live identity",
        );
        // 11:41 chip: compact treated console as the live payer while included
        // SuperGrok period limits still had room.
        assert_included_not_console_dollars(
            SamplingIdentityKind::ConsoleKey,
            "console sampling identity with included SuperGrok period limits still the primary pool",
        );
    }

    /// Included SuperGrok period limits used percent is not the console
    /// Billing Credits dollar. Compact must paint included %, never $47.03.
    #[test]
    fn included_supergrok_period_limits_percent_is_not_the_billing_credits_dollar_balance() {
        use xai_grok_sampling_types::billing_credits_usd_from_included_period_percent;

        assert_eq!(
            billing_credits_usd_from_included_period_percent(47.03),
            None
        );
        let text = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            47.03,
            Some(8_994),
            ConsoleTeamPrepaidGap::Loading,
            Some(4_703),
        );
        assert_eq!(text, "SuperGrok period · 47%");
        assert!(
            !text.contains("$47.03") && !text.contains("$89.94") && !text.contains('$'),
            "included used % must not paint as Billing Credits dollars: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("billing credits"),
            "compact must not treat included % as the Billing Credits card: {text}"
        );
        let theme = Theme::default();
        let mut balance = bal(47.03);
        balance.prepaid_balance_cents = Some(4_703);
        let line = credit_status_line_for_live_session(
            Some(&balance),
            SamplingIdentityKind::SuperGrokSession,
            Some(8_994),
            ConsoleTeamPrepaidGap::Loading,
            false,
            &theme,
            false,
        )
        .expect("Build session paints compact credits");
        let painted = line_text(&line);
        assert!(
            painted.contains("SuperGrok period") && painted.contains('%'),
            "compact must name included SuperGrok period limits, not Credits-card dollars: {painted}"
        );
        assert!(
            !painted.contains("$47.03") && !painted.contains("$89.94"),
            "compact must not paint Credits-card dollars from included %: {painted}"
        );
    }

    /// Sticky pin still applies when free period is full (true after-full path).
    #[test]
    fn status_identity_sticky_console_when_free_period_full_and_memo_out() {
        assert_eq!(
            status_sampling_identity_for_compact_meter(
                SamplingIdentityKind::SuperGrokSession,
                true,
                100.0,
                true,
            ),
            SamplingIdentityKind::ConsoleKey
        );
        // Cold free period + memo → sticky console still allowed (unknown headroom).
        assert_eq!(
            status_sampling_identity_for_compact_meter(
                SamplingIdentityKind::SuperGrokSession,
                false,
                0.0,
                true,
            ),
            SamplingIdentityKind::ConsoleKey
        );
        // Tracked console stays console when included SuperGrok period limits
        // are full (true after-full path).
        assert_eq!(
            status_sampling_identity_for_compact_meter(
                SamplingIdentityKind::ConsoleKey,
                true,
                100.0,
                true,
            ),
            SamplingIdentityKind::ConsoleKey
        );
        // Included SuperGrok period limits with room win compact identity,
        // even if tracked sampling is console.
        assert_eq!(
            status_sampling_identity_for_compact_meter(
                SamplingIdentityKind::ConsoleKey,
                true,
                6.0,
                true,
            ),
            SamplingIdentityKind::SuperGrokSession
        );
        // No memo + headroom → SuperGrok.
        assert_eq!(
            status_sampling_identity_for_compact_meter(
                SamplingIdentityKind::SuperGrokSession,
                true,
                42.0,
                false,
            ),
            SamplingIdentityKind::SuperGrokSession
        );
    }

    /// Named contract (P3/P5): included SuperGrok period limits still have room
    /// → active driver is included SuperGrok period limits even when SuperGrok
    /// dollar credits and team prepaid are known on the account.
    #[test]
    fn active_driver_free_period_headroom_even_with_dollar_credits_and_team_prepaid() {
        let d = active_spend_driver(
            SamplingIdentityKind::SuperGrokSession,
            true,
            6.0,
            Some(10_029),
        );
        assert_eq!(d, ActiveSpendDriver::SuperGrokFreePeriod);
        assert_eq!(d.as_wire(), "supergrok_free_period");
        assert_eq!(d.as_human(), "SuperGrok period");
        // Team prepaid is not an input; driver ignores it by construction.
        assert_ne!(d.as_wire(), "console_key");
        assert_ne!(d.as_wire(), "supergrok_extras");
    }

    /// Named contract: compact chrome must name the workspace it is showing,
    /// or say combined. Unlabeled `N%` can be read as the other SuperGrok
    /// login. SuperGrok Heavy is a distinct weekly pool and is not this meter.
    #[test]
    fn compact_included_workspace_qualifier_personal_business_or_combined() {
        assert_eq!(
            compact_included_workspace_qualifier(1, Some("personal")),
            Some("personal")
        );
        assert_eq!(
            compact_included_workspace_qualifier(1, Some("business")),
            Some("business")
        );
        assert_eq!(
            compact_included_workspace_qualifier(2, Some("personal")),
            Some("combined"),
            "two distinct pools must not wear the live JWT's personal/business label"
        );
        assert_eq!(
            compact_included_workspace_qualifier(2, Some("business")),
            Some("combined")
        );
        assert_eq!(
            compact_included_workspace_qualifier(1, None),
            None,
            "do not invent personal or business when the role is unknown"
        );
        assert_eq!(compact_included_workspace_qualifier(1, Some("team")), None);
        assert_eq!(
            compact_included_workspace_qualifier_for_painted(
                1,
                Some("business"),
                Some(40.0),
                Some(5.0),
            ),
            Some("combined"),
            "unified-collapsed remaining must not stamp business on a percent business did not poll"
        );
        assert_eq!(
            compact_included_workspace_qualifier_for_painted(1, None, Some(40.0), Some(5.0)),
            Some("combined"),
            "unlabeled only when the painted percent is the live JWT's own poll"
        );
        assert_eq!(
            compact_included_workspace_qualifier_for_painted(1, None, Some(5.0), Some(5.0)),
            None
        );
        assert_eq!(
            compact_included_workspace_qualifier_for_painted(
                1,
                Some("personal"),
                Some(5.0),
                Some(5.0),
            ),
            Some("personal")
        );
    }

    /// Named contract: a single-pool included SuperGrok period reading paints
    /// the workspace (`personal` / `business`) in compact chrome. SuperGrok is
    /// paid. This is not SuperGrok dollar credits and not SuperGrok Heavy.
    #[test]
    fn compact_meter_names_personal_workspace_when_that_pool_is_the_reading() {
        let text = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            true,
            5.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            compact_included_workspace_qualifier(1, Some("personal")),
        );
        assert_eq!(text, "SuperGrok period · personal · 5%");
        assert!(
            !text.to_ascii_lowercase().contains("business"),
            "must not imply the business principal: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("heavy"),
            "must not mash SuperGrok Heavy into included SuperGrok period limits: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("extras"),
            "must not teach extras as a nickname: {text}"
        );
    }

    #[test]
    fn compact_meter_names_business_workspace_when_that_pool_is_the_reading() {
        let text = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            true,
            5.0,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            compact_included_workspace_qualifier(1, Some("business")),
        );
        assert_eq!(text, "SuperGrok period · business · 5%");
        assert!(
            !text.to_ascii_lowercase().contains("personal"),
            "must not imply the personal principal: {text}"
        );
    }

    /// Named contract: compact chrome must not stamp `business` or `personal`
    /// (the live JWT role / `is_unified_billing_user` collapse) on a used
    /// percent that workspace did not independently poll.
    ///
    /// If remaining collapsed two polls into one pool (max remaining),
    /// compact must still not stamp `business` on a percent that workspace
    /// did not independently poll. Process-cache remaining must not create
    /// that collapse by copying the live JWT flag. SuperGrok is paid. Do
    /// not invent remaining. Do not call any pool used up. SuperGrok Heavy
    /// is not this meter.
    #[test]
    fn compact_meter_does_not_stamp_live_workspace_on_unified_collapsed_other_pool_percent() {
        use chrono::{TimeZone, Utc};
        use xai_grok_shell::auth::{
            IncludedPoolReading, chrome_included_usage_from_combined, combined_included_remaining,
        };

        let reset_personal = Utc.timestamp_opt(1_000, 0).single().unwrap();
        let reset_business = Utc.timestamp_opt(2_000, 0).single().unwrap();
        // Guard identity chrome if remaining were already collapsed.
        let collapsed = combined_included_remaining(&[
            IncludedPoolReading {
                identity_id: "personal".into(),
                usage_pct: Some(5.0),
                reset_at: Some(reset_personal),
                is_unified_billing_user: Some(true),
            },
            IncludedPoolReading {
                identity_id: "business".into(),
                usage_pct: Some(40.0),
                reset_at: Some(reset_business),
                is_unified_billing_user: Some(true),
            },
        ]);
        let live_business_included_used_pct = 40.0;
        let (known, painted_pct) =
            chrome_included_usage_from_combined(true, live_business_included_used_pct, &collapsed);
        assert_eq!(
            collapsed.distinct_pool_count, 1,
            "precondition: unified copy collapses two independent polls to one pool"
        );
        assert_eq!(
            painted_pct, 5.0,
            "precondition: collapse paints max remaining (personal 5%), not business 40%"
        );

        // Identity count without the copied unified flag: two independent polls.
        let independent = combined_included_remaining(&[
            IncludedPoolReading {
                identity_id: "personal".into(),
                usage_pct: Some(5.0),
                reset_at: Some(reset_personal),
                is_unified_billing_user: None,
            },
            IncludedPoolReading {
                identity_id: "business".into(),
                usage_pct: Some(40.0),
                reset_at: Some(reset_business),
                is_unified_billing_user: None,
            },
        ]);
        assert_eq!(independent.distinct_pool_count, 2);

        let workspace = compact_included_workspace_qualifier_for_painted(
            independent.distinct_pool_count,
            Some("business"),
            Some(live_business_included_used_pct),
            Some(painted_pct),
        );
        let text = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            known,
            painted_pct,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            workspace,
        );
        assert_ne!(
            text, "SuperGrok period · business · 5%",
            "must not stamp business on a percent that workspace did not poll: {text}"
        );
        assert!(
            text.contains("combined"),
            "painted 5% is not the live business poll (40%); say combined: {text}"
        );
        assert!(
            !text.contains("business") && !text.contains("personal"),
            "must not wear a live JWT workspace word on the other pool's percent: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("extras"),
            "must not teach extras as a nickname: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("heavy"),
            "must not mash SuperGrok Heavy into included SuperGrok period limits: {text}"
        );

        let workspace_unknown = compact_included_workspace_qualifier_for_painted(
            independent.distinct_pool_count,
            None,
            Some(live_business_included_used_pct),
            Some(painted_pct),
        );
        let unlabeled = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            known,
            painted_pct,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            workspace_unknown,
        );
        assert_ne!(
            unlabeled, "SuperGrok period · 5%",
            "unlabeled only when the painted percent is the live JWT's own poll: {unlabeled}"
        );
        assert!(
            unlabeled.contains("combined"),
            "unknown role still must not imply one workspace for a collapsed foreign percent: {unlabeled}"
        );

        let live_personal_included_used_pct = 5.0;
        let (_, painted_from_personal_live) =
            chrome_included_usage_from_combined(true, live_personal_included_used_pct, &collapsed);
        let workspace_personal = compact_included_workspace_qualifier_for_painted(
            independent.distinct_pool_count,
            Some("personal"),
            Some(live_personal_included_used_pct),
            Some(painted_from_personal_live),
        );
        let personal_text = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            true,
            painted_from_personal_live,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            workspace_personal,
        );
        // Live personal 5% matching painted 5% is still one collapsed view of
        // two independent polls (business independently polled 40%). Do not
        // stamp personal on that combined remaining either.
        assert_ne!(
            personal_text, "SuperGrok period · personal · 5%",
            "must not stamp personal when a distinct business poll exists: {personal_text}"
        );
        assert!(
            personal_text.contains("combined"),
            "independent pools must say combined even if live percent matches painted: {personal_text}"
        );
    }

    /// Named contract: compact process-cache remaining must not copy a live
    /// JWT `is_unified_billing_user` onto a sibling SuperGrok row. Wire
    /// `is_unified_billing_user == true` on a row still counts that identity
    /// once. Combined remaining still groups by SuperGrok identity_id.
    /// Matching independent polls must not collapse remaining from a copied
    /// flag. SuperGrok is paid. Do not invent remaining. Do not call any
    /// pool used up.
    #[test]
    #[serial_test::serial]
    fn process_cache_remaining_does_not_copy_live_jwt_unified_flag_onto_sibling_supergrok_row() {
        use xai_grok_shell::auth::{
            clear_included_billing_cache, included_remaining_from_usage_pct,
            remember_supergrok_included_billing,
        };

        clear_included_billing_cache();
        remember_supergrok_included_billing(
            "personal-oidc",
            62.0,
            Some("2026-08-24T19:25:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );
        remember_supergrok_included_billing(
            "business-oidc",
            62.4,
            Some("2026-08-24T19:25:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
        );

        let mut live = bal(62.0);
        live.is_unified_billing_user = Some(true);

        let combined = combined_included_from_active_and_process_cache(Some(&live));
        let personal_rem = included_remaining_from_usage_pct(62.0);
        let business_rem = included_remaining_from_usage_pct(62.4);
        assert_eq!(
            combined.distinct_pool_count, 2,
            "copied live JWT unified flag must not collapse two SuperGrok identities into one pool"
        );
        assert_eq!(
            combined.remaining_units,
            personal_rem + business_rem,
            "remaining must sum both identities, not max of a copied unified pool"
        );
        assert_ne!(
            combined.remaining_units,
            personal_rem.max(business_rem),
            "must not treat a copied is_unified_billing_user as one remaining number"
        );

        let independent = independent_included_from_active_and_process_cache(Some(&live));
        assert_eq!(
            independent.distinct_pool_count, 2,
            "identity count without a copied flag must stay two SuperGrok identities"
        );

        clear_included_billing_cache();
    }

    /// Named contract: combined remaining across distinct personal and
    /// business pools must not wear a single-workspace label.
    #[test]
    fn compact_meter_says_combined_not_one_workspace_when_distinct_pools() {
        use chrono::{TimeZone, Utc};
        use xai_grok_shell::auth::{
            IncludedPoolReading, chrome_included_usage_from_combined, combined_included_remaining,
        };

        let combined = combined_included_remaining(&[
            IncludedPoolReading {
                identity_id: "personal".into(),
                usage_pct: Some(5.0),
                reset_at: Some(Utc.timestamp_opt(1_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
            IncludedPoolReading {
                identity_id: "business".into(),
                usage_pct: Some(40.0),
                reset_at: Some(Utc.timestamp_opt(2_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
        ]);
        let (known, pct) = chrome_included_usage_from_combined(true, 5.0, &combined);
        let workspace =
            compact_included_workspace_qualifier(combined.distinct_pool_count, Some("personal"));
        let text = compact_meter_text_for_live_identity_with_workspace(
            SamplingIdentityKind::SuperGrokSession,
            known,
            pct,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(10_029),
            workspace,
        );
        assert!(
            text.contains("SuperGrok period"),
            "must stay on SuperGrok period chrome: {text}"
        );
        assert!(
            text.contains("combined"),
            "must say combined, not one workspace: {text}"
        );
        assert!(
            !text.contains("personal") && !text.contains("business"),
            "combined chrome must not imply one workspace: {text}"
        );
        assert!(
            !text.contains("SuperGrok dollar credits"),
            "must not paint SuperGrok dollar credits while a sibling included pool has remaining: {text}"
        );
    }

    /// Named contract: personal included SuperGrok period limits full plus
    /// SuperGrok dollar credits must stay on included chrome while a distinct
    /// Business pool still has remaining. Combined used percent, not SuperGrok
    /// dollar credits as the live driver.
    #[test]
    fn compact_meter_stays_included_while_sibling_pool_has_remaining() {
        use chrono::{TimeZone, Utc};
        use xai_grok_shell::auth::{
            IncludedPoolReading, chrome_included_usage_from_combined, combined_included_remaining,
        };

        let combined = combined_included_remaining(&[
            IncludedPoolReading {
                identity_id: "personal".into(),
                usage_pct: Some(100.0),
                reset_at: Some(Utc.timestamp_opt(1_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
            IncludedPoolReading {
                identity_id: "business".into(),
                usage_pct: Some(40.0),
                reset_at: Some(Utc.timestamp_opt(2_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
        ]);
        assert!(combined.remaining_units > 0);
        let (known, pct) = chrome_included_usage_from_combined(true, 100.0, &combined);
        let text = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            known,
            pct,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            Some(10_029),
        );
        assert!(
            text.contains("SuperGrok period"),
            "must stay on SuperGrok period chrome while a sibling pool has remaining: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("extras"),
            "must not paint SuperGrok extras while a sibling included pool has remaining: {text}"
        );
    }

    /// Named contract: active spend driver stays included SuperGrok period
    /// limits while any distinct pool has remaining, even if the live JWT is
    /// at 100% with SuperGrok dollar credits.
    #[test]
    fn active_spend_driver_stays_included_while_any_distinct_pool_has_remaining() {
        use chrono::{TimeZone, Utc};
        use xai_grok_shell::auth::{
            IncludedPoolReading, chrome_included_usage_from_combined, combined_included_remaining,
        };

        let combined = combined_included_remaining(&[
            IncludedPoolReading {
                identity_id: "personal".into(),
                usage_pct: Some(100.0),
                reset_at: Some(Utc.timestamp_opt(1_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
            IncludedPoolReading {
                identity_id: "business".into(),
                usage_pct: Some(40.0),
                reset_at: Some(Utc.timestamp_opt(2_000, 0).single().unwrap()),
                is_unified_billing_user: None,
            },
        ]);
        let (known, pct) = chrome_included_usage_from_combined(true, 100.0, &combined);
        let d = active_spend_driver(
            SamplingIdentityKind::SuperGrokSession,
            known,
            pct,
            Some(10_029),
        );
        assert_eq!(d, ActiveSpendDriver::SuperGrokFreePeriod);
        assert_eq!(d.as_human(), "SuperGrok period");
        assert_ne!(d.as_wire(), "supergrok_extras");
    }

    /// Design A after-burner: included SuperGrok period limits ≥ 100% plus
    /// SuperGrok dollar credits → SuperGrok dollar credits driver
    /// (`ActiveSpendDriver::SuperGrokDollarCredits` / wire `supergrok_extras`).
    #[test]
    fn active_driver_afterburner_dollar_credits_when_free_period_full() {
        let d = active_spend_driver(
            SamplingIdentityKind::SuperGrokSession,
            true,
            100.0,
            Some(453),
        );
        assert_eq!(d, ActiveSpendDriver::SuperGrokDollarCredits);
        assert_eq!(d.as_wire(), "supergrok_extras");
        assert_eq!(d.as_human(), "SuperGrok dollar credits");
        assert!(
            !d.as_human().to_ascii_lowercase().contains("extras"),
            "Active human label must not teach extras as a nickname: {}",
            d.as_human()
        );
        // Console live always console_key.
        assert_eq!(
            active_spend_driver(SamplingIdentityKind::ConsoleKey, true, 6.0, Some(9999)),
            ActiveSpendDriver::ConsoleKey
        );
        // Included SuperGrok period limits full, no SuperGrok dollar credits →
        // included-period form (100% chrome).
        assert_eq!(
            active_spend_driver(SamplingIdentityKind::SuperGrokSession, true, 100.0, None),
            ActiveSpendDriver::SuperGrokFreePeriod
        );
        assert_eq!(
            active_spend_driver(SamplingIdentityKind::SuperGrokSession, true, 100.0, Some(0)),
            ActiveSpendDriver::SuperGrokFreePeriod
        );
    }

    // --- Work C: free SuperGrok period compact; quiet footer while free period has room ---

    /// Named contract (Work C): free SuperGrok period headroom + team prepaid
    /// present → compact status is free SuperGrok period %, never console or
    /// team prepaid dollars. Prompt footer under personal SuperGrok principal
    /// stays quiet on team $ (no long not-active-spend team prepaid line).
    #[test]
    fn work_c_free_period_headroom_intent_compact_and_quiet_footer() {
        // Compact Design A: free SuperGrok period 15% with team prepaid $340 known.
        let compact = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            15.0,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(10_029),
        );
        assert_eq!(
            compact, "SuperGrok period · 15%",
            "compact must name free SuperGrok period %, got {compact}"
        );
        assert!(
            !compact.to_ascii_lowercase().contains("console")
                && !compact.contains("340")
                && !compact.contains('$'),
            "must not paint team prepaid as compact free SuperGrok period: {compact}"
        );
        assert_eq!(
            active_spend_driver(
                SamplingIdentityKind::SuperGrokSession,
                true,
                15.0,
                Some(10_029),
            ),
            ActiveSpendDriver::SuperGrokFreePeriod,
            "activeDriver stays free SuperGrok period (not secondary team $)"
        );

        // Footer: free SuperGrok period has room → no team secondary domination.
        let b = bal_period(15.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let footer = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            true,  // usage_visible (personal SuperGrok principal)
            false, // not gateway
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(116_292), // ~$1162.92 Grok Build class
        );
        assert!(
            footer.is_none(),
            "free SuperGrok period with room must keep footer free of team $: {footer:?}"
        );
    }

    /// Named contract (operator screenshot 2026-08-09; cleaned same day): free
    /// SuperGrok period at ~27% used is the active meter. Team prepaid $340 and
    /// Grok Build class $1162.92 must **not** dominate the prompt footer with
    /// `not the active spend path: team prepaid remaining $340 · Grok Build
    /// class $1162.92` next to model name / always-approve. Compact stays free
    /// SuperGrok period; team wallets stay on `/limits`.
    #[test]
    fn operator_screenshot_free_period_primary_footer_not_long_team_prepaid() {
        let compact = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            27.0,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert_eq!(compact, "SuperGrok period · 27%");
        assert_eq!(
            active_spend_driver(SamplingIdentityKind::SuperGrokSession, true, 27.0, None),
            ActiveSpendDriver::SuperGrokFreePeriod
        );
        assert_eq!(
            active_spend_driver(SamplingIdentityKind::SuperGrokSession, true, 27.0, None).as_wire(),
            "supergrok_free_period"
        );

        let b = bal_period(27.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let footer = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(116_292), // ~$1162.92 Grok Build class (screenshot)
        );
        assert!(
            footer.is_none(),
            "must not paint long team prepaid / Grok Build class footer while free SuperGrok period is primary with room: {footer:?}"
        );
        // Same contract if only team prepaid is warm (no class cents).
        let prepaid_only = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
        );
        assert!(
            prepaid_only.is_none(),
            "must not paint team prepaid remaining alone while free SuperGrok period has room: {prepaid_only:?}"
        );
    }

    /// Named contract (Work C): management path cold under SuperGrok live while
    /// free SuperGrok period has room → quiet footer (no loading team prepaid
    /// noise next to model). After free SuperGrok period is full, loading may
    /// surface under not-active-spend.
    #[test]
    fn work_c_cold_management_cache_quiet_while_free_period_has_room() {
        let b = bal_period(15.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let text = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert!(
            text.is_none(),
            "cold management must not dominate footer while free SuperGrok period has room: {text:?}"
        );

        let mut full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        full.prepaid_balance_cents = Some(5_00); // quiet SuperGrok dollar credits path
        let (after_full, _) = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&full),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            None,
            ConsoleTeamPrepaidGap::Loading,
            None,
        )
        .expect("after free period full, cold management may surface loading");
        let lower = after_full.to_ascii_lowercase();
        assert!(
            lower.contains("not the active spend path") && lower.contains("loading team prepaid"),
            "after free period full, cold cache may show not-active-spend loading: {after_full}"
        );
    }

    /// Named contract (Work C): team AuthMeta / consumer surface off
    /// (`usage_visible = false`) hides footer secondary team chips while compact
    /// free SuperGrok period % still paints. A≠B is principal/cache, not
    /// project cwd.
    #[test]
    fn work_c_usage_visible_false_hides_settlement_footer_not_compact_intent() {
        let b = bal_period(15.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let footer = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            false, // billing_surface_visible / usage_visible off (team AuthMeta)
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(116_292),
        );
        assert!(
            footer.is_none(),
            "usage_visible false must hide footer secondary team $ (AuthMeta gate): {footer:?}"
        );
        // Compact free SuperGrok period is a separate path (not gated by usage_visible).
        let compact = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            15.0,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert_eq!(
            compact, "SuperGrok period · 15%",
            "compact free SuperGrok period still paints when footer secondary team $ is gated off"
        );
    }

    /// Named contract (Work C): free SuperGrok period with room never lets
    /// secondary team dollars replace or dominate free SuperGrok period chrome.
    #[test]
    fn work_c_settlement_footer_does_not_replace_free_period_intent() {
        let b = bal_period(6.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let footer = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            None,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(116_292),
        );
        assert!(
            footer.is_none(),
            "mid free SuperGrok period must not paint secondary team-only footer: {footer:?}"
        );
        let compact = compact_meter_text_for_live_identity(
            SamplingIdentityKind::SuperGrokSession,
            true,
            6.0,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(10_029),
        );
        assert!(
            compact.starts_with("SuperGrok period ·") && compact.contains("6%"),
            "compact stays free SuperGrok period: {compact}"
        );
        assert!(
            !compact.to_ascii_lowercase().contains("settlement")
                && !compact.to_ascii_lowercase().contains("active spend")
                && !compact.contains("340"),
            "compact must not carry secondary team dollars: {compact}"
        );
    }

    /// Named contract (G5): SuperGrok OIDC is live, included SuperGrok period
    /// limits are known at 6%, SuperGrok dollar credits and team prepaid exist
    /// as separate meters, and the console API key is not live. Settlement
    /// chrome must not name console as the live payer.
    #[test]
    fn settlement_chrome_supergrok_oidc_must_not_name_console_as_live_payer() {
        let live = SamplingIdentityKind::SuperGrokSession;
        let included_pct = 6.0;
        let supergrok_dollar_credits = Some(10_029);
        let team_prepaid = Some(34_000);

        let compact = compact_meter_text_for_live_identity(
            live,
            true,
            included_pct,
            team_prepaid,
            ConsoleTeamPrepaidGap::Loading,
            supergrok_dollar_credits,
        );
        assert_eq!(compact, "SuperGrok period · 6%");
        assert!(
            !compact.to_ascii_lowercase().contains("console"),
            "compact must not name console as live payer: {compact}"
        );

        let driver = active_spend_driver(live, true, included_pct, supergrok_dollar_credits);
        assert_eq!(driver.as_human(), "SuperGrok period");
        assert_ne!(driver.as_wire(), "console_key");

        let mut balance = bal(included_pct);
        balance.prepaid_balance_cents = supergrok_dollar_credits;
        let theme = Theme::default();
        let status = credit_status_line_for_live_session(
            Some(&balance),
            live,
            team_prepaid,
            ConsoleTeamPrepaidGap::Loading,
            false,
            &theme,
            false,
        )
        .expect("SuperGrok OIDC Build session paints compact status");
        let status_text: String = status.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(status_text, "SuperGrok period · 6%");
        assert!(
            !status_text.to_ascii_lowercase().contains("console"),
            "status line must not name console as live payer: {status_text}"
        );

        let footer = usage_warning_for_session_with_identity_principal_gap_and_postpaid(
            Some(&balance),
            None,
            None,
            true,
            false,
            false,
            live,
            None,
            team_prepaid,
            ConsoleTeamPrepaidGap::Loading,
            None,
        );
        assert!(
            footer
                .as_ref()
                .is_none_or(|(text, _)| !text.starts_with("Console key")),
            "footer must not start with Console key when SuperGrok is live: {footer:?}"
        );

        // This file formats `/usage` SuperGrok-live follow-up, not `/limits`
        // headers. Ban payer phrases only; a separate team prepaid meter line
        // may still mention console.
        let usage =
            format_usage_summary_with_live_identity(Some(&balance), None, live, team_prepaid);
        assert!(
            !usage.contains("Live sampling: console key"),
            "/usage must not name console as live sampling: {usage}"
        );
        assert!(
            !usage.contains("Active: console key"),
            "/usage must not name console as Active: {usage}"
        );
    }
}
