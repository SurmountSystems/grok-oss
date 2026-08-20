//! `grok limits` — agent-usable live sampling principal + spend meters.
//!
//! Standalone CLI (no TUI). Reuses shell auth resolve, SuperGrok credits poll
//! (`fetch_credits_config_with_session`), and Management team prepaid. Never
//! prints raw API keys, JWTs, or management secrets.
//!
//! Meters stay distinct: included SuperGrok period limits % ≠ SuperGrok dollar credits ≠
//! console team prepaid ≠ team postpaid OAuth/API class ≠ team default credits
//! (dashboard allotment) ≠ Management usage series window.
//! Named words persist `$GROK_HOME/limits_pins.json` (not `[auth]`). grok-oss
//! limits JSON is a client printout, not xAI billing truth. A client 100% /
//! remaining 0 / $0 printout must not mark SuperGrok used up.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use crate::views::credit_bar::{
    ActiveSpendDriver, ConsoleTeamPrepaidGap, CreditBalance, SamplingIdentityKind,
    active_spend_driver,
};
use crate::views::limits_snapshot::{
    ConsoleTeamPostpaidGap, ConsoleTeamPostpaidMeter, ConsoleTeamUsageSeriesSummary,
    DiscoveredIdentities, LimitsSnapshot, PrincipalLimitsInput,
    active_driver_line_for_snapshot_with_meter_source, chrome_included_from_limits_snapshot,
    format_limits_detail_with_meter_source, honesty_notes_for_snapshot,
};

/// Same named words on TUI `/limits` and CLI `grok-oss limits`.
pub const LIMITS_WORD_STAY_SUPERGROK: &str = "stay-supergrok";
/// Persist that the operator wants the console key.
pub const LIMITS_WORD_USE_CONSOLE: &str = "use-console";
/// Persist which meter chrome should emphasize.
pub const LIMITS_WORD_METER: &str = "meter";
/// Named ForceRefresh collect (same policy as explicit `/limits` open).
pub const LIMITS_WORD_REFRESH: &str = "refresh";

/// Usage listing for unknown extra args (slash and CLI share these words).
pub fn limits_named_words_usage() -> &'static str {
    "/limits, /limits --json, /limits stay-supergrok, /limits use-console, /limits meter included|dollar-credits|console|combined, or /limits refresh"
}

/// CLI args for `grok limits` / `grok limits multipoll`.
#[derive(Clone, Debug, Default, Eq, PartialEq, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct LimitsArgs {
    /// Emit machine-readable JSON (schemaVersion 1). No secrets.
    #[arg(long)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<LimitsCommand>,
}

/// Subcommands under `grok limits`.
#[derive(Clone, Debug, Eq, PartialEq, clap::Subcommand)]
#[command(rename_all = "kebab-case")]
pub enum LimitsCommand {
    /// Persist stay SuperGrok and clear a false exhaust memo.
    StaySupergrok,
    /// Persist that the operator wants the console key (sidecar, not `[auth]`).
    UseConsole,
    /// Persist which meter chrome `/limits` should emphasize.
    Meter {
        #[arg(value_enum)]
        source: LimitsMeterWord,
    },
    /// Force-refresh live meters (same ForceRefresh as explicit collect).
    Refresh,
    /// Sample live limits N times and classify path (P1) vs free-period series (P2).
    ///
    /// Writes JSONL samples plus a summary under `--out-dir` (default
    /// `~/.agents/reports/limits-multipoll-<utc>/`). Exit **0** when the
    /// limits-first path is OK or skipped; exit **non-zero only** on path
    /// failure (console live while free SuperGrok period limits still have
    /// room). Free SuperGrok period staying flat is measurement only and does
    /// **not** fail the process.
    Multipoll(MultipollArgs),
}

/// `grok-oss limits meter` / `/limits meter` source words.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum LimitsMeterWord {
    Included,
    DollarCredits,
    Console,
    Combined,
}

impl LimitsMeterWord {
    pub fn as_word(self) -> &'static str {
        match self {
            Self::Included => "included",
            Self::DollarCredits => "dollar-credits",
            Self::Console => "console",
            Self::Combined => "combined",
        }
    }

    pub fn to_meter_source(self) -> xai_grok_shell::auth::limits_pins::MeterSource {
        match self {
            Self::Included => xai_grok_shell::auth::limits_pins::MeterSource::Included,
            Self::DollarCredits => xai_grok_shell::auth::limits_pins::MeterSource::DollarCredits,
            Self::Console => xai_grok_shell::auth::limits_pins::MeterSource::Console,
            Self::Combined => xai_grok_shell::auth::limits_pins::MeterSource::Combined,
        }
    }

    pub fn from_word(s: &str) -> Option<Self> {
        match s {
            "included" => Some(Self::Included),
            "dollar-credits" => Some(Self::DollarCredits),
            "console" => Some(Self::Console),
            "combined" => Some(Self::Combined),
            _ => None,
        }
    }
}

/// Parsed `/limits` leftover words (same strings as CLI subcommands).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitsNamedAction {
    Show,
    Json,
    StaySupergrok,
    UseConsole,
    Meter(LimitsMeterWord),
    Refresh,
}

/// Parse TUI `/limits` leftover args using the same words as `grok-oss limits`.
pub fn parse_limits_named_args(args: &str) -> Result<LimitsNamedAction, String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts.as_slice() {
        [] => Ok(LimitsNamedAction::Show),
        ["--json"] | ["json"] => Ok(LimitsNamedAction::Json),
        [w] if *w == LIMITS_WORD_STAY_SUPERGROK => Ok(LimitsNamedAction::StaySupergrok),
        [w] if *w == LIMITS_WORD_USE_CONSOLE => Ok(LimitsNamedAction::UseConsole),
        [w] if *w == LIMITS_WORD_REFRESH => Ok(LimitsNamedAction::Refresh),
        [w] if *w == LIMITS_WORD_METER => Err(format!(
            "Unknown argument: {args}. Use {}",
            limits_named_words_usage()
        )),
        [w, src] if *w == LIMITS_WORD_METER => match LimitsMeterWord::from_word(src) {
            Some(src) => Ok(LimitsNamedAction::Meter(src)),
            None => Err(format!(
                "Unknown argument: {args}. Use {}",
                limits_named_words_usage()
            )),
        },
        _ => Err(format!(
            "Unknown argument: {args}. Use {}",
            limits_named_words_usage()
        )),
    }
}

/// Persist a named pin action. Refresh/show/json do not persist.
pub fn apply_limits_named_action(action: LimitsNamedAction) -> Result<String, String> {
    use xai_grok_shell::auth::limits_pins::{
        StaySupergrokApply, apply_meter_source, apply_stay_supergrok, apply_use_console,
    };
    match action {
        LimitsNamedAction::Show | LimitsNamedAction::Json | LimitsNamedAction::Refresh => {
            Ok(String::new())
        }
        LimitsNamedAction::StaySupergrok => match apply_stay_supergrok() {
            Ok(StaySupergrokApply::Applied) => Ok(
                "Stay SuperGrok pin written. Exhaust memo cleared. SuperGrok will be used again without requiring console credits."
                    .into(),
            ),
            Ok(StaySupergrokApply::BlockedByPreferredApiKey) => Err(
                "Console is pinned by [auth] preferred_method = api_key. stay-supergrok does not override that stock key."
                    .into(),
            ),
            Err(e) => Err(format!("Could not write stay SuperGrok pin: {e}")),
        },
        LimitsNamedAction::UseConsole => apply_use_console()
            .map(|()| {
                "Use-console pin written. The operator asked for the console key (sidecar, not a new [auth] key)."
                    .into()
            })
            .map_err(|e| format!("Could not write use-console pin: {e}")),
        LimitsNamedAction::Meter(src) => apply_meter_source(src.to_meter_source())
            .map(|()| format!("Meter source pin written: {}.", src.as_word()))
            .map_err(|e| format!("Could not write meter source pin: {e}")),
    }
}

/// Args for `grok limits multipoll`.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
pub struct MultipollArgs {
    /// Number of live samples (default 2; need ≥2 for free-period series class).
    #[arg(long, default_value_t = 2)]
    pub samples: usize,
    /// Seconds to sleep between sample ends (default 30; matches flat-detector
    /// [`xai_grok_shell::auth::DEFAULT_MIN_WINDOW`]).
    #[arg(long, default_value_t = 30)]
    pub sleep_secs: u64,
    /// Directory for `samples.jsonl` + `summary.json` (created if missing).
    /// Default: `~/.agents/reports/limits-multipoll-<utc>/` when HOME is set,
    /// else under the process temp dir.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

/// Management prepaid/postpaid/usage-series process-cache policy for a product
/// path.
///
/// Pure contract so unit tests lock "explicit collect/open busts / background
/// poll does not" without an app harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagementMeterCachePolicy {
    /// Explicit `grok limits` collect **or** TUI `/limits` open/refresh: clear
    /// prepaid+postpaid+usage-series process caches before Management fetch.
    ForceRefresh,
    /// TUI silent/background `FetchBilling` (turn end, session start, modal
    /// zero-countdown): honor ≤Ns process TTL (do **not** clear).
    HonorProcessTtl,
}

/// Policy for CLI/agent `grok limits` collect (force Management re-fetch).
pub fn management_meter_cache_policy_for_explicit_limits_collect() -> ManagementMeterCachePolicy {
    ManagementMeterCachePolicy::ForceRefresh
}

/// Policy for TUI operator-driven `/limits` open (slash, status click, or
/// in-TUI `/limits --json`). Same force class as CLI collect.
pub fn management_meter_cache_policy_for_explicit_limits_open() -> ManagementMeterCachePolicy {
    ManagementMeterCachePolicy::ForceRefresh
}

/// Policy for TUI background / silent `FetchBilling` Management prepaid path.
///
/// Must stay [`HonorProcessTtl`]: polls must **not** call
/// `clear_console_team_billing_meter_caches`.
pub fn management_meter_cache_policy_for_background_billing_poll() -> ManagementMeterCachePolicy {
    ManagementMeterCachePolicy::HonorProcessTtl
}

/// Whether product should clear Management prepaid+postpaid+usage-series
/// process caches.
///
/// True only for [`ManagementMeterCachePolicy::ForceRefresh`] when a management
/// key is present. Background HonorProcessTtl never clears.
pub fn should_clear_management_meter_caches(
    policy: ManagementMeterCachePolicy,
    has_management_key: bool,
) -> bool {
    has_management_key && policy == ManagementMeterCachePolicy::ForceRefresh
}

/// Whether TUI explicit `/limits` open should queue silent `FetchBilling`.
///
/// With a management key, always queue so force-bust is followed by live
/// prepaid (+ postpaid into process cache). Also when dual SuperGrok sibling
/// included rows are still empty. Do **not** use this for background polls.
pub fn should_queue_silent_billing_on_explicit_limits(
    has_management_key: bool,
    needs_sibling_billing: bool,
) -> bool {
    has_management_key || needs_sibling_billing
}

/// Whether a billing fetch path should live-call Management postpaid preview
/// (result lands in process cache; TTL honored unless caches were cleared).
///
/// Explicit `/limits` clears first when key present, so this becomes a true
/// live fetch. Background polls reuse warm cache within TTL.
pub fn should_live_fetch_console_team_postpaid_with_billing(has_management_key: bool) -> bool {
    has_management_key
}

/// Whether a billing fetch path should live-call Management usage series
/// (POST analytics; result lands in process cache; TTL honored unless caches
/// were cleared).
///
/// Same gate as postpaid: management key present. Background `FetchBilling`
/// and TUI `/limits` silent refresh both use this so series is not CLI-only.
/// Does **not** invent unbounded spam: shell honors the 60s process TTL.
pub fn should_live_fetch_console_team_usage_series_with_billing(has_management_key: bool) -> bool {
    has_management_key
}

/// JSON schema version for `grok limits --json`.
pub const SCHEMA_VERSION: &str = "1";

/// Machine-readable limits report (no secrets).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsCliReport {
    pub schema_version: &'static str,
    /// `supergrok_session` or `console_key` — next-request / live sampling principal.
    pub live_sampling: &'static str,
    /// Full live line (matches `/limits` human copy).
    pub live_sampling_label: String,
    /// SuperGrok principal role when live is SuperGrok and known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_principal_role: Option<String>,
    /// Active spend driver for free-period-first token economy (Design A).
    ///
    /// Wire: `supergrok_free_period` | `supergrok_extras` | `console_key`.
    /// Distinct from [`Self::live_sampling`] when SuperGrok session is live
    /// but free period is full and SuperGrok dollar extras drive after-burner.
    pub active_driver: &'static str,
    /// Human label for the active driver (matches `/limits` **Active:** line,
    /// including a `meter_source` pin when one is set). grok-oss limits JSON
    /// is a client printout, not xAI billing truth.
    pub active_driver_label: String,
    pub supergrok: SuperGrokCliSection,
    pub console: ConsoleCliSection,
    /// True when process + durable SuperGrok included poll history shows free
    /// SuperGrok period % flat across a multi-poll window (≥2 polls, ≥30s wall
    /// by default). Ticket / multipoll evidence for unproven free-period debit.
    /// Does **not** invent a higher free-period %; measurement only.
    pub flat_poll_unproven_debit: bool,
    /// True when every sample in the flat window carried Grok Build product %.
    /// Only meaningful when [`Self::flat_poll_unproven_debit`] is true.
    pub flat_poll_observed_build: bool,
    /// True when every sample in the flat window carried SuperGrok $ extras.
    /// Only meaningful when [`Self::flat_poll_unproven_debit`] is true.
    pub flat_poll_observed_extras: bool,
    /// Non-secret warnings (fetch failures, no auth, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Stored SuperGrok logins and console key fingerprints (no secrets).
    pub discovered_identities: DiscoveredIdentities,
}

