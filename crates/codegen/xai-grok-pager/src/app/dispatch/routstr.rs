//! `/routstr` product surface: balance, fund gates, top up / refund honesty,
//! and background address watch.

use super::status::dispatch_show_usage;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::block::RenderBlock;

/// Max consecutive watch error system messages before status-only updates.
/// Semantics: exactly this many error lines may enter scrollback; further
/// errors still update `routstr_watch_status` (footer) only.
const WATCH_ERROR_SCROLLBACK_CAP: u32 = 2;

/// Resolve grok home for wallet paths (same layout as CLI).
fn grok_home() -> std::path::PathBuf {
    xai_grok_shell::util::grok_home::grok_home()
}

fn push_system_to_agent(app: &mut AppView, agent_id: AgentId, text: impl Into<String>) {
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        agent.scrollback.push_block(RenderBlock::system(text));
    }
}

fn push_system_active(app: &mut AppView, text: impl Into<String>) {
    let ActiveView::Agent(id) = app.active_view else {
        return;
    };
    push_system_to_agent(app, id, text);
}

fn push_system_lines_active(app: &mut AppView, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    push_system_active(app, lines.join("\n"));
}

/// `/routstr balance` — refresh billing so Routstr float shows in usage.
pub(super) fn dispatch_routstr_balance(app: &mut AppView) -> Vec<Effect> {
    let mut effects = dispatch_show_usage(app);
    if effects.is_empty() {
        effects.push(Effect::FetchAppBilling);
    }
    effects
}

/// Cancel a staged spend if present; notify the staging agent.
///
/// Returns `true` when a pending spend was cleared.
fn clear_pending_routstr_spend(app: &mut AppView, reason: &str) -> bool {
    let Some(pending) = app.pending_routstr_spend.take() else {
        return false;
    };
    push_system_to_agent(
        app,
        pending.agent_id,
        format!(
            "Cancelled staged spend ({reason}): {} sats → {}.",
            pending.amount_sats, pending.address
        ),
    );
    true
}

/// `/routstr fund` — probe vault (async); never mint on keyring errors.
///
/// Also cancels any staged `/routstr spend` so unlock cannot authorize a
/// stale (possibly broadcast) spend after the user switched to fund.
pub(super) fn dispatch_routstr_fund(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        // No agent view: cannot show system block either.
        return vec![];
    };
    let _ = clear_pending_routstr_spend(app, "running /routstr fund");
    push_system_to_agent(
        app,
        id,
        "Checking local Bitcoin wallet (SeedVault). Recovery phrases are never stored in chat history.",
    );
    vec![Effect::RoutstrFundProbe {
        agent_id: id,
        grok_home: grok_home(),
    }]
}

/// Complete re-entry after `/routstr unlock <phrase>`.
///
/// If a pending spend was staged via `/routstr spend`, unlock authorizes that
/// spend (not fund). BIP-39 never enters chat history — only the unlock path
/// carries [`SensitiveString`] into a blocking task.
///
/// Spend completion is bound to the **staging** agent (`pending.agent_id`),
/// not merely the current active view, so switching agents cannot mis-route
/// a money path.
pub(super) fn dispatch_routstr_fund_reentry(
    app: &mut AppView,
    phrase: crate::app::actions::SensitiveString,
    password: Option<crate::app::actions::SensitiveString>,
) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    if phrase.is_empty() {
        push_system_to_agent(app, id, "Recovery phrase re-entry cancelled.");
        return vec![];
    }
    if let Some(pending) = app.pending_routstr_spend.take() {
        // Bind money path to staging agent; reject if active agent differs so
        // the user cannot authorize agent-A's spend from agent-B by accident.
        if pending.agent_id != id {
            // Restore pending so the correct agent can still unlock.
            let staging = pending.agent_id;
            app.pending_routstr_spend = Some(pending);
            push_system_to_agent(
                app,
                id,
                format!(
                    "Staged spend belongs to another agent session (agent {staging:?}). \
                     Switch back to that agent and run /routstr unlock there, or cancel \
                     with /routstr fund on the staging agent."
                ),
            );
            return vec![];
        }
        let mode = if pending.broadcast {
            "broadcast"
        } else {
            "dry-run"
        };
        push_system_to_agent(
            app,
            pending.agent_id,
            format!(
                "Authorizing on-chain spend ({mode}): {} sats → {}. Recovery phrase is not stored in chat.",
                pending.amount_sats, pending.address
            ),
        );
        return vec![Effect::RoutstrSpendComplete {
            agent_id: pending.agent_id,
            grok_home: grok_home(),
            phrase,
            password,
            address: pending.address,
            amount_sats: pending.amount_sats,
            broadcast: pending.broadcast,
            fee_rate_sat_vb: pending.fee_rate_sat_vb,
        }];
    }
    push_system_to_agent(
        app,
        id,
        "Unlocking receive address (backup gate re-entry). Do not fund until the address is shown.",
    );
    vec![Effect::RoutstrFundComplete {
        agent_id: id,
        grok_home: grok_home(),
        phrase,
        password,
    }]
}

