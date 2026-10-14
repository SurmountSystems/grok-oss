//! `/multiline`: toggle multiline input mode.
//!
//! In multiline mode, Enter in the middle of a draft inserts a newline and
//! Shift+Enter sends the prompt. Enter at the end of the last line still
//! sends (or interjects if a turn is running). Empty-composer mid-turn Enter
//! still force-sends the top queued follow-up (send now), same as normal
//! mode.
//! Toggled via `Ctrl+M`, this slash command, or the settings modal.
//!
//! Dispatches `Action::SetMultilineMode(!current)`. Per-session only (no disk persistence).

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// Toggle multiline input mode via `/multiline`.
pub struct MultilineCommand;

impl SlashCommand for MultilineCommand {
    slash_meta! {
        name: "multiline",
        aliases: ["ml"],
        description: "Toggle multiline input mode (swap Enter and Shift+Enter)",
        usage: "/multiline",
        session_scoped: true,
        // Dashboard dispatch/peek own their own multiline flag (`DashboardState::multiline_mode`); same swap as the agent prompt.
        offered_when_session_less: true,
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let new = !ctx.pager_state.multiline_mode;
        // Grok OSS: Operator: refuse enabling session Multiline when
        // `[ui] allow_session_multiline` is false (Settings → Editor).
        if new && !crate::appearance::cache::load_allow_session_multiline() {
            return CommandResult::Message(
                "Session Multiline is disabled in Settings → Editor (`allow_session_multiline`)."
                    .to_string(),
            );
        }
        CommandResult::Action(Action::SetMultilineMode(new))
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
        multiline_mode: bool,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot {
                multiline_mode,
                yolo_mode: false,
                ..PagerLocalSnapshot::default()
            },
        }
    }

    /// Off dispatches `SetMultilineMode(true)`.
    #[test]
    fn run_when_off_dispatches_set_to_true() {
        crate::appearance::cache::set_allow_session_multiline(true);
        let cmd = MultilineCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        let result = cmd.run(&mut ctx, "");
        match result {
            CommandResult::Action(Action::SetMultilineMode(b)) => {
                assert!(b, "off → should dispatch SetMultilineMode(true)");
            }
            other => panic!("expected Action::SetMultilineMode(true), got {other:?}"),
        }
    }

    /// `/multiline` when on dispatches `Action::SetMultilineMode(false)`.
    #[test]
    fn run_when_on_dispatches_set_to_false() {
        crate::appearance::cache::set_allow_session_multiline(false);
        let cmd = MultilineCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, true);
        let result = cmd.run(&mut ctx, "");
        match result {
            CommandResult::Action(Action::SetMultilineMode(b)) => {
                assert!(!b, "on → should dispatch SetMultilineMode(false)");
            }
            other => panic!("expected Action::SetMultilineMode(false), got {other:?}"),
        }
        crate::appearance::cache::set_allow_session_multiline(true);
    }

    /// `/multiline` ignores args (no-arg command).
    #[test]
    fn run_ignores_args() {
        crate::appearance::cache::set_allow_session_multiline(true);
        let cmd = MultilineCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        let result = cmd.run(&mut ctx, "extra args ignored");
        assert!(matches!(
            result,
            CommandResult::Action(Action::SetMultilineMode(true))
        ));
    }

    /// `/ml` alias resolves via registry.
    #[test]
    fn alias_ml_resolves_via_registry() {
        use std::sync::Arc;
        let reg = crate::slash::registry::CommandRegistry::new(vec![Arc::new(MultilineCommand)]);
        let resolved = reg.get("ml").expect("/ml alias must resolve to a command");
        assert_eq!(
            resolved.name(),
            "multiline",
            "/ml alias must resolve to MultilineCommand"
        );
    }
}