/// Active spend driver from a `/limits` snapshot (same Design A logic as status).
///
/// Uses primary SuperGrok free-period % and SuperGrok dollar extras when live
/// is SuperGrok. Console live always returns console key. Team prepaid remaining
/// and team Grok Build settlement are never the `activeDriver` label here
/// (intent chrome only; settlement honesty notes name those meters separately).
pub fn active_spend_driver_from_snapshot(snap: &LimitsSnapshot) -> ActiveSpendDriver {
    let extras_cents = snap.primary.dollar_extras.as_ref().map(|d| d.balance_cents);
    let (included_known, included_pct) = chrome_included_from_limits_snapshot(snap);
    active_spend_driver(
        snap.live_identity,
        included_known,
        included_pct,
        extras_cents,
    )
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuperGrokCliSection {
    pub principals: Vec<PrincipalCliMeter>,
    pub shared_unified_pool: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalCliMeter {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Included allowance used percent (0–100+), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_used_pct: Option<f64>,
    /// Floored remaining % of included pool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_remaining_pct: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_label: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_reset: Option<String>,
    /// SuperGrok dollar extras remaining (USD), when observed and positive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dollar_extras_usd: Option<f64>,
    /// False when only included was polled (sibling) — extras not claimed empty.
    pub dollar_extras_observed: bool,
    /// Grok Build product usage % from wire `productUsage` when present.
    /// Distinct from top-level `includedUsedPct` (account-level included %).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grok_build_usage_pct: Option<f64>,
    /// True when this principal's JWT polled credits successfully.
    /// False when poll failed or the free-period row was only shared-pool fill.
    pub poll_succeeded: bool,
    /// Free SuperGrok period included % provenance:
    /// `live_poll` | `process_cache` | `shared_pool_fill`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_source: Option<&'static str>,
    /// Short poll fail class when known (`auth`, `network`, `other`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_error_class: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleCliSection {
    /// Inference console / Business API key on file or env.
    pub key_available: bool,
    /// Live sampling is currently the console key.
    pub is_live: bool,
    /// Console team prepaid remaining USD when Management meter known
    /// (`GET …/prepaid/balance` `total.val` abs). Not console.x.ai Billing Credits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_prepaid_usd: Option<f64>,
    /// Honest gap when team prepaid dollars unknown (snake-ish display key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_prepaid_gap: Option<&'static str>,
    /// Team postpaid invoice period total USD (Management preview). Distinct
    /// from [`Self::team_prepaid_usd`] and SuperGrok $ extras.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_period_total_usd: Option<f64>,
    /// OAuth / Grok Build class spend USD on the postpaid invoice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_oauth_class_usd: Option<f64>,
    /// API / ApiKey class spend USD on the postpaid invoice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_api_class_usd: Option<f64>,
    /// Team default credits (dashboard allotment) USD from postpaid preview.
    /// Distinct from [`Self::team_prepaid_usd`] (prepaid wallet remaining).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_default_credits_usd: Option<f64>,
    /// Honest gap when postpaid preview unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_gap: Option<&'static str>,
    /// Management usage series OAuth / Grok Build class USD over the day window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_usage_series_oauth_class_usd: Option<f64>,
    /// Management usage series API-key class USD over the day window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_usage_series_api_class_usd: Option<f64>,
    /// Management usage series window start (`YYYY-MM-DD HH:MM:SS`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_usage_series_start_time: Option<String>,
    /// Management usage series window end (`YYYY-MM-DD HH:MM:SS`, exclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_usage_series_end_time: Option<String>,
}

/// Predict which identity the next first-party sample would burn (read-only).
///
/// Does **not** mutate hop / exhaust state. Mirrors dual-auth primary rules:
/// console-primary pin, session out of allowance with console ready, else
/// session when present.
pub fn predict_live_sampling(
    session_present: bool,
    console_present: bool,
    preferred_console_primary: bool,
    session_out_of_allowance: bool,
) -> SamplingIdentityKind {
    match (session_present, console_present) {
        (false, true) => SamplingIdentityKind::ConsoleKey,
        (true, false) => SamplingIdentityKind::SuperGrokSession,
        (false, false) => SamplingIdentityKind::SuperGrokSession,
        (true, true) => {
            if preferred_console_primary || session_out_of_allowance {
                SamplingIdentityKind::ConsoleKey
            } else {
                SamplingIdentityKind::SuperGrokSession
            }
        }
    }
}

/// Live sampling wire value for JSON (`supergrok_session` | `console_key`).
pub fn live_sampling_wire(kind: SamplingIdentityKind) -> &'static str {
    match kind {
        SamplingIdentityKind::SuperGrokSession => "supergrok_session",
        SamplingIdentityKind::ConsoleKey => "console_key",
    }
}

/// Gap wire value for JSON (stable, no spaces).
pub fn prepaid_gap_wire(gap: ConsoleTeamPrepaidGap) -> &'static str {
    match gap {
        ConsoleTeamPrepaidGap::MissingManagementKey => "no_management_key",
        ConsoleTeamPrepaidGap::MissingTeamId => "no_management_team_id",
        ConsoleTeamPrepaidGap::Loading => "loading_team_prepaid",
        ConsoleTeamPrepaidGap::Unavailable => "team_prepaid_unavailable",
    }
}

/// Postpaid gap wire value for JSON (stable, no spaces).
pub fn postpaid_gap_wire(gap: ConsoleTeamPostpaidGap) -> &'static str {
    gap.as_wire()
}

/// Build CLI report + `/limits`-style snapshot from hermetic parts (no I/O).
pub fn build_limits_cli_from_parts(
    live: SamplingIdentityKind,
    live_role: Option<&str>,
    principals: &[PrincipalLimitsInput],
    console_key_available: bool,
    console_prepaid_cents: Option<i64>,
    console_prepaid_gap: ConsoleTeamPrepaidGap,
    notes: Vec<String>,
) -> (LimitsCliReport, LimitsSnapshot) {
    build_limits_cli_from_parts_with_postpaid(
        live,
        live_role,
        principals,
        console_key_available,
        console_prepaid_cents,
        console_prepaid_gap,
        None,
        ConsoleTeamPostpaidGap::MissingManagementKey,
        notes,
    )
}

/// Same as [`build_limits_cli_from_parts`] with optional Management postpaid preview.
pub fn build_limits_cli_from_parts_with_postpaid(
    live: SamplingIdentityKind,
    live_role: Option<&str>,
    principals: &[PrincipalLimitsInput],
    console_key_available: bool,
    console_prepaid_cents: Option<i64>,
    console_prepaid_gap: ConsoleTeamPrepaidGap,
    console_postpaid: Option<ConsoleTeamPostpaidMeter>,
    console_postpaid_gap: ConsoleTeamPostpaidGap,
    notes: Vec<String>,
) -> (LimitsCliReport, LimitsSnapshot) {
    let snap = if principals.is_empty() {
        LimitsSnapshot::from_billing(None, None, live)
    } else if principals.len() == 1 {
        let p = &principals[0];
        let mut s = LimitsSnapshot::from_billing(p.balance.as_ref(), p.autotopup.as_ref(), live);
        if !live.is_console() {
            s.live_principal_label = live_role
                .map(str::to_owned)
                .or_else(|| p.role_label.clone());
        }
        s
    } else {
        LimitsSnapshot::from_principals(principals, live, live_role)
    };
    // Single-slot `from_billing` labels the section "SuperGrok"; refresh
    // the discovered role from `live_principal_label` after that write.
    let snap = snap
        .with_console_balance_cents(console_prepaid_cents)
        .with_console_prepaid_gap(console_prepaid_gap)
        .with_console_postpaid(console_postpaid)
        .with_console_postpaid_gap(console_postpaid_gap)
        .with_console_key_available(console_key_available || live.is_console())
        .infer_discovered_identities_from_slots();
    // Flat-poll honesty from process S1 history (defaults false when cold).
    let snap = attach_flat_poll_from_history(snap);

    let report = report_from_snapshot(&snap, notes);
    (report, snap)
}

/// Set flat-poll honesty flags from process poll history (not test-only).
///
/// Product path for `/limits` and `limits --json`. History empty → flags stay
/// false. Does not invent inference counters. Observed Build / extras flags
/// come from the same series so honesty copy does not overclaim.
pub fn attach_flat_poll_from_history(snap: LimitsSnapshot) -> LimitsSnapshot {
    let ev = xai_grok_shell::auth::flat_poll_evidence_from_history();
    snap.with_flat_poll_unproven_debit(ev.unproven)
        .with_flat_poll_observed_meters(ev.observed_build, ev.observed_extras)
}

/// Build a machine-readable CLI report from a `/limits` snapshot (no I/O).
///
/// Used by `grok limits --json` and in-TUI `/limits --json` (scrollback dump).
/// No `meter_source` pin: Design A spend-order chrome. Tests stay hermetic
/// (they do not read the operator sidecar).
pub fn report_from_snapshot(snap: &LimitsSnapshot, notes: Vec<String>) -> LimitsCliReport {
    report_from_snapshot_with_meter_source(snap, notes, None)
}

/// Same as [`report_from_snapshot`], with JSON `active_driver_label` honoring
/// the same `meter_source` pin as human CLI and TUI **Active:**.
///
/// Wire `active_driver` stays Design A spend-order. Combined is only when
/// remaining is across distinct SuperGrok identities. grok-oss limits JSON
/// is a client printout, not xAI billing truth. Do not invent remaining. Do
/// not call any pool used up.
pub fn report_from_snapshot_with_meter_source(
    snap: &LimitsSnapshot,
    notes: Vec<String>,
    meter_source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
) -> LimitsCliReport {
    let role_from_label = |label: &str| -> Option<String> {
        label
            .strip_prefix("SuperGrok (")
            .and_then(|s| s.strip_suffix(')'))
            .map(str::to_owned)
    };
    let mut principals = vec![principal_cli(
        &snap.primary,
        role_from_label(&snap.primary.label).or_else(|| snap.live_principal_label.clone()),
    )];
    for extra in &snap.extra_principals {
        principals.push(principal_cli(extra, role_from_label(&extra.label)));
    }

    // Slice 3: machine-readable notes get the same honesty phrases as human
    // `/limits` body (dedupe if a collector already pushed the same text).
    // Dual poll fail + shared-pool fill lines match human format_limits_detail.
    let mut notes = notes;
    for honesty in crate::views::limits_snapshot::dual_poll_honesty_notes_for_snapshot(snap) {
        if !notes.iter().any(|n| n == &honesty) {
            notes.push(honesty);
        }
    }
    for honesty in honesty_notes_for_snapshot(snap) {
        if !notes.iter().any(|n| n == &honesty) {
            notes.push(honesty);
        }
    }

    let driver = active_spend_driver_from_snapshot(snap);
    LimitsCliReport {
        schema_version: SCHEMA_VERSION,
        live_sampling: live_sampling_wire(snap.live_identity),
        live_sampling_label: snap.live_sampling_line(),
        live_principal_role: snap.live_principal_label.clone(),
        active_driver: driver.as_wire(),
        active_driver_label: active_driver_line_for_snapshot_with_meter_source(snap, meter_source),
        flat_poll_unproven_debit: snap.flat_poll_unproven_debit,
        flat_poll_observed_build: snap.flat_poll_observed_build,
        flat_poll_observed_extras: snap.flat_poll_observed_extras,
        supergrok: SuperGrokCliSection {
            principals,
            shared_unified_pool: snap.shared_unified_supergrok_pool,
        },
        discovered_identities: snap.discovered_identities.clone(),
        console: ConsoleCliSection {
            key_available: snap.console.key_available,
            is_live: snap.console.is_live,
            team_prepaid_usd: snap.console.balance_cents.map(|c| c.abs() as f64 / 100.0),
            team_prepaid_gap: if snap.console.balance_cents.is_some() {
                None
            } else {
                Some(prepaid_gap_wire(snap.console.prepaid_gap))
            },
            team_postpaid_period_total_usd: snap
                .console
                .postpaid
                .as_ref()
                .map(|p| p.period_total_cents.abs() as f64 / 100.0),
            team_postpaid_oauth_class_usd: snap
                .console
                .postpaid
                .as_ref()
                .map(|p| p.oauth_class_cents.abs() as f64 / 100.0),
            team_postpaid_api_class_usd: snap
                .console
                .postpaid
                .as_ref()
                .map(|p| p.api_class_cents.abs() as f64 / 100.0),
            team_default_credits_usd: snap
                .console
                .postpaid
                .as_ref()
                .and_then(|p| p.default_credits_cents.map(|c| c.abs() as f64 / 100.0)),
            team_postpaid_gap: if snap.console.postpaid.is_some() {
                None
            } else {
                Some(postpaid_gap_wire(snap.console.postpaid_gap))
            },
            team_usage_series_oauth_class_usd: snap
                .console
                .usage_series
                .as_ref()
                .map(|s| s.oauth_class_usd),
            team_usage_series_api_class_usd: snap
                .console
                .usage_series
                .as_ref()
                .map(|s| s.api_class_usd),
            team_usage_series_start_time: snap
                .console
                .usage_series
                .as_ref()
                .map(|s| s.start_time.clone()),
            team_usage_series_end_time: snap
                .console
                .usage_series
                .as_ref()
                .map(|s| s.end_time.clone()),
        },
        notes,
    }
}

fn principal_cli(
    p: &crate::views::limits_snapshot::PrincipalLimitsSlot,
    role: Option<String>,
) -> PrincipalCliMeter {
    PrincipalCliMeter {
        label: p.label.clone(),
        role,
        included_used_pct: p.included.as_ref().map(|i| i.used_pct),
        included_remaining_pct: p.included.as_ref().map(|i| i.remaining_pct_floored()),
        period_label: p.included.as_ref().map(|i| i.period_label),
        next_reset: p
            .included
            .as_ref()
            .and_then(|i| i.next_reset_display.clone()),
        dollar_extras_usd: p
            .dollar_extras
            .as_ref()
            .map(|d| d.balance_cents.abs() as f64 / 100.0),
        dollar_extras_observed: p.dollar_extras_observed,
        // From snapshot (CreditBalance after FetchBilling / live collect).
        // Live CLI collect may also `apply_grok_build_usage_pcts` by index.
        grok_build_usage_pct: p.grok_build_usage_pct,
        poll_succeeded: p.poll_succeeded,
        included_source: p.included_source.as_wire(),
        poll_error_class: p.poll_error_class,
    }
}

/// Attach Grok Build `productUsage` % onto CLI principals by index order.
///
/// Hermetic helper for `grok limits --json` after a live credits poll, and for
/// tests. In-TUI `/limits --json` prefers values already on the snapshot from
/// cached [`CreditBalance::grok_build_usage_pct`] (set when FetchBilling saw
/// productUsage). Does not invent percents: only copies from `build_pcts_by_index`.
pub fn apply_grok_build_usage_pcts(
    report: &mut LimitsCliReport,
    build_pcts_by_index: &[Option<f64>],
) {
    for (i, pct) in build_pcts_by_index.iter().enumerate() {
        if let (Some(p), Some(pct)) = (report.supergrok.principals.get_mut(i), pct) {
            p.grok_build_usage_pct = Some(*pct);
        }
    }
}

/// Human multi-line body (same shape as in-TUI `/limits` detail).
pub fn format_limits_human(snap: &LimitsSnapshot, notes: &[String]) -> String {
    format_limits_human_with_meter_source(snap, notes, None)
}

