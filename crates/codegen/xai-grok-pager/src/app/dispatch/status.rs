//! Session status, sharing, privacy, usage, and info dispatchers.

use agent_client_protocol as acp;

use super::ctx::get_active_agent;
use super::settings::ui::refresh_open_settings_modals;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView};
use crate::notifications::{NotificationEvent, NotificationEventKind};
use crate::scrollback::block::RenderBlock;

/// Toggle YOLO mode (auto-approve all permissions).
///
/// When turning ON: auto-approve all currently queued permissions and
/// restore the stashed prompt. Future incoming permissions will be
/// auto-approved in `handle_permission_request`.
///
/// Share the current session via a public URL.
///
/// Produces Effect::ShareSession which spawns an async ACP ext request.
/// On completion, TaskResult::ShareSessionComplete shows the URL in scrollback.
pub(super) fn dispatch_share_session(app: &mut AppView) -> Vec<Effect> {
    if !app.sharing_enabled {
        app.show_toast("Sharing is disabled");
        return vec![];
    }
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        // No active session — error should have been caught by slash command,
        // but guard here just in case.
        return vec![];
    };

    vec![Effect::ShareSession {
        agent_id: id,
        session_id,
    }]
}

/// Show session info: fetch via x.ai/session/info and display in scrollback.
///
/// Produces Effect::ShowSessionInfo which spawns an async ACP ext request.
/// On completion, TaskResult::SessionInfoComplete shows the formatted info.
pub(super) fn dispatch_show_session_info(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        // No active session — error should have been caught by slash command,
        // but guard here just in case.
        return vec![];
    };

    vec![Effect::ShowSessionInfo {
        agent_id: id,
        session_id,
        show_resolved_model: app.show_resolved_model,
    }]
}

/// Show privacy and data retention status as a system message in scrollback.
///
/// Three-state display: Enterprise ZDR, coding data sharing opted out,
/// or opted in. Labels align with `CODING_DATA_SHARING_CHOICES` in
/// `settings/defs.rs` and the `coding_data_sharing_toast` format.
///
/// Also lists config knobs that `/privacy` does not change (technical
/// pointers only; no policy claims).
pub(super) fn dispatch_show_privacy_info(app: &mut AppView) -> Vec<Effect> {
    let mut lines = Vec::new();

    if app.is_zdr {
        // Enterprise ZDR -- the team has disabled retention entirely.
        lines.push("  Zero Data Retention: enabled");
        lines.push("  Your data is not retained or used for training (ZDR enabled).");
    } else if app.coding_data_retention_opt_out {
        // Coding data sharing opted out -- matches desktop's "Privacy mode" state.
        lines.push("  Privacy: privacy mode");
        lines.push("  Your code data will not be trained on or used to improve the product.");
        lines.push("");
        lines.push("  Use /privacy opt-in to share data and help improve the product.");
    } else {
        // Coding data sharing opted in -- matches desktop's "Share data" state.
        lines.push("  Privacy: share data");
        lines.push("  Usage and code data may be used by SpaceXAI to improve the product.");
        lines.push("");
        lines.push("  Use /privacy opt-out to enable privacy mode.");
    }

    // Config keys only; do not describe retention/training/analytics policy here.
    lines.push("");
    lines.push("  Other settings (not changed by /privacy):");
    lines.push("  - [features] telemetry / GROK_TELEMETRY_ENABLED");
    lines.push("  - [telemetry] trace_upload / GROK_TELEMETRY_TRACE_UPLOAD");
    lines.push("  - GROK_EXTERNAL_OTEL / OTEL_*");
    lines.push("");
    lines.push("  Learn more: https://x.ai/legal");
    let text = lines.join("\n");
    push_system_to_any_agent(app, &text);
    vec![]
}

/// State-only mutation for `coding_data_sharing`. SHELL-owned.
pub(super) fn set_coding_data_sharing_inner(app: &mut AppView, opted_in: bool) {
    app.coding_data_retention_opt_out = !opted_in;
}

