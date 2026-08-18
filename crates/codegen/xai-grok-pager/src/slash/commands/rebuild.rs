//! `/rebuild` — rebuild grok-oss from this tree and soft-relaunch live instances.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct RebuildCommand;

impl SlashCommand for RebuildCommand {
    fn name(&self) -> &str {
        "rebuild"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn description(&self) -> &str {
        "Rebuild grok-oss from this tree and gracefully relaunch live instances"
    }

    fn usage(&self) -> &str {
        "/rebuild"
    }

    fn takes_args(&self) -> bool {
        false
    }

    fn session_scoped(&self) -> bool {
        false
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error(
                "Usage: /rebuild\n\
                 Rebuilds this tree's grok-oss (just install), soft-relaunches \
                 reachable leaders, then re-execs this session on the new binary."
                    .into(),
            );
        }
        CommandResult::Action(Action::RebuildAndRelaunch)
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
    fn rebuild_is_registered_builtin() {
        let names: Vec<_> = builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "rebuild"),
            "builtin list missing rebuild: {names:?}"
        );
    }

    #[test]
    fn rebuild_run_returns_action() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        assert!(matches!(
            RebuildCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::RebuildAndRelaunch)
        ));
        assert!(matches!(
            RebuildCommand.run(&mut ctx, "extra"),
            CommandResult::Error(_)
        ));
    }
}
