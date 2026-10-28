//! `/new`: create a new session.
//!
//! Session `/clear` is [`ClearCommand`]: leftover/ghost Subagents rows on this
//! conversation. It is not an alias of `/new`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// Start a new agent session, clearing the current conversation.
pub struct NewCommand;

impl SlashCommand for NewCommand {
    slash_meta! {
        name: "new",
        description: "Start a new session",
        usage: "/new",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::NewSession)
    }
}

/// Session `/clear` (the slash the Operator types): hide leftover/ghost
/// Subagents rows in this conversation so header tasks is the live list.
/// Dock `[clear]` stays the mouse control for the same hide set.
pub struct ClearCommand;

impl SlashCommand for ClearCommand {
    slash_meta! {
        name: "clear",
        description: "Clear leftover Subagents rows in this conversation",
        usage: "/clear",
        session_scoped: true,
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session".to_string());
        }
        CommandResult::Action(Action::ClearLeftoverSubagentRows)
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

    fn run_clear(sid: Option<&agent_client_protocol::SessionId>) -> CommandResult {
        let models = ModelState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: sid,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        };
        ClearCommand.run(&mut ctx, "")
    }

    #[test]
    fn clear_without_session_errors() {
        match run_clear(None) {
            CommandResult::Error(msg) => assert!(msg.contains("No active session")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn clear_with_session_is_not_new_session() {
        let sid = agent_client_protocol::SessionId::from("s1".to_string());
        let result = run_clear(Some(&sid));
        assert!(
            !matches!(result, CommandResult::Action(Action::NewSession)),
            "session /clear must not only start /new, got {result:?}"
        );
        assert!(matches!(
            result,
            CommandResult::Action(Action::ClearLeftoverSubagentRows)
        ));
    }
}
