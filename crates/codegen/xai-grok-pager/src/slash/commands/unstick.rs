//! `/unstick` -- resend the last L1 prompt as if the network dropped it.
//!
//! Not `/resume` (session picker) and not continue interrupted turn.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct UnstickCommand;

impl SlashCommand for UnstickCommand {
    fn name(&self) -> &str {
        "unstick"
    }

    fn description(&self) -> &str {
        "Resend the last parent prompt as if the network dropped it"
    }

    fn usage(&self) -> &str {
        "/unstick"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error("Usage: /unstick".into());
        }
        CommandResult::Action(Action::UnstickLastL1Prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::slash::commands::builtin_commands;

    static DEFAULT_BUNDLE_STATE: crate::app::bundle::BundleState =
        crate::app::bundle::BundleState {
            has_cache: false,
            version: String::new(),
            personas: Vec::new(),
            roles: Vec::new(),
            agents: Vec::new(),
            skills: Vec::new(),
            persona_details: Vec::new(),
            role_details: Vec::new(),
        };

    fn make_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn unstick_is_registered_builtin() {
        let names: Vec<_> = builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "unstick"),
            "builtin list missing unstick: {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "resume"),
            "/unstick must not replace /resume"
        );
    }

    #[test]
    fn unstick_does_not_collide_with_resume_slash() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let unstick = UnstickCommand.run(&mut ctx, "");
        let resume = crate::slash::commands::resume::ResumeCommand.run(&mut ctx, "");
        assert!(
            matches!(unstick, CommandResult::Action(Action::UnstickLastL1Prompt)),
            "/unstick must dispatch UnstickLastL1Prompt, got {unstick:?}"
        );
        assert!(
            matches!(resume, CommandResult::Action(Action::ShowSessionPicker)),
            "/resume must stay the session picker, got {resume:?}"
        );
        assert!(
            !matches!(unstick, CommandResult::Action(Action::ShowSessionPicker)),
            "/unstick must not open the session picker"
        );
        assert!(
            !matches!(
                unstick,
                CommandResult::Action(Action::StartPausedOrInterruptedWork)
            ),
            "/unstick must not merge with /start"
        );
    }
}