/// Human `/limits` body with a named meter-source pin on the **Active:** line.
pub fn format_limits_human_with_meter_source(
    snap: &LimitsSnapshot,
    notes: &[String],
    meter_source: Option<xai_grok_shell::auth::limits_pins::MeterSource>,
) -> String {
    let mut out = format_limits_detail_with_meter_source(snap, meter_source);
    // Honesty notes already appear in the body via format_limits_detail;
    // skip duplicates when also present in the CLI notes list (JSON still
    // keeps them for machine consumers).
    let extra: Vec<&str> = notes
        .iter()
        .map(String::as_str)
        .filter(|n| !out.contains(n))
        .collect();
    if !extra.is_empty() {
        out.push_str("\n\nNotes:\n");
        for n in extra {
            out.push_str("  - ");
            out.push_str(n);
            out.push('\n');
        }
    }
    // Ensure trailing newline for shell friendliness.
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Pretty-print the CLI JSON report (same shape as `grok limits --json`).
pub fn format_limits_json_pretty(report: &LimitsCliReport) -> Result<String> {
    let mut s = serde_json::to_string_pretty(report)?;
    if !s.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}

/// Write report as JSON or human text.
pub fn write_limits_output(
    report: &LimitsCliReport,
    snap: &LimitsSnapshot,
    json: bool,
    writer: &mut impl Write,
) -> Result<()> {
    if json {
        writer.write_all(format_limits_json_pretty(report)?.as_bytes())?;
    } else {
        write!(
            writer,
            "{}",
            format_limits_human_with_meter_source(
                snap,
                &report.notes,
                xai_grok_shell::auth::limits_pins::load_limits_pins().meter_source,
            )
        )?;
    }
    Ok(())
}

fn postpaid_meter_from_snapshot(
    m: &xai_grok_shell::auth::LimitsSnapshotManagement,
) -> Option<ConsoleTeamPostpaidMeter> {
    let team = m.team_id.as_deref().filter(|s| !s.is_empty())?;
    let period_total_cents = m.postpaid_period_total_cents?;
    Some(ConsoleTeamPostpaidMeter::from_preview(
        &xai_grok_shell::auth::ConsoleTeamPostpaidPreview {
            team_id: team.to_owned(),
            period_total_cents,
            oauth_class_cents: m.postpaid_oauth_class_cents.unwrap_or(0),
            api_class_cents: m.postpaid_api_class_cents.unwrap_or(0),
            other_class_cents: m.postpaid_other_class_cents.unwrap_or(0),
            default_credits_cents: m.postpaid_default_credits_cents,
            default_credits_issued_cents: m.postpaid_default_credits_issued_cents,
            billing_cycle_year: m.postpaid_billing_cycle_year,
            billing_cycle_month: m.postpaid_billing_cycle_month,
        },
    ))
}

fn usage_series_from_snapshot(
    m: &xai_grok_shell::auth::LimitsSnapshotManagement,
) -> Option<ConsoleTeamUsageSeriesSummary> {
    let team = m.team_id.as_deref().filter(|s| !s.is_empty())?;
    let start = m.usage_series_start.clone()?;
    let end = m.usage_series_end.clone()?;
    Some(ConsoleTeamUsageSeriesSummary::from_series(
        &xai_grok_shell::auth::ConsoleTeamUsageSeries {
            team_id: team.to_owned(),
            start_time: start,
            end_time: end,
            timezone: m
                .usage_series_timezone
                .clone()
                .unwrap_or_else(|| "Etc/GMT".into()),
            rows: Vec::new(),
            oauth_class_usd: m.usage_series_oauth_usd.unwrap_or(0.0),
            api_class_usd: m.usage_series_api_usd.unwrap_or(0.0),
            other_class_usd: m.usage_series_other_usd.unwrap_or(0.0),
            limit_reached: m.usage_series_limit_reached.unwrap_or(false),
        },
    ))
}

/// Map a hub snapshot identity into pager [`CreditBalance`] (no secrets).
fn credit_balance_from_snapshot_identity(
    row: &xai_grok_shell::auth::LimitsSnapshotIdentity,
) -> CreditBalance {
    let included_usage_known = row.usage_pct.is_some();
    let usage_pct = row.usage_pct.unwrap_or(0.0).clamp(0.0, 100.0);
    let period_end_at = row.period_end.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    });
    let period_end_display = period_end_at.map(|dt| {
        dt.with_timezone(&chrono::Local)
            .format("%B %-d, %H:%M")
            .to_string()
    });
    CreditBalance {
        usage_pct,
        effective_usage_pct: usage_pct,
        period_end_display,
        period_end_at,
        pay_as_you_go: false,
        on_demand_cap_cents: None,
        on_demand_used_cents: Some(0),
        prepaid_balance_cents: row.extras_cents,
        period_type: row.period_type.clone(),
        is_unified_billing_user: row.is_unified_billing_user,
        grok_build_usage_pct: row.grok_build_usage_pct,
        included_usage_known,
    }
}

/// Map shell billing config into pager [`CreditBalance`] (shared fields only).
pub fn credit_balance_from_billing_config(
    c: &xai_grok_shell::extensions::billing::BillingConfig,
) -> CreditBalance {
    let limit = c.monthly_limit.as_ref().map(|v| v.val).unwrap_or(0);
    let used = c.used.as_ref().map(|v| v.val).unwrap_or(0);
    let has_credit_pct = c.credit_usage_percent.is_some();
    let (included_opt, _) = xai_grok_shell::extensions::billing::included_usage_and_period_end(c);
    let included_usage_known = included_opt.is_some();
    let usage_pct = included_opt.map(|pct| pct.clamp(0.0, 100.0)).unwrap_or(0.0);
    let period_end_raw = c
        .current_period
        .as_ref()
        .and_then(|p| p.end.clone())
        .or_else(|| c.billing_period_end.clone());
    let period_end_at = period_end_raw.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    });
    let period_end_display = period_end_at.map(|dt| {
        dt.with_timezone(&chrono::Local)
            .format("%B %-d, %H:%M")
            .to_string()
    });
    let on_demand_val = c.on_demand_cap.as_ref().map(|v| v.val).unwrap_or(0);
    let pay_as_you_go = on_demand_val > 0;
    let on_demand_cap_cents = if on_demand_val > 0 {
        Some(on_demand_val)
    } else {
        None
    };
    let on_demand_used_cents = c
        .on_demand_used
        .as_ref()
        .map(|v| v.val)
        .unwrap_or_else(|| (used - limit).max(0));
    let effective_usage_pct = if !included_usage_known {
        0.0
    } else if on_demand_val > 0 {
        if usage_pct >= 100.0 {
            (on_demand_used_cents as f64 / on_demand_val as f64 * 100.0).min(100.0)
        } else if has_credit_pct {
            usage_pct
        } else {
            let total_budget = limit + on_demand_val;
            if total_budget > 0 {
                (used as f64 / total_budget as f64 * 100.0).min(100.0)
            } else {
                0.0
            }
        }
    } else {
        usage_pct
    };
    let period_type = c
        .current_period
        .as_ref()
        .and_then(|p| p.period_type.clone());
    CreditBalance {
        usage_pct,
        effective_usage_pct,
        period_end_display,
        period_end_at,
        pay_as_you_go,
        on_demand_cap_cents,
        on_demand_used_cents: Some(on_demand_used_cents),
        prepaid_balance_cents: c.prepaid_balance.as_ref().map(|v| v.val),
        period_type,
        is_unified_billing_user: c.is_unified_billing_user,
        grok_build_usage_pct: xai_grok_shell::extensions::billing::grok_build_usage_percent(c),
        included_usage_known,
    }
}

/// Collect live principal + meters (network + disk). Safe for agent shell use.
pub async fn collect_limits_report() -> Result<(LimitsCliReport, LimitsSnapshot)> {
    let grok_home = xai_grok_shell::util::grok_home::grok_home();
    collect_limits_report_at(&grok_home).await
}

