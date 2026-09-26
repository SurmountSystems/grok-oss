//! Finalizing a turn from a terminal turn signal.
//!
//! The pager learns a turn reached its terminal outcome from two rails: the
//! fire-and-forget `x.ai/session/prompt_complete` broadcast (the one-release
//! compat path for not-yet-upgraded leaders) and the durable, persisted+replayed
//! `XaiSessionUpdate::TurnCompleted`. Both converge on
//! [`finalize_turn_from_terminal`] so the turn-finalize behavior lives in one
//! place — and so a viewer that re-attaches mid-turn can finalize the turn from
//! replay instead of staying stuck on "Waiting…".

use crate::scrollback::blocks::SessionEvent;

use super::agent::AgentId;
use super::agent_view::AgentView;
use super::app_view::AppView;

/// Push a turn-terminal marker ("Turn completed/cancelled/failed"), folding
/// any pending stop/stop_failure hook runs into it so they render inline
/// (right-justified) on the marker line instead of as a standalone block.
///
/// All three marker rails route through here: the driver's `PromptResponse`,
/// the lost-RPC reconcile, and the viewer finalize. (Wake turns route through
/// `finish_wake_turn` in acp_handler, which maps their stop reason and calls
/// here only when a marker is due.) `event == None`
/// (bash turns, rate-limit / re-auth UX that replaces the marker) flushes the
/// held hooks as the legacy standalone lifecycle block so failures stay
/// visible.
///
/// A stamped stash folds only on an exact ending-id match. On a mismatch it
/// flushes standalone (the ending turn is THE turn — an older stash has no
/// marker coming). An unstamped stash keeps the legacy
/// stashed-during-this-turn heuristic.
pub(super) fn push_turn_terminal_marker(
    agent: &mut AgentView,
    event: Option<SessionEvent>,
    ending_prompt_id: Option<&str>,
) {
    let pending = agent.pending_stop_hooks.take();
    let groups = match pending {
        None => Vec::new(),
        Some(pending) => {
            let stale = match (pending.prompt_id.as_deref(), ending_prompt_id) {
                (Some(stashed), Some(ending)) => stashed != ending,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if stale {
                for (name, runs) in pending.groups {
                    agent.scrollback.push_lifecycle_hooks(name, runs);
                }
                Vec::new()
            } else {
                pending.groups
            }
        }
    };

    match event {
        Some(event) => {
            agent.push_end_marker_block(event, groups, ending_prompt_id.map(str::to_string));
        }
        None => {
            for (name, runs) in groups {
                agent.scrollback.push_lifecycle_hooks(name, runs);
            }
        }
    }
}

/// What applying a terminal turn signal did to one agent.
pub(super) enum TerminalApply {
    /// No change: a driver turn the signal does not provably match, or a
    /// duplicate/stale terminal for an already-finished viewer turn.
    Ignored,
    /// Driver: the lost-RPC reconcile was armed. The turn is NOT finished — the
    /// `PromptResponse` RPC owns the driver's lifecycle. Reported as a state
    /// change so the reconcile sweep's animation tick stays scheduled.
    ReconcileArmed,
    /// Viewer: the turn was finished and (for non-rate-limit reasons) a terminal
    /// marker pushed. The caller drops any stale running-prompt adoption.
    ViewerFinalized,
}

/// Arm lost-`PromptResponse` reconcile for the driver turn we own.
///
/// - **Exact** `prompt_id` match → arm (canonical).
/// - **Missing** wire `promptId` (`None` or empty) → arm on `current_prompt_id`
///   only when the turn is not mid-tool/thinking/compact/retry (legacy /
///   broken `TurnCompleted` payloads).
/// - **Non-empty mismatch** → ignore (stale/peer terminal must not kill a
///   newer live turn after grace).
///
/// Never clobber an existing arm for a different pid; keep earliest
/// `received_at` when re-arming the same pid.
fn arm_driver_turn_end_reconcile(
    agent: &mut AgentView,
    session_id: &str,
    prompt_id: Option<&str>,
    stop_reason: Option<&str>,
    agent_result: Option<&str>,
    cancel_trigger: Option<&str>,
) -> bool {
    if agent.session.loading_replay {
        return false;
    }
    if !(agent.session.state.is_turn_running() || agent.session.state.is_cancelling()) {
        return false;
    }
    let Some(current) = agent.session.current_prompt_id.clone() else {
        return false;
    };

    let (arm_pid, arm_via) = match prompt_id {
        Some(pid) if pid == current.as_str() => (current, "exact"),
        Some("") => {
            if driver_mid_active_work(agent) {
                return false;
            }
            (current, "empty_wire_pid")
        }
        Some(_) => return false,
        None => {
            if driver_mid_active_work(agent) {
                return false;
            }
            (current, "missing_wire_pid")
        }
    };

    if let Some(pending) = agent.pending_turn_end_reconcile.as_ref() {
        if pending.prompt_id != arm_pid {
            return false;
        }
        // Same pid already armed — keep earliest received_at; refresh outcome.
        let received_at = pending.received_at;
        agent.pending_turn_end_reconcile = Some(super::agent_view::PendingTurnEnd {
            prompt_id: arm_pid.clone(),
            stop_reason: stop_reason.map(str::to_string),
            agent_result: agent_result.map(str::to_string),
            cancel_trigger: cancel_trigger.map(str::to_string),
            received_at,
        });
        crate::unified_log::info(
            "turn.end_reconcile.armed",
            Some(session_id),
            Some(serde_json::json!({
                "prompt_id": arm_pid,
                "wire_prompt_id": prompt_id,
                "arm_via": arm_via,
                "stop_reason": stop_reason,
                "refreshed": true,
            })),
        );
        return true;
    }

    crate::unified_log::info(
        "turn.end_reconcile.armed",
        Some(session_id),
        Some(serde_json::json!({
            "prompt_id": arm_pid,
            "wire_prompt_id": prompt_id,
            "arm_via": arm_via,
            "stop_reason": stop_reason,
        })),
    );
    agent.pending_turn_end_reconcile = Some(super::agent_view::PendingTurnEnd {
        prompt_id: arm_pid,
        stop_reason: stop_reason.map(str::to_string),
        agent_result: agent_result.map(str::to_string),
        cancel_trigger: cancel_trigger.map(str::to_string),
        received_at: std::time::Instant::now(),
    });
    true
}

/// Formatted `TurnFailed` marker for an errored turn, or `None` when a
/// dedicated banner (re-auth, overflow, disk-full, request-failed) already
/// covers the failure.
pub(in crate::app) fn turn_failed_event(
    scrollback: &crate::scrollback::state::ScrollbackState,
    agent_result: Option<&str>,
    elapsed: std::time::Duration,
) -> Option<SessionEvent> {
    if super::dispatch::scrollback_has_recent_error_banner(scrollback) {
        return None;
    }
    let raw = agent_result.unwrap_or("unknown error");
    Some(SessionEvent::TurnFailed {
        error: crate::app::error_display::format_request_failure(None, None, raw).message(),
        elapsed: Some(elapsed),
    })
}

fn driver_mid_active_work(agent: &AgentView) -> bool {
    use crate::acp::tracker::TurnActivity;
    // A write whose delta stream died is positive evidence of a dead stream;
    // whatever shows through it (an open thinking block, an earlier tool)
    // must not block lost-response recovery.
    if agent.session.tracker.has_stale_tool_call_write() {
        return false;
    }
    match agent.session.tracker.activity() {
        Some(
            TurnActivity::ToolRunning { .. }
            | TurnActivity::Thinking
            | TurnActivity::AutoCompacting
            | TurnActivity::Retrying { .. }
            | TurnActivity::WritingToolCall(_),
        ) => true,
        Some(TurnActivity::Responding | TurnActivity::Waiting(_)) | None => false,
    }
}

/// Finalize a turn from a terminal signal, shared by the `prompt_complete`
/// broadcast and the durable `TurnCompleted` update so both behave identically.
///
/// DRIVER (`!attached_as_viewer`): the `PromptResponse` RPC owns the turn
/// lifecycle (it carries context this signal lacks: error classes, rewind
/// bookkeeping, adoption hand-off), so do NOT finish the turn here — that would
/// race/double-finish on every normal turn end (the signal is emitted BEFORE the
/// RPC response is written). But the RPC response can be LOST in transit (leader
/// response routing / reconnect races), and
/// it is the ONLY exit from `TurnRunning`/`TurnCancelling`. So when the signal
/// refers to the turn this client is driving (exact pid, or missing/empty pid
/// while not mid-tool), arm a deferred reconcile: if the RPC lands within the
/// grace window it disarms this (see `TaskResult::PromptResponse`); otherwise
/// the event loop finishes the turn from it (`reconcile_overdue_turn_ends`).
///
/// VIEWER (`attached_as_viewer`): a viewer adopts the driver's turn and never
/// receives its `PromptResponse`, so this is its only non-interactive exit from
/// `TurnRunning`. Finish the turn and push the "Turn completed/cancelled/failed"
/// marker mapped from `stop_reason`. Idempotent: a duplicate/stale terminal for
/// an already-finished turn pushes nothing and returns [`TerminalApply::Ignored`].
///
/// `cancel_trigger` is the signal's `_meta.cancelTrigger`, when stamped:
/// `"send_now"` marks the cancel as the silent half of a cancel-and-send, so
/// the `TurnCancelled` marker is suppressed (the sender's new prompt renders
/// as the next turn). Absent meta means a normal cancel — except when this
/// client just dispatched the send-now (`AgentView::expect_send_now_cancel`,
/// the older-shell fallback, consumed here on the viewer finalize).
pub(super) fn finalize_turn_from_terminal(
    agent: &mut AgentView,
    session_id: &str,
    prompt_id: Option<&str>,
    stop_reason: Option<&str>,
    agent_result: Option<&str>,
    cancel_trigger: Option<&str>,
) -> TerminalApply {
    if !agent.attached_as_viewer {
        if arm_driver_turn_end_reconcile(
            agent,
            session_id,
            prompt_id,
            stop_reason,
            agent_result,
            cancel_trigger,
        ) {
            return TerminalApply::ReconcileArmed;
        }
        return TerminalApply::Ignored;
    }

    // Viewer: the driver's turn ended — exit TurnRunning. Only act when a turn
    // is actually in progress so a stray/duplicate signal is harmless (a
    // duplicate finds the turn already finished here and pushes no marker).
    if !agent.session.state.is_busy() && agent.session.current_prompt_id.is_none() {
        return TerminalApply::Ignored;
    }

    // Capture elapsed BEFORE `mark_turn_finished()` clears `turn_started_at`. The
    // anchor was back-dated from the authoritative `turnStartMs` on adoption, so
    // this reads the same wall-clock duration the driver shows.
    let elapsed = agent.turn_elapsed().unwrap_or_default();
    // Read before `finish_turn()` clears it; keys the pending stop-hook stash.
    let ending_prompt_id = agent
        .session
        .current_prompt_id
        .clone()
        .or_else(|| prompt_id.map(str::to_string));

    // Viewer never receives PromptResponse. Record the outcome this signal
    // already named. Tokens were not on this signal.
    record_terminal_stop_reason(
        agent,
        stop_reason,
        agent_result,
        agent.turn_elapsed(),
        matches!(stop_reason, Some("cancelled")),
    );

    // Viewer never receives PromptResponse; write metrics if the row is still open.
    agent.complete_live_prompt_task(ending_prompt_id.as_deref(), None);
    agent.session.finish_turn(&mut agent.scrollback);

    // Wire meta wins; else the client-side expectation (older-shell fallback).
    // Taken at every viewer finalize so it can't go stale.
    let expected_send_now = agent.expect_send_now_cancel.take();
    let send_now_cancel = match cancel_trigger {
        Some(trigger) => trigger == "send_now",
        None => expected_send_now.is_some(),
    };

    // A viewer never receives the driver's `PromptResponse` RPC — the source of
    // the driver's "Worked for X" marker. Surface the equivalent here.
    // The signal only carries a coarse `stop_reason` (no doom-loop category, no
    // driver-local rate-limit / re-auth context), so map it to the closest event:
    let event = match stop_reason {
        // Send-now cancel: no marker (the sender's new prompt renders as the
        // next turn; neither cancelled nor a substitute completed).
        Some("cancelled") if send_now_cancel => None,
        Some("cancelled") => Some(SessionEvent::TurnCancelled { elapsed }),
        // Rate limits drive a dedicated UX on the driver and are not actionable
        // from a viewer — don't surface a stray "Turn failed" line.
        Some("rate_limit") => None,
        Some("error") => turn_failed_event(&agent.scrollback, agent_result, elapsed),
        // end_turn / max_tokens / max_turn_requests / refusal / unknown → done.
        _ => Some(SessionEvent::TurnCompleted {
            elapsed: Some(elapsed),
        }),
    };
    push_turn_terminal_marker(agent, event, ending_prompt_id.as_deref());

    agent.mark_turn_finished();
    // Keep live soft-park; when plan mode is still on without a
    // reverse-request, park the five-CTA panel.
    agent.dismiss_plan_approval_after_turn_if_stale();
    agent.surface_idle_plan_review_if_needed();

    TerminalApply::ViewerFinalized
}

/// Map a [`finalize_turn_from_terminal`] outcome to the redraw/tick bool that
/// BOTH terminal rails (`prompt_complete` and the live `TurnCompleted`) RETURN
/// DIRECTLY, applying the viewer-finalize side effect. Keeping this mapping in
/// one place is load-bearing: the live `TurnCompleted` arm must return this
/// instead of routing through `changed && is_active` (see below).
///
/// - `Ignored` -> `false`.
/// - `ReconcileArmed` -> `true` UNCONDITIONALLY (not gated on visibility). The
///   lost-RPC reconcile sweep rides the animation tick, and the event loop only
///   re-arms the tick when a batch reports a change. A background-tab driver
///   (`is_active == false`) that armed the reconcile must still report the change
///   or `reconcile_overdue_turn_ends` never fires and the turn strands on
///   "Waiting…" — the exact bug this rail fixes.
/// - `ViewerFinalized` -> `true` only when `is_active` (drop pending adoption).
pub(super) fn apply_terminal_outcome(
    outcome: TerminalApply,
    app: &mut AppView,
    agent_id: AgentId,
    is_active: bool,
) -> bool {
    match outcome {
        TerminalApply::Ignored => false,
        TerminalApply::ReconcileArmed => true,
        TerminalApply::ViewerFinalized => {
            if let Some(p) = app.pending_running_adoptions.remove(&agent_id)
                && let Some(agent) = app.agents.get_mut(&agent_id)
            {
                agent.discard_pending_adoption_updates(&p.prompt_id);
            }
            is_active
        }
    }
}

/// Driver `PromptResponse` already knows success, HTTP status, and the
/// formatted error text. Tokens are recorded only when `usage` already
/// carried `inputTokens` or `outputTokens`. This does not touch the
/// session ledger or `live_tokens_used`.
pub(in crate::app) fn record_driver_prompt_outcome(
    agent: &AgentView,
    result: &Result<agent_client_protocol::PromptResponse, String>,
    http_status: Option<u16>,
    usage_meta: Option<&serde_json::Map<String, serde_json::Value>>,
    elapsed: Option<std::time::Duration>,
    was_cancelling: bool,
) {
    if agent.bash_turn || was_cancelling {
        return;
    }
    let outcome = match result {
        Ok(_) => Some(crate::uptime::Outcome::ModelRequestSucceeded),
        Err(err) => outcome_from_failure(http_status, err),
    };
    if let Some(outcome) = outcome {
        persist_completed(agent, outcome, elapsed, fetched_token_count(usage_meta));
    }
}

/// Wake turn that skips `PromptResponse`. A silent wake with no output is
/// not a completed model request.
pub(in crate::app) fn record_wake_outcome(
    agent: &AgentView,
    stop_reason: &str,
    agent_result: Option<&str>,
    had_output: bool,
    elapsed: Option<std::time::Duration>,
) {
    if agent.bash_turn {
        return;
    }
    let outcome = match stop_reason {
        "cancelled" | "rate_limit" => None,
        "error" => outcome_from_failure(None, agent_result.unwrap_or("")),
        _ if had_output => Some(crate::uptime::Outcome::ModelRequestSucceeded),
        _ => None,
    };
    if let Some(outcome) = outcome {
        persist_completed(agent, outcome, elapsed, None);
    }
}

/// Viewer finalize and lost-response reconcile. `token_count` stays
/// not-fetched: those signals do not carry usage.
pub(in crate::app) fn record_terminal_stop_reason(
    agent: &AgentView,
    stop_reason: Option<&str>,
    error_text: Option<&str>,
    elapsed: Option<std::time::Duration>,
    was_cancelling: bool,
) {
    if agent.bash_turn || was_cancelling {
        return;
    }
    let outcome = match stop_reason {
        Some("cancelled") | Some("rate_limit") => None,
        Some("error") => outcome_from_failure(None, error_text.unwrap_or("")),
        _ => Some(crate::uptime::Outcome::ModelRequestSucceeded),
    };
    if let Some(outcome) = outcome {
        persist_completed(agent, outcome, elapsed, None);
    }
}

/// First time this process paints a recognized outage banner. Later frames
/// of the same text do not append another piece.
pub(in crate::app) fn note_painted_announcement(
    session_id: &str,
    title: Option<&str>,
    message: Option<&str>,
) {
    let text = match (title.unwrap_or("").trim(), message.unwrap_or("").trim()) {
        ("", "") => return,
        (title, "") => title.to_string(),
        ("", message) => message.to_string(),
        (title, message) => format!("{title}\n{message}"),
    };
    use std::sync::Mutex;
    static LAST_PAINTED: Mutex<String> = Mutex::new(String::new());
    let mut last = match LAST_PAINTED.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if *last == text {
        return;
    }
    *last = text.clone();
    drop(last);
    #[cfg(not(test))]
    {
        let dir = crate::uptime::uptime_dir(&xai_grok_config::grok_home());
        let now = chrono::Utc::now().timestamp_millis();
        let _ = crate::uptime::record_announcement_if_recognized(&dir, session_id, &text, now);
    }
    #[cfg(test)]
    {
        let _ = session_id;
    }
}

/// `/announcements hide` keeps the stored row. This does not delete pieces.
pub(in crate::app) fn note_announcement_hide() {
    #[cfg(not(test))]
    {
        let dir = crate::uptime::uptime_dir(&xai_grok_config::grok_home());
        if let Ok(store) = crate::uptime::UptimeStore::open(&dir) {
            let _ = crate::uptime::hide_announcement_keeps_row(&store);
        }
    }
}

fn outcome_from_failure(http_status: Option<u16>, text: &str) -> Option<crate::uptime::Outcome> {
    if text.contains("Stopped: repeating sentence") || text.contains("repetitive_generation") {
        return Some(crate::uptime::Outcome::RepeatingSentenceStop);
    }
    if http_status == Some(500)
        || text.contains("Server error (500)")
        || text.contains("status 500")
    {
        return Some(crate::uptime::Outcome::Http500);
    }
    if matches!(http_status, Some(408) | Some(504))
        || text.contains("Request timed out")
        || text.contains("timed out")
        || text.contains("No response from the model")
        || text.contains("idle_timeout")
    {
        return Some(crate::uptime::Outcome::Timeout);
    }
    None
}

fn fetched_token_count(
    usage_meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<i64> {
    let usage = usage_meta?.get("usage")?.as_object()?;
    let input = usage.get("inputTokens");
    let output = usage.get("outputTokens");
    if input.is_none() && output.is_none() {
        return None;
    }
    let number = |value: Option<&serde_json::Value>| -> i64 {
        value
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
            })
            .unwrap_or(0)
    };
    Some(number(input).saturating_add(number(output)))
}

