//! Credit balance indicator for the agent status bar.
//!
//! Shows the user's coding credit usage as a compact status bar item.
//! Fetches real data from the `x.ai/billing` agent extension.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::theme::Theme;

/// Credit balance state from the billing API.
#[derive(Debug, Clone)]
pub struct CreditBalance {
    /// Usage as a percentage of the allowance (0.0–100.0).
    ///
    /// Only meaningful when [`Self::included_usage_known`] is true. When
    /// unknown, chrome must paint an honest placeholder (`...%`), never a
    /// silent `0%` lie.
    pub usage_pct: f64,
    /// Usage as a percentage of total budget (free + on-demand when enabled).
    pub effective_usage_pct: f64,
    /// Billing period end as a formatted local wall-clock string (no zone
    /// label), e.g. "Mar 31, 12:00".
    pub period_end_display: Option<String>,
    /// Absolute period end (UTC) when billing provided an RFC 3339 end.
    /// Used by `/limits` live countdown; display string stays local format.
    /// With [`Self::period_type`], also drives free SuperGrok period linear-burn
    /// pacing (start derived when wire start is absent).
    pub period_end_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether pay-as-you-go (on-demand) billing is enabled.
    pub pay_as_you_go: bool,
    /// On-demand spending cap in USD cents (e.g. 500 = $5.00).
    pub on_demand_cap_cents: Option<i64>,
    /// On-demand usage this period in USD cents.
    pub on_demand_used_cents: Option<i64>,
    /// Remaining prepaid ("bought") credit balance in USD cents.
    pub prepaid_balance_cents: Option<i64>,
    /// Usage period type from the billing response (the proto enum name, e.g.
    /// `USAGE_PERIOD_TYPE_WEEKLY`). Drives the "Weekly/Monthly limit" label.
    pub period_type: Option<String>,
    /// From credits config `is_unified_billing_user` (`None` if absent).
    /// `Some(true)` = unified pool / buy-credits UX; `Some(false)` = legacy
    /// on-demand / PAYG UX.
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
/// API key while SuperGrok billing still reports personal prepaid extras. The
/// footer must not present those extras as what Build is burning.
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

/// Gap after a billing fetch completed with cents still unknown.
pub fn resolve_console_team_prepaid_gap_after_billing_fetch() -> ConsoleTeamPrepaidGap {
    ConsoleTeamPrepaidGap::after_billing_fetch(
        xai_grok_shell::auth::resolve_management_api_key_default().is_some(),
        xai_grok_shell::auth::resolve_management_team_id_default().is_some(),
    )
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
/// pool** — never SuperGrok prepaid extras while console is what Build burns.
///
/// `supergrok_out_of_allowance_with_console_ready` is true when dual-auth can
/// use a console key and the SuperGrok session fingerprint is still memoized
/// out of allowance (process + durable `$GROK_HOME/exhausted_credits/`).
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

impl CreditBalance {
    /// Label for the percentage allowance, chosen from the period type:
    /// "Weekly limit" / "Monthly limit", falling back to "Usage" when unknown.
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
    /// A known "no / disabled auto top-up" state — distinct from an unresolved
    /// `None`, which means the rule hasn't been fetched yet.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            topup_amount_cents: None,
            max_amount_cents: None,
        }
    }
}

