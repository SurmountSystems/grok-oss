//! Publish a safe heartbeat into `$GROK_HOME/active_sessions.json`.
//!
//! Title comes only from the on-disk session summary. Never from the latest
//! user prompt, tool arguments, tokens, or message text.

use std::sync::atomic::{AtomicBool, Ordering};

use agent_client_protocol as acp;
use xai_grok_active_sessions::{
    HeartbeatPhrase, HeartbeatUpdate, SessionActivity, format_safe_activity_line,
};

use super::actions::Effect;
use super::agent_view::AgentView;

static GLOBAL_WORK_PAUSED: AtomicBool = AtomicBool::new(false);
/// Set when this process has registered at least one window. Turn-end
/// heartbeats stay off until then so unit tests that only finish a turn do
/// not create `active_sessions.json` under the default grok home.
static HEARTBEAT_WRITER_ENABLED: AtomicBool = AtomicBool::new(false);

/// Remember whether fearless global pause is engaged so turn-end heartbeats
/// can still say "paused" without needing `AppView`.
pub(crate) fn set_global_work_paused(paused: bool) {
    GLOBAL_WORK_PAUSED.store(paused, Ordering::Relaxed);
}

pub(crate) fn global_work_paused() -> bool {
    GLOBAL_WORK_PAUSED.load(Ordering::Relaxed)
}

pub(crate) struct HeartbeatSnapshot {
    pub activity: SessionActivity,
    pub activity_line: Option<String>,
}

pub(crate) fn snapshot(agent: &AgentView, paused: bool) -> HeartbeatSnapshot {
    HeartbeatSnapshot {
        activity: activity_of(agent, paused),
        activity_line: line_of(agent, paused),
    }
}

fn activity_of(agent: &AgentView, paused: bool) -> SessionActivity {
    if paused {
        SessionActivity::Idle
    } else if agent.session.state.is_busy() {
        SessionActivity::Working
    } else {
        SessionActivity::Idle
    }
}

fn line_of(agent: &AgentView, paused: bool) -> Option<String> {
    let phrase = if paused {
        HeartbeatPhrase::Paused
    } else if agent.session.state.is_turn_running() {
        HeartbeatPhrase::TurnRunning
    } else {
        HeartbeatPhrase::Idle
    };
    let model = agent.session.models.current_model_name().or_else(|| {
        agent
            .session
            .models
            .current_model_id_str()
            .map(str::to_string)
    });
    let subagent_count =
        crate::app::subagent::live_subagent_list(agent.subagent_sessions.values()).len() as u32;
    format_safe_activity_line(model.as_deref(), phrase, subagent_count)
}

/// Title from `summary.json` only. Never reads prompt files or last-turn text.
pub(crate) fn title_from_on_disk_summary(cwd: &str, session_id: &acp::SessionId) -> Option<String> {
    let info = xai_grok_shell::session::info::Info {
        id: session_id.clone(),
        cwd: cwd.to_string(),
    };
    let path = xai_grok_shell::session::persistence::session_dir(&info).join("summary.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let summary: xai_grok_shell::session::persistence::Summary = serde_json::from_str(&raw).ok()?;
    summary.display_title_opt()
}

pub(crate) fn register_effect(agent: &AgentView) -> Option<Effect> {
    let session_id = agent.session.session_id.clone()?;
    let snap = snapshot(agent, global_work_paused());
    Some(Effect::RegisterActiveSession {
        session_id,
        cwd: agent.session.cwd.display().to_string(),
        activity: snap.activity,
        activity_line: snap.activity_line,
    })
}

pub(crate) fn enable_writer() {
    HEARTBEAT_WRITER_ENABLED.store(true, Ordering::Relaxed);
}

pub(crate) fn write_blocking(
    session_id: &acp::SessionId,
    cwd: &str,
    activity: SessionActivity,
    activity_line: Option<String>,
) {
    enable_writer();
    let title = title_from_on_disk_summary(cwd, session_id);
    if let Err(e) = xai_grok_active_sessions::heartbeat(
        std::process::id(),
        session_id,
        HeartbeatUpdate {
            activity,
            title,
            activity_line,
        },
    ) {
        tracing::warn!(?e, "Failed to write active-session heartbeat");
    }
}

pub(crate) fn write_from_agent(agent: &AgentView) {
    if !HEARTBEAT_WRITER_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some(session_id) = agent.session.session_id.as_ref() else {
        return;
    };
    let snap = snapshot(agent, global_work_paused());
    let title = title_from_on_disk_summary(&agent.session.cwd.display().to_string(), session_id);
    match xai_grok_active_sessions::try_heartbeat(
        std::process::id(),
        session_id,
        HeartbeatUpdate {
            activity: snap.activity,
            title,
            activity_line: snap.activity_line,
        },
    ) {
        Ok(Some(true)) => {}
        Ok(Some(false)) => tracing::debug!(
            session_id = %session_id.0,
            "Skipped active-session heartbeat; no matching row"
        ),
        Ok(None) => tracing::debug!(
            session_id = %session_id.0,
            "Skipped active-session heartbeat under lock contention"
        ),
        Err(e) => tracing::warn!(?e, "Failed to write active-session heartbeat"),
    }
}
