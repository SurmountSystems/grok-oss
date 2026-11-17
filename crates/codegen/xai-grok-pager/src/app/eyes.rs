//! Short TOON look of a session. This is not a scrape of the painted terminal.
//!
//! A session that is still starting reports the ASCII status `Starting session...`
//! (three dots). Queued text is copied from the session. Nothing here calls the model.

use crate::app::agent::AgentState;
use crate::app::agent_view::{ActivePane, AgentView};
use crate::app::app_view::{ActiveView, AppView};

const LOOK_LINE_CAP: usize = 10;
const TRANSCRIPT_TAIL: usize = 3;
const STARTING_STATUS: &str = "Starting session...";

/// UI fields the short TOON look prints.
pub trait SessionUiState {
    fn status_text(&self) -> String;
    fn composer_text(&self) -> String;
    fn composer_focused(&self) -> bool;
    fn queued_prompts(&self) -> Vec<String>;
    fn transcript_lines(&self) -> Vec<String>;
}

/// Session this process spawned for `grok-oss eyes look`.
///
/// It is still starting. It does not attach to an open window and it does not call the model.
#[derive(Debug, Clone, Copy)]
pub struct SpawnedEyesSession;

impl SessionUiState for SpawnedEyesSession {
    fn status_text(&self) -> String {
        STARTING_STATUS.to_string()
    }

    fn composer_text(&self) -> String {
        String::new()
    }

    fn composer_focused(&self) -> bool {
        true
    }

    fn queued_prompts(&self) -> Vec<String> {
        Vec::new()
    }

    fn transcript_lines(&self) -> Vec<String> {
        Vec::new()
    }
}

impl SessionUiState for AppView {
    fn status_text(&self) -> String {
        let Some(agent) = active_agent(self) else {
            return String::new();
        };
        if agent.session_starting_since.is_some() {
            return STARTING_STATUS.to_string();
        }
        match &agent.session.state {
            AgentState::Idle => "Idle".to_string(),
            AgentState::TurnRunning => "Turn running".to_string(),
            AgentState::TurnCancelling => "Turn cancelling".to_string(),
            AgentState::CommandRunning { .. } => "Command running".to_string(),
            AgentState::CommandCancelling { .. } => "Command cancelling".to_string(),
        }
    }

    fn composer_text(&self) -> String {
        active_agent(self)
            .map(|agent| agent.prompt.text().to_string())
            .unwrap_or_default()
    }

    fn composer_focused(&self) -> bool {
        active_agent(self).is_some_and(|agent| agent.active_pane == ActivePane::Prompt)
    }

    fn queued_prompts(&self) -> Vec<String> {
        let Some(agent) = active_agent(self) else {
            return Vec::new();
        };
        agent
            .session
            .pending_prompts
            .iter()
            .map(|prompt| prompt.text.clone())
            .collect()
    }

    fn transcript_lines(&self) -> Vec<String> {
        let Some(agent) = active_agent(self) else {
            return Vec::new();
        };
        transcript_tail(agent)
    }
}

fn active_agent(app: &AppView) -> Option<&AgentView> {
    let ActiveView::Agent(id) = app.active_view else {
        return None;
    };
    app.agents.get(&id)
}

fn transcript_tail(agent: &AgentView) -> Vec<String> {
    let len = agent.scrollback.len();
    let start = len.saturating_sub(TRANSCRIPT_TAIL);
    let mut lines = Vec::new();
    for idx in start..len {
        let Some(entry) = agent.scrollback.get(idx) else {
            continue;
        };
        let Some(text) = entry.block.copy_text(false) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            lines.push(line.to_string());
        }
    }
    let skip = lines.len().saturating_sub(TRANSCRIPT_TAIL);
    lines.into_iter().skip(skip).collect()
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

/// Short TOON list: status, composer text and focus, each queued prompt, and the last few transcript lines.
/// At most ten lines. Not a cell grid.
pub fn short_toon_look(session: &impl SessionUiState) -> String {
    let mut lines = Vec::new();
    lines.push(format!("status: {}", one_line(&session.status_text())));
    lines.push(format!("composer: {}", one_line(&session.composer_text())));
    lines.push(format!(
        "composer_focused: {}",
        if session.composer_focused() {
            "true"
        } else {
            "false"
        }
    ));
    for text in session.queued_prompts() {
        if lines.len() >= LOOK_LINE_CAP {
            break;
        }
        lines.push(format!("queue: {}", one_line(&text)));
    }
    if lines.len() < LOOK_LINE_CAP {
        for text in session.transcript_lines() {
            if lines.len() >= LOOK_LINE_CAP {
                break;
            }
            let flat = one_line(&text);
            if flat.is_empty() {
                continue;
            }
            lines.push(format!("transcript: {flat}"));
        }
    }
    lines.join("\n")
}
