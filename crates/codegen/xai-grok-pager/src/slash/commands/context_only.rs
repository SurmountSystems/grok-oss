//! `/context-only` -- toggle diagnostic mode: no tools advertised or executed.
//!
//! Off → `SetPermissionMode(ContextOnly)`.
//! Already context-only → `SetPermissionMode(Ask)` (toggle off).
//!
//! The dispatcher owns state mutation, persistence (with rollback), and toast.

use crate::app::actions::{Action, PermissionModeKind};
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Toggle context-only permission mode (no tools).
pub struct ContextOnlyCommand;

impl SlashCommand for ContextOnlyCommand {
    fn name(&self) -> &str {
        "context-only"
    }

    fn description(&self) -> &str {
        "Toggle context-only mode (no tools; conversation only)"
    }

    fn usage(&self) -> &str {
        "/context-only"
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let currently = ctx.pager_state.context_only_mode
            && !ctx.pager_state.yolo_mode
            && !ctx.pager_state.auto_mode;
        let kind = if currently {
            PermissionModeKind::Ask
        } else {
            PermissionModeKind::ContextOnly
        };
        CommandResult::Action(Action::SetPermissionMode(kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn make_ctx<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
        context_only_mode: bool,
        yolo_mode: bool,
        auto_mode: bool,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot {
                yolo_mode,
                auto_mode,
                context_only_mode,
                auto_mode_gate: true,
                ..PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn off_turns_context_only_on() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false, false, false);
        assert!(matches!(
            ContextOnlyCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::SetPermissionMode(PermissionModeKind::ContextOnly))
        ));
    }

    #[test]
    fn on_turns_context_only_off() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, true, false, false);
        assert!(matches!(
            ContextOnlyCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::SetPermissionMode(PermissionModeKind::Ask))
        ));
    }

    #[test]
    fn always_approve_switches_to_context_only() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, true, true, false);
        assert!(matches!(
            ContextOnlyCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::SetPermissionMode(PermissionModeKind::ContextOnly))
        ));
    }

    #[test]
    fn ignores_args() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false, false, false);
        assert!(matches!(
            ContextOnlyCommand.run(&mut ctx, "extra"),
            CommandResult::Action(Action::SetPermissionMode(PermissionModeKind::ContextOnly))
        ));
    }
}