/// Stage `/routstr spend` then require unlock re-entry (no BIP-39 on this action).
pub(super) fn dispatch_routstr_spend(
    app: &mut AppView,
    address: String,
    amount_sats: u64,
    broadcast: bool,
    fee_rate_sat_vb: u64,
) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    if app.pending_routstr_spend.is_some() {
        let _ = clear_pending_routstr_spend(app, "superseded by a new /routstr spend");
    }
    app.pending_routstr_spend = Some(crate::app::app_view::PendingRoutstrSpend {
        agent_id: id,
        address: address.clone(),
        amount_sats,
        broadcast,
        fee_rate_sat_vb,
    });
    let mode = if broadcast {
        "with network broadcast"
    } else {
        "dry-run only (not broadcast)"
    };
    push_system_to_agent(
        app,
        id,
        format!(
            "Staged on-chain spend ({mode}): {amount_sats} sats → {address} (fee rate {fee_rate_sat_vb} sat/vB).\n\
             Authorize with: /routstr unlock <recovery phrase words…>\n\
             Optional AEAD password: /routstr unlock pw:<password> <phrase…>\n\
             Recovery words are never stored in chat history. Cancel by staging a different spend or running /routstr fund \
             (fund cancels any staged spend so unlock cannot broadcast a stale one)."
        ),
    );
    vec![]
}

/// Handle spend task result (system block only; no secrets).
///
/// Does **not** clear `pending_routstr_spend`. Unlock already `take()`s the
/// staged params into the in-flight effect. Clearing here would silently drop
/// a **newer** stage created while the prior spend task was still running
/// (no cancel notice). Leave any re-staged pending intact for the next unlock.
pub(super) fn handle_routstr_spend_completed(
    app: &mut AppView,
    agent_id: AgentId,
    result: Result<xai_grok_shell::auth::RoutstrSpendSuccess, String>,
) -> Vec<Effect> {
    match result {
        Ok(success) => {
            push_system_to_agent(app, agent_id, success.lines.join("\n"));
        }
        Err(message) => {
            push_system_to_agent(
                app,
                agent_id,
                format!("Spend failed (not broadcast unless explorer accepted):\n{message}"),
            );
        }
    }
    vec![]
}

/// Honest top-up stub (shared copy with CLI).
pub(super) fn dispatch_routstr_topup(app: &mut AppView, sats: Option<u64>) -> Vec<Effect> {
    let lines = grok_bitcoin_wallet::funding_cli::topup_next_steps_lines(sats);
    push_system_lines_active(app, &lines);
    vec![]
}

/// Honest refund stub (shared copy with CLI).
pub(super) fn dispatch_routstr_refund(app: &mut AppView) -> Vec<Effect> {
    let lines = grok_bitcoin_wallet::funding_cli::refund_next_steps_lines();
    push_system_lines_active(app, &lines);
    vec![]
}

/// Start background watch for `address` on the **active** agent (slash path).
pub(super) fn dispatch_routstr_watch(app: &mut AppView, address: String) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    start_routstr_watch_for_agent(app, id, address, /*immediate*/ true)
}

