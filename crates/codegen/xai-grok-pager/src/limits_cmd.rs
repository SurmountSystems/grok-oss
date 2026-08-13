//! `grok limits` — agent-usable live sampling principal + spend meters.
//!
//! Standalone CLI (no TUI). Reuses shell auth resolve, SuperGrok credits poll
//! (`fetch_credits_config_with_session`), and Management team prepaid. Never
//! prints raw API keys, JWTs, or management secrets.
//!
//! Meters stay distinct: SuperGrok included weekly % ≠ SuperGrok $ extras ≠
//! console team prepaid.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::views::credit_bar::{ConsoleTeamPrepaidGap, CreditBalance, SamplingIdentityKind};
use crate::views::limits_snapshot::{
    LimitsSnapshot, PrincipalLimitsInput, format_limits_detail, honesty_notes_for_snapshot,
};

/// CLI args for `grok limits`.
#[derive(Clone, Debug, Default, Eq, PartialEq, clap::Args)]
pub struct LimitsArgs {
    /// Emit machine-readable JSON (schemaVersion 1). No secrets.
    #[arg(long)]
    pub json: bool,
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
    pub supergrok: SuperGrokCliSection,
    pub console: ConsoleCliSection,
    /// Non-secret warnings (fetch failures, no auth, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
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
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleCliSection {
    /// Inference console / Business API key on file or env.
    pub key_available: bool,
    /// Live sampling is currently the console key.
    pub is_live: bool,
    /// Console team prepaid remaining USD when Management meter known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_prepaid_usd: Option<f64>,
    /// Honest gap when team prepaid dollars unknown (snake-ish display key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_prepaid_gap: Option<&'static str>,
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
    let snap = snap
        .with_console_balance_cents(console_prepaid_cents)
        .with_console_prepaid_gap(console_prepaid_gap)
        .with_console_key_available(console_key_available || live.is_console());

    let report = report_from_snapshot(&snap, notes);
    (report, snap)
}

/// Build a machine-readable CLI report from a `/limits` snapshot (no I/O).
///
/// Used by `grok limits --json` and in-TUI `/limits --json` (scrollback dump).
pub fn report_from_snapshot(snap: &LimitsSnapshot, notes: Vec<String>) -> LimitsCliReport {
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
    let mut notes = notes;
    for honesty in honesty_notes_for_snapshot(snap) {
        if !notes.iter().any(|n| n == honesty) {
            notes.push(honesty.to_string());
        }
    }

    LimitsCliReport {
        schema_version: SCHEMA_VERSION,
        live_sampling: live_sampling_wire(snap.live_identity),
        live_sampling_label: snap.live_sampling_line(),
        live_principal_role: snap.live_principal_label.clone(),
        supergrok: SuperGrokCliSection {
            principals,
            shared_unified_pool: snap.shared_unified_supergrok_pool,
        },
        console: ConsoleCliSection {
            key_available: snap.console.key_available,
            is_live: snap.console.is_live,
            team_prepaid_usd: snap.console.balance_cents.map(|c| c.abs() as f64 / 100.0),
            team_prepaid_gap: if snap.console.balance_cents.is_some() {
                None
            } else {
                Some(prepaid_gap_wire(snap.console.prepaid_gap))
            },
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
    let mut out = format_limits_detail(snap);
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
        write!(writer, "{}", format_limits_human(snap, &report.notes))?;
    }
    Ok(())
}

/// Map shell billing config into pager [`CreditBalance`] (shared fields only).
pub fn credit_balance_from_billing_config(
    c: &xai_grok_shell::extensions::billing::BillingConfig,
) -> CreditBalance {
    let limit = c.monthly_limit.as_ref().map(|v| v.val).unwrap_or(0);
    let used = c.used.as_ref().map(|v| v.val).unwrap_or(0);
    let has_credit_pct = c.credit_usage_percent.is_some();
    let usage_pct = match c.credit_usage_percent {
        Some(pct) => pct.clamp(0.0, 100.0),
        None if limit > 0 => (used as f64 / limit as f64 * 100.0).min(100.0),
        None => 0.0,
    };
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
    let effective_usage_pct = if on_demand_val > 0 {
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

    // SuperGrok credits poll (all principals) — included-safe, not inference.
    let targets = xai_grok_shell::auth::load_supergrok_billing_poll_targets(grok_home);
    let active_id = xai_grok_shell::auth::active_supergrok_identity_id(grok_home);
    let listings = xai_grok_shell::auth::read_auth_json(&grok_home.join("auth.json"))
        .map(|map| xai_grok_shell::auth::list_supergrok_principal_listings(&map))
        .unwrap_or_default();

    // identity_id → balance from live fetch. Process included cache only
    // (ranking helpers); do **not** mark exhaust memos here — CLI is a
    // read-only report path, not hop policy.
    let mut balances: std::collections::BTreeMap<String, CreditBalance> =
        std::collections::BTreeMap::new();
    // identity_id → Grok Build productUsage % when present on wire.
    let mut build_usage: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    let mut active_included_full = false;

    for target in &targets {
        match xai_grok_shell::extensions::billing::fetch_credits_config_with_session(
            &proxy_base,
            &target.access_token,
            &target.user_id,
        )
        .await
        {
            Ok(resp) => {
                if let Some(config) = resp.config.as_ref() {
                    let bal = credit_balance_from_billing_config(config);
                    let (usage_pct, period_end) =
                        xai_grok_shell::extensions::billing::included_usage_and_period_end(config);
                    if let Some(pct) = usage_pct {
                        let period_type = config
                            .current_period
                            .as_ref()
                            .and_then(|p| p.period_type.as_deref());
                        xai_grok_shell::auth::remember_supergrok_included_billing(
                            &target.identity_id,
                            pct,
                            period_end.as_deref(),
                            period_type,
                        );
                        if active_id.as_deref() == Some(target.identity_id.as_str()) && pct >= 100.0
                        {
                            active_included_full = true;
                        }
                    }
                    if let Some(prepaid) = config.prepaid_balance.as_ref() {
                        xai_grok_shell::auth::remember_supergrok_dollar_extras(
                            &target.identity_id,
                            prepaid.val,
                        );
                    }
                    if let Some(build_pct) =
                        xai_grok_shell::extensions::billing::grok_build_usage_percent(config)
                    {
                        build_usage.insert(target.identity_id.clone(), build_pct);
                    }
                    balances.insert(target.identity_id.clone(), bal);
                } else {
                    notes.push(format!(
                        "SuperGrok billing for {} returned no config",
                        target.identity_id
                    ));
                }
            }
            Err(e) => {
                // Never include tokens from the error path; shell errors are status/body.
                notes.push(format!(
                    "SuperGrok billing poll failed for {}: {e}",
                    short_id(&target.identity_id)
                ));
            }
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
    let has_mgmt_key = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    let console_prepaid_cents = if has_mgmt_key {
        match xai_grok_shell::auth::fetch_console_team_prepaid_balance_default().await {
            Some(meter) => Some(meter.balance_cents),
            None => {
                notes.push("console team prepaid fetch failed or empty".into());
                xai_grok_shell::auth::cached_console_team_prepaid_cents_default()
            }
        }
    } else {
        None
    };
    // Re-check team after fetch (discovery may have filled process cache).
    let has_mgmt_team = xai_grok_shell::auth::resolve_management_team_id_default().is_some();
    let console_prepaid_gap = if console_prepaid_cents.is_some() {
        ConsoleTeamPrepaidGap::Loading // unused when cents present
    } else {
        ConsoleTeamPrepaidGap::after_billing_fetch(has_mgmt_key, has_mgmt_team)
    };
    if console_prepaid_cents.is_none() {
        if let Some(hint) = xai_grok_shell::auth::console_team_prepaid_setup_note(
            !has_mgmt_key,
            has_mgmt_key && !has_mgmt_team,
        ) {
            notes.push(hint);
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
                rows.push(PrincipalLimitsInput {
                    label: xai_grok_shell::auth::principal_limits_label(role),
                    role_label: Some(p.role_label.to_string()),
                    balance: bal,
                    autotopup: None,
                    included_billing_only: included_only,
                });
                ids.push(Some(p.identity_id.clone()));
            }
            (rows, ids)
        };

    let (mut report, snap) = build_limits_cli_from_parts(
        live,
        live_role,
        &principals,
        console_key_available,
        console_prepaid_cents,
        console_prepaid_gap,
        notes,
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

/// Run `grok limits` / `grok limits --json`.
pub async fn run(args: LimitsArgs) -> Result<()> {
    let (report, snap) = collect_limits_report().await?;
    write_limits_output(&report, &snap, args.json, &mut std::io::stdout().lock())?;
    Ok(())
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
        }
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
            human.contains("SuperGrok dollar extras: $15"),
            "extras: {human}"
        );
        assert!(human.contains("Console API:"), "console section: {human}");
        assert!(
            human.contains("Balance: no management key"),
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

    /// Named contract (Slice 3 review): flat-poll note on CLI human + JSON notes,
    /// once each, no Notes: double.
    #[test]
    fn human_and_json_surface_flat_poll_note_once_no_dedupe_double() {
        use crate::views::limits_honesty::{
            NOTE_FLAT_POLL_UNPROVEN_DEBIT, NOTE_INCLUDED_PCT_IS_BILLING_POLL,
        };

        let input = PrincipalLimitsInput {
            label: "SuperGrok".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
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
        snap = snap.with_flat_poll_unproven_debit(true);
        let report = report_from_snapshot(&snap, vec![]);
        let human = format_limits_human(&snap, &report.notes);

        // JSON notes: both honesty phrases once.
        let flat_in_notes = report
            .notes
            .iter()
            .filter(|n| n.as_str() == NOTE_FLAT_POLL_UNPROVEN_DEBIT)
            .count();
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
            human.contains(NOTE_FLAT_POLL_UNPROVEN_DEBIT),
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
            human.contains("Balance: $25"),
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

    /// Named contract: wire productUsage Build % surfaces on limits JSON when set.
    #[test]
    fn json_report_includes_grok_build_usage_pct_when_applied() {
        let input = PrincipalLimitsInput {
            label: "SuperGrok (personal)".into(),
            role_label: Some("personal".into()),
            balance: Some(bal(65.0)),
            autotopup: None,
            included_billing_only: false,
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
        };
        let business = PrincipalLimitsInput {
            label: "SuperGrok (business)".into(),
            role_label: Some("business".into()),
            balance: Some(bal(90.0)),
            autotopup: None,
            included_billing_only: false,
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
                },
                PrincipalLimitsInput {
                    label: "SuperGrok (business)".into(),
                    role_label: Some("business".into()),
                    balance: Some(bal(20.0)),
                    autotopup: None,
                    included_billing_only: false,
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
}