/// Set coding-data-sharing preference. SHELL-owned, auth-metadata-backed
/// (persists via ACP ext-request, NOT `~/.grok/config.toml`).
pub(super) fn set_coding_data_sharing(app: &mut AppView, opted_in: bool) -> Vec<Effect> {
    // ── Guard 1: Enterprise ZDR ──────────────────────────────────────
    if app.is_zdr {
        app.show_toast("\u{2717} Cannot change: Zero Data Retention enabled");
        return vec![];
    }
    // ── Guard 2: Non-admin team member ───────────────────────────────
    if app.team_name.is_some() {
        let is_admin = app
            .team_role
            .as_deref()
            .is_some_and(|r| r.eq_ignore_ascii_case("admin"));
        if !is_admin {
            app.show_toast("\u{2717} Data sharing is controlled by your team admin");
            return vec![];
        }
    }
    // Synthetic AgentId(0) when no agents (welcome banner Accept).
    let agent_id = match app.active_view {
        crate::app::app_view::ActiveView::Agent(id) => id,
        _ => app
            .agents
            .keys()
            .next()
            .copied()
            .unwrap_or(crate::app::agent::AgentId(0)),
    };

    let prev = !app.coding_data_retention_opt_out;

    // ── Idempotent path: toast but skip the ACP round-trip. ──────────
    if prev == opted_in {
        app.show_toast(&coding_data_sharing_toast(opted_in));
        return vec![];
    }

    // ── Optimistic mutation: state, then UI feedback, then effect. ───
    set_coding_data_sharing_inner(app, opted_in);
    refresh_open_settings_modals(app);
    app.show_toast(&coding_data_sharing_toast(opted_in));

    tracing::info!(
        target: "settings",
        key = "coding_data_sharing",
        opted_in,
        "setting changed",
    );

    vec![Effect::SetCodingDataSharing {
        agent_id,
        opted_in,
        rollback_to_opted_in: prev,
    }]
}

/// Format the `Coding data sharing` toast. Asymmetric: opt-in
/// (privacy-degrading) uses ⚠ + consequence text; opt-out (safe
/// default) uses ✓. Uses display names from the registry catalog.
pub(super) fn coding_data_sharing_toast(opted_in: bool) -> String {
    let display = display_for_coding_data_sharing_canonical(opted_in);
    if opted_in {
        // Privacy-degrading: warn glyph + spelled-out consequence.
        format!(
            "\u{26A0} Coding data sharing: {display} \u{2014} code samples may be retained \
             for training"
        )
    } else {
        // Safe default — uniform ✓ glyph.
        format!("\u{2713} Coding data sharing: {display}")
    }
}

/// Display string for the canonical bool. Keep aligned with
/// `CODING_DATA_SHARING_CHOICES` in `settings/defs.rs`.
fn display_for_coding_data_sharing_canonical(opted_in: bool) -> &'static str {
    if opted_in { "Opt in" } else { "Opt out" }
}

/// Scrub an untrusted error string for toast display. Substitutes a
/// generic placeholder when the input exceeds 120 chars or contains
/// control / bidi-override characters (prevents escape-sequence
/// injection and visual spoofing). Full error stays in tracing logs.
pub(super) fn scrub_error_for_toast(error: &str) -> String {
    const MAX_TOAST_ERROR_LEN: usize = 120;
    if error.len() > MAX_TOAST_ERROR_LEN
        || error
            .chars()
            .any(crate::render::line_utils::is_unsafe_display_char)
    {
        "server error (see logs for details)".to_string()
    } else {
        error.to_string()
    }
}

/// Push a system message to the active agent's scrollback, or to any available
/// agent if on the welcome screen.
fn push_system_to_any_agent(app: &mut AppView, msg: &str) {
    let block = crate::scrollback::block::RenderBlock::system(msg.to_string());
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        agent.scrollback.push_block(block);
        return;
    }
    if let Some(agent) = app.agents.values_mut().next() {
        agent.scrollback.push_block(block);
    }
}

/// Show context info: fetch via x.ai/session/info and display rich breakdown.
///
/// Produces Effect::ShowContextInfo which spawns an async ACP ext request.
/// On completion, TaskResult::ContextInfoComplete shows the formatted info.
pub(super) fn dispatch_show_context_info(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        return vec![];
    };

    vec![Effect::ShowContextInfo {
        agent_id: id,
        session_id,
    }]
}

