//! `/start`: continue paused or interrupted work. Not the session picker.

use super::queue::maybe_drain_queue_and_note_peek;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::{ActiveView, AppView};

const NOTHING_HELD_TOAST: &str = "There is no paused or interrupted work to start in this session.";

/// Start held work. Never toggles pause on. Never opens the session picker.
pub(super) fn dispatch_start_paused_or_interrupted(app: &mut AppView) -> Vec<Effect> {
    if app.global_work_pause.is_active() {
        return super::global_pause::dispatch_resume_global_pause(app);
    }
    if let Some(effects) = try_continue_canceled_turn(app) {
        return effects;
    }
    if app.soft_stop.is_holding() {
        return release_soft_stop_hold(app);
    }
    app.show_toast(NOTHING_HELD_TOAST);
    vec![]
}

fn try_continue_canceled_turn(app: &mut AppView) -> Option<Vec<Effect>> {
    use xai_grok_shell::session::canceled_turn_resume::{
        auto_resume_toast, clear_canceled_turn_resume, load_canceled_turn_resume,
        should_auto_resume_on_restart,
    };

    let ActiveView::Agent(id) = app.active_view else {
        return None;
    };
    let agent = app.agents.get_mut(&id)?;
    let sid = agent.session.session_id.as_ref()?.0.to_string();
    let cwd = agent.session.cwd.to_string_lossy().into_owned();
    let Ok(Some(marker)) = load_canceled_turn_resume(&cwd, &sid) else {
        return None;
    };
    // Operator typed `/start`: apply a valid marker even when the restart
    // setting is off. Still never invent an empty prompt.
    if !should_auto_resume_on_restart(true, Some(&marker)) {
        return None;
    }
    let text = marker.prompt_text;
    if text.trim().is_empty() {
        return None;
    }
    agent.show_toast(auto_resume_toast());
    agent.session.enqueue_prompt_front(text);
    let _ = clear_canceled_turn_resume(&cwd, &sid);
    Some(maybe_drain_queue_and_note_peek(app, id))
}

fn release_soft_stop_hold(app: &mut AppView) -> Vec<Effect> {
    let (_phase, toast) = app.soft_stop.toggle();
    app.show_toast(&toast);
    let mut effects = Vec::new();
    let ids: Vec<AgentId> = app.agents.keys().copied().collect();
    for id in ids {
        effects.extend(maybe_drain_queue_and_note_peek(app, id));
    }
    effects
}