/// Start watch bound to an explicit `agent_id` (async completions must use this).
///
/// Does **not** consult `app.active_view`. First poll is immediate when
/// `immediate` is true; subsequent re-arms sleep between polls.
///
/// **Singleton watch (intentional):** process-wide generation/address on
/// `AppView`, not per-agent concurrent watches. Tick *messages* still target
/// the owning `agent_id`. Semantics match
/// [`grok_bitcoin_wallet::watcher::WatchTaskLifecycle`] (`start` bumps
/// generation; stale ticks must be dropped via [`watch_tick_accepts`]).
pub(super) fn start_routstr_watch_for_agent(
    app: &mut AppView,
    agent_id: AgentId,
    address: String,
    immediate: bool,
) -> Vec<Effect> {
    let address = address.trim().to_owned();
    if address.is_empty() {
        push_system_to_agent(app, agent_id, "Watch requires a receive address.");
        return vec![];
    }
    if !app.agents.contains_key(&agent_id) {
        return vec![];
    }
    // Supersede note when another agent held the singleton watch.
    if let Some(prev_agent) = app.routstr_watch_agent_id
        && prev_agent != agent_id
        && app.routstr_watch_address.is_some()
    {
        push_system_to_agent(
            app,
            agent_id,
            "Note: superseding the process-wide address watch previously owned by another agent session.",
        );
        push_system_to_agent(
            app,
            prev_agent,
            "Address watch superseded by another agent session (singleton watch).",
        );
    }
    // Mirror WatchTaskLifecycle::start (bump generation, set address).
    app.routstr_watch_generation = app.routstr_watch_generation.saturating_add(1);
    let generation = app.routstr_watch_generation;
    app.routstr_watch_address = Some(address.clone());
    app.routstr_watch_agent_id = Some(agent_id);
    app.routstr_watch_status = Some(format!("Watching {address}: starting"));
    app.routstr_watch_last_scrollback = None;
    app.routstr_watch_error_streak = 0;
    push_system_to_agent(
        app,
        agent_id,
        format!(
            "Watching receive address for deposits (mempool.space, ~{}s between polls). \
             Use /routstr stop to cancel. (One process-wide watch at a time.)",
            grok_bitcoin_wallet::watcher::DEFAULT_WATCH_POLL_INTERVAL.as_secs()
        ),
    );
    let network = std::env::var("GROK_BITCOIN_NETWORK").unwrap_or_else(|_| "mainnet".into());
    vec![Effect::RoutstrWatchLoop {
        agent_id,
        address,
        generation,
        network,
        skip_sleep: immediate,
    }]
}

pub(super) fn dispatch_routstr_watch_stop(app: &mut AppView) -> Vec<Effect> {
    if app.routstr_watch_address.is_none() && app.routstr_watch_generation == 0 {
        push_system_active(app, "No address watch is running.");
        return vec![];
    }
    // Mirror WatchTaskLifecycle::stop (bump generation while running, clear address).
    app.routstr_watch_generation = app.routstr_watch_generation.saturating_add(1);
    app.routstr_watch_address = None;
    app.routstr_watch_agent_id = None;
    app.routstr_watch_status = None;
    app.routstr_watch_last_scrollback = None;
    app.routstr_watch_error_streak = 0;
    push_system_active(app, "Address watch stopped.");
    vec![]
}

/// Whether a watch tick should apply — same rule as
/// [`grok_bitcoin_wallet::watcher::WatchTaskLifecycle::accepts`].
fn watch_tick_accepts(app: &AppView, tick_generation: u64, address: &str) -> bool {
    app.routstr_watch_address.is_some()
        && app.routstr_watch_generation > 0
        && tick_generation == app.routstr_watch_generation
        && app.routstr_watch_address.as_deref() == Some(address)
}

/// Handle probe TaskResult: guide unlock or CLI for new wallet.
pub(super) fn handle_routstr_fund_probed(
    app: &mut AppView,
    agent_id: AgentId,
    probe: xai_grok_shell::auth::RoutstrFundProbe,
) -> Vec<Effect> {
    use xai_grok_shell::auth::RoutstrFundProbe;
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    match probe {
        RoutstrFundProbe::NeedCliNewWallet { aead_hint } => {
            agent.scrollback.push_block(RenderBlock::system(format!(
                "No local Bitcoin wallet found.\n\
                 Creating a recovery phrase must happen in a private terminal so words \
                 are not written to chat history.\n\
                 Run: grok routstr fund\n\
                 Seed storage: OS keyring when available, otherwise {aead_hint}\n\
                 Never provider_credentials.json.\n\
                 After fund completes, run /routstr fund then /routstr unlock <phrase> here."
            )));
            vec![]
        }
        RoutstrFundProbe::KeyringBlocked { message } => {
            agent
                .scrollback
                .push_block(RenderBlock::system(format!("Fund blocked: {message}")));
            vec![]
        }
        RoutstrFundProbe::Error { message } => {
            agent
                .scrollback
                .push_block(RenderBlock::system(format!("Fund error: {message}")));
            vec![]
        }
        RoutstrFundProbe::NeedPassword => {
            agent.scrollback.push_block(RenderBlock::system(
                "Local wallet is password-wrapped. Unlock with:\n\
                 /routstr unlock pw:<password> <recovery phrase words…>\n\
                 Password must be a single token (no spaces). Words are not re-displayed.",
            ));
            vec![]
        }
        RoutstrFundProbe::NeedReentry => {
            agent.scrollback.push_block(RenderBlock::system(
                "Local wallet found. Re-enter your recovery phrase (not re-displayed):\n\
                 /routstr unlock <word1 word2 … word12>\n\
                 Then the receive address is shown and deposit watching can start.",
            ));
            vec![]
        }
    }
}