/// `/limits` — SuperGrok included / dollar extras / console path detail.
///
/// Opens a **dismissible popup modal** (not a scrollback dump). Pure view of
/// the cached billing snapshot + live sampling identity. While open, the modal
/// ticks a d/h/m/s countdown and re-samples billing when the countdown hits zero.
///
/// When two SuperGrok principals exist in `auth.json`, stacks dual rows
/// (active principal gets the polled billing cache; siblings honest absence
/// unless process-local included billing was remembered for them).
/// Console team prepaid cents come from agent/app cache or Management process
/// cache; missing → honest not-configured / loading / unavailable (never a soft
/// "no $ meter yet" placeholder). Empty SuperGrok cache → "no data yet".
pub(super) fn dispatch_show_limits(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    if !app.agents.contains_key(&id) {
        return vec![];
    }
    let (balance, autotopup, live, console_prepaid) = {
        let agent = app.agents.get(&id).expect("checked contains_key");
        let balance = agent
            .credit_balance
            .clone()
            .or_else(|| app.credit_balance.clone());
        let autotopup = agent.auto_topup.clone().or_else(|| app.auto_topup.clone());
        let console_prepaid = agent
            .console_team_prepaid_cents
            .or(app.console_team_prepaid_cents)
            .or_else(xai_grok_shell::auth::cached_console_team_prepaid_cents_default);
        (balance, autotopup, agent.sampling_identity, console_prepaid)
    };
    let has_mgmt_key = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    let has_mgmt_team = xai_grok_shell::auth::resolve_management_team_id_default().is_some();
    // Management key alone is enough to attempt team-id discovery + prepaid.
    let configured = has_mgmt_key;
    let console_key_available =
        xai_grok_shell::auth::console_inference_key_present_default() || live.is_console();
    // Distinct missing key vs loading vs post-fetch gaps (ignored when cents known).
    let prepaid_gap = if console_prepaid.is_some() {
        crate::views::credit_bar::ConsoleTeamPrepaidGap::Loading
    } else {
        crate::views::credit_bar::ConsoleTeamPrepaidGap::from_management_config(
            has_mgmt_key,
            has_mgmt_team,
        )
    };
    let snap = build_limits_snapshot(
        balance.as_ref(),
        autotopup.as_ref(),
        live,
        console_prepaid,
        prepaid_gap,
        console_key_available,
    );
    // Dual SuperGrok: any principal still missing included meters → silent
    // refresh (sibling poll fills process cache). Unified fill may already
    // paint the shared pool on cold siblings; only fetch when a row is still empty.
    let needs_sibling_billing = !snap.extra_principals.is_empty()
        && (snap.primary.included.is_none()
            || snap.extra_principals.iter().any(|p| p.included.is_none()));

    if let Some(agent) = app.agents.get_mut(&id) {
        agent.active_modal = Some(crate::views::modal::ActiveModal::Limits {
            state: Box::new(crate::views::limits_modal::LimitsModalState::new(snap)),
        });
    }
    // Silent FetchBilling when Management prepaid is cold, or dual SuperGrok
    // sibling included is still empty after build (no unified fill).
    let mut effects = Vec::new();
    if (console_prepaid.is_none() && configured) || needs_sibling_billing {
        effects.push(Effect::FetchBilling {
            agent_id: id,
            silent: true,
        });
    }
    effects
}