async fn collect_limits_report_at(grok_home: &Path) -> Result<(LimitsCliReport, LimitsSnapshot)> {
    let mut notes: Vec<String> = Vec::new();

    // Config (disk): preferred method + proxy base for SuperGrok billing.
    let (preferred_console_primary, proxy_base) =
        match xai_grok_shell::config::load_effective_config_disk_only() {
            Ok(raw) => match xai_grok_shell::agent::config::Config::new_from_toml_cfg(&raw) {
                Ok(cfg) => {
                    let preferred = cfg.grok_com_config.preferred_method;
                    let console_primary =
                        xai_grok_shell::auth::preferred_is_console_primary(preferred);
                    (console_primary, cfg.endpoints.proxy_url())
                }
                Err(e) => {
                    notes.push(format!("config parse warning: {e}"));
                    (
                        false,
                        xai_grok_shell::agent::config::EndpointsConfig::default().proxy_url(),
                    )
                }
            },
            Err(e) => {
                notes.push(format!("config load warning: {e}"));
                (
                    false,
                    xai_grok_shell::agent::config::EndpointsConfig::default().proxy_url(),
                )
            }
        };

    let dual = xai_grok_shell::auth::collect_dual_auth_status(grok_home);
    let session_present = dual.session_present;
    let console_present = dual.console_key_paths_present();
    let console_key_available = console_present;

    // SuperGrok credits via the flock snapshot hub. Explicit collect is
    // ForceRefresh: this process fetches once if it holds exclusive flock,
    // otherwise waits on the leader and reuses that snapshot.
    let targets = xai_grok_shell::auth::load_supergrok_billing_poll_targets(grok_home);
    let active_id = xai_grok_shell::auth::active_supergrok_identity_id(grok_home);
    let listings = xai_grok_shell::auth::read_auth_json(&grok_home.join("auth.json"))
        .map(|map| xai_grok_shell::auth::list_supergrok_principal_listings(&map))
        .unwrap_or_default();

    // identity_id → balance from hub snapshot (leader HTTP or follower file).
    // Process included cache only (ranking helpers); do **not** mark exhaust
    // memos here — CLI is a read-only report path, not hop policy.
    let mut balances: std::collections::BTreeMap<String, CreditBalance> =
        std::collections::BTreeMap::new();
    // identity_id → Grok Build productUsage % when present on wire.
    let mut build_usage: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    let mut active_included_full = false;

    let now_ms = xai_grok_shell::auth::limits_snapshot_hub::now_unix_ms();
    let home_for_fetch = grok_home.to_path_buf();
    let proxy_for_fetch = proxy_base.clone();
    let (hub_role, hub_doc) =
        xai_grok_shell::extensions::billing::collect_billing_via_snapshot_hub(
            grok_home,
            xai_grok_shell::auth::LimitsSnapshotMode::ForceRefresh,
            now_ms,
            || {
                let home_for_fetch = home_for_fetch.clone();
                let proxy_for_fetch = proxy_for_fetch.clone();
                async move {
                    if should_clear_management_meter_caches(
                        management_meter_cache_policy_for_explicit_limits_collect(),
                        xai_grok_shell::auth::resolve_management_api_key_default().is_some(),
                    ) {
                        xai_grok_shell::auth::clear_console_team_billing_meter_caches();
                    }
                    xai_grok_shell::extensions::billing::fetch_supergrok_credits_snapshot_document(
                        &home_for_fetch,
                        &proxy_for_fetch,
                        None,
                        None,
                        None,
                    )
                    .await
                }
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let _ = hub_role;

    for row in &hub_doc.identities {
        if row.poll_outcome == xai_grok_shell::auth::POLL_OUTCOME_OK {
            if let Some(pct) = row.usage_pct {
                if active_id.as_deref() == Some(row.identity_id.as_str()) && pct >= 100.0 {
                    active_included_full = true;
                }
            }
            if let Some(build_pct) = row.grok_build_usage_pct {
                build_usage.insert(row.identity_id.clone(), build_pct);
            }
            if row.usage_pct.is_some() || row.extras_cents.is_some() {
                balances.insert(
                    row.identity_id.clone(),
                    credit_balance_from_snapshot_identity(row),
                );
            } else {
                notes.push(format!(
                    "SuperGrok billing for {} returned no config",
                    row.identity_id
                ));
            }
        } else if row.poll_outcome != xai_grok_shell::auth::POLL_OUTCOME_NEVER {
            let err_text = match row.poll_outcome.as_str() {
                xai_grok_shell::auth::POLL_OUTCOME_AUTH => "auth failed",
                xai_grok_shell::auth::POLL_OUTCOME_NETWORK => "network error",
                _ => "billing poll failed",
            };
            let listing = listings.iter().find(|p| p.identity_id == row.identity_id);
            let note = if let Some(p) = listing {
                xai_grok_shell::auth::format_supergrok_billing_fail_note(
                    p.role_label,
                    &p.fingerprint,
                    err_text,
                )
            } else {
                xai_grok_shell::auth::format_supergrok_billing_fail_note(
                    "unknown",
                    short_id(&row.identity_id),
                    err_text,
                )
            };
            notes.push(note);
        }
    }

    if session_present && targets.is_empty() {
        notes.push("SuperGrok session present but no pollable token (re-run `grok login`)".into());
    }
    if !session_present && !console_present {
        notes.push("No SuperGrok session or console key configured (`grok login`)".into());
    }

    // Console team prepaid / business credits remaining (Management API).
    // Distinct from SuperGrok $ extras and from inference XAI_API_KEY.
    // Key alone is enough: team id may come from config/env or key validation.
    //
    // Explicit `grok limits` collect: bust ≤60s process cache so dollars are
    // not stuck from a warm background poll. TUI background FetchBilling still
    // honors the TTL (see `management_meter_cache_policy_for_background_billing_poll`);
    // TUI explicit `/limits` open uses the same ForceRefresh clear (see
    // `management_meter_cache_policy_for_explicit_limits_open`).
    // App state still keeps last-good cents on fetch None.
    let has_mgmt_key = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    // Management HTTP ran only inside the hub leader fetch (if this process
    // held the flock). Followers read prepaid/postpaid/series from the snapshot.
    let console_prepaid_cents = hub_doc
        .management
        .as_ref()
        .and_then(|m| m.prepaid_cents)
        .or_else(xai_grok_shell::auth::cached_console_team_prepaid_cents_default);
    if has_mgmt_key && console_prepaid_cents.is_none() {
        notes.push("console team prepaid fetch failed or empty".into());
    }
    // Team postpaid invoice preview (OAuth vs API class). Same Management key;
    // distinct meter family from prepaid remaining.
    let console_postpaid = hub_doc
        .management
        .as_ref()
        .and_then(postpaid_meter_from_snapshot)
        .or_else(|| {
            xai_grok_shell::auth::cached_console_team_postpaid_default()
                .map(|m| ConsoleTeamPostpaidMeter::from_preview(&m))
        });
    if has_mgmt_key && console_postpaid.is_none() {
        notes.push("console team postpaid preview fetch failed or empty".into());
    }
    // Re-check team after fetch (discovery may have filled process cache).
    let has_mgmt_team = xai_grok_shell::auth::resolve_management_team_id_default().is_some();
    let console_prepaid_gap = if console_prepaid_cents.is_some() {
        ConsoleTeamPrepaidGap::Loading // unused when cents present
    } else {
        ConsoleTeamPrepaidGap::after_billing_fetch(has_mgmt_key, has_mgmt_team)
    };
    let console_postpaid_gap = if console_postpaid.is_some() {
        ConsoleTeamPostpaidGap::Unavailable // unused when meter present
    } else {
        ConsoleTeamPostpaidGap::after_billing_fetch(has_mgmt_key, has_mgmt_team)
    };
    // Management usage series (POST analytics). Same key/team; optional window
    // summary for spend over time / by description class. Not prepaid, not
    // included SuperGrok period limits.
    let console_usage_series = hub_doc
        .management
        .as_ref()
        .and_then(usage_series_from_snapshot)
        .or_else(|| {
            xai_grok_shell::auth::cached_console_team_usage_series_default(
                xai_grok_shell::auth::USAGE_SERIES_DEFAULT_DAY_WINDOW,
            )
            .map(|s| ConsoleTeamUsageSeriesSummary::from_series(&s))
        });
    if has_mgmt_key && console_usage_series.is_none() {
        notes.push("console team usage series fetch failed or empty".into());
    }
    if console_prepaid_cents.is_none() {
        if let Some(hint) = xai_grok_shell::auth::console_team_prepaid_setup_note(
            !has_mgmt_key,
            has_mgmt_key && !has_mgmt_team,
        ) {
            notes.push(hint);
        }
    }
    // Postpaid gap note only when prepaid already has data (avoid double setup
    // wall) or when key is set but postpaid specifically failed.
    if console_postpaid.is_none() {
        if console_prepaid_cents.is_some() {
            if let Some(hint) = xai_grok_shell::auth::console_team_postpaid_setup_note(
                !has_mgmt_key,
                has_mgmt_key && !has_mgmt_team,
            ) {
                notes.push(hint);
            }
        } else if has_mgmt_key && has_mgmt_team {
            notes.push("console team postpaid (OAuth vs API) unavailable this poll".into());
        }
    }

    // Existing exhaust memo (from prior TUI/session) **or** just-polled
    // included ≥100% with dual-auth ready — prediction only, no memo write.
    let session_out =
        xai_grok_shell::auth::supergrok_out_of_allowance_with_console_ready(grok_home)
            || (dual.dual_auth_ready() && active_included_full);
    let live = predict_live_sampling(
        session_present,
        console_present,
        preferred_console_primary,
        session_out,
    );

    let live_role = if live.is_console() {
        None
    } else {
        active_id.as_ref().and_then(|aid| {
            listings
                .iter()
                .find(|p| &p.identity_id == aid)
                .map(|p| p.role_label)
        })
    };

    // Build principal inputs: active first when multi.
    let mut ordered = listings;
    if let Some(ref aid) = active_id {
        ordered.sort_by_key(|p| if &p.identity_id == aid { 0u8 } else { 1u8 });
    }

    // Track identity order alongside principal rows so productUsage % can
    // attach to the matching `grok limits --json` principal.
    let (principals, principal_ids): (Vec<PrincipalLimitsInput>, Vec<Option<String>>) =
        if ordered.is_empty() {
            // Single anonymous SuperGrok section when we have a balance without listing.
            if let Some((id, bal)) = balances.iter().next() {
                (
                    vec![PrincipalLimitsInput {
                        label: "SuperGrok".into(),
                        role_label: None,
                        balance: Some(bal.clone()),
                        autotopup: None,
                        included_billing_only: false,
                        poll_succeeded: Some(true),
                        poll_error_class: None,
                    }],
                    vec![Some(id.clone())],
                )
            } else {
                (Vec::new(), Vec::new())
            }
        } else {
            let mut rows = Vec::with_capacity(ordered.len());
            let mut ids = Vec::with_capacity(ordered.len());
            for p in &ordered {
                let role = if p.role_label == "business" {
                    xai_grok_shell::auth::SupergrokAccountRole::Business
                } else {
                    xai_grok_shell::auth::SupergrokAccountRole::Personal
                };
                let is_active = active_id.as_deref() == Some(p.identity_id.as_str());
                let (bal, included_only) = if let Some(b) = balances.get(&p.identity_id) {
                    (Some(b.clone()), false)
                } else {
                    (None, !is_active)
                };
                let outcome = xai_grok_shell::auth::supergrok_billing_poll_outcome(&p.identity_id);
                let (poll_succeeded, poll_error_class) = match outcome.kind {
                    xai_grok_shell::auth::SupergrokBillingPollOutcomeKind::Ok => (Some(true), None),
                    xai_grok_shell::auth::SupergrokBillingPollOutcomeKind::AuthFailed => {
                        (Some(false), Some("auth"))
                    }
                    xai_grok_shell::auth::SupergrokBillingPollOutcomeKind::OtherFailed => {
                        (Some(false), outcome.error_class)
                    }
                    xai_grok_shell::auth::SupergrokBillingPollOutcomeKind::Never => {
                        // Balance present without outcome → live this collect
                        // before outcome writers ran; empty → cold/fill path.
                        if bal.is_some() && !included_only {
                            (Some(true), None)
                        } else {
                            (Some(false), None)
                        }
                    }
                };
                rows.push(PrincipalLimitsInput {
                    label: xai_grok_shell::auth::principal_limits_label(role),
                    role_label: Some(p.role_label.to_string()),
                    balance: bal,
                    autotopup: None,
                    included_billing_only: included_only,
                    poll_succeeded,
                    poll_error_class,
                });
                ids.push(Some(p.identity_id.clone()));
            }
            (rows, ids)
        };

    let (report, snap) = build_limits_cli_from_parts_with_postpaid(
        live,
        live_role,
        &principals,
        console_key_available,
        console_prepaid_cents,
        console_prepaid_gap,
        console_postpaid,
        console_postpaid_gap,
        notes,
    );
    // Attach usage series after base build so existing hermetic helpers stay
    // unchanged; rebuild report so JSON includes series fields.
    let (report, snap) = match console_usage_series {
        Some(series) => {
            let snap = snap.with_console_usage_series(Some(series));
            let report = report_from_snapshot_with_meter_source(
                &snap,
                report.notes,
                xai_grok_shell::auth::limits_pins::load_limits_pins().meter_source,
            );
            (report, snap)
        }
        None => (report, snap),
    };
    // Overlay stored SuperGrok roles + fingerprints (no secrets). Slot
    // inference has no JWT; doctor listings are the source of truth.
    let snap = snap.with_discovered_identities(DiscoveredIdentities::from_dual_auth(&dual));
    let mut report = report_from_snapshot_with_meter_source(
        &snap,
        report.notes,
        xai_grok_shell::auth::limits_pins::load_limits_pins().meter_source,
    );
    let build_pcts: Vec<Option<f64>> = principal_ids
        .iter()
        .map(|id| id.as_ref().and_then(|i| build_usage.get(i).copied()))
        .collect();
    apply_grok_build_usage_pcts(&mut report, &build_pcts);
    Ok((report, snap))
}

fn short_id(id: &str) -> &str {
    let t = id.trim();
    if t.len() <= 12 {
        t
    } else {
        // identity ids are not secrets but keep log-ish lines short
        &t[..12]
    }
}

/// Run `grok limits` / `grok limits --json` / named pin words / `grok limits multipoll`.
pub async fn run(args: LimitsArgs) -> Result<()> {
    match args.command {
        None | Some(LimitsCommand::Refresh) => {
            let (report, snap) = collect_limits_report().await?;
            write_limits_output(&report, &snap, args.json, &mut std::io::stdout().lock())?;
            Ok(())
        }
        Some(LimitsCommand::StaySupergrok) => {
            let msg = apply_limits_named_action(LimitsNamedAction::StaySupergrok)
                .map_err(|e| anyhow::anyhow!(e))?;
            writeln!(std::io::stdout().lock(), "{msg}")?;
            Ok(())
        }
        Some(LimitsCommand::UseConsole) => {
            let msg = apply_limits_named_action(LimitsNamedAction::UseConsole)
                .map_err(|e| anyhow::anyhow!(e))?;
            writeln!(std::io::stdout().lock(), "{msg}")?;
            Ok(())
        }
        Some(LimitsCommand::Meter { source }) => {
            let msg = apply_limits_named_action(LimitsNamedAction::Meter(source))
                .map_err(|e| anyhow::anyhow!(e))?;
            writeln!(std::io::stdout().lock(), "{msg}")?;
            Ok(())
        }
        Some(LimitsCommand::Multipoll(mp)) => run_multipoll(mp).await,
    }
}

// ---------------------------------------------------------------------------
// Limits-first path certainty (Slice E2)
//
// Pure checker for `grok limits --json` when limits-first is on. Does not
// rewrite rank / ExhaustedAll. Live CLI is flaky for CI; unit tests own the
// contract; `just check-limits-first-live` runs the same rules after rebuild.
// ---------------------------------------------------------------------------

/// Config flags that `limits --json` does not always surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitsFirstPathCheckContext {
    /// `[auth] auto_use_included_limits` (limits before credits).
    pub auto_use_included_limits: bool,
    /// `preferred_method = api_key` (operator pin: console first by design).
    pub preferred_is_api_key: bool,
}

/// Outcome of [`check_limits_first_path_json`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitsFirstPathCheck {
    /// Invariant holds (or no claim required under this config / meters).
    Ok,
    /// Limits-first not active; checker does not apply.
    Skipped { reason: &'static str },
    /// Console is live primary while SuperGrok included weekly used is still
    /// below 100% under limits-first. Path is wrong.
    Fail { message: String },
}

impl LimitsFirstPathCheck {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok | Self::Skipped { .. })
    }

    pub fn is_fail(&self) -> bool {
        matches!(self, Self::Fail { .. })
    }
}

/// Pure path gate for limits-first (plan C1 / C3).
///
/// When `auto_use_included_limits` is on and preferred is **not** `api_key`:
/// if any SuperGrok principal reports included weekly used **below 100%**,
/// live sampling must be SuperGrok session and `console.isLive` must be false.
///
/// Skips when limits-first is off, when preferred pins console, or when no
/// included-used percent is present (cannot claim violation without a meter).
///
/// Does **not** assert extras-after-full (step 2); that needs a live window
/// when included weekly used is actually ≥ 100%.
pub fn check_limits_first_path_json(
    value: &serde_json::Value,
    ctx: LimitsFirstPathCheckContext,
) -> LimitsFirstPathCheck {
    if !ctx.auto_use_included_limits {
        return LimitsFirstPathCheck::Skipped {
            reason: "auto_use_included_limits is off",
        };
    }
    if ctx.preferred_is_api_key {
        return LimitsFirstPathCheck::Skipped {
            reason: "preferred_method=api_key pins console by design",
        };
    }

    let included_below_100 = any_supergrok_included_used_below_100(value);
    let Some(true) = included_below_100 else {
        return LimitsFirstPathCheck::Ok;
    };

    let live = value
        .get("liveSampling")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let console_is_live = value
        .get("console")
        .and_then(|c| c.get("isLive"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let console_is_primary = console_is_live || live == "console_key";
    if !console_is_primary {
        return LimitsFirstPathCheck::Ok;
    }

    let pct_hint = format_included_used_hint(value);
    LimitsFirstPathCheck::Fail {
        message: format!(
            "limits-first path broken: console is live primary while SuperGrok \
             included weekly used is still below 100%{pct_hint} \
             (liveSampling={live:?}, console.isLive={console_is_live}). \
             Expected SuperGrok session only (omit console from the chain)."
        ),
    }
}

/// Same as [`check_limits_first_path_json`] from a JSON text blob (live CLI).
pub fn check_limits_first_path_json_str(
    json: &str,
    ctx: LimitsFirstPathCheckContext,
) -> Result<LimitsFirstPathCheck, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("limits --json parse error: {e}"))?;
    Ok(check_limits_first_path_json(&value, ctx))
}

// ---------------------------------------------------------------------------
// Multipoll evidence harness (token economy proof)
//
// Pure classification of fixture / live sample series. Network I/O only in
// [`run_multipoll`]. Exit non-zero only on path failure (P1); free SuperGrok
// period flat (P2) is measurement and never fails the process alone.
// ---------------------------------------------------------------------------

/// Free SuperGrok period used-% series across multipoll samples (P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreePeriodSeriesClass {
    /// Free SuperGrok period used % stepped between samples (P2 stepped).
    Stepped,
    /// Free SuperGrok period used % stayed the same (P2 flat / unproven debit).
    Flat,
    /// Fewer than two samples or no `includedUsedPct` to compare.
    Insufficient,
}

impl FreePeriodSeriesClass {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Stepped => "stepped",
            Self::Flat => "flat",
            Self::Insufficient => "insufficient",
        }
    }

    pub fn free_period_stepped(self) -> bool {
        matches!(self, Self::Stepped)
    }
}

/// Combined multipoll verdict: path (P1) + free SuperGrok period series (P2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipollClassification {
    /// Limits-first path check across samples (any path fail → Fail).
    pub path: LimitsFirstPathCheck,
    /// Free SuperGrok period used % flat / stepped / insufficient.
    pub free_period: FreePeriodSeriesClass,
}

impl MultipollClassification {
    /// True when path is Ok or Skipped (not a path failure).
    pub fn path_ok(&self) -> bool {
        self.path.is_ok()
    }

    /// True only when free SuperGrok period used % stepped between samples.
    pub fn free_period_stepped(&self) -> bool {
        self.free_period.free_period_stepped()
    }

    /// Process exit code: **0** path ok/skip; **1** path fail only.
    /// Free SuperGrok period flat never forces non-zero by itself.
    pub fn exit_code(&self) -> i32 {
        if self.path.is_fail() { 1 } else { 0 }
    }
}

/// Classify multipoll fixture samples (no network).
///
/// **P1 path:** any sample that fails [`check_limits_first_path_json`] makes
/// the whole series Fail (console live under free SuperGrok period headroom).
/// Otherwise last non-Ok result wins when all are Skipped; all Ok → Ok.
///
/// **P2 included SuperGrok period limits:** see [`classify_free_period_series`].
pub fn classify_multipoll_samples(
    samples: &[serde_json::Value],
    ctx: LimitsFirstPathCheckContext,
) -> MultipollClassification {
    let path = classify_multipoll_path(samples, ctx);
    let free_period = classify_free_period_series(samples);
    MultipollClassification { path, free_period }
}

/// Path (P1) only across samples.
pub fn classify_multipoll_path(
    samples: &[serde_json::Value],
    ctx: LimitsFirstPathCheckContext,
) -> LimitsFirstPathCheck {
    if samples.is_empty() {
        return LimitsFirstPathCheck::Skipped {
            reason: "no multipoll samples",
        };
    }
    let mut last_skip: Option<LimitsFirstPathCheck> = None;
    let mut saw_ok = false;
    for s in samples {
        match check_limits_first_path_json(s, ctx) {
            fail @ LimitsFirstPathCheck::Fail { .. } => return fail,
            LimitsFirstPathCheck::Ok => saw_ok = true,
            skip @ LimitsFirstPathCheck::Skipped { .. } => last_skip = Some(skip),
        }
    }
    if saw_ok {
        LimitsFirstPathCheck::Ok
    } else {
        last_skip.unwrap_or(LimitsFirstPathCheck::Ok)
    }
}

/// Free SuperGrok period used % series (P2) from multipoll `limits --json` values.
///
/// For each SuperGrok principal label, collect `includedUsedPct` across samples
/// that report it. Any label with two or more known values that are not all
/// equal → **Stepped**. If every comparable series stays equal → **Flat**.
/// Fewer than two samples or no comparable series → **Insufficient**.
pub fn classify_free_period_series(samples: &[serde_json::Value]) -> FreePeriodSeriesClass {
    if samples.len() < 2 {
        return FreePeriodSeriesClass::Insufficient;
    }
    // label -> ordered list of includedUsedPct across samples (skip missing)
    let mut by_label: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();
    for s in samples {
        let Some(principals) = s
            .get("supergrok")
            .and_then(|g| g.get("principals"))
            .and_then(|p| p.as_array())
        else {
            continue;
        };
        for p in principals {
            let label = p
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("SuperGrok")
                .to_owned();
            if let Some(pct) = p.get("includedUsedPct").and_then(|v| v.as_f64()) {
                by_label.entry(label).or_default().push(pct);
            }
        }
    }
    let mut saw_comparable = false;
    for series in by_label.values() {
        if series.len() < 2 {
            continue;
        }
        saw_comparable = true;
        let first = series[0];
        if series.iter().any(|v| *v != first) {
            return FreePeriodSeriesClass::Stepped;
        }
    }
    if saw_comparable {
        FreePeriodSeriesClass::Flat
    } else {
        FreePeriodSeriesClass::Insufficient
    }
}