/// Handle fund complete TaskResult.
pub(super) fn handle_routstr_fund_completed(
    app: &mut AppView,
    agent_id: AgentId,
    result: Result<xai_grok_shell::auth::RoutstrFundSuccess, String>,
) -> Vec<Effect> {
    match result {
        Ok(success) => {
            present_receive_address(app, agent_id, &success.address, Some(&success.lines));
            // Bind watch to the task's agent, not active_view (multi-session safe).
            start_routstr_watch_for_agent(app, agent_id, success.address, /*immediate*/ true)
        }
        Err(e) => {
            push_system_to_agent(app, agent_id, format!("Fund failed: {e}"));
            vec![]
        }
    }
}

/// Show receive address + BIP21 QR and copy address to clipboard with toast.
///
/// `preamble_lines` are optional fund-success lines (status / network). Never
/// invents BOLT11; on-chain / BIP21 only.
pub(super) fn present_receive_address(
    app: &mut AppView,
    agent_id: AgentId,
    address: &str,
    preamble_lines: Option<&[String]>,
) {
    let address = address.trim();
    if address.is_empty() {
        push_system_to_agent(app, agent_id, "No receive address to display.");
        return;
    }
    let mut block_lines: Vec<String> = preamble_lines.map(|p| p.to_vec()).unwrap_or_default();
    let display_lines = grok_bitcoin_wallet::funding_cli::receive_address_display_lines(
        address, /*include_qr*/ true,
    );
    // Avoid duplicating the bare address line when preamble already printed it.
    for line in display_lines {
        if block_lines.iter().any(|existing| existing == &line) {
            continue;
        }
        block_lines.push(line);
    }
    let clipboard = grok_bitcoin_wallet::funding_cli::receive_address_clipboard(address);
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        if !block_lines.is_empty() {
            agent
                .scrollback
                .push_block(RenderBlock::system(block_lines.join("\n")));
        }
        // Copy + toast (route-aware delivery message via clipboard helper).
        let _ = agent.copy_to_clipboard(&clipboard);
    }
}

/// `/routstr qr [address]` — re-show QR + copy for an address (or last watch).
pub(super) fn dispatch_routstr_qr(app: &mut AppView, address: Option<String>) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let resolved = address
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .or_else(|| app.routstr_watch_address.clone());
    match resolved {
        Some(addr) => {
            present_receive_address(app, id, &addr, None);
        }
        None => {
            push_system_to_agent(
                app,
                id,
                "Usage: /routstr qr <receive-address>\n\
                 Or start a watch first so /routstr qr can reuse that address.",
            );
        }
    }
    vec![]
}

/// Whether this tick status should also land in scrollback (transitions only).
///
/// For errors, call **before** incrementing `routstr_watch_error_streak` so
/// `WATCH_ERROR_SCROLLBACK_CAP` is the actual number of error lines allowed
/// (`streak < CAP` with streak still counting already-shown errors).
fn watch_tick_should_scrollback(
    app: &AppView,
    status_line: &str,
    confirmed: bool,
    is_error: bool,
) -> bool {
    if confirmed {
        return true;
    }
    if is_error {
        return app.routstr_watch_error_streak < WATCH_ERROR_SCROLLBACK_CAP;
    }
    app.routstr_watch_last_scrollback.as_deref() != Some(status_line)
}