/// `/limits --json` — same cache snapshot as the modal, as pretty JSON in
/// conversation scrollback (schema matches `grok limits --json`). No modal.
///
/// When `CreditBalance.grok_build_usage_pct` was set by FetchBilling (wire
/// `productUsage`), the JSON includes `grokBuildUsagePct` on that principal —
/// same field as live `grok limits --json` collect. Sibling process-cache rows
/// stay without Build % until a full credits poll observes it.
pub(super) fn dispatch_show_limits_json(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    if !app.agents.contains_key(&id) {
        return vec![];
    }
    let Some(snap) = rebuild_limits_snapshot_for_agent(app, id) else {
        return vec![];
    };
    let report = crate::limits_cmd::report_from_snapshot(&snap, Vec::new());
    let json = match crate::limits_cmd::format_limits_json_pretty(&report) {
        Ok(s) => s,
        Err(e) => {
            if let Some(agent) = app.agents.get_mut(&id) {
                agent.scrollback.push_block(RenderBlock::system(format!(
                    "Failed to format limits JSON: {e}"
                )));
            }
            return vec![];
        }
    };
    // Fenced so chat is readable; body is the same JSON as CLI --json.
    let text = format!("```json\n{}\n```", json.trim_end());
    if let Some(agent) = app.agents.get_mut(&id) {
        // Bypass modal; commit into transcript both human and agent can see.
        agent.active_modal = None;
        agent.scrollback.push_block(RenderBlock::system(text));
    }
    // Same silent refresh policy as modal when caches are cold.
    let console_prepaid = app
        .agents
        .get(&id)
        .and_then(|a| a.console_team_prepaid_cents)
        .or(app.console_team_prepaid_cents)
        .or_else(xai_grok_shell::auth::cached_console_team_prepaid_cents_default);
    let has_mgmt_key = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    // Key alone: team id may be discovered during FetchBilling.
    let configured = has_mgmt_key;
    let needs_sibling_billing = !snap.extra_principals.is_empty()
        && (snap.primary.included.is_none()
            || snap.extra_principals.iter().any(|p| p.included.is_none()));
    let mut effects = Vec::new();
    if (console_prepaid.is_none() && configured) || needs_sibling_billing {
        effects.push(Effect::FetchBilling {
            agent_id: id,
            silent: true,
        });
    }
    effects
}

/// Rebuild limits snapshot from current caches (for open modal refresh).
pub(super) fn rebuild_limits_snapshot_for_agent(
    app: &AppView,
    agent_id: crate::app::agent::AgentId,
) -> Option<crate::views::limits_snapshot::LimitsSnapshot> {
    let agent = app.agents.get(&agent_id)?;
    let balance = agent
        .credit_balance
        .as_ref()
        .or(app.credit_balance.as_ref());
    let autotopup = agent.auto_topup.as_ref().or(app.auto_topup.as_ref());
    let console_prepaid = agent
        .console_team_prepaid_cents
        .or(app.console_team_prepaid_cents)
        .or_else(xai_grok_shell::auth::cached_console_team_prepaid_cents_default);
    let has_mgmt_key = xai_grok_shell::auth::resolve_management_api_key_default().is_some();
    let has_mgmt_team = xai_grok_shell::auth::resolve_management_team_id_default().is_some();
    let console_key_available = xai_grok_shell::auth::console_inference_key_present_default()
        || agent.sampling_identity.is_console();
    let prepaid_gap = if console_prepaid.is_some() {
        crate::views::credit_bar::ConsoleTeamPrepaidGap::Loading
    } else {
        crate::views::credit_bar::ConsoleTeamPrepaidGap::from_management_config(
            has_mgmt_key,
            has_mgmt_team,
        )
    };
    Some(build_limits_snapshot(
        balance,
        autotopup,
        agent.sampling_identity,
        console_prepaid,
        prepaid_gap,
        console_key_available,
    ))
}

