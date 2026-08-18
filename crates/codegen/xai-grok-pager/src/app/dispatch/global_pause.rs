//! Global work pause: interrupt every in-process session, hold queues, resume
//! only truly incomplete work.

use super::queue::maybe_drain_queue_and_note_peek;
use super::turn::do_cancel_turn_for;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::AppView;
use crate::app::global_work_pause::{GlobalWorkPause, PausedSessionSnapshot};
use std::time::Instant;

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

pub(super) fn dispatch_resume_global_pause(app: &mut AppView) -> Vec<Effect> {
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
        // newer typed follow-ups that arrived while paused.
        agent.session.enqueue_prompt_front(text);
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
