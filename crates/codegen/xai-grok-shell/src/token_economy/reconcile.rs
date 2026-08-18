//! Double-entry reconcile: local usage book vs remote Management meters.
//!
//! Side-by-side books with gap honesty when local cost ticks are missing.
//! Never invents dollars for included SuperGrok period percent.

use super::ledger::LocalBookSummary;

/// Remote Management-side numbers for a reconcile window (optional fields).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemoteBookSummary {
    /// Team API class spend in **USD** (usage series). Distinct from cents.
    pub api_class_usd: Option<f64>,
    /// Team OAuth class spend in **USD**.
    pub oauth_class_usd: Option<f64>,
    /// Console team prepaid remaining in USD cents (when known).
    pub prepaid_remaining_cents: Option<i64>,
    /// Postpaid OAuth class cents (invoice preview).
    pub postpaid_oauth_class_cents: Option<i64>,
    /// Postpaid API class cents.
    pub postpaid_api_class_cents: Option<i64>,
    /// Window label for the remote series (e.g. "last 7 days UTC").
    pub window_label: Option<String>,
    /// True when no management key / remote fetch unavailable.
    pub remote_unavailable: bool,
    /// Setup note when remote is missing.
    pub remote_setup_note: Option<String>,
}

/// Included SuperGrok period context (percent only, not USD).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SuperGrokPeriodContext {
    pub usage_pct: Option<f64>,
    pub period_label: Option<String>,
    pub pacing_sentence: Option<String>,
}

/// Full double-entry report for operator UI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DoubleEntryReport {
    pub local: LocalBookSummary,
    pub remote: RemoteBookSummary,
    pub supergrok_period: SuperGrokPeriodContext,
    /// Absolute path of `grok_oss.db` when known.
    pub grok_oss_db_path: Option<String>,
}

/// USD ticks scale: 1e10 ticks = $1 (matches sampling types / usage.jsonl).
pub const USD_TICKS_PER_DOLLAR: f64 = 10_000_000_000.0;

/// Convert cost_usd_ticks sum to dollars when present.
pub fn ticks_to_usd(ticks: i64) -> f64 {
    ticks as f64 / USD_TICKS_PER_DOLLAR
}