/// Build `/limits` view-model: dual SuperGrok rows when multi-principal store.
fn build_limits_snapshot(
    balance: Option<&crate::views::credit_bar::CreditBalance>,
    autotopup: Option<&crate::views::credit_bar::AutoTopupInfo>,
    live: crate::views::credit_bar::SamplingIdentityKind,
    console_team_prepaid_cents: Option<i64>,
    console_team_prepaid_gap: crate::views::credit_bar::ConsoleTeamPrepaidGap,
    console_key_available: bool,
) -> crate::views::limits_snapshot::LimitsSnapshot {
    use crate::views::limits_snapshot::{LimitsSnapshot, PrincipalLimitsInput};
    use xai_grok_shell::auth::{
        SupergrokAccountRole, active_supergrok_identity_id, included_billing_fields_snapshot,
        list_supergrok_principal_listings, principal_limits_label, read_auth_json,
    };

    let home = xai_grok_shell::util::grok_home::grok_home();
    let listings = read_auth_json(&home.join("auth.json"))
        .map(|map| list_supergrok_principal_listings(&map))
        .unwrap_or_default();

    if listings.len() < 2 {
        // Single principal (or none): keep classic single SuperGrok section.
        let mut snap = LimitsSnapshot::from_billing(balance, autotopup, live)
            .with_console_balance_cents(console_team_prepaid_cents)
            .with_console_prepaid_gap(console_team_prepaid_gap)
            .with_console_key_available(console_key_available);
        if listings.len() == 1 && !live.is_console() {
            snap.live_principal_label = Some(listings[0].role_label.to_string());
        }
        return snap;
    }

    let active_id = active_supergrok_identity_id(&home);
    let billing_by_id = included_billing_fields_snapshot();

    // Order: active identity first (gets the live billing cache), then others.
    let mut ordered = listings;
    if let Some(ref aid) = active_id {
        ordered.sort_by_key(|p| if &p.identity_id == aid { 0u8 } else { 1u8 });
    }

    let inputs: Vec<PrincipalLimitsInput> = ordered
        .iter()
        .map(|p| {
            let role = if p.role_label == "business" {
                SupergrokAccountRole::Business
            } else {
                SupergrokAccountRole::Personal
            };
            let is_active = active_id.as_deref() == Some(p.identity_id.as_str());
            // Active principal: use pager credit cache (full meters).
            // Others: process billing memory for this identity only (included %
            // + prepaidBalance when the sibling credits poll observed it).
            // Never copy active CreditBalance onto a sibling identity.
            let (bal, topup, included_billing_only) = if is_active {
                (balance.cloned(), autotopup.cloned(), false)
            } else if let Some(fields) = billing_by_id.get(&p.identity_id) {
                // Per-slot process cache only — never reuse active CreditBalance.
                // Date format matches credit_balance_from_config (`%B`, full month)
                // so dual rows do not look like two different clocks (Aug vs August).
                // prepaid_balance_cents from sibling poll = Extra Usage Credits
                // for that principal (or shared pool under unified billing).
                let bal = fields.usage_pct.map(|pct| {
                    crate::views::credit_bar::CreditBalance {
                        usage_pct: pct,
                        effective_usage_pct: pct,
                        period_end_display: fields.reset_at.map(|dt| {
                            dt.with_timezone(&chrono::Local)
                                .format("%B %-d, %H:%M")
                                .to_string()
                        }),
                        period_end_at: fields.reset_at,
                        pay_as_you_go: false,
                        on_demand_cap_cents: None,
                        on_demand_used_cents: None,
                        // Sibling credits poll prepaidBalance → Extra Usage Credits.
                        prepaid_balance_cents: fields.prepaid_balance_cents,
                        // Plumb period_type so copy says "weekly"/"monthly"
                        // instead of bare "Included allowance".
                        period_type: fields.period_type.clone(),
                        is_unified_billing_user: None,
                        // Sibling process cache does not store productUsage.
                        grok_build_usage_pct: None,
                    }
                });
                // included_billing_only when we never saw prepaid on this slot
                // (honest "no data yet" unless unified fill shares the pool).
                let included_only = fields.prepaid_balance_cents.is_none();
                (bal, None, included_only)
            } else {
                // Never-polled sibling: included-only absence (not "none on file"
                // for dollar extras). Unified fill + silent FetchBilling fill later.
                (None, None, true)
            };
            PrincipalLimitsInput {
                label: principal_limits_label(role),
                role_label: Some(p.role_label.to_string()),
                balance: bal,
                autotopup: topup,
                included_billing_only,
            }
        })
        .collect();

    let live_role = if live.is_console() {
        None
    } else {
        active_id.as_ref().and_then(|aid| {
            ordered
                .iter()
                .find(|p| &p.identity_id == aid)
                .map(|p| p.role_label)
        })
    };

    LimitsSnapshot::from_principals(&inputs, live, live_role)
        .with_console_balance_cents(console_team_prepaid_cents)
        .with_console_prepaid_gap(console_team_prepaid_gap)
        .with_console_key_available(console_key_available)
}

/// `/usage` — session token/cost, then consumer credits when visible.
/// Credits are chained after the session block so layout stays ordered.
pub(super) fn dispatch_show_usage(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let session_id = {
        let Some(agent) = app.agents.get_mut(&id) else {
            return vec![];
        };
        agent.session.session_id.clone()
    };
    match session_id {
        Some(session_id) => vec![Effect::FetchSessionUsage {
            agent_id: id,
            session_id,
        }],
        None => {
            if let Some(agent) = app.agents.get_mut(&id) {
                agent.scrollback.push_block(RenderBlock::system(
                    "Session usage is unavailable until the session starts.".to_string(),
                ));
            }
            append_consumer_billing_surface(app, id)
        }
    }
}

