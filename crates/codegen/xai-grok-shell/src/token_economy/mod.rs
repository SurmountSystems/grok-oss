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
    TokenEconomyConfigError, clear_token_economy_live, implement_effort_policy_active,
    reset_token_economy_live_to_defaults, resolve_grok_oss_database_path, set_token_economy_live,
    set_token_economy_live_bool, set_token_economy_live_int, token_economy_from_disk,
    token_economy_from_toml,
};
pub use implement_effort::{
    ImplementEffortRewrite, apply_implement_effort_policy, is_implement_command,
    parse_implement_effort,
};
pub use ledger::{
    IngestStats, LocalBookSummary, LocalUsageEvent, RemoteMeterSample, count_reconciliation_runs,
    ingest_all_sessions_usage, ingest_usage_jsonl, insert_local_usage_event,
    insert_reconciliation_run, insert_remote_meter_sample, latest_remote_sample,
    local_usage_event_exists, parse_usage_jsonl, summarize_local_book,
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
    build_double_entry_report_under(
        cfg,
        remote,
        supergrok_period,
        refresh_from_sessions,
        &xai_grok_config::grok_home(),
    )
}

/// Same as [`build_double_entry_report_with_options`] but ingest walks
/// `grok_home/sessions` (tests pass a temp home; `/spend` uses `$GROK_HOME`).
pub fn build_double_entry_report_under(
    cfg: &TokenEconomyConfig,
    remote: RemoteBookSummary,
    supergrok_period: SuperGrokPeriodContext,
    refresh_from_sessions: bool,
    grok_home: &std::path::Path,
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
            refresh_local_book_from_sessions_under(&store, grok_home);
        }
        if let Ok(summary) = summarize_local_book(&store, None, None) {
            report.local = summary;
        }
    }

    report
}

/// `/spend` path: ingest session `usage.jsonl`, summarize, persist a
/// `reconciliation_run` row (fail-open).
pub fn run_spend_double_entry(
    cfg: &TokenEconomyConfig,
    remote: RemoteBookSummary,
    supergrok_period: SuperGrokPeriodContext,
    grok_home: &std::path::Path,
) -> DoubleEntryReport {
    let report = build_double_entry_report_under(cfg, remote, supergrok_period, true, grok_home);
    persist_spend_reconciliation_fail_open(cfg, &report);
    report
}

/// Persist a `reconciliation_run` row when the store opens (fail-open).
pub fn persist_spend_reconciliation_fail_open(
    cfg: &TokenEconomyConfig,
    report: &DoubleEntryReport,
) {
    let Some(store) = crate::grok_oss::try_open_from_token_economy_config(cfg) else {
        return;
    };
    let notes = gap_honesty_line(&report.local, &report.remote);
    let _ = insert_reconciliation_run(
        &store,
        "local_window",
        "local_window",
        &report.local,
        report
            .remote
            .api_class_usd
            .map(|u| (u * 100.0).round() as i64),
        report
            .remote
            .oauth_class_usd
            .map(|u| (u * 100.0).round() as i64),
        &notes,
    );
}

/// Fail-open ingest of all session `usage.jsonl` files under `$GROK_HOME/sessions`.
pub fn refresh_local_book_from_sessions(store: &crate::grok_oss::GrokOssStore) -> IngestStats {
    refresh_local_book_from_sessions_under(store, &xai_grok_config::grok_home())
}

/// Fail-open ingest under an explicit grok home (session `usage.jsonl` walk).
pub fn refresh_local_book_from_sessions_under(
    store: &crate::grok_oss::GrokOssStore,
    grok_home: &std::path::Path,
) -> IngestStats {
    ingest_all_sessions_usage(store, grok_home)
}

#[cfg(test)]
mod spend_path_tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    #[test]
    fn spend_path_ingests_usage_jsonl_and_records_reconciliation() {
        let tmp = TempDir::new().unwrap();
        let db = tmp.path().join("grok_oss.db");
        let sess = tmp
            .path()
            .join("sessions")
            .join("enc-cwd")
            .join("sess-spend-restore");
        std::fs::create_dir_all(&sess).unwrap();
        let event_ulid = "01SPENDLEDGERRESTORE0000001";
        let line = format!(
            r#"{{"schema_version":1,"event_ulid":"{event_ulid}","timestamp":"2026-08-13T12:00:00.000Z","turn_type":"main","agent_kind":"main","session_id":"sess-spend-restore","input_tokens":4242,"output_tokens":7,"total_tokens":4249,"cost_usd_ticks":123456,"cost_missing":false,"incomplete":false}}"#
        );
        std::fs::write(sess.join("usage.jsonl"), format!("{line}\n")).unwrap();

        let cfg = TokenEconomyConfig {
            grok_oss_database_path: Some(db.clone()),
            ..TokenEconomyConfig::default()
        };

        let report = run_spend_double_entry(
            &cfg,
            RemoteBookSummary::default(),
            SuperGrokPeriodContext::default(),
            tmp.path(),
        );
        let empty = format_double_entry_report(&DoubleEntryReport::default());
        let body = format_double_entry_report(&report);
        assert_ne!(
            body, empty,
            "/spend path must format the live ledger, not DoubleEntryReport::default()"
        );
        assert_eq!(report.local.events, 1);
        assert_eq!(report.local.input_tokens, 4242);
        assert!(report.grok_oss_db_path.is_some());

        let store = open_at(&db).unwrap();
        assert!(local_usage_event_exists(&store, event_ulid).unwrap());
        assert!(count_reconciliation_runs(&store).unwrap() >= 1);
    }
}
