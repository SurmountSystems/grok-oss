//! `/note` — store an operator mid-session note (not a pending main-turn prompt).
//!
//! Bare `/note` lists notes as a system block. `/note <text> [#tag…]` stores
//! a session-local annotation without enqueueing the agent queue.

use crate::app::actions::Action;
use crate::app::agent::parse_note_input;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Operator session notes — not prompts.
pub struct NoteCommand;

impl SlashCommand for NoteCommand {
    fn name(&self) -> &str {
        "note"
    }

    fn aliases(&self) -> &[&str] {
        &["notes"]
    }

    fn description(&self) -> &str {
        "Leave a mid-session note (does not queue a turn)"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn usage(&self) -> &str {
        "/note [text] [#tag…]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[note text] [#tag…]")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session".to_string());
        }
        let (body, tags) = parse_note_input(args);
        if body.is_empty() {
            // Bare `/note`, tags-only, or empty → list notes.
            CommandResult::Action(Action::ShowNotes)
        } else {
            CommandResult::Action(Action::AddSessionNote { text: body, tags })
        }
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

    fn run(sid: Option<&agent_client_protocol::SessionId>, args: &str) -> CommandResult {
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
        NoteCommand.run(&mut ctx, args)
    }

    #[test]
    fn no_session_errors() {
        match run(None, "hello") {
            CommandResult::Error(msg) => assert!(msg.contains("No active session")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn bare_note_lists() {
        let sid = agent_client_protocol::SessionId::from("s1".to_string());
        assert!(matches!(
            run(Some(&sid), ""),
            CommandResult::Action(Action::ShowNotes)
        ));
        assert!(matches!(
            run(Some(&sid), "   "),
            CommandResult::Action(Action::ShowNotes)
        ));
    }

    #[test]
    fn note_with_text_adds_without_queue() {
        let sid = agent_client_protocol::SessionId::from("s1".to_string());
        match run(Some(&sid), "check hold #queue") {
            CommandResult::Action(Action::AddSessionNote { text, tags }) => {
                assert_eq!(text, "check hold");
                assert_eq!(tags, vec!["queue"]);
            }
            other => panic!("expected AddSessionNote, got {other:?}"),
        }
    }

    #[test]
    fn available_in_minimal_by_default() {
        assert!(NoteCommand.available_in_minimal());
    }

    #[test]
    fn notes_alias() {
        assert_eq!(NoteCommand.aliases(), &["notes"]);
    }
}