/// Commit a session-usage block if still on `session_id`, then consumer credits.
pub(super) fn commit_session_usage_block(
    app: &mut AppView,
    agent_id: AgentId,
    session_id: &acp::SessionId,
    text: String,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if agent.session.session_id.as_ref() != Some(session_id) {
        return vec![];
    }
    agent.scrollback.push_block(RenderBlock::system(text));
    append_consumer_billing_surface(app, agent_id)
}

/// Consumer credit follow-up for `/usage` (redirect or non-silent billing fetch).
pub(super) fn append_consumer_billing_surface(app: &mut AppView, agent_id: AgentId) -> Vec<Effect> {
    if !app.usage_visible {
        return vec![];
    }
    // Remote-settings kill switch (`grok_build_usage_redirect_url`): link out
    // instead of fetching billing from the backend.
    if let Some(url) = app.usage_billing_redirect_url.clone() {
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.scrollback.push_block(RenderBlock::System(
                crate::scrollback::blocks::SystemMessageBlock::new(format!(
                    "Please check your usage on {url}"
                )),
            ));
        }
        return vec![];
    }
    if !app.agents.contains_key(&agent_id) {
        return vec![];
    }
    // Non-silent: the effect also pulls the auto top-up rule so the summary
    // renders usage, prepaid credits, and auto top-up together.
    vec![Effect::FetchBilling {
        agent_id,
        silent: false,
    }]
}

/// `/usage manage` — open consumer billing. No-op when the surface is hidden.
pub(super) fn dispatch_manage_billing(app: &mut AppView) -> Vec<Effect> {
    if !app.usage_visible {
        return vec![];
    }
    super::router::dispatch(
        crate::app::actions::Action::OpenUrl("https://grok.com/?_s=usage".to_string()),
        app,
    )
}

/// Commit a one-line "update available" notice into the active agent's
/// scrollback. Minimal mode has no welcome screen (the full TUI's update
/// surface), so the background update check's result is shown here instead
/// No-op when there is no active agent.
pub(crate) fn commit_minimal_update_notice(app: &mut AppView, latest_version: &str) {
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        agent.scrollback.push_block(RenderBlock::system(format!(
            "Update available: v{latest_version} — restart to apply."
        )));
    }
}

/// `/queue` — commit a read-only list of the queued prompts as a system block.
/// The text is built by [`crate::app::status_blocks::queue_block_text`]; this
/// just resolves the active agent and pushes it. Works in every render mode; the
/// primary inspection surface in minimal, which has no interactive `QueuePane`.
pub(super) fn dispatch_show_queue(app: &mut AppView) -> Vec<Effect> {
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        let text = crate::app::status_blocks::queue_block_text(agent);
        agent.scrollback.push_block(RenderBlock::system(text));
    }
    vec![]
}

/// `/tasks` — commit a read-only list of background tasks, subagents, and
/// scheduled (`/loop`) tasks as a system block. The text is built by
/// [`crate::app::status_blocks::tasks_block_text`]; this just resolves the
/// active agent and pushes it. Works in every render mode; the primary snapshot
/// surface in minimal, which has no interactive `TasksPane`.
pub(super) fn dispatch_show_tasks(app: &mut AppView) -> Vec<Effect> {
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        let text = crate::app::status_blocks::tasks_block_text(agent);
        agent.scrollback.push_block(RenderBlock::system(text));
    }
    vec![]
}

/// Clear completed/cancelled todos from the live board (shell archives + Plan).
///
/// No-op toast when the board has nothing finished. Does not use merge:false.
pub(super) fn dispatch_clear_completed_todos(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        agent.show_toast("No active session");
        return vec![];
    };
    let done = agent.todo.counts().completed + agent.todo.counts().cancelled;
    if done == 0 {
        agent.show_toast("No completed todos to clear");
        return vec![];
    }
    vec![Effect::ClearCompletedTodos { session_id }]
}

