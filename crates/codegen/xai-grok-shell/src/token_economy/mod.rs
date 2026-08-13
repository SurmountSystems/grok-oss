//! Token Economy product: implement-effort policy, period pacing, double-entry ledger.
//!
//! Durable books live in [`crate::grok_oss`] (`$GROK_HOME/grok_oss.db`).
//! Economic **context** soft-cap remains under `[ui] economic_mode`.

pub mod config;
pub mod implement_effort;
pub mod ledger;
pub mod period_pacing;
pub mod reconcile;

pub use config::{
    DEFAULT_DESIRED_IMPLEMENT_EFFORT, DEFAULT_MAX_IMPLEMENT_EFFORT, TokenEconomyConfig,
    TokenEconomyConfigError, implement_effort_policy_active, resolve_grok_oss_database_path,
    token_economy_from_disk, token_economy_from_toml,
};
pub use implement_effort::{
    ImplementEffortRewrite, apply_implement_effort_policy, is_implement_command,
    parse_implement_effort,
};
pub use ledger::{
    IngestStats, LocalBookSummary, LocalUsageEvent, RemoteMeterSample, ingest_all_sessions_usage,
    ingest_usage_jsonl, insert_local_usage_event, insert_reconciliation_run,
    insert_remote_meter_sample, latest_remote_sample, parse_usage_jsonl, summarize_local_book,
    try_insert_local_usage_event, try_insert_remote_meter_sample,
};
pub use period_pacing::{
    PeriodPacing, compute_period_pacing, derive_period_start_from_end_and_type, parse_rfc3339_utc,
    period_pacing_from_bounds, resolve_period_start,
};
pub use reconcile::{
    DoubleEntryReport, RemoteBookSummary, SuperGrokPeriodContext, format_double_entry_report,
    format_limits_spend_section, gap_honesty_line, ticks_to_usd,
};

/// Build a double-entry report from local DB + optional remote fields + SuperGrok period.
///
/// Opens/uses the store fail-open: if DB open fails, local book is empty and path is None.
/// Does **not** walk session trees (use [`refresh_local_book_from_sessions`] on
/// `/spend` / turn-end so `/limits` format stays cheap).
pub fn build_double_entry_report(
    cfg: &TokenEconomyConfig,
    remote: RemoteBookSummary,
    supergrok_period: SuperGrokPeriodContext,
) -> DoubleEntryReport {
    build_double_entry_report_with_options(cfg, remote, supergrok_period, false)
}

/// Like [`build_double_entry_report`], optionally refreshing local book from
/// `$GROK_HOME/sessions/**/usage.jsonl` first (idempotent ingest).
pub fn build_double_entry_report_with_options(
    cfg: &TokenEconomyConfig,
    remote: RemoteBookSummary,
    supergrok_period: SuperGrokPeriodContext,
    refresh_from_sessions: bool,
) -> DoubleEntryReport {
    let mut report = DoubleEntryReport {
        remote,
        supergrok_period,
        ..Default::default()
    };

    if !cfg.local_spend_ledger && !cfg.reconcile_management_usage {
        return report;
    }

    let Some(store) = crate::grok_oss::try_open_from_token_economy_config(cfg) else {
        return report;
    };
    report.grok_oss_db_path = Some(store.path().display().to_string());

    if cfg.local_spend_ledger {
        if refresh_from_sessions {
            refresh_local_book_from_sessions(&store);
        }
        if let Ok(summary) = summarize_local_book(&store, None, None) {
            report.local = summary;
        }
    }

    report
}

/// Fail-open ingest of all session `usage.jsonl` files under `$GROK_HOME/sessions`.
pub fn refresh_local_book_from_sessions(store: &crate::grok_oss::GrokOssStore) -> IngestStats {
    let home = xai_grok_config::grok_home();
    ingest_all_sessions_usage(store, &home)
}
