//! `/start`: continue paused or interrupted work. Not the session picker.

use super::queue::maybe_drain_queue_and_note_peek;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::{ActiveView, AppView};

const NOTHING_HELD_TOAST: &str = "There is no paused or interrupted work to start in this session.";

/// Start held work. Never toggles pause on. Never opens the session picker.
pub(super) fn dispatch_start_paused_or_interrupted(app: &mut AppView) -> Vec<Effect> {
    // After Plan Exit, parked Isolated Preview must not trap `/start`.
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        agent.leave_parked_isolated_preview();
    }
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
    // Scrollback is empty at process start. Read chat_history.jsonl here
    // as well as through operator_prompt_already_issued_as_human_turn.
    // `/goal <rest>` matches `A goal has been set: <rest>` through
    // `operator_text_matches_recorded`, not a second mapper.
    if agent.operator_prompt_already_issued_as_human_turn(&text)
        || disk_chat_history_already_has_prompt(&cwd, &sid, &text)
    {
        // `/start` must not requeue a finished Human turn as continue_prior_work.
        // Occupancy drop spares that flag, so the chat-history skip lives here.
        let _ = clear_canceled_turn_resume(&cwd, &sid);
        return None;
    }
    agent.show_toast(auto_resume_toast());
    agent.session.enqueue_continue_prior_work_front(text);
    let _ = clear_canceled_turn_resume(&cwd, &sid);
    Some(maybe_drain_queue_and_note_peek(app, id))
}

/// True when `chat_history.jsonl` already has this Operator prompt as a
/// Human turn. Scrollback is not consulted. `/goal` uses
/// `operator_text_matches_recorded`.
fn disk_chat_history_already_has_prompt(cwd: &str, sid: &str, text: &str) -> bool {
    let Some(blob) = xai_grok_shell::session::prompt_wal::chat_history_path(cwd, sid)
        .and_then(|path| std::fs::read_to_string(path).ok())
    else {
        return false;
    };
    xai_grok_shell::session::prompt_wal::user_texts_from_chat_history_jsonl(&blob)
        .iter()
        .any(|recorded| {
            xai_grok_shell::session::prompt_wal::operator_text_matches_recorded(text, recorded)
        })
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