/// Outcome of an auto top-up rule fetch, so a transient failure doesn't clear a
/// previously cached rule.
#[derive(Debug, Clone)]
pub enum AutoTopupFetch {
    /// A definitive rule state (a real rule, or [`AutoTopupInfo::disabled`] when
    /// the backend reports none). Stored as the *known* auto top-up state.
    Resolved(AutoTopupInfo),
    /// Fetch failed — keep the cached value (last-known-good). A stored `None`
    /// therefore means "not yet known", not "no auto top-up".
    Unchanged,
    /// The rule is not applicable (no prepaid credits) — reset the cache to
    /// "unknown" so a later credits period doesn't read a stale rule.
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

/// Build the `/usage` summary block shown in scrollback.
///
/// Always shows usage % and (when known) the next reset time. The SuperGrok
/// extras block is rendered only when the user has a positive prepaid balance
/// from the grok.com session billing fetch (not console.x.ai team credits):
/// - no prepaid balance       → extras block omitted entirely
/// - auto top-up off/unknown  → `Auto topup: disabled` (no max line)
/// - auto top-up on, no max   → `Auto topup: $N`
/// - auto top-up on, max set  → `Auto topup: $N` + `Max monthly topup: $M`
///
/// When wire `productUsage` carried Grok Build %, that line is always shown
/// (branch 2b); never invented when absent.
///
/// SuperGrok-primary path only. When live sampling is a console key, use
/// [`format_usage_summary_with_live_identity`] so SuperGrok extras are never
/// sold as the live console spend.
pub fn format_usage_summary(balance: &CreditBalance, autotopup: Option<&AutoTopupInfo>) -> String {
    format_usage_summary_with_live(
        balance,
        autotopup,
        SamplingIdentityKind::SuperGrokSession,
        chrono::Utc::now(),
    )
}

/// Like [`format_usage_summary`] with live identity (console honesty) and clock.
pub fn format_usage_summary_with_live(
    balance: &CreditBalance,
    autotopup: Option<&AutoTopupInfo>,
    live: SamplingIdentityKind,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    // Floor to match the backend SpendingLimiter's `as u8` truncation
    // (99.994% → 99%, never 100% until truly exhausted). Unknown included
    // reading must not paint a silent 0%.
    let mut lines = vec![if balance.included_usage_known {
        format!(
            "{}: {}%",
            balance.usage_label(),
            balance.usage_pct.floor() as i64
        )
    } else {
        format!("{}: not yet available", balance.usage_label())
    }];
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

    // Billing stores credit / top-up amounts as negative cents (accounting
    // convention); display the absolute USD value, matching the web clients.
    // Label as SuperGrok extras so the footer is never mistaken for console
    // team prepaid credits (those are a different pool on console.x.ai).
    if let Some(prepaid) = balance
        .prepaid_balance_cents
        .map(i64::abs)
        .filter(|c| *c > 0)
    {
        lines.push(String::new());
        lines.push(format!("SuperGrok extras: {}", fmt_dollars(prepaid)));
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

    // Legacy on-demand (pay-as-you-go) billing — shown only when enabled, for
    // users on the older monthly + on-demand model. Amounts always carry cents
    // (e.g. `$50.00`), matching the web client.
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
/// **not** present SuperGrok session billing / SuperGrok $ extras as the live
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
    flat_poll_observed_extras: bool,
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
    // Honest included reading only when the balance carries a known meter
    // (mirrors snapshot primary.included.is_some() for the single-cache path).
    let has_included_reading = balance.is_some_and(|b| b.included_usage_known);
    let notes = crate::views::limits_honesty::honesty_notes_for_limits(
        crate::views::limits_honesty::LimitsHonestyInput {
            live: sampling_identity,
            has_included_reading,
            flat_poll_unproven_debit,
            flat_poll_observed_build,
            flat_poll_observed_extras,
            oauth_postpaid_dominates,
            // `/usage` SuperGrok path does not attach console team prepaid $;
            // lag note is for `/limits` when Management prepaid is shown.
            has_console_team_prepaid_reading: false,
            // Default credits live on `/limits` postpaid preview, not `/usage`.
            has_team_default_credits_reading: false,
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

/// The prompt's usage/credits warning as `(text, critical)`, or `None`
/// (`critical` = yellow, else grey; team users with `usage_visible = false`
/// never warn). Behaviour splits by billing model — prepaid credits,
/// pay-as-you-go on-demand, or the included-allowance percentage — with exact
/// thresholds and copy pinned by the unit tests.
///
/// Gateway light-frontend (`kind: "chat"`) sessions must not surface Build
/// coding-credit warnings — use [`usage_warning_for_session`] with
/// `gateway_chat = true` so the prompt shows no fake local sampler telemetry.
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
/// When live primary is a **console key**, never presents SuperGrok prepaid
/// extras as the spend meter (personal SuperGrok $ is a different pool). Shows
/// console team prepaid dollars when Management API cents are known, else an
/// honest gap (`console key · no management key` / `no management team id` /
/// loading / unavailable).
///
/// When live primary is SuperGrok, prepaid is labeled **SuperGrok extras left**
/// — never generic "Credits left".
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
/// with SuperGrok session extras. When cents are `None`, uses
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

    // Console / Business API key is live: do not show SuperGrok prepaid extras
    // or included-% as if they were the pool Build is burning. When Management
    // prepaid cents are known, show plain console team prepaid dollars.
    // Honest gap still beats the wrong SuperGrok number.
    if sampling_identity.is_console() {
        let label = sampling_identity.as_str();
        let mut chars = label.chars();
        let labeled = match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        };
        if let Some(cents) = console_team_prepaid_cents {
            let remaining = cents.abs();
            let text = format!("{labeled} · team prepaid: {}", fmt_dollars(remaining));
            let critical = remaining <= LOW_BALANCE_CENTS;
            return Some((text, critical));
        }
        return Some((
            format!("{labeled} · {}", console_team_prepaid_gap.as_display_str()),
            false,
        ));
    }

    let balance = balance?;
    let role_suffix = live_principal_role
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({s})"))
        .unwrap_or_default();

    // A non-zero prepaid balance (stored as signed cents) means SuperGrok
    // extras / bought credits from the session billing path.
    let credits = balance
        .prepaid_balance_cents
        .map(i64::abs)
        .filter(|c| *c > 0);

    let Some(credits_cents) = credits else {
        // No known included % → no percentage warning (avoid inventing "100% left").
        if !balance.included_usage_known {
            return None;
        }
        // Pay-as-you-go (legacy on-demand): warn on dollars left in the cap once
        // the included allowance is spent.
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
            // "Left" = complement of floored usage, so it agrees with the
            // floored summary (99.994% → "1% left", not "0%").
            let remaining = (100 - pct.floor() as i64).max(0);
            let label = balance.usage_label();
            return Some((
                format!("{label} left{role_suffix}: {remaining}%"),
                pct > 95.0,
            ));
        }
        return None;
    };

    // Extras are only drawn down at 100% included usage; don't warn before then.
    if balance.usage_pct < 100.0 {
        return None;
    }

    let credits_warning = || {
        (
            format!(
                "SuperGrok extras left{role_suffix}: {}",
                fmt_dollars(credits_cents)
            ),
            true,
        )
    };

    // Auto top-up gates the warning: unknown → silent; disabled → warn when low;
    // enabled w/o max → never; enabled w/ max → warn below one top-up amount.
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

/// Build the credit balance indicator as a `Line<'static>`.
///
/// Shows just `XX%` in the status bar (weekly included usage). No "Credits used"
/// label — percent alone is enough; click opens Limits for detail.
///
/// Gateway light-frontend (`kind: "chat"`) sessions must not show Build coding
/// credits — use [`credit_bar_line_for_session`] with `gateway_chat = true`
/// (returns `None`). remote settings / managed opt-in for chat entry can share the
/// same gate later; for now it only zeros/suppresses misleading local telemetry.
pub fn credit_bar_line(balance: &CreditBalance, hovered: bool, theme: &Theme) -> Line<'static> {
    credit_bar_line_for_session(balance, hovered, theme, false)
        .expect("non-chat credit_bar_line always renders")
}

/// Like [`credit_bar_line`], but returns `None` for gateway/chat-kind sessions
/// so the status bar never implies Build sampler / coding-credit usage.
///
/// When included usage is unknown, paints the same honest `...%` placeholder
/// as a cold cache — never a silent `0%`.
pub fn credit_bar_line_for_session(
    balance: &CreditBalance,
    hovered: bool,
    theme: &Theme,
    gateway_chat: bool,
) -> Option<Line<'static>> {
    if gateway_chat {
        return None;
    }
    if !balance.included_usage_known {
        return Some(credit_bar_loading_line(hovered, theme));
    }
    let pct = balance.usage_pct;
    let color = if pct >= 100.0 {
        theme.accent_error
    } else if pct >= 80.0 {
        theme.warning
    } else {
        theme.accent_success
    };

    // Compact: percent only (implicit weekly included). Never "Credits used:".
    // Optional linear-burn chip when bounds exist (space-friendly short form).
    let text = match balance.pacing_chip(SamplingIdentityKind::SuperGrokSession, chrono::Utc::now())
    {
        Some(chip) if chip.len() <= 28 => format!("{pct:.0}% · {chip}"),
        _ => format!("{pct:.0}%"),
    };

    let style = Style::default().fg(color).bg(theme.bg_base);
    Some(Line::from(Span::styled(text, style)))
}

/// Status-bar placeholder when SuperGrok limits are in play but billing has
/// not warmed yet. Always visible and clickable (`ShowLimits`); never blank
/// until the first successful fetch.
///
/// ASCII `...` only (no unicode ellipsis). Dim so warm percent still reads as
/// the primary signal once data arrives.
pub fn credit_bar_loading_line(hovered: bool, theme: &Theme) -> Line<'static> {
    let text = "...%";
    let mut style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
    if hovered {
        style = style.add_modifier(ratatui::style::Modifier::BOLD);
    }
    Line::from(Span::styled(text, style))
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
        // Even with an auto-topup rule present, zero prepaid → no credits block.
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
            "Usage: 25%\n\nSuperGrok extras: $100\nAuto topup: disabled"
        );
        // A disabled rule renders the same.
        assert_eq!(
            format_usage_summary(&b, Some(&topup(false, Some(2000), Some(10000)))),
            "Usage: 25%\n\nSuperGrok extras: $100\nAuto topup: disabled"
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
            "Usage: 25%\n\nSuperGrok extras: $100\nAuto topup: $20"
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
            "Usage: 25%\nNext reset: June 14, 16:00\n\nSuperGrok extras: $100\nAuto topup: $20\nMax monthly topup: $100"
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
            "Usage: 25%\n\nSuperGrok extras: $12.50\nAuto topup: $5.50"
        );
    }

    #[test]
    fn summary_abs_negative_billing_amounts() {
        // Billing returns credit / top-up amounts as negative cents; the
        // summary must render them as positive USD (matching the web).
        let b = CreditBalance {
            prepaid_balance_cents: Some(-500),
            ..bal(100.0)
        };
        assert_eq!(
            format_usage_summary(&b, Some(&topup(true, Some(-500), Some(-1000)))),
            "Usage: 100%\n\nSuperGrok extras: $5\nAuto topup: $5\nMax monthly topup: $10"
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

    // ── usage_label / period type ────────────────────────────────────

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
        // Unknown / unspecified / absent → falls back to "Usage".
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
        // Match the backend SpendingLimiter (`as u8` truncation): 99.994% must
        // render as 99%, not round up to 100%.
        let almost = bal_period(99.994, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(format_usage_summary(&almost, None), "Weekly limit: 99%");
        // A true 100% still shows 100%.
        let full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(format_usage_summary(&full, None), "Weekly limit: 100%");
    }

    #[test]
    fn warning_percent_left_is_floor_complement() {
        // 99.994% used → floored to 99% → "1% left" (not "0% left"), so the
        // warning and the floored summary always sum to 100.
        let almost = bal_period(99.994, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(
            usage_warning(&almost, None, true),
            Some(("Weekly limit left: 1%".to_string(), true))
        );
        // A true 100% (no credits) → "0% left".
        let full = bal_period(100.0, "USAGE_PERIOD_TYPE_WEEKLY");
        assert_eq!(
            usage_warning(&full, None, true),
            Some(("Weekly limit left: 0%".to_string(), true))
        );
    }

    // ── usage_warning (prompt info row) ──────────────────────────────

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
        // At 100% usage with prepaid credits, but the rule isn't known yet
        // (None) — never warn; it resolves on the next billing fetch.
        let b = CreditBalance {
            prepaid_balance_cents: Some(100),
            ..bal(100.0)
        };
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_credits_suppressed_below_full_usage() {
        // Low credits + no auto top-up, but the included allowance still has
        // room (usage < 100%) → no warning (credits aren't being spent yet).
        let disabled = topup(false, None, None);
        let low = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(0.0)
        };
        assert_eq!(usage_warning(&low, Some(&disabled), true), None);
        // Same balance once the allowance is exhausted → warn.
        let exhausted = CreditBalance {
            prepaid_balance_cents: Some(453),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&exhausted, Some(&disabled), true),
            Some(("SuperGrok extras left: $4.53".to_string(), true))
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
            Some(("SuperGrok extras left: $4.53".to_string(), true))
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
            Some(("SuperGrok extras left: $10".to_string(), true))
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
        // $15 balance, $20 top-up amount, $100 max → below one top-up → warn.
        let b = CreditBalance {
            prepaid_balance_cents: Some(1500),
            ..bal(100.0)
        };
        assert_eq!(
            usage_warning(&b, Some(&topup(true, Some(2000), Some(10000))), true),
            Some(("SuperGrok extras left: $15".to_string(), true))
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
            Some(("SuperGrok extras left: $4.53".to_string(), true))
        );
    }

    #[test]
    fn warning_credits_take_precedence_over_usage() {
        // A credits user below 100% usage gets no warning at all (no usage-%
        // warning, and credits aren't being spent yet) — unlike a non-credits
        // user, who would see "Usage left: 1%" at 99%.
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

    // ── Meter honesty: live sampling identity (console vs SuperGrok) ─

    #[test]
    fn usage_summary_console_live_names_team_prepaid_not_supergrok_extras() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            period_end_display: Some("Jul 30, 12:00".into()),
            ..bal(100.0)
        };
        let text = format_usage_summary_with_live_identity(
            Some(&b),
            None,
            SamplingIdentityKind::ConsoleKey,
            Some(12_500),
        );
        assert!(text.contains("Live sampling: console key"), "{text}");
        assert!(text.contains("Console team prepaid: $125"), "{text}");
        assert!(
            !text.contains("SuperGrok extras"),
            "console live must not sell SuperGrok extras as live: {text}"
        );
        assert!(
            !text.contains("Weekly limit:"),
            "console live must not lead with SuperGrok session %: {text}"
        );
    }

    #[test]
    fn usage_summary_console_live_without_prepaid_honest_gap() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            ..bal(100.0)
        };
        let text = format_usage_summary_with_live_identity_and_gap(
            Some(&b),
            None,
            SamplingIdentityKind::ConsoleKey,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
        );
        assert!(
            text.contains("Console team prepaid: no management key"),
            "{text}"
        );
        assert!(
            !text.contains("no $ meter yet"),
            "soft placeholder retired: {text}"
        );
        assert!(
            !text.contains("no management key/team id"),
            "mushy combined gap retired: {text}"
        );
        assert!(!text.contains("SuperGrok extras"), "{text}");
    }

    #[test]
    fn usage_summary_console_live_missing_team_id_distinct_from_missing_key() {
        let text = format_usage_summary_with_live_identity_and_gap(
            None,
            None,
            SamplingIdentityKind::ConsoleKey,
            None,
            ConsoleTeamPrepaidGap::MissingTeamId,
        );
        assert!(
            text.contains("Console team prepaid: no management team id"),
            "{text}"
        );
        assert!(!text.contains("no management key/team id"), "{text}");
        assert!(
            !text.contains("no management key\n") && !text.ends_with("no management key"),
            "missing team must not read as missing key alone: {text}"
        );
    }

    #[test]
    fn usage_summary_console_live_configured_cold_shows_loading_not_unavailable() {
        // Product cold path: from_management_config / default resolve → Loading.
        let cold = ConsoleTeamPrepaidGap::from_management_config(true, true);
        assert_eq!(cold, ConsoleTeamPrepaidGap::Loading);
        let text = format_usage_summary_with_live_identity_and_gap(
            None,
            None,
            SamplingIdentityKind::ConsoleKey,
            None,
            cold,
        );
        assert!(
            text.contains("Console team prepaid: loading team prepaid..."),
            "{text}"
        );
        assert!(
            !text.contains("team prepaid unavailable"),
            "configured cold must not read as hard fail: {text}"
        );
        assert!(!text.contains("no $ meter yet"), "{text}");
    }

    #[test]
    fn usage_summary_console_live_post_fetch_miss_shows_unavailable() {
        let post = ConsoleTeamPrepaidGap::after_billing_fetch(true, true);
        assert_eq!(post, ConsoleTeamPrepaidGap::Unavailable);
        let unavailable = format_usage_summary_with_live_identity_and_gap(
            None,
            None,
            SamplingIdentityKind::ConsoleKey,
            None,
            post,
        );
        assert!(
            unavailable.contains("Console team prepaid: team prepaid unavailable"),
            "{unavailable}"
        );
        assert!(!unavailable.contains("no $ meter yet"), "{unavailable}");
        assert!(
            !unavailable.contains("loading team prepaid"),
            "post-fetch miss is unavailable, not loading: {unavailable}"
        );
    }

    #[test]
    fn console_team_prepaid_gap_display_strings_are_honest() {
        // Named contract: missing key vs missing team vs loading vs unavailable
        // are distinct plain operator-visible strings (no mushy key/team mash).
        assert_eq!(
            ConsoleTeamPrepaidGap::MissingManagementKey.as_display_str(),
            "no management key"
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::MissingTeamId.as_display_str(),
            "no management team id"
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::Loading.as_display_str(),
            "loading team prepaid..."
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::Unavailable.as_display_str(),
            "team prepaid unavailable"
        );
        // Distinct config → distinct variants.
        assert_eq!(
            ConsoleTeamPrepaidGap::from_management_config(false, false),
            ConsoleTeamPrepaidGap::MissingManagementKey
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::from_management_config(false, true),
            ConsoleTeamPrepaidGap::MissingManagementKey
        );
        // Key alone → Loading (team id may be discovered via key validation).
        assert_eq!(
            ConsoleTeamPrepaidGap::from_management_config(true, false),
            ConsoleTeamPrepaidGap::Loading
        );
        // Configured + cents unknown (cold) → Loading, not unavailable.
        assert_eq!(
            ConsoleTeamPrepaidGap::from_management_config(true, true),
            ConsoleTeamPrepaidGap::Loading
        );
        // Post-fetch miss → Unavailable; unconfigured stays distinct miss.
        assert_eq!(
            ConsoleTeamPrepaidGap::after_billing_fetch(true, true),
            ConsoleTeamPrepaidGap::Unavailable
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::after_billing_fetch(false, true),
            ConsoleTeamPrepaidGap::MissingManagementKey
        );
        assert_eq!(
            ConsoleTeamPrepaidGap::after_billing_fetch(true, false),
            ConsoleTeamPrepaidGap::MissingTeamId
        );
        // Never emit the retired mushy combined string.
        for gap in [
            ConsoleTeamPrepaidGap::MissingManagementKey,
            ConsoleTeamPrepaidGap::MissingTeamId,
            ConsoleTeamPrepaidGap::Loading,
            ConsoleTeamPrepaidGap::Unavailable,
        ] {
            assert!(
                !gap.as_display_str().contains("key/team"),
                "retired mushy gap: {:?}",
                gap
            );
            assert!(!gap.as_display_str().contains("no $ meter yet"));
        }
    }

    #[test]
    fn footer_console_live_without_mgmt_config_keeps_honest_gap() {
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            None,
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
        );
        let (text, critical) = w.expect("console gap");
        assert!(
            text.contains("no management key"),
            "missing key footer: {text}"
        );
        assert!(
            !text.contains("no management key/team id"),
            "mushy combined gap retired: {text}"
        );
        assert!(!text.contains("no $ meter yet"), "{text}");
        assert!(!text.contains('$'), "must not invent dollars: {text}");
        assert!(!critical);
    }

    #[test]
    fn footer_console_live_missing_team_id_distinct_from_missing_key() {
        // Cold: key present, team not pinned → Loading (discovery may fill team).
        let cold = ConsoleTeamPrepaidGap::from_management_config(true, false);
        assert_eq!(cold, ConsoleTeamPrepaidGap::Loading);
        // Post-fetch miss after discovery failed → explicit MissingTeamId.
        let gap = ConsoleTeamPrepaidGap::after_billing_fetch(true, false);
        assert_eq!(gap, ConsoleTeamPrepaidGap::MissingTeamId);
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            None,
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            None,
            gap,
        );
        let (text, critical) = w.expect("team gap");
        assert!(
            text.contains("Console key · no management team id"),
            "missing team footer: {text}"
        );
        assert!(
            !text.contains("no management key/team id"),
            "mushy combined retired: {text}"
        );
        // Must not be the missing-key line (operator needs team_id, not another key).
        assert!(
            !text.ends_with("no management key"),
            "must distinguish missing team from missing key: {text}"
        );
        assert!(!text.contains("no $ meter yet"), "{text}");
        assert!(!text.contains('$'), "must not invent dollars: {text}");
        assert!(!critical);
    }

    #[test]
    fn footer_console_live_with_mgmt_key_and_team_shows_prepaid_not_gap() {
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            None,
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            Some(12_500),
            ConsoleTeamPrepaidGap::Unavailable, // ignored when cents present
        );
        let (text, critical) = w.expect("prepaid");
        assert!(text.contains("team prepaid: $125"), "{text}");
        assert!(!text.contains("no $ meter yet"), "{text}");
        assert!(!text.contains("no management key"), "{text}");
        assert!(!critical);
    }

    #[test]
    fn footer_console_live_configured_cold_shows_loading_not_unavailable() {
        // Same wiring as footer render: resolve gap from management config.
        let gap = ConsoleTeamPrepaidGap::from_management_config(true, true);
        assert_eq!(gap, ConsoleTeamPrepaidGap::Loading);
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            None,
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            None,
            gap,
        );
        let (text, critical) = w.expect("gap");
        assert!(
            text.contains("loading team prepaid..."),
            "configured cold footer must load, not hard-fail: {text}"
        );
        assert!(
            !text.contains("team prepaid unavailable"),
            "configured cold must not say unavailable: {text}"
        );
        assert!(!text.contains("no $ meter yet"), "{text}");
        assert!(!critical);
    }

    #[test]
    fn footer_console_live_configured_unavailable_not_soft_placeholder() {
        // Explicit post-fail / after-fetch path still uses Unavailable.
        let w = usage_warning_for_session_with_identity_principal_and_gap(
            None,
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            None,
            ConsoleTeamPrepaidGap::Unavailable,
        );
        let (text, _) = w.expect("gap");
        assert!(text.contains("team prepaid unavailable"), "{text}");
        assert!(!text.contains("no $ meter yet"), "{text}");
    }

    #[test]
    fn usage_summary_supergrok_live_keeps_session_billing() {
        use crate::views::limits_honesty::NOTE_INCLUDED_PCT_IS_BILLING_POLL;

        let b = CreditBalance {
            prepaid_balance_cents: Some(10000),
            ..bal(25.0)
        };
        let text = format_usage_summary_with_live_identity(
            Some(&b),
            None,
            SamplingIdentityKind::SuperGrokSession,
            Some(12_500),
        );
        // SuperGrok-primary still uses session billing; console cents are not mixed in.
        assert!(
            text.starts_with("Usage: 25%\n\nSuperGrok extras: $100\nAuto topup: disabled"),
            "session billing body: {text}"
        );
        assert!(
            !text.contains("Console team prepaid"),
            "must not mix console prepaid into SuperGrok usage: {text}"
        );
        assert!(
            !text.contains("$125"),
            "console cents must not appear on SuperGrok path: {text}"
        );
        // Branch 2b: SuperGrok live usage surfaces base poll honesty (not burn claim).
        assert!(
            text.contains(NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "base poll honesty on SuperGrok live usage: {text}"
        );
    }

    /// Named contract (branch 2b): `/usage` surfaces Grok Build productUsage %
    /// when wire has it; never invents when None.
    #[test]
    fn usage_summary_surfaces_grok_build_product_usage_when_on_wire() {
        let b = CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            grok_build_usage_pct: Some(54.0),
            prepaid_balance_cents: Some(10029),
            ..bal(65.0)
        };
        let text = format_usage_summary(&b, None);
        assert!(
            text.contains("Grok Build product usage: 54% used"),
            "usage summary must surface Build % when on wire: {text}"
        );
        let cold = CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            prepaid_balance_cents: Some(10029),
            ..bal(65.0)
        };
        let cold_text = format_usage_summary(&cold, None);
        assert!(
            !cold_text.contains("Grok Build product usage:"),
            "must not invent Build %: {cold_text}"
        );
    }

    /// Named contract (branch 2b): SuperGrok live + flat-poll flag → honesty
    /// note on `/usage` (footer/scrollback surface), not only `/limits`.
    #[test]
    fn usage_summary_supergrok_live_surfaces_flat_poll_honesty() {
        use crate::views::limits_honesty::flat_poll_unproven_debit_note;

        let b = CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            prepaid_balance_cents: Some(10029),
            ..bal(65.0)
        };
        // Extras observed on balance; Build not on wire this call.
        let expected = flat_poll_unproven_debit_note(false, true);
        let text = format_usage_summary_with_live_identity_gap_and_honesty(
            Some(&b),
            None,
            SamplingIdentityKind::SuperGrokSession,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            true,  // flat_poll_unproven_debit
            false, // observed_build
            true,  // observed_extras
            false, // oauth_postpaid_dominates
        );
        assert!(
            text.contains(&expected),
            "flat-poll honesty required on usage when flag set: {text}"
        );
        assert!(
            text.contains("included debit is unproven"),
            "must say debit unproven: {text}"
        );
        assert!(
            !text.contains("Grok Build product %"),
            "must not claim Build flat when not observed: {text}"
        );
        // Console live: never SuperGrok flat honesty.
        let console = format_usage_summary_with_live_identity_gap_and_honesty(
            Some(&b),
            None,
            SamplingIdentityKind::ConsoleKey,
            Some(34_000),
            ConsoleTeamPrepaidGap::MissingManagementKey,
            true,
            true,
            true,
            true,
        );
        assert!(
            !console.contains("included debit is unproven"),
            "console live must not sell SuperGrok flat honesty: {console}"
        );
    }

    /// Named contract C6 on `/usage`: SuperGrok live + OAuth postpaid dominates.
    #[test]
    fn usage_summary_supergrok_live_surfaces_c6_team_usage_honesty() {
        use crate::views::limits_honesty::NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS;

        let b = CreditBalance {
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            ..bal(65.0)
        };
        let text = format_usage_summary_with_live_identity_gap_and_honesty(
            Some(&b),
            None,
            SamplingIdentityKind::SuperGrokSession,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            false, // flat
            false, // build
            false, // extras
            true,  // oauth
        );
        assert!(
            text.contains(NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "C6 honesty on usage when OAuth postpaid dominates: {text}"
        );
        assert!(
            text.contains("without proving") && text.contains("included weekly"),
            "must not sell team Usage $ as SuperGrok included debit: {text}"
        );
    }

    #[test]
    fn sampling_identity_labels_are_plain_language() {
        assert_eq!(
            SamplingIdentityKind::SuperGrokSession.as_str(),
            "SuperGrok session"
        );
        assert_eq!(SamplingIdentityKind::ConsoleKey.as_str(), "console key");
        assert!(SamplingIdentityKind::ConsoleKey.is_console());
        assert!(!SamplingIdentityKind::SuperGrokSession.is_console());
    }

    #[test]
    fn footer_names_live_principal_role_on_included_warning() {
        let bal = CreditBalance {
            usage_pct: 96.0,
            effective_usage_pct: 96.0,
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            ..bal(96.0)
        };
        let w = usage_warning_for_session_with_identity_and_principal(
            Some(&bal),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            None,
        );
        let (text, _) = w.expect("warning at 96%");
        assert!(
            text.contains("Weekly limit left (business):"),
            "footer should name live SuperGrok principal: {text}"
        );
        assert!(text.contains("4%"), "{text}");
    }

    #[test]
    fn sampling_identity_from_hop_reason_destination() {
        assert_eq!(
            sampling_identity_from_hop_reason(
                "Switched SuperGrok session → console key (out of allowance)"
            ),
            Some(SamplingIdentityKind::ConsoleKey)
        );
        assert_eq!(
            sampling_identity_from_hop_reason(
                "Switched console key → SuperGrok session (out of allowance)"
            ),
            Some(SamplingIdentityKind::SuperGrokSession)
        );
        assert_eq!(
            sampling_identity_from_hop_reason("Switched to next console key (rate limited)"),
            Some(SamplingIdentityKind::ConsoleKey)
        );
        assert_eq!(sampling_identity_from_hop_reason("rate limited"), None);
    }

    /// Contract: live primary = console after allowance mark → meter must not
    /// present SuperGrok prepaid extras as bare "Credits left" / SuperGrok
    /// extras $ without a console active-identity label.
    #[test]
    fn warning_console_primary_does_not_show_supergrok_extras_dollars() {
        // Dogfood shape: SuperGrok included full + ~$9.96 personal extras still
        // in billing, but samples run on the console key.
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            ..bal(100.0)
        };
        let disabled = topup(false, None, None);
        let w = usage_warning_for_session_with_identity(
            Some(&b),
            Some(&disabled),
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
        );
        let (text, critical) = w.expect("console primary should show honest console meter copy");
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("console"),
            "must label active identity as console: {text}"
        );
        assert!(
            !text.starts_with("SuperGrok extras left:"),
            "must not lead with SuperGrok extras $ while on console: {text}"
        );
        assert!(
            !text.starts_with("Credits left:"),
            "must not use bare Credits left: {text}"
        );
        // SuperGrok personal extras dollar amount must not be the primary story.
        assert!(
            !text.contains("$9.96"),
            "must not show SuperGrok extras dollars as meter primary: {text}"
        );
        assert!(
            !critical,
            "honest console absence is not a critical low-balance warn"
        );
    }

    /// Named contract: console live + Management prepaid fixture → plain
    /// **team prepaid** dollars (never SuperGrok extras labels).
    #[test]
    fn console_live_with_management_fixture_shows_prepaid_balance() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            ..bal(100.0)
        };
        let w = usage_warning_for_session_with_identity_and_principal(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            Some(12_500),
        );
        let (text, critical) = w.expect("console prepaid meter");
        let lower = text.to_ascii_lowercase();
        assert!(lower.contains("console"), "identity: {text}");
        assert!(
            lower.contains("team prepaid"),
            "plain console team prepaid label: {text}"
        );
        assert!(text.contains("$125"), "management prepaid dollars: {text}");
        assert!(
            !text.contains("$9.96") && !text.contains("SuperGrok extras"),
            "must not show SuperGrok extras while console prepaid present: {text}"
        );
        assert!(
            !text.contains("no $ meter yet"),
            "must not claim absence when cents present: {text}"
        );
        assert!(!critical, "$125 is above low-balance threshold");
    }

    /// Contract: live primary = SuperGrok with prepaid extras → existing extras
    /// path still works and is labeled SuperGrok.
    #[test]
    fn warning_supergrok_primary_still_shows_labeled_extras() {
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            ..bal(100.0)
        };
        let disabled = topup(false, None, None);
        assert_eq!(
            usage_warning_for_session_with_identity(
                Some(&b),
                Some(&disabled),
                None,
                true,
                false,
                false,
                SamplingIdentityKind::SuperGrokSession,
            ),
            Some(("SuperGrok extras left: $9.96".to_string(), true))
        );
        // Legacy openrouter wrapper defaults to SuperGrok session identity.
        assert_eq!(
            usage_warning_for_session_with_openrouter(
                Some(&b),
                Some(&disabled),
                None,
                true,
                false,
                false,
            ),
            Some(("SuperGrok extras left: $9.96".to_string(), true))
        );
    }

    #[test]
    fn warning_console_primary_suppresses_supergrok_included_pct_too() {
        // Included-% about SuperGrok is also the wrong pool when console is live.
        let b = bal_period(92.0, "USAGE_PERIOD_TYPE_WEEKLY");
        let w = usage_warning_for_session_with_identity(
            Some(&b),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
        );
        let (text, _) = w.expect("console meter copy");
        assert!(text.to_ascii_lowercase().contains("console"), "{text}");
        assert!(
            !text.contains("Weekly limit"),
            "must not show SuperGrok included % as primary while on console: {text}"
        );
    }

    /// Named contract (`bug:credits-meter-wrong-pool`): silent sticky console
    /// (SuperGrok still memoized out of allowance + dual-auth ready) must not
    /// present SuperGrok prepaid extras when tracked UI identity is still the
    /// default SuperGrokSession (no hop toast yet / after restart).
    #[test]
    fn meter_identity_prefers_console_when_supergrok_memo_exhausted() {
        assert_eq!(
            meter_sampling_identity(SamplingIdentityKind::SuperGrokSession, true),
            SamplingIdentityKind::ConsoleKey
        );
        // Tracked console stays console.
        assert_eq!(
            meter_sampling_identity(SamplingIdentityKind::ConsoleKey, true),
            SamplingIdentityKind::ConsoleKey
        );
        // Live SuperGrok when memo not exhausted.
        assert_eq!(
            meter_sampling_identity(SamplingIdentityKind::SuperGrokSession, false),
            SamplingIdentityKind::SuperGrokSession
        );
    }

    #[test]
    fn warning_silent_sticky_console_does_not_show_supergrok_extras() {
        // Dogfood shape: SuperGrok included full + prepaid extras still in
        // billing payload, samples already on console via silent prefer_live
        // (tracked UI still SuperGrokSession default).
        let b = CreditBalance {
            prepaid_balance_cents: Some(996),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            ..bal(100.0)
        };
        let disabled = topup(false, None, None);
        let identity = meter_sampling_identity(
            SamplingIdentityKind::SuperGrokSession,
            true, // SuperGrok out of allowance + console ready
        );
        let w = usage_warning_for_session_with_identity(
            Some(&b),
            Some(&disabled),
            None,
            true,
            false,
            false,
            identity,
        );
        let (text, critical) = w.expect("console honest meter");
        assert!(
            text.to_ascii_lowercase().contains("console"),
            "must label console live pool: {text}"
        );
        assert!(
            !text.contains("$9.96") && !text.starts_with("SuperGrok extras left:"),
            "must not sell SuperGrok extras as live spend: {text}"
        );
        assert!(
            !critical,
            "honest console absence is not critical low-balance"
        );
    }

    /// Named contract: Cleared exhaust under console auth primary must not
    /// re-label meter SuperGrok while preferred_method / login is console key.
    #[test]
    fn allowance_cleared_keeps_console_when_console_auth_primary() {
        assert_eq!(
            sampling_identity_after_allowance_sync(false, true, true),
            Some(SamplingIdentityKind::ConsoleKey)
        );
        // Session primary + period reset → SuperGrok meter again.
        assert_eq!(
            sampling_identity_after_allowance_sync(false, true, false),
            Some(SamplingIdentityKind::SuperGrokSession)
        );
        assert_eq!(
            sampling_identity_after_allowance_sync(true, false, false),
            Some(SamplingIdentityKind::ConsoleKey)
        );
        assert_eq!(
            sampling_identity_after_allowance_sync(false, false, false),
            None
        );
        // Marked wins over cleared if both somehow true.
        assert_eq!(
            sampling_identity_after_allowance_sync(true, true, true),
            Some(SamplingIdentityKind::ConsoleKey)
        );
    }

    // ── usage_warning: OpenRouter account credits ────────────────────

    #[test]
    fn warning_openrouter_shows_balance_always() {
        assert_eq!(
            usage_warning_for_session_with_openrouter(
                Some(&bal(50.0)),
                None,
                Some(&or_bal(6386)),
                true,
                false,
                true,
            ),
            Some(("OpenRouter credits left: $63.86".to_string(), false))
        );
        // Low balance → critical (yellow).
        assert_eq!(
            usage_warning_for_session_with_openrouter(
                None,
                None,
                Some(&or_bal(500)),
                true,
                false,
                true,
            ),
            Some(("OpenRouter credits left: $5".to_string(), true))
        );
        // OR model without a fetched balance → no warning (don't fall back to xAI).
        assert_eq!(
            usage_warning_for_session_with_openrouter(
                Some(&CreditBalance {
                    prepaid_balance_cents: Some(9999),
                    ..bal(100.0)
                }),
                Some(&topup(false, None, None)),
                None,
                true,
                false,
                true,
            ),
            None
        );
        // Non-OR model ignores OR balance.
        assert_eq!(
            usage_warning_for_session_with_openrouter(
                Some(&bal(50.0)),
                None,
                Some(&or_bal(6386)),
                true,
                false,
                false,
            ),
            None
        );
    }

    // ── usage_warning: pay-as-you-go (monthly on-demand) ─────────────

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
        // $50 cap, $42 used → $8 left → grey (above $5).
        let grey = pay_as_you_go(100.0, 5000, 4200);
        assert_eq!(
            usage_warning(&grey, None, true),
            Some(("Pay-as-you-go limit left: $8".to_string(), false))
        );
        // $50 cap, $46 used → $4 left → critical (yellow).
        let yellow = pay_as_you_go(100.0, 5000, 4600);
        assert_eq!(
            usage_warning(&yellow, None, true),
            Some(("Pay-as-you-go limit left: $4".to_string(), true))
        );
    }

    #[test]
    fn warning_pay_as_you_go_boundaries() {
        // Exactly $10 left → show, grey.
        let at_ten = pay_as_you_go(100.0, 5000, 4000);
        assert_eq!(
            usage_warning(&at_ten, None, true),
            Some(("Pay-as-you-go limit left: $10".to_string(), false))
        );
        // Exactly $5 left → critical (yellow).
        let at_five = pay_as_you_go(100.0, 5000, 4500);
        assert_eq!(
            usage_warning(&at_five, None, true),
            Some(("Pay-as-you-go limit left: $5".to_string(), true))
        );
    }

    #[test]
    fn warning_pay_as_you_go_above_threshold_silent() {
        // $20 left (> $10) → no warning.
        let b = pay_as_you_go(100.0, 5000, 3000);
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_pay_as_you_go_suppressed_below_full_usage() {
        // Pay-as-you-go users get NO percentage warning before the included
        // allowance is exhausted, even with low on-demand room remaining.
        let b = pay_as_you_go(95.0, 5000, 4800);
        assert_eq!(usage_warning(&b, None, true), None);
    }

    #[test]
    fn warning_pay_as_you_go_fractional_dollars() {
        // $50 cap, $46.50 used → $3.50 left → critical, fractional formatting.
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
        // Compact status: percent only — no "Credits used" prefix.
        assert_eq!(text, "24%");
        assert!(!text.contains("Credits"));
    }

    #[test]
    fn test_color_thresholds() {
        let theme = Theme::default();

        let low = credit_bar_line(&bal(50.0), false, &theme);
        assert_eq!(low.spans[0].style.fg, Some(theme.accent_success));

        let high = credit_bar_line(&bal(85.0), false, &theme);
        assert_eq!(high.spans[0].style.fg, Some(theme.warning));

        let over = credit_bar_line(&bal(100.0), false, &theme);
        assert_eq!(over.spans[0].style.fg, Some(theme.accent_error));
    }

    #[test]
    fn test_zero_percent() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(0.0), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "0%");
        assert_eq!(line.spans[0].style.fg, Some(theme.accent_success));
    }

    /// Named contract: unknown included meter must not paint a silent `0%`.
    #[test]
    fn unknown_included_usage_paints_loading_placeholder_not_zero() {
        let theme = Theme::default();
        let mut unknown = bal(0.0);
        unknown.included_usage_known = false;
        let line = credit_bar_line(&unknown, false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "...%");
        assert!(!text.contains("0%"), "unknown must not look like true zero");
    }

    /// Named contract: true zero (known reading of 0%) stays `0%`.
    #[test]
    fn true_zero_included_usage_paints_zero_percent() {
        let theme = Theme::default();
        let known_zero = bal(0.0);
        assert!(known_zero.included_usage_known);
        let text: String = credit_bar_line(&known_zero, false, &theme)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(text, "0%");
    }

    /// Console live meter is prepaid dollars (or honest gap), never SuperGrok %.
    #[test]
    fn usage_warning_console_live_names_console_prepaid_not_supergrok_pct() {
        let mut supergrok = bal(0.0);
        supergrok.included_usage_known = false;
        let warn = usage_warning_for_session_with_identity_principal_and_gap(
            Some(&supergrok),
            None,
            None,
            true,
            false,
            false,
            SamplingIdentityKind::ConsoleKey,
            None,
            Some(2500),
            ConsoleTeamPrepaidGap::MissingManagementKey,
        )
        .expect("console live should show team prepaid");
        assert!(
            warn.0.to_ascii_lowercase().contains("console") && warn.0.contains("$25"),
            "console live meter must name console prepaid, got {:?}",
            warn.0
        );
        assert!(
            !warn.0.contains("0%"),
            "must not show SuperGrok 0% on console live"
        );
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
        assert_eq!(text, "150%");
        assert_eq!(line.spans[0].style.fg, Some(theme.accent_error));
    }

    #[test]
    fn test_fractional_percentage_rounds_display() {
        let theme = Theme::default();
        let line = credit_bar_line(&bal(33.7), false, &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "34%");
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
        assert_eq!(text, "50%");
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
        assert_eq!(text, "...%");
        assert!(!text.contains("Credits"));
        assert_eq!(line.spans[0].style.fg, Some(theme.gray_dim));
        // No unicode ellipsis.
        assert!(!text.contains('\u{2026}'));
    }
}