/// Format the operator-facing double-entry text.
///
/// Meters stay distinct: included SuperGrok period limits % ≠ SuperGrok top-up $ ≠
/// console team prepaid ≠ postpaid OAuth vs API class ≠ local calculated spend.
pub fn format_double_entry_report(report: &DoubleEntryReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("Spend (double-entry)".to_string());
    lines.push(String::new());

    // Window A: included SuperGrok period context
    lines.push("Included SuperGrok period limits (context, not USD)".to_string());
    match (
        report.supergrok_period.usage_pct,
        report.supergrok_period.period_label.as_deref(),
    ) {
        (Some(pct), Some(label)) => {
            lines.push(format!(
                "  Included SuperGrok period used: {:.0}% ({label})",
                pct.floor()
            ));
        }
        (Some(pct), None) => {
            lines.push(format!(
                "  Included SuperGrok period used: {:.0}%",
                pct.floor()
            ));
        }
        _ => lines.push("  Included SuperGrok period used: not known yet".to_string()),
    }
    if let Some(pacing) = &report.supergrok_period.pacing_sentence {
        lines.push(format!("  {pacing}"));
    }
    lines.push(
        "  (This is included SuperGrok period limits percent, not SuperGrok top-up dollars \
and not console team prepaid.)"
            .to_string(),
    );
    lines.push(String::new());

    // Local book
    lines.push("Local book (session usage.jsonl → grok_oss.db)".to_string());
    lines.push(format!("  Events: {}", report.local.events));
    lines.push(format!(
        "  Tokens (input/output/total): {} / {} / {}",
        report.local.input_tokens, report.local.output_tokens, report.local.total_tokens
    ));
    match report.local.cost_usd_ticks_sum {
        Some(ticks) => {
            lines.push(format!(
                "  Local calculated spend (known cost ticks): ${:.6} ({} ticks)",
                ticks_to_usd(ticks),
                ticks
            ));
        }
        None => {
            lines.push(
                "  Local calculated spend: no cost ticks reported on any call in this window."
                    .to_string(),
            );
        }
    }
    if report.local.cost_missing_events > 0 {
        lines.push(format!(
            "  Local cost not reported on {} call(s); gap vs remote USD is not fully comparable.",
            report.local.cost_missing_events
        ));
    }
    lines.push(String::new());

    // Remote book
    lines.push("Remote book (Management / console team meters)".to_string());
    if report.remote.remote_unavailable {
        lines.push("  Remote Management samples unavailable for this view.".to_string());
        if let Some(note) = &report.remote.remote_setup_note {
            lines.push(format!("  {note}"));
        } else {
            lines.push(
                "  Add a Management API key (and team id when required) to pull team API vs OAuth class USD."
                    .to_string(),
            );
        }
    } else {
        if let Some(label) = &report.remote.window_label {
            lines.push(format!("  Window: {label}"));
        }
        match (report.remote.api_class_usd, report.remote.oauth_class_usd) {
            (Some(api), Some(oauth)) => {
                lines.push(format!(
                    "  Console team usage series — API class: ${api:.4}; OAuth class: ${oauth:.4}"
                ));
            }
            (Some(api), None) => {
                lines.push(format!(
                    "  Console team usage series — API class: ${api:.4}"
                ));
            }
            (None, Some(oauth)) => {
                lines.push(format!(
                    "  Console team usage series — OAuth class: ${oauth:.4}"
                ));
            }
            (None, None) => {
                lines.push("  Console team usage series: no sample yet".to_string());
            }
        }
        if let Some(cents) = report.remote.prepaid_remaining_cents {
            lines.push(format!(
                "  Console team prepaid remaining: ${:.2}",
                cents.abs() as f64 / 100.0
            ));
        }
        if report.remote.postpaid_api_class_cents.is_some()
            || report.remote.postpaid_oauth_class_cents.is_some()
        {
            let api = report.remote.postpaid_api_class_cents.unwrap_or(0);
            let oauth = report.remote.postpaid_oauth_class_cents.unwrap_or(0);
            lines.push(format!(
                "  Console team postpaid invoice (period) — API class: ${:.2}; OAuth class: ${:.2}",
                api as f64 / 100.0,
                oauth as f64 / 100.0
            ));
        }
    }
    lines.push(String::new());

    // Gap honesty
    lines.push("Reconciliation".to_string());
    lines.push(gap_honesty_line(&report.local, &report.remote));
    lines.push(String::new());
    lines.push(
        "Meters stay distinct: included SuperGrok period limits % · SuperGrok top-up $ · console team prepaid \
· postpaid OAuth vs API class · local calculated spend."
            .to_string(),
    );
    if let Some(path) = &report.grok_oss_db_path {
        lines.push(format!("Durable books: {path} (uniquely grok-oss store)."));
    }

    lines.join("\n")
}

/// Gap line: only comparable when both sides have USD-like units and local cost present.
pub fn gap_honesty_line(local: &LocalBookSummary, remote: &RemoteBookSummary) -> String {
    if local.cost_missing_events > 0 && local.cost_usd_ticks_sum.is_none() {
        return format!(
            "Local cost not reported on {} call(s); gap vs remote USD not comparable.",
            local.cost_missing_events
        );
    }
    if local.cost_missing_events > 0 {
        return format!(
            "Local cost not reported on {} call(s); gap vs remote USD not fully comparable \
(local sum covers only calls that reported cost ticks).",
            local.cost_missing_events
        );
    }
    if remote.remote_unavailable
        || (remote.api_class_usd.is_none() && remote.oauth_class_usd.is_none())
    {
        return "Remote USD sample missing; gap not computed. Local book still shown above."
            .to_string();
    }
    if local.cost_usd_ticks_sum.is_none() {
        return "Local cost ticks absent; gap vs remote USD not comparable.".to_string();
    }
    // Both sides have some USD-like data and local has complete cost ticks for window events.
    let local_usd = ticks_to_usd(local.cost_usd_ticks_sum.unwrap_or(0));
    let remote_usd = remote.api_class_usd.unwrap_or(0.0) + remote.oauth_class_usd.unwrap_or(0.0);
    let gap = local_usd - remote_usd;
    format!(
        "Local known calculated spend ${local_usd:.6} vs remote series API+OAuth ${remote_usd:.4} \
(difference ${gap:.4}). These meters can cover different scopes (session-local vs team-wide); \
use as a check, not a single fused total."
    )
}