/// Compact multipoll sample fields extracted from `limits --json` (no secrets).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipollSampleFields {
    pub sample_index: usize,
    pub captured_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_sampling: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_is_live: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flat_poll_unproven_debit: Option<bool>,
    pub principals: Vec<MultipollPrincipalFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_oauth_class_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_postpaid_api_class_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipollPrincipalFields {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_used_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dollar_credits_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grok_build_usage_pct: Option<f64>,
}

/// Pull multipoll evidence fields from one `limits --json` value.
pub fn extract_multipoll_sample_fields(
    sample_index: usize,
    captured_at: &str,
    value: &serde_json::Value,
) -> MultipollSampleFields {
    let live_sampling = value
        .get("liveSampling")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let active_driver = value
        .get("activeDriver")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let console_is_live = value
        .get("console")
        .and_then(|c| c.get("isLive"))
        .and_then(|v| v.as_bool());
    let flat_poll_unproven_debit = value.get("flatPollUnprovenDebit").and_then(|v| v.as_bool());
    let team_postpaid_oauth_class_usd = value
        .get("console")
        .and_then(|c| c.get("teamPostpaidOauthClassUsd"))
        .and_then(|v| v.as_f64());
    let team_postpaid_api_class_usd = value
        .get("console")
        .and_then(|c| c.get("teamPostpaidApiClassUsd"))
        .and_then(|v| v.as_f64());
    let mut principals = Vec::new();
    if let Some(arr) = value
        .get("supergrok")
        .and_then(|s| s.get("principals"))
        .and_then(|p| p.as_array())
    {
        for p in arr {
            principals.push(MultipollPrincipalFields {
                label: p
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SuperGrok")
                    .to_owned(),
                role: p.get("role").and_then(|v| v.as_str()).map(str::to_owned),
                included_used_pct: p.get("includedUsedPct").and_then(|v| v.as_f64()),
                // SuperGrok dollar credits (wire still dollarExtrasUsd).
                dollar_credits_usd: p.get("dollarExtrasUsd").and_then(|v| v.as_f64()),
                grok_build_usage_pct: p.get("grokBuildUsagePct").and_then(|v| v.as_f64()),
            });
        }
    }
    MultipollSampleFields {
        sample_index,
        captured_at: captured_at.to_owned(),
        live_sampling,
        active_driver,
        console_is_live,
        flat_poll_unproven_debit,
        principals,
        team_postpaid_oauth_class_usd,
        team_postpaid_api_class_usd,
    }
}

/// Default multipoll output directory with UTC timestamp.
pub fn default_multipoll_out_dir() -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let base = std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|home| PathBuf::from(home).join(".agents/reports"))
        .unwrap_or_else(|| std::env::temp_dir().join("grok-limits-multipoll"));
    base.join(format!("limits-multipoll-{stamp}"))
}

/// Load path-check context from live dual-auth / preferred config (disk).
pub fn multipoll_path_context_from_disk() -> LimitsFirstPathCheckContext {
    let grok_home = xai_grok_shell::util::grok_home::grok_home();
    let dual = xai_grok_shell::auth::collect_dual_auth_status(&grok_home);
    let preferred_is_api_key = dual.preferred_method == Some("api_key");
    LimitsFirstPathCheckContext {
        auto_use_included_limits: dual.auto_use_included_limits,
        preferred_is_api_key,
    }
}

/// Live multipoll: N samples, sleep between ends, JSONL + summary, plain exit.
pub async fn run_multipoll(args: MultipollArgs) -> Result<()> {
    let n = args.samples.max(1);
    let sleep = Duration::from_secs(args.sleep_secs);
    let out_dir = args.out_dir.unwrap_or_else(default_multipoll_out_dir);
    std::fs::create_dir_all(&out_dir)?;
    let samples_path = out_dir.join("samples.jsonl");
    let summary_path = out_dir.join("summary.json");
    let fields_path = out_dir.join("fields.jsonl");

    let ctx = multipoll_path_context_from_disk();
    let mut raw_samples: Vec<serde_json::Value> = Vec::with_capacity(n);
    let mut fields_rows: Vec<MultipollSampleFields> = Vec::with_capacity(n);

    let mut samples_file = std::fs::File::create(&samples_path)?;
    let mut fields_file = std::fs::File::create(&fields_path)?;

    for i in 0..n {
        let captured_at = chrono::Utc::now().to_rfc3339();
        let (report, _snap) = collect_limits_report().await?;
        let json_text = format_limits_json_pretty(&report)?;
        let value: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|e| anyhow::anyhow!("serialize limits report: {e}"))?;

        // Full report line (evidence).
        writeln!(samples_file, "{}", serde_json::to_string(&value)?)?;
        let fields = extract_multipoll_sample_fields(i, &captured_at, &value);
        writeln!(fields_file, "{}", serde_json::to_string(&fields)?)?;
        fields_rows.push(fields);
        raw_samples.push(value);

        if i + 1 < n {
            eprintln!(
                "limits multipoll: sample {}/{} done; sleeping {}s before next",
                i + 1,
                n,
                args.sleep_secs
            );
            tokio::time::sleep(sleep).await;
        }
    }

    let class = classify_multipoll_samples(&raw_samples, ctx);
    let flat_ev = xai_grok_shell::auth::flat_poll_evidence_from_history();

    let path_status = match &class.path {
        LimitsFirstPathCheck::Ok => "ok".to_owned(),
        LimitsFirstPathCheck::Skipped { reason } => format!("skipped ({reason})"),
        LimitsFirstPathCheck::Fail { message } => format!("fail ({message})"),
    };
    let free_period_status = class.free_period.as_wire();

    let summary = serde_json::json!({
        "schemaVersion": "1",
        "kind": "limits_multipoll_summary",
        "samples": n,
        "sleepSecs": args.sleep_secs,
        "outDir": out_dir.display().to_string(),
        "samplesJsonl": samples_path.display().to_string(),
        "fieldsJsonl": fields_path.display().to_string(),
        "pathOk": class.path_ok(),
        "pathStatus": path_status,
        "freePeriodSeries": free_period_status,
        "freePeriodStepped": class.free_period_stepped(),
        "flatPollUnprovenDebit": flat_ev.unproven,
        "flatPollObservedBuild": flat_ev.observed_build,
        "flatPollObservedExtras": flat_ev.observed_extras,
        "autoUseIncludedLimits": ctx.auto_use_included_limits,
        "preferredIsApiKey": ctx.preferred_is_api_key,
        "sampleFields": fields_rows,
    });
    std::fs::write(
        &summary_path,
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;

    // Plain human summary on stdout.
    println!("limits multipoll summary");
    println!(
        "  samples:     {n} (sleep {}s between ends)",
        args.sleep_secs
    );
    println!("  out dir:     {}", out_dir.display());
    println!("  samples:     {}", samples_path.display());
    println!("  fields:      {}", fields_path.display());
    println!("  summary:     {}", summary_path.display());
    println!(
        "  P1 path:     {}",
        if class.path_ok() {
            format!("OK ({path_status})")
        } else {
            format!("FAIL ({path_status})")
        }
    );
    println!(
        "  P2 included SuperGrok period limits: {free_period_status}{}",
        if class.free_period == FreePeriodSeriesClass::Flat {
            " (flat is measurement only; not a path fail)"
        } else {
            ""
        }
    );
    println!(
        "  flatPollUnprovenDebit (process history): {}",
        flat_ev.unproven
    );
    if !class.path_ok() {
        eprintln!("limits multipoll: path failure (exit 1)");
        std::process::exit(1);
    }
    Ok(())
}

/// True when any SuperGrok principal has `includedUsedPct` strictly below 100.
/// `None` when no principal reports that field.
fn any_supergrok_included_used_below_100(value: &serde_json::Value) -> Option<bool> {
    let principals = value
        .get("supergrok")
        .and_then(|s| s.get("principals"))
        .and_then(|p| p.as_array())?;
    let mut saw_pct = false;
    for p in principals {
        if let Some(pct) = p.get("includedUsedPct").and_then(|v| v.as_f64()) {
            saw_pct = true;
            if pct < 100.0 {
                return Some(true);
            }
        }
    }
    if saw_pct { Some(false) } else { None }
}

