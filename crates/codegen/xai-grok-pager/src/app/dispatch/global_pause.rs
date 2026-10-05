//! Global work pause: interrupt every in-process session, hold queues, resume
//! only truly incomplete work.

use super::queue::maybe_drain_queue_and_note_peek;
use super::turn::do_cancel_turn_for;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView};
use crate::app::global_work_pause::{GlobalWorkPause, PausedSessionSnapshot};
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SessionEvent;
use crate::scrollback::state::ScrollbackState;
use std::time::Instant;

/// Idle `[pause]` after AUTO compact failed over the sampling window retries
/// manual `/compact` (AUTO suppress does not apply) and continues the last
/// real prompt. Stay on SuperGrok if that is the live identity. Do not tell
/// the operator to add credits from a client remaining printout.
const OVER_WINDOW_COMPACT_RETRY_TOAST: &str = "Retrying compact so this session can continue. \
     Stay on SuperGrok if that is the live identity.";

/// Toggle fearless global pause across every agent session in this process.
pub(super) fn dispatch_toggle_global_pause(app: &mut AppView) -> Vec<Effect> {
    if app.global_work_pause.is_active() {
        dispatch_resume_global_pause(app)
    } else {
        dispatch_engage_global_pause(app)
    }
}

fn capture_snapshots(app: &AppView) -> Vec<PausedSessionSnapshot> {
    app.agents
        .iter()
        .map(|(id, agent)| {
            let turn_running = agent.session.state.is_turn_running();
            let pending_queue_len = agent.session.pending_prompts.len();
            let in_flight = agent
                .session
                .in_flight_prompt
                .as_ref()
                .map(|p| p.text.clone());
            PausedSessionSnapshot::capture(
                *id,
                agent.session.session_id.as_ref().map(|s| s.0.to_string()),
                turn_running,
                pending_queue_len,
                in_flight,
            )
        })
        .collect()
}

fn dispatch_engage_global_pause(app: &mut AppView) -> Vec<Effect> {
    if let Some(effects) = try_unstick_idle_over_window_compact_fail(app) {
        return effects;
    }
    let snapshots = capture_snapshots(app);
    // Collect agent ids that need a turn cancel before we mutably borrow app.
    let to_cancel: Vec<AgentId> = snapshots
        .iter()
        .filter(|s| s.interrupted_running_turn)
        .map(|s| s.agent_id)
        .collect();

    app.global_work_pause.engage(Instant::now(), snapshots);
    crate::app::active_session_heartbeat::set_global_work_paused(true);
    for agent in app.agents.values() {
        crate::app::active_session_heartbeat::write_from_agent(agent);
    }
    let toast = app.global_work_pause.engage_toast();
    app.show_toast(&toast);

    let mut effects = Vec::new();
    // Cancel running turns on every held session (not only the focused one).
    // Prefer stopping subagents with the turn so work truly freezes.
    for id in to_cancel {
        // Write the existing continue-interrupted-turn marker before the
        // in-flight stash is dropped. The live [pause] chip holds work in
        // RAM; this file is what last-session on start and `/start` use if
        // this process then dies. Same payload as `/rebuild` mid-turn.
        persist_canceled_turn_resume_for_pause(app, id);
        // Drop the local in-flight stash after capture so cancel does not
        // leave a dangling rewind candidate beside the resume-once queue.
        if let Some(agent) = app.agents.get_mut(&id) {
            agent.session.in_flight_prompt = None;
        }
        // No local rewind: resume re-queues the stashed prompt once.
        effects.extend(do_cancel_turn_for(app, id, true, false));
    }
    effects
}

