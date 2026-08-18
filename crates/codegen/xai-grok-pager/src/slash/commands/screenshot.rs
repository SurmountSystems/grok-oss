//! `/screenshot` — capture the current rendered TUI frame as a PNG.
//!
//! Writes under `$GROK_HOME/screenshots/` (toast shows the path). The
//! capture runs after the next present so the file matches what is on
//! screen. Not an OS screenshot of other windows — only this pager frame.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Capture the current TUI frame to a PNG file.
pub struct ScreenshotCommand;

impl SlashCommand for ScreenshotCommand {
    fn name(&self) -> &str {
        "screenshot"
    }

    fn description(&self) -> &str {
        "Capture the current TUI frame as a PNG image"
    }

    fn usage(&self) -> &str {
        "/screenshot"
    }

    /// Works on welcome and in-session (no session required).
    fn session_scoped(&self) -> bool {
        false
    }

    /// Offered on the dashboard / welcome surface as well.
    fn offered_when_session_less(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error("Usage: /screenshot (no arguments)".to_string());
        }
        CommandResult::Action(Action::CaptureTuiScreenshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::actions::Action;
    use crate::slash::command::{CommandExecCtx, CommandResult};

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
    fn screenshot_command_emits_capture_action() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let result = ScreenshotCommand.run(&mut ctx, "");
        assert!(
            matches!(result, CommandResult::Action(Action::CaptureTuiScreenshot)),
            "expected CaptureTuiScreenshot, got {result:?}"
        );
    }

    #[test]
    fn screenshot_command_rejects_args() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let result = ScreenshotCommand.run(&mut ctx, "extra");
        assert!(matches!(result, CommandResult::Error(_)));
    }
}
