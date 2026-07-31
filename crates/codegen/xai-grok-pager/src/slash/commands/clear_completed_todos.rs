//! `/clear-completed-todos` — archive finished board rows off the live session board.
//!
//! Same backend as the todo pane **Clear done** chrome control and focused `X`.
//! Does not hide-only (`h`); does not wipe open work via merge:false.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Clear completed and cancelled todos from the live board.
pub struct ClearCompletedTodosCommand;

impl SlashCommand for ClearCompletedTodosCommand {
    fn name(&self) -> &str {
        "clear-completed-todos"
    }

    fn description(&self) -> &str {
        "Clear completed todos from the session board"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn usage(&self) -> &str {
        "/clear-completed-todos"
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session".to_string());
        }
        CommandResult::Action(Action::ClearCompletedTodos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    static DEFAULT_BUNDLE_STATE: BundleState = BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn run_with_session(sid: Option<&agent_client_protocol::SessionId>) -> CommandResult {
        let models = ModelState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: sid,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        };
        ClearCompletedTodosCommand.run(&mut ctx, "")
    }

    #[test]
    fn no_session_errors() {
        match run_with_session(None) {
            CommandResult::Error(msg) => assert!(msg.contains("No active session")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn with_session_dispatches_clear() {
        let sid = agent_client_protocol::SessionId::from("s1".to_string());
        assert!(matches!(
            run_with_session(Some(&sid)),
            CommandResult::Action(Action::ClearCompletedTodos)
        ));
    }
}