/// Prompt text for continue-interrupted-turn when pause cancels a running turn.
///
/// Prefer the in-flight rewind stash. After first server activity that stash
/// is cleared, so fall back to the last real user prompt in scrollback.
/// Skip bash/cron bubbles so a `!` or scheduled line is not re-queued as
/// the interrupted turn.
fn pause_cancel_resume_prompt(agent: &crate::app::agent_view::AgentView) -> Option<String> {
    if let Some(stashed) = agent.session.in_flight_prompt.as_ref() {
        let text = stashed.text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    last_user_prompt_full_text(&agent.scrollback)
}

/// Idle `[pause]` on a session already over the sampling window after compact
/// failed or AUTO skipped must unstick: retry `/compact`, then continue the
/// last user prompt. Empty global pause is a no-op here (`Resumed · nothing
/// pending`) and leaves 507K/500K stuck.
fn idle_session_needs_over_window_compact_unstick(app: &AppView) -> bool {
    let ActiveView::Agent(id) = app.active_view else {
        return false;
    };
    let Some(agent) = app.agents.get(&id) else {
        return false;
    };
    agent.session.state.is_idle()
        && agent.session.pending_prompts.is_empty()
        && session_is_over_sampling_window(agent)
        && scrollback_has_compaction_failed(agent)
}

fn try_unstick_idle_over_window_compact_fail(app: &mut AppView) -> Option<Vec<Effect>> {
    if !idle_session_needs_over_window_compact_unstick(app) {
        return None;
    }
    let ActiveView::Agent(id) = app.active_view else {
        return None;
    };
    let continue_text = {
        let agent = app.agents.get(&id)?;
        continue_prompt_after_compact(agent)
    };
    let agent = app.agents.get_mut(&id)?;
    // Compact first. Continue unfinished work only when that body is not
    // already a Human turn. Occupancy drop must not keep issued text as a
    // Prompt row (`continue_prior_work` is pause-resume, not this path).
    agent.session.enqueue_command("/compact".into());
    if let Some(text) = continue_text {
        agent.session.enqueue_continue_prior_work(text);
    }
    agent.sync_queue_pane();
    app.show_toast(OVER_WINDOW_COMPACT_RETRY_TOAST);
    Some(maybe_drain_queue_and_note_peek(app, id))
}

/// Last real work after Compact, not `/compact`, and not a body that already
/// issued as a Human turn. Calls the compact-continue helpers so they stay
/// on the live unstick path.
fn continue_prompt_after_compact(agent: &AgentView) -> Option<String> {
    let text = last_real_user_prompt_for_compact_continue(agent)?;
    if is_compact_slash(&text) {
        return None;
    }
    // Scrollback still has that leftover `/implement` as a Human turn.
    // After HTTP 502 that is unfinished work to continue, not the
    // compact-fail stale-slash skip.
    if operator_prompt_already_in_scrollback(agent, &text)
        && !compact_fail_followed_by_http_502(agent)
    {
        return None;
    }
    Some(text)
}

fn last_real_user_prompt_for_compact_continue(agent: &AgentView) -> Option<String> {
    last_user_prompt_full_text(&agent.scrollback)
}

fn is_compact_slash(text: &str) -> bool {
    crate::slash::queue_schedule::is_compact_slash(text)
}

fn session_is_over_sampling_window(agent: &AgentView) -> bool {
    let Some(ctx) = agent.context_state.as_ref() else {
        return false;
    };
    ctx.total > 0 && ctx.used >= ctx.total
}

fn scrollback_has_compaction_failed(agent: &AgentView) -> bool {
    let len = agent.scrollback.len();
    for idx in (0..len).rev() {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::SessionEvent(block) = &entry.block
            && matches!(block.event, SessionEvent::CompactionFailed { .. })
        {
            return true;
        }
    }
    false
}

fn compact_fail_followed_by_http_502(agent: &AgentView) -> bool {
    let mut saw_fail = false;
    for idx in 0..agent.scrollback.len() {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        let RenderBlock::SessionEvent(block) = &entry.block else {
            continue;
        };
        match &block.event {
            SessionEvent::CompactionFailed { .. } => saw_fail = true,
            SessionEvent::RequestFailed {
                headline, detail, ..
            } if saw_fail && (headline.contains("502") || detail.contains("502")) => {
                return true;
            }
            SessionEvent::RetryFailed { error, .. } if saw_fail && error.contains("502") => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn operator_prompt_already_in_scrollback(agent: &AgentView, text: &str) -> bool {
    let needle = text.trim();
    if needle.is_empty() {
        return false;
    }
    for idx in 0..agent.scrollback.len() {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        let RenderBlock::UserPrompt(block) = &entry.block else {
            continue;
        };
        if xai_grok_shell::session::prompt_wal::operator_text_matches_recorded(needle, &block.text)
        {
            return true;
        }
    }
    false
}

fn last_user_prompt_full_text(scrollback: &ScrollbackState) -> Option<String> {
    let len = scrollback.len();
    for idx in (0..len).rev() {
        let Some(entry) = scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(block) = &entry.block {
            if block.is_bash || block.is_cron {
                continue;
            }
            let text = block.text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn persist_canceled_turn_resume_for_pause(app: &AppView, id: AgentId) {
    use xai_grok_shell::session::canceled_turn_resume::{
        ProcessShutdownResumeArm, arm_and_persist_process_shutdown_cancel_resume,
    };
    let Some(agent) = app.agents.get(&id) else {
        return;
    };
    if !agent.session.state.is_turn_running() {
        return;
    }
    let Some(session_id) = agent.session.session_id.as_ref().map(|s| s.0.to_string()) else {
        return;
    };
    let Some(prompt_text) = pause_cancel_resume_prompt(agent) else {
        return;
    };
    arm_and_persist_process_shutdown_cancel_resume(ProcessShutdownResumeArm {
        cwd: agent.session.cwd.to_string_lossy().into_owned(),
        session_id,
        prompt_text,
        prompt_id: agent.session.current_prompt_id.clone(),
    });
}

pub(super) fn dispatch_resume_global_pause(app: &mut AppView) -> Vec<Effect> {
    if idle_session_needs_over_window_compact_unstick(app) {
        let _ = app.global_work_pause.disengage();
        crate::app::active_session_heartbeat::set_global_work_paused(false);
        for agent in app.agents.values() {
            crate::app::active_session_heartbeat::write_from_agent(agent);
        }
        if let Some(effects) = try_unstick_idle_over_window_compact_fail(app) {
            return effects;
        }
    }
    let snaps = app.global_work_pause.disengage();
    crate::app::active_session_heartbeat::set_global_work_paused(false);
    for agent in app.agents.values() {
        crate::app::active_session_heartbeat::write_from_agent(agent);
    }
    let mut resumed_count = 0usize;
    let mut had_pending = false;
    let mut effects = Vec::new();

    for mut snap in snaps {
        if snap.had_incomplete_work() {
            had_pending = true;
        }
        if !snap.needs_resume_requeue() {
            continue;
        }
        let Some(text) = snap.resume_prompt_once.clone() else {
            continue;
        };
        let Some(agent) = app.agents.get_mut(&snap.agent_id) else {
            // Session gone: do not invent a replacement agent.
            continue;
        };
        // Only re-queue when idle or still cancelling. Never invent a new agent.
        if agent.session.state.is_busy() && !agent.session.state.is_cancelling() {
            continue;
        }
        // Front of local queue so the interrupted turn continues before
        // newer typed follow-ups that arrived while paused. Mark continue
        // so matching an earlier Human turn does not look like stale
        // occupancy.
        agent.session.enqueue_continue_prior_work_front(text);
        snap.mark_resume_consumed();
        resumed_count += 1;
        effects.extend(maybe_drain_queue_and_note_peek(app, snap.agent_id));
    }

    // Drain sessions that only had queued work (no mid-turn stash).
    let agent_ids: Vec<AgentId> = app.agents.keys().copied().collect();
    for id in agent_ids {
        effects.extend(maybe_drain_queue_and_note_peek(app, id));
    }

    app.show_toast(&GlobalWorkPause::disengage_toast(
        resumed_count,
        had_pending,
    ));
    effects
}