/// Open the hidden `/gboom` easter egg as a modal over the active agent
/// view. Requires a graphics-capable terminal (kitty protocol or iTerm2);
/// otherwise a toast explains why nothing happened. On session-less
/// surfaces (dashboard, welcome) this is a silent no-op.
///
/// Targets the top-level agent view (where the prompt lives), not a
/// focused subagent view: the modal's tick/draw plumbing runs on the
/// top-level view, mirroring the video viewer.
pub(super) fn dispatch_open_gboom(app: &mut AppView) -> Vec<Effect> {
    use crate::terminal::image::{GraphicsProtocol, detect_graphics_protocol};
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    if detect_graphics_protocol() == GraphicsProtocol::None {
        agent.show_toast(
            "No demons here \u{2014} GBOOM needs a graphics-capable terminal \
             (kitty, Ghostty, WezTerm, iTerm2)",
        );
        return vec![];
    }
    // Close other media modals: they share the kitty placement id. Drop the
    // image viewer's in-flight loader too (its close path clears both —
    // a leaked rx would mis-feed the next image viewer's poll loop).
    agent.image_viewer = None;
    agent.image_load_rx = None;
    agent.video_viewer = None;
    agent.gboom = Some(crate::gboom::GboomState::new());
    vec![]
}

/// Emit a `SessionReady` notification for the given agent.
///
/// Takes `&NotificationService` separately from `&AgentView` to avoid
/// borrow-checker conflicts when `agent` is borrowed from `app.agents`.
pub(super) fn notify_session_ready(
    notification_service: &crate::notifications::NotificationService,
    agent: &AgentView,
) {
    notification_service.notify(NotificationEvent {
        kind: NotificationEventKind::SessionReady,
        title: "Grok".into(),
        body: NotificationEventKind::SessionReady.as_str().into(),
        session_id: agent.session.session_id.as_ref().map(|s| s.0.to_string()),
    });
}

// TaskResult handlers.

pub(super) fn handle_coding_data_sharing_updated(
    app: &mut AppView,
    agent_id: AgentId,
    opted_in: bool,
) -> Vec<Effect> {
    // Re-anchor mirror to server-confirmed value (defense-in-depth against
    // server reshaping the boolean). `agent_id` discarded — privacy is
    // app-level, not per-agent.
    set_coding_data_sharing_inner(app, opted_in);
    refresh_open_settings_modals(app);
    // Re-toast on confirmation. Without this, a slow ACP round-trip would
    // leave the user with only the optimistic toast (already faded) and no
    // server-confirmed feedback.
    app.show_toast(&coding_data_sharing_toast(opted_in));
    tracing::info!(
        target: "settings",
        key = "coding_data_sharing",
        ?agent_id,
        opted_in,
        "ACP update confirmed; mirror re-anchored",
    );
    let mut effects = vec![];
    // Ack only after successful opt-in from the privacy banner Accept path.
    if app.privacy_banner_accept_inflight {
        app.privacy_banner_accept_inflight = false;
        if opted_in {
            effects.extend(ack_privacy_banner(app));
        }
    }
    effects
}

pub(super) fn handle_coding_data_sharing_failed(
    app: &mut AppView,
    agent_id: AgentId,
    error: String,
    rollback_to_opted_in: bool,
) -> Vec<Effect> {
    // Revert optimistic mutation: inner → refresh → toast. `agent_id`
    // discarded — privacy is global.
    set_coding_data_sharing_inner(app, rollback_to_opted_in);
    refresh_open_settings_modals(app);
    // Scrub long/unsafe error strings before toasting.
    let scrubbed = scrub_error_for_toast(&error);
    app.show_toast(&format!(
        "\u{2717} Couldn't update coding data sharing: {scrubbed}"
    ));
    tracing::warn!(
        target: "settings",
        key = "coding_data_sharing",
        ?agent_id,
        rollback_to_opted_in,
        %error,
        "ACP update failed; reverted optimistic mutation",
    );
    // Accept failure: no ack; clear inflight so the banner stays.
    app.privacy_banner_accept_inflight = false;
    vec![]
}

/// Stamp `[privacy].privacy_banner_acked` (in-memory + disk).
pub(in crate::app::dispatch) fn ack_privacy_banner(app: &mut AppView) -> Vec<Effect> {
    let acked_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    app.privacy_banner_acked = Some(acked_at.clone());
    vec![Effect::PersistPrivacyBannerAcked { acked_at }]
}