/// Section body suitable for embedding under `/limits`.
pub fn format_limits_spend_section(report: &DoubleEntryReport) -> String {
    let mut lines = vec!["Spend (double-entry summary)".to_string()];
    lines.push(format!("  Local events: {}", report.local.events));
    if let Some(ticks) = report.local.cost_usd_ticks_sum {
        lines.push(format!(
            "  Local calculated spend (known ticks): ${:.6}",
            ticks_to_usd(ticks)
        ));
    } else {
        lines.push("  Local calculated spend: no cost ticks in window".to_string());
    }
    if report.local.cost_missing_events > 0 {
        lines.push(format!(
            "  Local cost missing on {} call(s)",
            report.local.cost_missing_events
        ));
    }
    if report.remote.remote_unavailable {
        lines.push("  Remote Management book: unavailable".to_string());
    } else if let (Some(api), Some(oauth)) =
        (report.remote.api_class_usd, report.remote.oauth_class_usd)
    {
        lines.push(format!(
            "  Remote console team series — API ${api:.4} · OAuth ${oauth:.4}"
        ));
    } else {
        lines.push("  Remote console team series: no sample yet".to_string());
    }
    lines.push(format!(
        "  {}",
        gap_honesty_line(&report.local, &report.remote)
    ));
    lines.push("  Full view: /spend".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_when_all_cost_missing() {
        let local = LocalBookSummary {
            events: 3,
            cost_missing_events: 3,
            cost_usd_ticks_sum: None,
            ..Default::default()
        };
        let remote = RemoteBookSummary {
            api_class_usd: Some(1.0),
            oauth_class_usd: Some(0.2),
            ..Default::default()
        };
        let line = gap_honesty_line(&local, &remote);
        assert!(line.contains("not comparable"), "{line}");
        assert!(line.contains("3"));
    }

    #[test]
    fn report_names_meters_distinctly() {
        let report = DoubleEntryReport {
            local: LocalBookSummary {
                events: 2,
                cost_missing_events: 1,
                cost_usd_ticks_sum: Some(5_000_000_000),
                input_tokens: 100,
                output_tokens: 10,
                total_tokens: 110,
            },
            remote: RemoteBookSummary {
                api_class_usd: Some(0.4),
                oauth_class_usd: Some(0.1),
                window_label: Some("last 7 days UTC".into()),
                ..Default::default()
            },
            supergrok_period: SuperGrokPeriodContext {
                usage_pct: Some(42.0),
                period_label: Some("weekly".into()),
                pacing_sentence: Some(
                    "Included SuperGrok period burn is 5% ahead of linear burn for this billing period."
                        .into(),
                ),
            },
            grok_oss_db_path: Some("/tmp/grok_oss.db".into()),
        };
        let text = format_double_entry_report(&report);
        assert!(text.contains("Included SuperGrok period"));
        assert!(text.contains("not SuperGrok top-up dollars"));
        assert!(text.contains("console team prepaid") || text.contains("Console team"));
        assert!(text.contains("Local book"));
        assert!(text.contains("Remote book"));
        assert!(text.contains("not fully comparable") || text.contains("not comparable"));
        assert!(text.contains("ahead of linear burn"));
        assert!(!text.contains("SuperGrok money"));
        assert!(text.contains("grok_oss.db"));
    }

    #[test]
    fn limits_section_points_to_spend() {
        let report = DoubleEntryReport::default();
        let s = format_limits_spend_section(&report);
        assert!(s.contains("/spend"));
        assert!(s.contains("double-entry"));
    }

    #[test]
    fn remote_unavailable_copy() {
        let report = DoubleEntryReport {
            remote: RemoteBookSummary {
                remote_unavailable: true,
                remote_setup_note: Some("No management key on file.".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let text = format_double_entry_report(&report);
        assert!(text.contains("unavailable") || text.contains("Management"));
        assert!(text.contains("No management key"));
    }
}