#[cfg(not(test))]
fn session_id_of(agent: &AgentView) -> String {
    agent
        .session
        .session_id
        .as_ref()
        .map(|sid| sid.0.to_string())
        .unwrap_or_default()
}

#[cfg(not(test))]
fn model_id_of(agent: &AgentView) -> String {
    agent
        .session
        .models
        .current_model_id_str()
        .unwrap_or("")
        .to_string()
}

#[cfg(not(test))]
fn latency_ms(elapsed: Option<std::time::Duration>) -> Option<i64> {
    elapsed.and_then(|d| i64::try_from(d.as_millis()).ok())
}

/// Unit-test builds skip the piece write so pager tests do not append into
/// the operator uptime directory. The grok-oss binary calls
/// `record_completed_observation`.
fn persist_completed(
    agent: &AgentView,
    outcome: crate::uptime::Outcome,
    elapsed: Option<std::time::Duration>,
    token_count: Option<i64>,
) {
    #[cfg(not(test))]
    {
        let dir = crate::uptime::uptime_dir(&xai_grok_config::grok_home());
        let now = chrono::Utc::now().timestamp_millis();
        let _ = crate::uptime::record_completed_observation(
            &dir,
            &session_id_of(agent),
            &model_id_of(agent),
            outcome,
            latency_ms(elapsed),
            token_count,
            now,
        );
    }
    #[cfg(test)]
    {
        let _ = (agent, outcome, elapsed, token_count);
    }
}

#[cfg(test)]
#[path = "turn_completion/tests.rs"]
mod tests;
