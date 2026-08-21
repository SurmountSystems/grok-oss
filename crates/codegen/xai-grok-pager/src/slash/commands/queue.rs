//! `/queue` -- list the queued prompts as a committed system block.
//!
//! Minimal mode has no interactive `QueuePane`, so `/queue` is the way to
//! inspect what's waiting behind the running turn. It works in every
//! render mode. The dispatcher (`dispatch_show_queue`) reads the merged
//! server + local queue and commits a read-only list; editing the queue is
//! out of scope here (use the queue pane in the full TUI).

use agent_client_protocol as acp;

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::queue_schedule::{
    QueueHold, parse_queue_hold_args, queue_later_command, queue_later_skill,
};

/// List the queued prompts.
pub struct QueueCommand;

impl SlashCommand for QueueCommand {
    fn name(&self) -> &str {
        "queue"
    }

    fn description(&self) -> &str {
        "List the prompt queue, or hold /compaction /plan /reports /finish"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn usage(&self) -> &str {
        "/queue [/compaction|/plan|/reports|/finish]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        false
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("/compaction | /plan | /reports | /finish")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session".to_string());
        }
        match parse_queue_hold_args(args) {
            Ok(None) => CommandResult::Action(Action::ShowQueue),
            Ok(Some(QueueHold::Command(text))) => queue_later_command(text),
            Ok(Some(QueueHold::Reports(focus))) => {
                let display = if focus.is_empty() {
                    "/reports".to_string()
                } else {
                    format!("/reports {focus}")
                };
                queue_later_skill(
                    display,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(
                        crate::slash::commands::reports::reports_instruction(&focus),
                    ))],
                )
            }
            Ok(Some(QueueHold::Finish(focus))) => {
                let display = if focus.is_empty() {
                    "/finish".to_string()
                } else {
                    format!("/finish {focus}")
                };
                queue_later_skill(
                    display,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(
                        crate::slash::commands::finish::finish_instruction(&focus),
                    ))],
                )
            }
            Err(msg) => CommandResult::Error(msg),
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

    fn ctx_with_session(models: &ModelState, sid: Option<&agent_client_protocol::SessionId>) {
        let mut ctx = CommandExecCtx {
            models,
            session_id: sid,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        };
        match (QueueCommand.run(&mut ctx, ""), sid.is_some()) {
            (CommandResult::Action(Action::ShowQueue), true) => {}
            (CommandResult::Error(msg), false) => assert!(msg.contains("No active session")),
            (other, has) => panic!("unexpected result {other:?} for has_session={has}"),
        }
    }

    #[test]
    fn no_session_errors() {
        let models = ModelState::default();
        ctx_with_session(&models, None);
    }

    #[test]
    fn with_session_dispatches_show_queue() {
        let models = ModelState::default();
        let sid = agent_client_protocol::SessionId::from("s1".to_string());
        ctx_with_session(&models, Some(&sid));
    }
}