/// Apply one watch tick if generation is still current.
pub(super) fn handle_routstr_watch_tick(
    app: &mut AppView,
    agent_id: AgentId,
    generation: u64,
    status_line: String,
    confirmed: bool,
    address: String,
) -> Vec<Effect> {
    // Keep in sync with WatchTaskLifecycle::accepts (singleton AppView fields).
    if !watch_tick_accepts(app, generation, &address) {
        return vec![];
    }
    app.routstr_watch_status = Some(status_line.clone());
    let is_error = status_line.starts_with("Watch error:");
    // Decide scrollback using the pre-increment streak so CAP means "this many
    // error lines", then bump (or reset) the streak for the next tick.
    if watch_tick_should_scrollback(app, &status_line, confirmed, is_error) {
        push_system_to_agent(app, agent_id, status_line.clone());
        if !is_error {
            app.routstr_watch_last_scrollback = Some(status_line.clone());
        }
    }
    if is_error {
        app.routstr_watch_error_streak = app.routstr_watch_error_streak.saturating_add(1);
    } else {
        app.routstr_watch_error_streak = 0;
    }
    if confirmed {
        app.routstr_watch_address = None;
        app.routstr_watch_agent_id = None;
        app.routstr_watch_status = Some("Deposit confirmed enough for next funding steps.".into());
        app.routstr_watch_last_scrollback = None;
        app.routstr_watch_error_streak = 0;
        push_system_to_agent(
            app,
            agent_id,
            "Deposit has enough confirmations. Channel open / Cashu acquire remain residual. \
             Use /routstr topup for node float next steps.",
        );
        return vec![];
    }
    // Re-arm poll loop while generation is current (stop bumps generation).
    // Subsequent polls sleep first for rate-limit honesty.
    let network = std::env::var("GROK_BITCOIN_NETWORK").unwrap_or_else(|_| "mainnet".into());
    vec![Effect::RoutstrWatchLoop {
        agent_id,
        address,
        generation,
        network,
        skip_sleep: false,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::actions::Action;
    use crate::app::dispatch::dispatch;
    use crate::app::dispatch::tests::test_app_with_agent;

    #[test]
    fn topup_and_refund_push_honest_copy() {
        let mut app = test_app_with_agent();
        let _ = dispatch(Action::RoutstrTopup { sats: Some(1000) }, &mut app);
        let _ = dispatch(Action::RoutstrRefund, &mut app);
        let agent = app.agents.values().next().unwrap();
        let text: String = (0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        let lower = text.to_ascii_lowercase();
        assert!(
            lower.contains("not wired") || lower.contains("not available"),
            "expected honest stub wording: {text}"
        );
        assert!(!lower.contains("invoice created"));
    }

    #[test]
    fn fund_probe_effect_emitted() {
        let mut app = test_app_with_agent();
        let effects = dispatch(Action::RoutstrFund, &mut app);
        assert!(
            matches!(effects.first(), Some(Effect::RoutstrFundProbe { .. })),
            "expected probe effect: {effects:?}"
        );
    }

    #[test]
    fn spend_stages_pending_without_bip39_and_unlock_routes_to_spend() {
        use crate::app::actions::SensitiveString;
        let mut app = test_app_with_agent();
        let effects = dispatch(
            Action::RoutstrSpend {
                address: "bc1qdest".into(),
                amount_sats: 1000,
                broadcast: false,
                fee_rate_sat_vb: 5,
            },
            &mut app,
        );
        assert!(effects.is_empty());
        let pending = app.pending_routstr_spend.as_ref().expect("pending spend");
        assert_eq!(pending.address, "bc1qdest");
        assert_eq!(pending.amount_sats, 1000);
        assert!(!pending.broadcast);
        assert_eq!(pending.agent_id, AgentId(0));

        let effects = dispatch(
            Action::RoutstrFundReentry {
                phrase: SensitiveString::new("abandon abandon abandon"),
                password: None,
            },
            &mut app,
        );
        assert!(
            matches!(
                effects.first(),
                Some(Effect::RoutstrSpendComplete {
                    amount_sats: 1000,
                    broadcast: false,
                    agent_id: AgentId(0),
                    ..
                })
            ),
            "unlock with pending spend must complete spend, not fund: {effects:?}"
        );
        assert!(
            app.pending_routstr_spend.is_none(),
            "pending consumed into effect"
        );
        let dbg = format!("{:?}", effects.first());
        assert!(!dbg.contains("abandon"), "Debug leaked phrase: {dbg}");
    }

    #[test]
    fn fund_clears_pending_spend_so_unlock_routes_to_fund() {
        use crate::app::actions::SensitiveString;
        let mut app = test_app_with_agent();
        let _ = dispatch(
            Action::RoutstrSpend {
                address: "bc1qstale".into(),
                amount_sats: 99_000,
                broadcast: true,
                fee_rate_sat_vb: 5,
            },
            &mut app,
        );
        assert!(app.pending_routstr_spend.is_some());

        let effects = dispatch(Action::RoutstrFund, &mut app);
        assert!(
            matches!(effects.first(), Some(Effect::RoutstrFundProbe { .. })),
            "expected fund probe: {effects:?}"
        );
        assert!(
            app.pending_routstr_spend.is_none(),
            "fund must cancel staged spend so unlock cannot broadcast it"
        );
        // System copy should note cancellation.
        let agent = app.agents.values().next().unwrap();
        let text: String = (0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(
            text.to_ascii_lowercase().contains("cancelled staged spend"),
            "expected cancel notice: {text}"
        );

        let effects = dispatch(
            Action::RoutstrFundReentry {
                phrase: SensitiveString::new("abandon abandon abandon"),
                password: None,
            },
            &mut app,
        );
        assert!(
            matches!(effects.first(), Some(Effect::RoutstrFundComplete { .. })),
            "after fund cancel, unlock must fund not spend: {effects:?}"
        );
        assert!(
            !matches!(effects.first(), Some(Effect::RoutstrSpendComplete { .. })),
            "must not complete stale spend: {effects:?}"
        );
    }

    #[test]
    fn unlock_rejects_spend_when_active_agent_differs_from_staging() {
        use crate::app::actions::SensitiveString;
        let mut app = test_app_with_agent();
        crate::app::dispatch::tests::insert_placeholder_agent(&mut app, AgentId(1));
        // Stage on agent 0.
        let _ = dispatch(
            Action::RoutstrSpend {
                address: "bc1qagent0".into(),
                amount_sats: 500,
                broadcast: true,
                fee_rate_sat_vb: 5,
            },
            &mut app,
        );
        assert_eq!(
            app.pending_routstr_spend.as_ref().map(|p| p.agent_id),
            Some(AgentId(0))
        );
        // Switch to agent 1 and try unlock.
        app.active_view = ActiveView::Agent(AgentId(1));
        let effects = dispatch(
            Action::RoutstrFundReentry {
                phrase: SensitiveString::new("abandon abandon abandon"),
                password: None,
            },
            &mut app,
        );
        assert!(
            effects.is_empty(),
            "cross-agent unlock must not authorize spend: {effects:?}"
        );
        assert!(
            app.pending_routstr_spend.is_some(),
            "pending must remain for the staging agent"
        );
        assert_eq!(
            app.pending_routstr_spend.as_ref().map(|p| p.agent_id),
            Some(AgentId(0))
        );
    }

    #[test]
    fn spend_task_complete_does_not_drop_newer_staged_spend() {
        use crate::app::actions::SensitiveString;
        let mut app = test_app_with_agent();
        // Stage A then unlock (consumes A into in-flight effect).
        let _ = dispatch(
            Action::RoutstrSpend {
                address: "bc1qstage-a".into(),
                amount_sats: 1000,
                broadcast: false,
                fee_rate_sat_vb: 5,
            },
            &mut app,
        );
        let unlock_effects = dispatch(
            Action::RoutstrFundReentry {
                phrase: SensitiveString::new("abandon abandon abandon"),
                password: None,
            },
            &mut app,
        );
        assert!(
            matches!(
                unlock_effects.first(),
                Some(Effect::RoutstrSpendComplete {
                    amount_sats: 1000,
                    ..
                })
            ),
            "stage A unlock: {unlock_effects:?}"
        );
        assert!(app.pending_routstr_spend.is_none());

        // Re-stage B while A is still "in flight".
        let _ = dispatch(
            Action::RoutstrSpend {
                address: "bc1qstage-b".into(),
                amount_sats: 2500,
                broadcast: true,
                fee_rate_sat_vb: 8,
            },
            &mut app,
        );
        let pending_b = app
            .pending_routstr_spend
            .as_ref()
            .expect("stage B must be pending");
        assert_eq!(pending_b.address, "bc1qstage-b");
        assert_eq!(pending_b.amount_sats, 2500);
        assert!(pending_b.broadcast);

        // Completion of A must not wipe B (no silent drop).
        let _ = handle_routstr_spend_completed(
            &mut app,
            AgentId(0),
            Ok(xai_grok_shell::auth::RoutstrSpendSuccess {
                payment_address: "bc1qstage-a".into(),
                payment_sats: 1000,
                fee_sats: 50,
                change_sats: 0,
                txid: "a".repeat(64),
                raw_hex: "ab".repeat(20),
                broadcast_txid: None,
                network_label: "mainnet".into(),
                lines: vec!["prepared stage A (simulated)".into()],
            }),
        );
        let still = app
            .pending_routstr_spend
            .as_ref()
            .expect("stage B must survive completion of A");
        assert_eq!(still.address, "bc1qstage-b");
        assert_eq!(still.amount_sats, 2500);
        assert!(still.broadcast);
        assert_eq!(still.fee_rate_sat_vb, 8);

        // Unlock still authorizes B, not a wiped/missing pending fund path.
        let effects = dispatch(
            Action::RoutstrFundReentry {
                phrase: SensitiveString::new("abandon abandon abandon"),
                password: None,
            },
            &mut app,
        );
        assert!(
            matches!(
                effects.first(),
                Some(Effect::RoutstrSpendComplete {
                    amount_sats: 2500,
                    broadcast: true,
                    ..
                })
            ),
            "unlock after A complete must still spend B: {effects:?}"
        );
    }

    #[test]
    fn watch_start_and_stop_bump_generation() {
        let mut app = test_app_with_agent();
        let effects = dispatch(
            Action::RoutstrWatch {
                address: "bc1qtest0000000000000000000000000000".into(),
            },
            &mut app,
        );
        assert!(matches!(
            effects.first(),
            Some(Effect::RoutstrWatchLoop {
                generation: 1,
                skip_sleep: true,
                ..
            })
        ));
        assert_eq!(app.routstr_watch_generation, 1);
        assert_eq!(app.routstr_watch_agent_id, Some(AgentId(0)));
        let _ = dispatch(Action::RoutstrWatchStop, &mut app);
        assert_eq!(app.routstr_watch_generation, 2);
        assert!(app.routstr_watch_address.is_none());
        assert!(app.routstr_watch_agent_id.is_none());
    }

    #[test]
    fn watch_start_notes_when_superseding_other_agent() {
        let mut app = test_app_with_agent();
        // Second agent placeholder (same helper as multi-agent dispatch tests).
        crate::app::dispatch::tests::insert_placeholder_agent(&mut app, AgentId(1));
        let id0 = AgentId(0);
        let id1 = AgentId(1);
        let _ = start_routstr_watch_for_agent(
            &mut app,
            id0,
            "bc1qfirst0000000000000000000000000".into(),
            true,
        );
        let _ = start_routstr_watch_for_agent(
            &mut app,
            id1,
            "bc1qsecond000000000000000000000000".into(),
            true,
        );
        assert_eq!(app.routstr_watch_agent_id, Some(id1));
        let a1 = app.agents.get(&id1).unwrap();
        let t1: String = (0..a1.scrollback.len())
            .filter_map(|i| a1.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(
            t1.to_ascii_lowercase().contains("superseding"),
            "new owner should see supersede note: {t1}"
        );
        let a0 = app.agents.get(&id0).unwrap();
        let t0: String = (0..a0.scrollback.len())
            .filter_map(|i| a0.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(
            t0.to_ascii_lowercase().contains("superseded"),
            "previous owner should be notified: {t0}"
        );
    }

    #[test]
    fn stale_watch_tick_dropped() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        app.routstr_watch_generation = 2;
        app.routstr_watch_address = Some("bc1q".into());
        let effects = handle_routstr_watch_tick(
            &mut app,
            id,
            1, // stale
            "old".into(),
            false,
            "bc1q".into(),
        );
        assert!(effects.is_empty());
        assert_ne!(app.routstr_watch_status.as_deref(), Some("old"));
    }

    #[test]
    fn keyring_blocked_probe_does_not_mint() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let _ = handle_routstr_fund_probed(
            &mut app,
            id,
            xai_grok_shell::auth::RoutstrFundProbe::KeyringBlocked {
                message: "could not read seed vault (down); not creating a new wallet.".into(),
            },
        );
        let agent = app.agents.get(&id).unwrap();
        let text: String = (0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(text.contains("not creating a new wallet"));
        assert!(!text.to_ascii_lowercase().contains("generating a new"));
    }

    #[test]
    fn fund_complete_auto_watch_uses_task_agent_not_active_view() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        // Simulate user left agent view while unlock was in flight.
        app.active_view = ActiveView::Welcome;
        let effects = handle_routstr_fund_completed(
            &mut app,
            id,
            Ok(xai_grok_shell::auth::RoutstrFundSuccess {
                address: "bc1qfundcomplete000000000000000000".into(),
                network_label: "mainnet".into(),
                step_label: "showing receive address".into(),
                lines: vec!["ok".into()],
            }),
        );
        assert!(
            matches!(
                effects.first(),
                Some(Effect::RoutstrWatchLoop {
                    agent_id: AgentId(0),
                    skip_sleep: true,
                    ..
                })
            ),
            "auto-watch must target fund agent even when active_view is Welcome: {effects:?}"
        );
        assert_eq!(
            app.routstr_watch_address.as_deref(),
            Some("bc1qfundcomplete000000000000000000")
        );
    }

    #[test]
    fn fund_complete_shows_qr_and_sets_clipboard_toast_on_task_agent() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let addr = "bc1q8zxz5kl6q30y2mzhx86gcwcz0t0hgzl2f2jpm5";
        let _ = handle_routstr_fund_completed(
            &mut app,
            id,
            Ok(xai_grok_shell::auth::RoutstrFundSuccess {
                address: addr.into(),
                network_label: "mainnet".into(),
                step_label: "showing receive address".into(),
                lines: vec![
                    "Backup confirmed. Receive address (mainnet):".into(),
                    addr.into(),
                ],
            }),
        );
        let agent = app.agents.get(&id).unwrap();
        let text: String = (0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(text.contains(addr), "address must appear: {text}");
        assert!(
            text.contains("bitcoin:") || text.contains("BIP21"),
            "BIP21 / QR section expected: {text}"
        );
        // No fabricated BOLT11 in on-chain fund path.
        assert!(
            !text.to_ascii_lowercase().contains("lnbc"),
            "must not invent BOLT11: {text}"
        );
        // Clipboard toast on the fund agent (copy_to_clipboard path).
        assert!(
            agent.toast.is_some(),
            "expected clipboard toast after fund complete"
        );
        let toast = agent.toast.as_ref().map(|(m, _)| m.as_str()).unwrap_or("");
        assert!(
            toast.starts_with("Copied")
                || toast.starts_with("Copy sent")
                || toast.starts_with("Copy failed")
                || toast.to_ascii_lowercase().contains("clipboard"),
            "unexpected clipboard toast: {toast}"
        );
    }

    #[test]
    fn routstr_qr_uses_watch_address_when_none_given() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        app.routstr_watch_address = Some("bc1qwatchqr00000000000000000000000".into());
        let _ = dispatch(Action::RoutstrQr { address: None }, &mut app);
        let agent = app.agents.get(&id).unwrap();
        let text: String = (0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .collect();
        assert!(
            text.contains("bc1qwatchqr00000000000000000000000"),
            "qr must reuse watch address: {text}"
        );
        assert!(agent.toast.is_some(), "qr path should toast clipboard copy");
    }

    #[test]
    fn watch_tick_dedupes_identical_status_scrollback() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        app.routstr_watch_generation = 1;
        app.routstr_watch_address = Some("bc1q".into());
        let status = "Watching bc1q: waiting for payment".to_string();
        let e1 = handle_routstr_watch_tick(&mut app, id, 1, status.clone(), false, "bc1q".into());
        assert!(matches!(
            e1.first(),
            Some(Effect::RoutstrWatchLoop {
                skip_sleep: false,
                ..
            })
        ));
        let len_after_first = app.agents.get(&id).unwrap().scrollback.len();
        let _ = handle_routstr_watch_tick(&mut app, id, 1, status, false, "bc1q".into());
        let len_after_second = app.agents.get(&id).unwrap().scrollback.len();
        assert_eq!(
            len_after_first, len_after_second,
            "identical status must not spam scrollback"
        );
    }

    #[test]
    fn watch_error_scrollback_allows_exactly_cap_lines() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        app.routstr_watch_generation = 1;
        app.routstr_watch_address = Some("bc1q".into());
        let len0 = app.agents.get(&id).unwrap().scrollback.len();

        for i in 0..(WATCH_ERROR_SCROLLBACK_CAP + 2) {
            let status = format!("Watch error: transient {i}");
            let _ = handle_routstr_watch_tick(&mut app, id, 1, status, false, "bc1q".into());
            // Footer/status always updates even past the cap.
            assert!(
                app.routstr_watch_status
                    .as_deref()
                    .is_some_and(|s| s.starts_with("Watch error:")),
                "status must update on every error tick"
            );
        }

        let agent = app.agents.get(&id).unwrap();
        let error_blocks: usize = (len0..agent.scrollback.len())
            .filter_map(|i| agent.scrollback.entry(i).map(|e| format!("{:?}", e.block)))
            .filter(|t| t.contains("Watch error:"))
            .count();
        assert_eq!(
            error_blocks, WATCH_ERROR_SCROLLBACK_CAP as usize,
            "exactly CAP error lines should reach scrollback; got {error_blocks}"
        );
        assert_eq!(
            app.routstr_watch_error_streak,
            WATCH_ERROR_SCROLLBACK_CAP + 2,
            "streak counts every consecutive error, including past CAP"
        );
    }
}