fn format_included_used_hint(value: &serde_json::Value) -> String {
    let Some(principals) = value
        .get("supergrok")
        .and_then(|s| s.get("principals"))
        .and_then(|p| p.as_array())
    else {
        return String::new();
    };
    let mut parts = Vec::new();
    for p in principals {
        let label = p
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("SuperGrok");
        if let Some(pct) = p.get("includedUsedPct").and_then(|v| v.as_f64()) {
            parts.push(format!("{label}={pct}%"));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::credit_bar::AutoTopupInfo;

    fn bal(pct: f64) -> CreditBalance {
        CreditBalance {
            usage_pct: pct,
            effective_usage_pct: pct,
            period_end_display: Some("August 1, 12:00".into()),
            period_end_at: None,
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: Some(1500),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            is_unified_billing_user: None,
            grok_build_usage_pct: None,
            included_usage_known: true,
        }
    }

    /// Named contract: explicit `grok limits` collect **and** TUI `/limits`
    /// open force-bust Management process caches; background FetchBilling
    /// honors TTL (no clear).
    #[test]
    fn management_meter_cache_policy_collect_force_background_honor_ttl() {
        assert_eq!(
            management_meter_cache_policy_for_explicit_limits_collect(),
            ManagementMeterCachePolicy::ForceRefresh,
            "explicit collect must force-refresh Management meters"
        );
        assert_eq!(
            management_meter_cache_policy_for_explicit_limits_open(),
            ManagementMeterCachePolicy::ForceRefresh,
            "TUI explicit /limits open must force-refresh (CLI parity)"
        );
        assert_eq!(
            management_meter_cache_policy_for_background_billing_poll(),
            ManagementMeterCachePolicy::HonorProcessTtl,
            "FetchBilling / background poll must not clear process caches"
        );
        assert_ne!(
            management_meter_cache_policy_for_explicit_limits_collect(),
            management_meter_cache_policy_for_background_billing_poll(),
            "collect and background poll policies must differ"
        );
        assert_eq!(
            management_meter_cache_policy_for_explicit_limits_open(),
            management_meter_cache_policy_for_explicit_limits_collect(),
            "TUI open and CLI collect share ForceRefresh"
        );
    }

    /// Named contract: clear only on ForceRefresh + management key present.
    #[test]
    fn should_clear_management_meter_caches_force_with_key_only() {
        assert!(
            should_clear_management_meter_caches(ManagementMeterCachePolicy::ForceRefresh, true),
            "ForceRefresh + key must clear"
        );
        assert!(
            !should_clear_management_meter_caches(ManagementMeterCachePolicy::ForceRefresh, false),
            "ForceRefresh without key must not clear"
        );
        assert!(
            !should_clear_management_meter_caches(
                ManagementMeterCachePolicy::HonorProcessTtl,
                true
            ),
            "background HonorProcessTtl must not clear even with key"
        );
        assert!(
            !should_clear_management_meter_caches(
                ManagementMeterCachePolicy::HonorProcessTtl,
                false
            ),
            "background without key must not clear"
        );
    }

    /// Named contract: explicit /limits queues silent FetchBilling when
    /// management key is present (even if app last-good prepaid is warm) or
    /// when dual SuperGrok sibling included is empty.
    #[test]
    fn should_queue_silent_billing_on_explicit_limits_when_key_or_sibling() {
        assert!(
            should_queue_silent_billing_on_explicit_limits(true, false),
            "management key → always queue after force-bust"
        );
        assert!(
            should_queue_silent_billing_on_explicit_limits(false, true),
            "sibling empty → queue SuperGrok billing refresh"
        );
        assert!(
            should_queue_silent_billing_on_explicit_limits(true, true),
            "key + sibling both true → queue"
        );
        assert!(
            !should_queue_silent_billing_on_explicit_limits(false, false),
            "no key and no sibling need → do not queue FetchBilling"
        );
    }

    /// Named contract: postpaid live-fetch rides billing when management key
    /// is present (TTL still honored unless caches cleared first).
    #[test]
    fn should_live_fetch_postpaid_with_billing_when_management_key() {
        assert!(
            should_live_fetch_console_team_postpaid_with_billing(true),
            "key present → live-call postpaid (cache or HTTP)"
        );
        assert!(
            !should_live_fetch_console_team_postpaid_with_billing(false),
            "no key → skip postpaid Management call"
        );
    }

    /// Named contract (P2): usage series live-fetch rides the same
    /// FetchBilling / silent `/limits` refresh path as postpaid when a
    /// management key is present (TTL still honored unless caches cleared).
    #[test]
    fn should_live_fetch_usage_series_with_billing_when_management_key() {
        assert!(
            should_live_fetch_console_team_usage_series_with_billing(true),
            "key present → live-call usage series into process cache (cache or HTTP)"
        );
        assert!(
            !should_live_fetch_console_team_usage_series_with_billing(false),
            "no key → skip usage series Management call"
        );
        // Same gate as postpaid so series is not a thinner CLI-only path.
        assert_eq!(
            should_live_fetch_console_team_usage_series_with_billing(true),
            should_live_fetch_console_team_postpaid_with_billing(true),
        );
        assert_eq!(
            should_live_fetch_console_team_usage_series_with_billing(false),
            should_live_fetch_console_team_postpaid_with_billing(false),
        );
    }

    #[test]
    fn predict_live_session_when_only_session() {
        assert_eq!(
            predict_live_sampling(true, false, false, false),
            SamplingIdentityKind::SuperGrokSession
        );
    }

    #[test]
    fn predict_live_console_when_only_console() {
        assert_eq!(
            predict_live_sampling(false, true, false, false),
            SamplingIdentityKind::ConsoleKey
        );
    }

    #[test]
    fn predict_live_console_when_preferred_console_primary() {
        assert_eq!(
            predict_live_sampling(true, true, true, false),
            SamplingIdentityKind::ConsoleKey
        );
    }

    #[test]
    fn predict_live_console_when_session_out_of_allowance() {
        assert_eq!(
            predict_live_sampling(true, true, false, true),
            SamplingIdentityKind::ConsoleKey
        );
    }

    #[test]
    fn predict_live_session_when_dual_and_session_has_headroom() {
        assert_eq!(
            predict_live_sampling(true, true, false, false),
            SamplingIdentityKind::SuperGrokSession
        );
    }

    #[test]
    fn human_output_starts_with_live_sampling_and_meters() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(24.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[input],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.starts_with("Live sampling: SuperGrok session (personal)"),
            "live line first: {human}"
        );
        assert!(
            human.contains("Included weekly allowance: 24% used · 76% remaining"),
            "included meter: {human}"
        );
        assert!(
            human.contains("SuperGrok dollar credits: $15"),
            "SuperGrok dollar credits: {human}"
        );
        assert!(
            !human.to_ascii_lowercase().contains("extras"),
            "/limits human text must not teach extras as a nickname: {human}"
        );
        assert!(human.contains("Console API:"), "console section: {human}");
        assert!(
            human.contains("Team prepaid remaining: no management key"),
            "honest gap: {human}"
        );
        // Slice 3: poll-reading honesty in body; no forbidden burn overclaim.
        assert!(
            human.contains("billing poll reading"),
            "honesty note for SuperGrok included %: {human}"
        );
        assert!(
            !human.contains("using SuperGrok limits"),
            "must not overclaim burn from %: {human}"
        );
        // Honesty is in the body once; Notes: must not duplicate it.
        assert_eq!(
            human.matches("billing poll reading").count(),
            1,
            "honesty note must not double in Notes: {human}"
        );
        // Named contract: no secrets
        assert!(!human.contains("Bearer "));
        assert!(!human.contains("eyJ"));
        assert!(!human.contains("xai-"));
        // JSON notes still carry honesty for machine consumers.
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("billing poll reading")),
            "json notes must include honesty: {:?}",
            report.notes
        );
    }

    /// Named contract (Slice 1): `flat_poll_unproven_debit` comes from process
    /// S1 poll history via [`attach_flat_poll_from_history`], not only the
    /// test-only `with_flat_poll_unproven_debit(true)` setter.
    #[test]
    #[serial_test::serial]
    fn limits_snapshot_sets_flat_poll_from_history_not_only_tests() {
        use crate::views::limits_honesty::flat_poll_unproven_debit_note;
        use chrono::{TimeZone, Utc};
        use xai_grok_shell::auth::{
            IncludedPollSample, clear_included_poll_history, record_included_poll_sample,
        };

        clear_included_poll_history();
        let sample = |secs: i64| IncludedPollSample {
            ts: Utc.timestamp_opt(secs, 0).single().expect("ts"),
            credit_usage_percent: 65.0,
            build_usage_percent: Some(54.0),
            prepaid_balance_cents: Some(10029),
        };
        // Two flat polls spanning > default 30s min window.
        record_included_poll_sample("team-surmount", sample(1_700_000_000));
        record_included_poll_sample("team-surmount", sample(1_700_000_090));

        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: Some("business".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        // build_limits_cli_from_parts attaches history (product path).
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            &[input],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert!(
            snap.flat_poll_unproven_debit,
            "product attach from history must set flat_poll_unproven_debit"
        );
        assert!(
            snap.flat_poll_observed_build && snap.flat_poll_observed_extras,
            "history with Build + extras must set observed flags"
        );
        assert!(
            report.flat_poll_unproven_debit,
            "limits --json must export flatPollUnprovenDebit for multipoll / ticket evidence"
        );
        assert!(
            report.flat_poll_observed_build && report.flat_poll_observed_extras,
            "limits --json must export observed Build/extras flags with flat-poll evidence"
        );
        let expected = flat_poll_unproven_debit_note(true, true);
        assert!(
            report.notes.iter().any(|n| n == &expected),
            "json notes must include flat-poll honesty from history: {:?}",
            report.notes
        );
        // Without history the flag must stay false (setter not required).
        clear_included_poll_history();
        let input2 = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: Some("business".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (_, snap_cold) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            &[input2],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert!(
            !snap_cold.flat_poll_unproven_debit,
            "cold history must not invent flat-poll evidence"
        );
        clear_included_poll_history();
    }

    /// Named contract (Slice 3 review): flat-poll note on CLI human + JSON notes,
    /// once each, no Notes: double.
    #[test]
    fn human_and_json_surface_flat_poll_note_once_no_dedupe_double() {
        use crate::views::limits_honesty::{
            NOTE_INCLUDED_PCT_IS_BILLING_POLL, flat_poll_unproven_debit_note,
        };

        xai_grok_shell::auth::clear_included_poll_history();
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (_, mut snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[input],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        // Explicit evidence (history may be cold in this unit test). Safe
        // default: included-only flat claim when observed flags not set.
        snap = snap
            .with_flat_poll_unproven_debit(true)
            .with_flat_poll_observed_meters(false, false);
        let report = report_from_snapshot(&snap, vec![]);
        assert!(
            report.flat_poll_unproven_debit,
            "report_from_snapshot must pass flat_poll_unproven_debit into limits --json"
        );
        assert!(
            !report.flat_poll_observed_build && !report.flat_poll_observed_extras,
            "observed flags must match snapshot (included-only window)"
        );
        let human = format_limits_human(&snap, &report.notes);
        let expected_flat = flat_poll_unproven_debit_note(false, false);

        // JSON notes: both honesty phrases once.
        let flat_in_notes = report.notes.iter().filter(|n| *n == &expected_flat).count();
        assert_eq!(
            flat_in_notes, 1,
            "json notes must include flat-poll note once: {:?}",
            report.notes
        );
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.as_str() == NOTE_INCLUDED_PCT_IS_BILLING_POLL),
            "json notes keep base honesty: {:?}",
            report.notes
        );

        // Human body: flat note once; no Notes: section double.
        assert!(
            human.contains(&expected_flat),
            "human body must include flat-poll note: {human}"
        );
        assert_eq!(
            human.matches("included debit is unproven").count(),
            1,
            "flat-poll note must not double under Notes:: {human}"
        );
        assert_eq!(
            human.matches("billing poll reading").count(),
            1,
            "base honesty must not double under Notes:: {human}"
        );
        // When only honesty notes are present, format_limits_human skips the
        // Notes: section entirely (all already in body).
        assert!(
            !human.contains("\n\nNotes:\n"),
            "honesty-only notes must not open a Notes: block: {human}"
        );
    }

    #[test]
    fn human_output_names_console_live_sampling() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: None,
            balance: Some(bal(90.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::ConsoleKey,
            None,
            &[input],
            true,
            Some(2500),
            ConsoleTeamPrepaidGap::Loading,
            vec![],
        );
        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.starts_with("Live sampling: console key"),
            "console live: {human}"
        );
        assert!(
            human.contains("Team prepaid remaining: $25"),
            "console team prepaid dollars: {human}"
        );
        assert_eq!(report.live_sampling, "console_key");
        assert_eq!(report.console.team_prepaid_usd, Some(25.0));
        assert!(report.console.is_live);
    }

    #[test]
    fn json_report_shape_and_no_secrets() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(42.5)),
            autotopup: Some(AutoTopupInfo {
                enabled: false,
                topup_amount_cents: None,
                max_amount_cents: None,
            }),
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[input],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingTeamId,
            vec!["note without secrets".into()],
        );
        let mut buf = Vec::new();
        write_limits_output(&report, &snap, true, &mut buf).expect("write json");
        let s = String::from_utf8(buf).expect("utf8");
        let v: serde_json::Value = serde_json::from_str(&s).expect("parse json");
        assert_eq!(v["schemaVersion"], "1");
        assert_eq!(v["liveSampling"], "supergrok_session");
        assert!(
            v["liveSamplingLabel"]
                .as_str()
                .unwrap_or("")
                .contains("SuperGrok session"),
            "label: {v}"
        );
        assert_eq!(v["livePrincipalRole"], "personal");
        assert_eq!(v["activeDriver"], "supergrok_free_period");
        assert!(
            v["activeDriverLabel"]
                .as_str()
                .unwrap_or("")
                .contains("included SuperGrok period limits"),
            "active driver label: {v}"
        );
        assert_eq!(v["supergrok"]["principals"][0]["includedUsedPct"], 42.5);
        assert_eq!(v["supergrok"]["principals"][0]["includedRemainingPct"], 58);
        assert_eq!(v["console"]["teamPrepaidGap"], "no_management_team_id");
        // teamPrepaidUsd omitted when unknown (skip_serializing_if)
        assert!(
            v["console"].get("teamPrepaidUsd").is_none(),
            "no prepaid usd when gap set: {v}"
        );
        // grokBuildUsagePct omitted until productUsage is applied from a live poll
        assert!(
            v["supergrok"]["principals"][0]
                .get("grokBuildUsagePct")
                .is_none(),
            "no invented Build %: {v}"
        );
        // No secret-looking keys
        let flat = s.to_ascii_lowercase();
        assert!(!flat.contains("access_token"));
        assert!(!flat.contains("api_key"));
        assert!(!flat.contains("authorization"));
        assert!(!flat.contains("eyj")); // JWT header base64
        assert!(!s.contains("sk-"));
    }

    /// Named contract (P3/P5): free period headroom + SuperGrok extras on account
    /// → `activeDriver` is free SuperGrok period (not extras, not console).
    #[test]
    fn limits_json_active_driver_free_period_with_extras_on_account() {
        let mut b = bal(6.0);
        b.prepaid_balance_cents = Some(10_029);
        let input = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(b),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            &[input],
            true,
            Some(34_000), // team prepaid known; not active driver
            ConsoleTeamPrepaidGap::Loading,
            vec![],
        );
        assert_eq!(report.active_driver, "supergrok_free_period");
        assert!(
            report
                .active_driver_label
                .contains("included SuperGrok period limits"),
            "label: {}",
            report.active_driver_label
        );
        assert_eq!(report.live_sampling, "supergrok_session");
        assert!(!report.console.is_live);
        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.contains("Active: included SuperGrok period limits"),
            "human must lead with active free period: {human}"
        );
        assert!(
            !human.contains("Active: SuperGrok extras") && !human.contains("Active: console key"),
            "must not claim extras/console active with free-period headroom: {human}"
        );
    }

    /// Named contract: free period full + extras → activeDriver SuperGrok extras.
    #[test]
    fn limits_json_active_driver_extras_afterburner() {
        let mut b = bal(100.0);
        b.prepaid_balance_cents = Some(453);
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: None,
            balance: Some(b),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            None,
            &[input],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(report.active_driver, "supergrok_extras");
        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.contains("Active: SuperGrok dollar credits"),
            "after-burner human: {human}"
        );
        assert!(
            !human.contains("Active: SuperGrok extras"),
            "/limits Active line must not teach extras as a nickname: {human}"
        );
    }

    /// Named contract: JSON `active_driver_label` honors the same
    /// `meter_source` pin as human CLI and TUI **Active:**. Wire
    /// `active_driver` stays Design A spend-order. grok-oss limits JSON is a
    /// client printout, not xAI billing truth. SuperGrok is paid. Do not
    /// invent remaining. Do not call any pool used up. Combined only when
    /// remaining is across distinct SuperGrok identities.
    #[test]
    fn json_active_driver_label_honors_meter_source_pin() {
        use crate::views::limits_snapshot::active_driver_line_for_snapshot_with_meter_source;
        use xai_grok_shell::auth::limits_pins::MeterSource;

        let mut b = bal(6.0);
        b.prepaid_balance_cents = Some(10_029);
        let input = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(b),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let (_, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("business"),
            &[input],
            true,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            vec![],
        );

        let dollars =
            report_from_snapshot_with_meter_source(&snap, vec![], Some(MeterSource::DollarCredits));
        assert_eq!(
            dollars.active_driver, "supergrok_free_period",
            "wire active_driver stays Design A spend-order"
        );
        assert_eq!(
            dollars.active_driver_label, "Active: SuperGrok dollar credits",
            "dollar-credits pin JSON label must match human Active:"
        );
        assert_eq!(
            dollars.active_driver_label,
            active_driver_line_for_snapshot_with_meter_source(
                &snap,
                Some(MeterSource::DollarCredits)
            ),
            "JSON label must match human CLI/TUI Active: for the same pin"
        );
        assert!(
            !dollars
                .active_driver_label
                .to_ascii_lowercase()
                .contains("extras"),
            "must not teach extras as a nickname: {}",
            dollars.active_driver_label
        );

        let console =
            report_from_snapshot_with_meter_source(&snap, vec![], Some(MeterSource::Console));
        assert_eq!(
            console.active_driver_label, "Active: console team prepaid / console API credits",
            "console pin JSON label must be complete American English: {}",
            console.active_driver_label
        );
        assert_eq!(console.active_driver, "supergrok_free_period");

        let included =
            report_from_snapshot_with_meter_source(&snap, vec![], Some(MeterSource::Included));
        assert_eq!(
            included.active_driver_label,
            "Active: included SuperGrok period limits"
        );

        let none = report_from_snapshot(&snap, vec![]);
        assert_eq!(
            none.active_driver_label, "Active: included SuperGrok period limits",
            "no pin stays Design A: {}",
            none.active_driver_label
        );

        let one_combined =
            report_from_snapshot_with_meter_source(&snap, vec![], Some(MeterSource::Combined));
        assert_eq!(
            one_combined.active_driver_label, "Active: included SuperGrok period limits",
            "combined pin plus one honest pool must not invent combined: {}",
            one_combined.active_driver_label
        );
        assert!(
            !one_combined
                .active_driver_label
                .to_ascii_lowercase()
                .contains("combined"),
            "combined pin with one honest pool must not name combined: {}",
            one_combined.active_driver_label
        );

        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(40.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(bal(10.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let dual = LimitsSnapshot::from_principals(
            &[personal, business],
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
        );
        let dual_report =
            report_from_snapshot_with_meter_source(&dual, vec![], Some(MeterSource::Combined));
        assert_eq!(
            dual_report.active_driver_label, "Active: combined",
            "combined pin plus two SuperGrok identities must name combined: {}",
            dual_report.active_driver_label
        );
        assert!(
            !dual_report.active_driver_label.contains("personal")
                && !dual_report.active_driver_label.contains("business"),
            "JSON combined must not flatten two identities into one workspace word: {}",
            dual_report.active_driver_label
        );
    }

    /// Named contract: wire productUsage Build % surfaces on limits JSON when set.
    #[test]
    fn json_report_includes_grok_build_usage_pct_when_applied() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (mut report, _snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[input],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        apply_grok_build_usage_pcts(&mut report, &[Some(61.2)]);
        assert_eq!(
            report.supergrok.principals[0].grok_build_usage_pct,
            Some(61.2)
        );
        let pretty = format_limits_json_pretty(&report).expect("json");
        let v: serde_json::Value = serde_json::from_str(&pretty).expect("parse");
        assert_eq!(v["supergrok"]["principals"][0]["includedUsedPct"], 65.0);
        assert_eq!(v["supergrok"]["principals"][0]["grokBuildUsagePct"], 61.2);
    }

    /// In-TUI `/limits --json` path: Build % on cached CreditBalance surfaces
    /// via report_from_snapshot (parity with CLI apply, no invent).
    #[test]
    fn report_from_snapshot_includes_grok_build_usage_pct_from_balance() {
        let mut balance = bal(65.0);
        balance.grok_build_usage_pct = Some(61.2);
        let input = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(balance),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, _snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[input],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(
            report.supergrok.principals[0].grok_build_usage_pct,
            Some(61.2),
            "snapshot path must carry Build % without apply_grok_build_usage_pcts"
        );
        let pretty = format_limits_json_pretty(&report).expect("json");
        let v: serde_json::Value = serde_json::from_str(&pretty).expect("parse");
        assert_eq!(v["supergrok"]["principals"][0]["grokBuildUsagePct"], 61.2);
    }

    /// Dual SuperGrok: Build % attaches by principal index (no swap); None omitted.
    #[test]
    fn apply_grok_build_usage_pcts_dual_principals_keeps_index_order() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(10.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(bal(90.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (mut report, _snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[personal, business],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        apply_grok_build_usage_pcts(&mut report, &[Some(10.0), Some(90.0)]);
        assert_eq!(report.supergrok.principals.len(), 2);
        assert_eq!(
            report.supergrok.principals[0].grok_build_usage_pct,
            Some(10.0),
            "personal index 0"
        );
        assert_eq!(
            report.supergrok.principals[1].grok_build_usage_pct,
            Some(90.0),
            "business index 1"
        );
        let pretty = format_limits_json_pretty(&report).expect("json");
        let v: serde_json::Value = serde_json::from_str(&pretty).expect("parse");
        assert_eq!(v["supergrok"]["principals"][0]["grokBuildUsagePct"], 10.0);
        assert_eq!(v["supergrok"]["principals"][1]["grokBuildUsagePct"], 90.0);

        // None slot stays omitted in JSON (skip_serializing_if).
        let (mut report2, _) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[
                PrincipalLimitsInput {
                    label: "SuperGrok (personal)".into(),
                    role_label: Some("personal".into()),
                    balance: Some(bal(10.0)),
                    autotopup: None,
                    included_billing_only: false,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
                PrincipalLimitsInput {
                    label: "SuperGrok (business)".into(),
                    role_label: Some("business".into()),
                    balance: Some(bal(20.0)),
                    autotopup: None,
                    included_billing_only: false,
                    poll_succeeded: None,
                    poll_error_class: None,
                },
            ],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        apply_grok_build_usage_pcts(&mut report2, &[Some(33.0), None]);
        assert_eq!(
            report2.supergrok.principals[0].grok_build_usage_pct,
            Some(33.0)
        );
        assert_eq!(report2.supergrok.principals[1].grok_build_usage_pct, None);
        let pretty2 = format_limits_json_pretty(&report2).expect("json");
        let v2: serde_json::Value = serde_json::from_str(&pretty2).expect("parse");
        assert_eq!(v2["supergrok"]["principals"][0]["grokBuildUsagePct"], 33.0);
        assert!(
            v2["supergrok"]["principals"][1]
                .get("grokBuildUsagePct")
                .is_none(),
            "None Build % omitted: {v2}"
        );
    }

    #[test]
    fn dual_principals_stack_in_report() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(10.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(CreditBalance {
                prepaid_balance_cents: None,
                ..bal(10.0)
            }),
            autotopup: None,
            included_billing_only: true,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[personal, business],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(report.supergrok.principals.len(), 2);
        let human = format_limits_human(&snap, &[]);
        assert!(human.contains("SuperGrok (personal):"), "{human}");
        assert!(human.contains("SuperGrok (business):"), "{human}");
    }

    /// Named contract (Slice A): both stored SuperGrok slots appear in
    /// `limits --json` as two principals plus a discovered-identities block
    /// (role + fingerprint, no secrets).
    #[test]
    fn limits_json_lists_two_supergrok_principals_when_both_slots_exist() {
        use crate::views::limits_snapshot::DiscoveredSupergrokSession;

        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(10.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(bal(20.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[personal, business],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(
            report.supergrok.principals.len(),
            2,
            "both SuperGrok slots must appear as principals"
        );
        let roles: Vec<Option<&str>> = report
            .supergrok
            .principals
            .iter()
            .map(|p| p.role.as_deref())
            .collect();
        assert!(
            roles.contains(&Some("personal")) && roles.contains(&Some("business")),
            "must name personal and business: {roles:?}"
        );
        assert_eq!(
            report.discovered_identities.supergrok_sessions.len(),
            2,
            "discovered identities must list both stored SuperGrok sessions: {:?}",
            report.discovered_identities
        );
        let discovered_roles: Vec<&str> = report
            .discovered_identities
            .supergrok_sessions
            .iter()
            .map(|s| s.role.as_str())
            .collect();
        assert!(
            discovered_roles.contains(&"personal") && discovered_roles.contains(&"business"),
            "discovered identities must name both roles: {discovered_roles:?}"
        );
        assert!(
            !report.discovered_identities.only_one_supergrok_session,
            "two slots is not a single SuperGrok session: {:?}",
            report.discovered_identities
        );
        assert!(
            report.discovered_identities.honesty.is_none(),
            "must not claim a single-session blind spot when both slots exist: {:?}",
            report.discovered_identities
        );

        let fp_p = xai_grok_shell::auth::fingerprint_session_token("personal-jwt-never-print");
        let fp_b = xai_grok_shell::auth::fingerprint_session_token("business-jwt-never-print");
        let fp_c = xai_grok_shell::auth::fingerprint_console_key("console-key-never-print");
        let snap = snap.with_discovered_identities(DiscoveredIdentities::from_sessions(
            vec![
                DiscoveredSupergrokSession {
                    role: "personal".into(),
                    fingerprint: Some(fp_p.clone()),
                    mode: Some("oidc".into()),
                },
                DiscoveredSupergrokSession {
                    role: "business".into(),
                    fingerprint: Some(fp_b.clone()),
                    mode: Some("oidc".into()),
                },
            ],
            vec![fp_c.clone()],
        ));
        let report = report_from_snapshot(&snap, report.notes);
        let pretty = format_limits_json_pretty(&report).expect("json");
        assert!(pretty.contains(&fp_p), "personal fingerprint: {pretty}");
        assert!(pretty.contains(&fp_b), "business fingerprint: {pretty}");
        assert!(pretty.contains(&fp_c), "console fingerprint: {pretty}");
        assert!(
            !pretty.contains("never-print") && !pretty.contains("personal-jwt"),
            "must not dump session secrets: {pretty}"
        );
        let human = format_limits_human(&snap, &report.notes);
        assert!(human.contains("Discovered identities"), "{human}");
        assert!(human.contains("SuperGrok (personal)"), "{human}");
        assert!(human.contains("SuperGrok (business)"), "{human}");
        assert!(human.contains(&fp_p) && human.contains(&fp_b), "{human}");
    }

    /// Named contract (Slice A): one stored SuperGrok session must not invent
    /// a Business / Team row. Honesty names the second grok-oss login.
    #[test]
    fn limits_json_honest_single_supergrok_session_cannot_see_team_plan() {
        let personal = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(42.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: Some(true),
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts(
            SamplingIdentityKind::SuperGrokSession,
            Some("personal"),
            &[personal],
            true,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(
            report.supergrok.principals.len(),
            1,
            "one stored session is one principal: {:?}",
            report.supergrok.principals
        );
        assert_eq!(
            report.supergrok.principals[0].role.as_deref(),
            Some("personal")
        );
        assert!(
            report
                .supergrok
                .principals
                .iter()
                .all(|p| p.role.as_deref() != Some("business")),
            "must not invent a Business principal: {:?}",
            report.supergrok.principals
        );
        assert!(
            report.discovered_identities.only_one_supergrok_session,
            "one slot must be marked as a single SuperGrok session: {:?}",
            report.discovered_identities
        );
        assert_eq!(
            report.discovered_identities.supergrok_sessions.len(),
            1,
            "must not invent a second SuperGrok session: {:?}",
            report.discovered_identities
        );
        assert_eq!(
            report.discovered_identities.supergrok_sessions[0].role,
            "personal"
        );
        assert!(
            report
                .discovered_identities
                .supergrok_sessions
                .iter()
                .all(|s| s.role != "business"),
            "must not invent a Business discovered row: {:?}",
            report.discovered_identities
        );
        let note = xai_grok_shell::auth::NOTE_SINGLE_SUPERGROK_SESSION_CANNOT_SEE_TEAM_PLAN;
        assert_eq!(
            report.discovered_identities.honesty.as_deref(),
            Some(note),
            "single-session honesty: {:?}",
            report.discovered_identities
        );
        let pretty = format_limits_json_pretty(&report).expect("json");
        let lower = pretty.to_ascii_lowercase();
        assert!(
            !lower.contains("\"role\": \"business\"") && !lower.contains("\"business\""),
            "JSON must not invent a Business / Team plan: {pretty}"
        );
        let human = format_limits_human(&snap, &report.notes);
        assert!(
            !human.contains("SuperGrok (business)"),
            "human /limits must not invent a Business row: {human}"
        );
        assert!(
            human.contains(note),
            "human /limits must say included SuperGrok period limits can only be checked for that login: {human}"
        );
        assert!(
            human.contains("Discovered identities"),
            "human /limits must surface discovered identities: {human}"
        );
    }

    /// Named contract (Slice 3 M3): postpaid OAuth/API under console family in
    /// `limits --json`, distinct from prepaid $ and SuperGrok extras; C6 honesty
    /// when SuperGrok live + OAuth dominates.
    #[test]
    fn limits_json_surfaces_postpaid_oauth_vs_api_and_c6_honesty() {
        use crate::views::limits_honesty::NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS;

        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: None,
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let postpaid = ConsoleTeamPostpaidMeter {
            period_total_cents: 20_756,
            oauth_class_cents: 20_176,
            api_class_cents: 580,
            other_class_cents: 0,
            default_credits_cents: Some(150_000),
        };
        let (report, snap) = build_limits_cli_from_parts_with_postpaid(
            SamplingIdentityKind::SuperGrokSession,
            None,
            &[input],
            true,
            Some(34_000), // prepaid remaining distinct from postpaid totals
            ConsoleTeamPrepaidGap::Loading,
            Some(postpaid),
            ConsoleTeamPostpaidGap::Unavailable,
            vec![],
        );
        assert_eq!(report.console.team_prepaid_usd, Some(340.0));
        assert_eq!(report.console.team_postpaid_period_total_usd, Some(207.56));
        assert_eq!(report.console.team_postpaid_oauth_class_usd, Some(201.76));
        assert_eq!(report.console.team_postpaid_api_class_usd, Some(5.80));
        assert_eq!(report.console.team_default_credits_usd, Some(1500.0));
        assert!(
            report.console.team_postpaid_gap.is_none(),
            "gap omitted when postpaid present"
        );
        assert!(
            report
                .notes
                .iter()
                .any(|n| n == NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS),
            "C6 honesty when SuperGrok live + OAuth dominates: {:?}",
            report.notes
        );
        assert!(
            report.notes.iter().any(|n| {
                n.contains("intent chrome")
                    || n.contains("spend-order driver")
                    || n.contains("not proof of which wallet")
            }),
            "intent-not-settlement honesty when SuperGrok live + team meters: {:?}",
            report.notes
        );

        let pretty = format_limits_json_pretty(&report).expect("json");
        let v: serde_json::Value = serde_json::from_str(&pretty).expect("parse");
        assert_eq!(v["console"]["teamPrepaidUsd"], 340.0);
        assert_eq!(v["console"]["teamPostpaidPeriodTotalUsd"], 207.56);
        assert_eq!(v["console"]["teamPostpaidOauthClassUsd"], 201.76);
        assert_eq!(v["console"]["teamPostpaidApiClassUsd"], 5.80);
        assert_eq!(v["console"]["teamDefaultCreditsUsd"], 1500.0);
        assert!(
            v["console"].get("teamPostpaidGap").is_none(),
            "no postpaid gap when meter present: {v}"
        );

        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.contains("Team postpaid OAuth / Grok Build class:")
                || human.contains("Team postpaid OAuth class:"),
            "human names OAuth class: {human}"
        );
        assert!(
            human.contains("Team postpaid API class:"),
            "human names API class: {human}"
        );
        assert!(
            human.contains("team Usage dollars") || human.contains("OAuth / Grok Build"),
            "human surfaces C6 honesty: {human}"
        );
        // Prepaid balance still distinct.
        assert!(
            human.contains("Team prepaid remaining: $340")
                || human.contains("Team prepaid remaining: $340.00"),
            "prepaid balance line still present: {human}"
        );
        // Item 5b: default credits own line, not folded into prepaid $340.
        assert!(
            human.contains(
                "Team default credits (dashboard allotment; not the prepaid wallet): $1500"
            ) || human.contains(
                "Team default credits (dashboard allotment; not the prepaid wallet): $1500.00"
            ),
            "default credits must be its own labeled line: {human}"
        );
        assert!(
            !human.contains("Team prepaid remaining: $1500"),
            "must not fold default credits into prepaid Balance line: {human}"
        );
    }

    /// Named contract (Item 5a/5b): usage series summary + default credits stay
    /// distinct from prepaid wallet dollars.
    #[test]
    fn limits_json_and_human_surface_usage_series_and_default_credits() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: None,
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let postpaid = ConsoleTeamPostpaidMeter {
            period_total_cents: 20_756,
            oauth_class_cents: 20_176,
            api_class_cents: 580,
            other_class_cents: 0,
            default_credits_cents: Some(150_000),
        };
        let series = ConsoleTeamUsageSeriesSummary {
            start_time: "2026-07-28 00:00:00".into(),
            end_time: "2026-08-04 00:00:00".into(),
            timezone: "Etc/GMT".into(),
            oauth_class_usd: 15.0,
            api_class_usd: 2.5,
            other_class_usd: 0.0,
            top_rows: vec![crate::views::limits_snapshot::ConsoleTeamUsageSeriesRow {
                label: "Grok Build OAuth grok-4.5-build".into(),
                class_wire: "oauth_grok_build",
                total_usd: 15.0,
            }],
            limit_reached: false,
        };
        let (report, snap) = build_limits_cli_from_parts_with_postpaid(
            SamplingIdentityKind::SuperGrokSession,
            None,
            &[input],
            true,
            Some(34_000),
            ConsoleTeamPrepaidGap::Loading,
            Some(postpaid),
            ConsoleTeamPostpaidGap::Unavailable,
            vec![],
        );
        let snap = snap.with_console_usage_series(Some(series));
        let report = report_from_snapshot(&snap, report.notes);

        assert_eq!(report.console.team_default_credits_usd, Some(1500.0));
        assert_eq!(report.console.team_usage_series_oauth_class_usd, Some(15.0));
        assert_eq!(report.console.team_usage_series_api_class_usd, Some(2.5));
        assert_eq!(
            report.console.team_usage_series_start_time.as_deref(),
            Some("2026-07-28 00:00:00")
        );

        let pretty = format_limits_json_pretty(&report).expect("json");
        let v: serde_json::Value = serde_json::from_str(&pretty).expect("parse");
        assert_eq!(v["console"]["teamDefaultCreditsUsd"], 1500.0);
        assert_eq!(v["console"]["teamUsageSeriesOauthClassUsd"], 15.0);
        assert_eq!(v["console"]["teamUsageSeriesApiClassUsd"], 2.5);
        assert_eq!(v["console"]["teamPrepaidUsd"], 340.0);

        let human = format_limits_human(&snap, &report.notes);
        assert!(
            human.contains("Team usage series"),
            "human names usage series: {human}"
        );
        assert!(
            human.contains("OAuth / Grok Build class: $15"),
            "series OAuth class: {human}"
        );
        assert!(
            human.contains("API-key class: $2.50") || human.contains("API-key class: $2.5"),
            "series API class: {human}"
        );
        assert!(
            human.contains("Team default credits (dashboard allotment; not the prepaid wallet)"),
            "default credits full label: {human}"
        );
        assert!(
            human.contains("Team prepaid remaining: $340")
                || human.contains("Team prepaid remaining: $340.00"),
            "prepaid stays $340: {human}"
        );
    }

    /// Named contract: no management key → postpaid gap in JSON (no invented $).
    #[test]
    fn limits_json_postpaid_gap_when_no_management_key() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: None,
            balance: Some(bal(10.0)),
            autotopup: None,
            included_billing_only: false,
            poll_succeeded: None,
            poll_error_class: None,
        };
        let (report, snap) = build_limits_cli_from_parts_with_postpaid(
            SamplingIdentityKind::SuperGrokSession,
            None,
            &[input],
            false,
            None,
            ConsoleTeamPrepaidGap::MissingManagementKey,
            None,
            ConsoleTeamPostpaidGap::MissingManagementKey,
            vec![],
        );
        assert_eq!(report.console.team_postpaid_gap, Some("no_management_key"));
        assert!(report.console.team_postpaid_period_total_usd.is_none());
        assert!(report.console.team_postpaid_oauth_class_usd.is_none());
        assert!(
            report.console.team_postpaid_api_class_usd.is_none(),
            "api class must be absent when postpaid gap is set"
        );
        let human = format_limits_human(&snap, &[]);
        assert!(
            human.contains("Team postpaid: needs management key"),
            "human postpaid gap: {human}"
        );
        // No C6 without OAuth evidence (C6 phrase, not the always-on license note
        // that also mentions team Usage dollars as the real settlement surface).
        assert!(
            !report
                .notes
                .iter()
                .any(|n| n.contains("can still move team Usage dollars")),
            "must not invent C6 without postpaid: {:?}",
            report.notes
        );
    }

    // ----- Slice E2: limits-first path certainty checker ---------------------

    fn limits_first_auto_ctx() -> LimitsFirstPathCheckContext {
        LimitsFirstPathCheckContext {
            auto_use_included_limits: true,
            preferred_is_api_key: false,
        }
    }

    fn sample_limits_json(
        live_sampling: &str,
        console_is_live: bool,
        included_used_pct: Option<f64>,
    ) -> serde_json::Value {
        let mut principal = serde_json::json!({
            "label": "SuperGrok (business)",
            "role": "business",
            "dollarExtrasObserved": true,
            "dollarExtrasUsd": 100.29
        });
        if let Some(pct) = included_used_pct {
            principal["includedUsedPct"] = serde_json::json!(pct);
            principal["includedRemainingPct"] =
                serde_json::json!(((100.0 - pct).floor() as i64).max(0));
        }
        serde_json::json!({
            "schemaVersion": "1",
            "liveSampling": live_sampling,
            "liveSamplingLabel": format!("Live sampling: {live_sampling}"),
            "supergrok": {
                "principals": [principal],
                "sharedUnifiedPool": true
            },
            "console": {
                "keyAvailable": true,
                "isLive": console_is_live,
                "teamPrepaidUsd": 340.0
            }
        })
    }

    /// Named contract (E2 / C1): SuperGrok session primary while included
    /// weekly used is below 100% under limits-first → Ok.
    #[test]
    fn check_limits_first_ok_when_supergrok_live_and_included_below_100() {
        let v = sample_limits_json("supergrok_session", false, Some(66.0));
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert_eq!(r, LimitsFirstPathCheck::Ok, "{r:?}");
        assert!(r.is_ok());
    }

    /// Named contract (E2 / C1 fail): console live primary while included
    /// weekly used is still below 100% under limits-first → Fail.
    #[test]
    fn check_limits_first_fails_when_console_live_and_included_below_100() {
        let v = sample_limits_json("console_key", true, Some(66.0));
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert!(r.is_fail(), "expected Fail, got {r:?}");
        match r {
            LimitsFirstPathCheck::Fail { message } => {
                assert!(
                    message.contains("console is live primary"),
                    "message names console: {message}"
                );
                assert!(
                    message.contains("below 100"),
                    "message names included weekly: {message}"
                );
                assert!(
                    message.contains("66"),
                    "message includes observed pct: {message}"
                );
            }
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    /// Named contract: liveSampling=console_key alone is enough to fail even
    /// if console.isLive were inconsistent/false.
    #[test]
    fn check_limits_first_fails_on_console_key_wire_even_if_is_live_false() {
        let v = sample_limits_json("console_key", false, Some(42.0));
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert!(r.is_fail(), "{r:?}");
    }

    /// Named contract: console.isLive alone fails even if liveSampling still
    /// says SuperGrok (inconsistent / wrong primary).
    #[test]
    fn check_limits_first_fails_on_console_is_live_even_if_wire_says_session() {
        let v = sample_limits_json("supergrok_session", true, Some(10.0));
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert!(r.is_fail(), "{r:?}");
    }

    /// Named contract: preferred_method=api_key pins console by design → skip.
    #[test]
    fn check_limits_first_skips_when_preferred_is_api_key() {
        let v = sample_limits_json("console_key", true, Some(50.0));
        let r = check_limits_first_path_json(
            &v,
            LimitsFirstPathCheckContext {
                auto_use_included_limits: true,
                preferred_is_api_key: true,
            },
        );
        assert_eq!(
            r,
            LimitsFirstPathCheck::Skipped {
                reason: "preferred_method=api_key pins console by design"
            }
        );
        assert!(r.is_ok());
    }

    /// Named contract: limits-first off → skip (classic dual-auth path).
    #[test]
    fn check_limits_first_skips_when_auto_use_off() {
        let v = sample_limits_json("console_key", true, Some(50.0));
        let r = check_limits_first_path_json(
            &v,
            LimitsFirstPathCheckContext {
                auto_use_included_limits: false,
                preferred_is_api_key: false,
            },
        );
        assert_eq!(
            r,
            LimitsFirstPathCheck::Skipped {
                reason: "auto_use_included_limits is off"
            }
        );
    }

    /// Named contract: included weekly used ≥ 100% → checker does not claim
    /// C1 violation (step 2/3 may put console primary when extras gone).
    #[test]
    fn check_limits_first_ok_when_included_full_even_if_console_live() {
        let v = sample_limits_json("console_key", true, Some(100.0));
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert_eq!(r, LimitsFirstPathCheck::Ok, "{r:?}");
    }

    /// Named contract: no includedUsedPct on wire → do not invent a fail.
    #[test]
    fn check_limits_first_ok_when_included_pct_unknown() {
        let v = sample_limits_json("console_key", true, None);
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert_eq!(r, LimitsFirstPathCheck::Ok, "{r:?}");
    }

    /// Named contract: any principal with included used below 100% is enough.
    #[test]
    fn check_limits_first_uses_any_principal_below_100() {
        let v = serde_json::json!({
            "schemaVersion": "1",
            "liveSampling": "console_key",
            "liveSamplingLabel": "Live sampling: console key",
            "supergrok": {
                "principals": [
                    {
                        "label": "SuperGrok (personal)",
                        "includedUsedPct": 100.0,
                        "dollarExtrasObserved": false
                    },
                    {
                        "label": "SuperGrok (business)",
                        "includedUsedPct": 66.0,
                        "dollarExtrasObserved": false
                    }
                ],
                "sharedUnifiedPool": true
            },
            "console": { "keyAvailable": true, "isLive": true }
        });
        let r = check_limits_first_path_json(&v, limits_first_auto_ctx());
        assert!(r.is_fail(), "any principal below 100 must fail: {r:?}");
    }

    /// Named contract: JSON string entry point used by live recipes.
    #[test]
    fn check_limits_first_path_json_str_parses_and_passes() {
        let v = sample_limits_json("supergrok_session", false, Some(66.0));
        let s = serde_json::to_string_pretty(&v).expect("serialize");
        let r = check_limits_first_path_json_str(&s, limits_first_auto_ctx()).expect("parse");
        assert_eq!(r, LimitsFirstPathCheck::Ok);
    }

    #[test]
    fn check_limits_first_path_json_str_rejects_garbage() {
        let err = check_limits_first_path_json_str("not-json", limits_first_auto_ctx())
            .expect_err("garbage");
        assert!(err.contains("parse error"), "{err}");
    }

    /// Live operator gate (not default CI). Set `LIMITS_FIRST_JSON` to a path
    /// holding `grok limits --json` output (or the JSON text itself when the
    /// path is `-` and stdin is not available — prefer a temp file).
    ///
    /// Env:
    /// - `LIMITS_FIRST_JSON` — required path to JSON file
    /// - `LIMITS_FIRST_AUTO_USE` — default `1` (on). Set `0` to force skip path.
    /// - `LIMITS_FIRST_PREFERRED_API_KEY` — default `0`. Set `1` if preferred is
    ///   api_key (expect skip / no fail).
    ///
    /// Run via `just check-limits-first-live` (rebuild first).
    #[test]
    #[ignore = "live: set LIMITS_FIRST_JSON to limits --json output path"]
    fn live_check_limits_first_from_env_json() {
        let path = std::env::var("LIMITS_FIRST_JSON")
            .expect("LIMITS_FIRST_JSON must point at a file with `grok limits --json` output");
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read LIMITS_FIRST_JSON={path}: {e}"));
        let auto = std::env::var("LIMITS_FIRST_AUTO_USE")
            .map(|v| v != "0" && v != "false")
            .unwrap_or(true);
        let preferred_api = std::env::var("LIMITS_FIRST_PREFERRED_API_KEY")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        let ctx = LimitsFirstPathCheckContext {
            auto_use_included_limits: auto,
            preferred_is_api_key: preferred_api,
        };
        let result = check_limits_first_path_json_str(&json, ctx).unwrap_or_else(|e| panic!("{e}"));
        match result {
            LimitsFirstPathCheck::Ok => {
                eprintln!("limits-first path check: Ok");
            }
            LimitsFirstPathCheck::Skipped { reason } => {
                eprintln!("limits-first path check: Skipped ({reason})");
            }
            LimitsFirstPathCheck::Fail { message } => {
                panic!("{message}");
            }
        }
    }

    // ----- Multipoll pure classification (fixtures, no network) -------------

    /// Named contract: console live under free SuperGrok period headroom →
    /// multipoll path fail (P1).
    #[test]
    fn multipoll_path_fail_when_console_live_under_free_period_headroom() {
        let a = sample_limits_json("console_key", true, Some(6.0));
        let b = sample_limits_json("console_key", true, Some(6.0));
        let class = classify_multipoll_samples(&[a, b], limits_first_auto_ctx());
        assert!(class.path.is_fail(), "path must fail: {:?}", class.path);
        assert!(!class.path_ok());
        assert_eq!(class.exit_code(), 1);
        assert_eq!(class.free_period, FreePeriodSeriesClass::Flat);
    }

    /// Named contract: free SuperGrok period used % stays flat across samples
    /// → P2 flat (measurement). Path OK still exit 0.
    #[test]
    fn multipoll_free_period_flat_measurement_exit_zero_when_path_ok() {
        let a = sample_limits_json("supergrok_session", false, Some(6.0));
        let b = sample_limits_json("supergrok_session", false, Some(6.0));
        let class = classify_multipoll_samples(&[a, b], limits_first_auto_ctx());
        assert_eq!(class.path, LimitsFirstPathCheck::Ok);
        assert!(class.path_ok());
        assert_eq!(class.free_period, FreePeriodSeriesClass::Flat);
        assert!(!class.free_period_stepped());
        assert_eq!(
            class.exit_code(),
            0,
            "flat free SuperGrok period limits must not fail exit"
        );
    }

    /// Named contract: free SuperGrok period used % steps → P2 stepped.
    #[test]
    fn multipoll_free_period_stepped_measurement() {
        let a = sample_limits_json("supergrok_session", false, Some(6.0));
        let b = sample_limits_json("supergrok_session", false, Some(7.0));
        let class = classify_multipoll_samples(&[a, b], limits_first_auto_ctx());
        assert!(class.path_ok());
        assert_eq!(class.free_period, FreePeriodSeriesClass::Stepped);
        assert!(class.free_period_stepped());
        assert_eq!(class.exit_code(), 0);
    }

    /// Named contract: path OK + free SuperGrok period flat still exit 0
    /// (P1/P2 separation — never fail only because period stayed flat).
    #[test]
    fn multipoll_path_ok_plus_flat_still_exit_zero() {
        let a = sample_limits_json("supergrok_session", false, Some(66.0));
        let b = sample_limits_json("supergrok_session", false, Some(66.0));
        let class = classify_multipoll_samples(&[a, b], limits_first_auto_ctx());
        assert!(class.path_ok());
        assert_eq!(class.free_period, FreePeriodSeriesClass::Flat);
        assert_eq!(class.exit_code(), 0);
    }

    /// Named contract: one sample alone cannot prove free-period stepped/flat.
    #[test]
    fn multipoll_free_period_insufficient_with_one_sample() {
        let a = sample_limits_json("supergrok_session", false, Some(6.0));
        assert_eq!(
            classify_free_period_series(&[a]),
            FreePeriodSeriesClass::Insufficient
        );
    }

    /// Named contract: path fail on any sample fails the multipoll series.
    #[test]
    fn multipoll_path_fail_if_any_sample_is_console_under_headroom() {
        let good = sample_limits_json("supergrok_session", false, Some(6.0));
        let bad = sample_limits_json("console_key", true, Some(6.0));
        let class = classify_multipoll_samples(&[good, bad], limits_first_auto_ctx());
        assert!(class.path.is_fail(), "{:?}", class.path);
        assert_eq!(class.exit_code(), 1);
        // Free period still flat (both 6.0) — measurement only.
        assert_eq!(class.free_period, FreePeriodSeriesClass::Flat);
    }

    #[test]
    fn multipoll_extract_fields_reads_active_driver_and_credits() {
        let mut v = sample_limits_json("supergrok_session", false, Some(6.0));
        v["activeDriver"] = serde_json::json!("supergrok_free_period");
        v["flatPollUnprovenDebit"] = serde_json::json!(true);
        v["console"]["teamPostpaidOauthClassUsd"] = serde_json::json!(123.45);
        let f = extract_multipoll_sample_fields(0, "2026-08-08T00:00:00Z", &v);
        assert_eq!(f.live_sampling.as_deref(), Some("supergrok_session"));
        assert_eq!(f.active_driver.as_deref(), Some("supergrok_free_period"));
        assert_eq!(f.console_is_live, Some(false));
        assert_eq!(f.flat_poll_unproven_debit, Some(true));
        assert_eq!(f.team_postpaid_oauth_class_usd, Some(123.45));
        assert_eq!(f.principals[0].included_used_pct, Some(6.0));
        assert_eq!(f.principals[0].dollar_credits_usd, Some(100.29));
    }
}