/// Accept: opt-in via settings path; ack only after ACP success.
pub(in crate::app::dispatch) fn dispatch_privacy_banner_accept(app: &mut AppView) -> Vec<Effect> {
    if app.privacy_banner_accept_inflight || !app.privacy_banner_should_show() {
        return vec![];
    }
    let effects = set_coding_data_sharing(app, true);
    // should_show guarantees opted-out + unguarded, so effects is only empty
    // if a guard regresses; leaving inflight false keeps Accept clickable.
    app.privacy_banner_accept_inflight = !effects.is_empty();
    effects
}

/// Customize: ack, then open settings on coding_data_sharing
/// (creates/switches agent when opened from welcome).
pub(in crate::app::dispatch) fn dispatch_privacy_banner_customize(
    app: &mut AppView,
) -> Vec<Effect> {
    if app.privacy_banner_accept_inflight || !app.privacy_banner_should_show() {
        return vec![];
    }
    let mut effects = ack_privacy_banner(app);
    effects.extend(super::settings::ui::dispatch_open_settings(
        app,
        Some("coding_data_sharing"),
    ));
    effects
}

pub(super) fn handle_context_info_complete(
    app: &mut AppView,
    agent_id: AgentId,
    info: Box<xai_grok_shell::session::SessionInfoResponse>,
) -> Vec<Effect> {
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        let model = info.data.model.as_deref().unwrap_or("unknown").to_string();
        // Take ownership of the snapshot once, hand a clone to the
        // agent's running counters, then move the original into the
        // scrollback block (which keeps it for theme-reactive
        // re-rendering). This still costs one clone but reads as
        // "the agent needs a copy" rather than "the block needs a
        // copy", which matches the lifetime story.
        let snapshot = info.data.context;
        agent.apply_full_context_info(snapshot.clone());
        agent
            .scrollback
            .push_block(crate::scrollback::block::RenderBlock::context_info(
                snapshot, model,
            ));
    }
    vec![]
}

// Action handlers.

pub(super) fn dispatch_copy_session_id(app: &mut AppView, index: usize) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    // Try agent modal first, then fall back to app fields (welcome screen).
    let id = get_active_agent(app)
        .and_then(|agent| {
            if let Some(ActiveModal::SessionPicker {
                entries: Some(ref e),
                ..
            }) = agent.active_modal
            {
                e.get(index).map(|entry| entry.id.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            app.session_picker_entries
                .as_ref()
                .and_then(|s| s.get(index))
                .map(|e| e.id.clone())
        });
    if let Some(id) = id {
        let delivery = crate::clipboard::copy_text_or_file(&id);
        app.show_toast(delivery.toast_message().as_ref());
    }
    vec![]
}

/// Open the onboarding tutorial overlay (top-level modal — works over both
/// the welcome screen and an agent session). Toggles: dispatching while
/// open closes instead of stacking.
pub(super) fn dispatch_open_tutorial(app: &mut AppView) -> Vec<Effect> {
    // Minimal mode has no modal host: the overlay would render nothing
    // while the app-level intercept swallowed all input.
    if app.screen_mode.is_minimal() {
        return vec![];
    }
    if app.tutorial.is_some() {
        app.tutorial = None;
        return vec![];
    }
    app.tutorial = Some(crate::views::tutorial::TutorialState::new());
    vec![]
}

pub(super) fn dispatch_show_release_notes(
    app: &mut AppView,
    title: String,
    content: String,
) -> Vec<Effect> {
    match app.active_view {
        ActiveView::Agent(id) => {
            if let Some(agent) = app.agents.get_mut(&id) {
                agent.active_modal = Some(crate::views::modal::ActiveModal::DocViewer {
                    title,
                    content,
                    scroll: 0,
                    window: crate::views::modal_window::ModalWindowState::new(),
                    cached_lines: None,
                    previous_palette: None,
                    standalone: true,
                });
            }
        }
        ActiveView::Welcome => {
            app.welcome_doc_viewer = Some(crate::views::modal::ActiveModal::DocViewer {
                title,
                content,
                scroll: 0,
                window: crate::views::modal_window::ModalWindowState::new(),
                cached_lines: None,
                previous_palette: None,
                standalone: true,
            });
        }
        _ => {}
    }
    vec![]
}
