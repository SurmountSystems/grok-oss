use crate::app::actions::Action;
<<<<<<< HEAD
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use crate::slash::{ModeSupport, Remedy};
=======
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
>>>>>>> e3fdf3ed (Merge 2 (#4))

pub struct JumpCommand;

impl SlashCommand for JumpCommand {
<<<<<<< HEAD
    slash_meta! {
        name: "jump",
        description: "Jump to a turn in the conversation",
        usage: "/jump",
        session_scoped: true,
        mode_support: ModeSupport::FullscreenOnly(Remedy::SwitchMode {
            why: "minimal scrolls with your terminal's native scrollback",
        }),
=======
    fn name(&self) -> &str {
        "jump"
    }

    fn description(&self) -> &str {
        "Jump to a turn in the conversation"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    /// Minimal mode has no interactive scrollback pane to scroll — the
    /// terminal's own scrollback covers it (same gate as `/find`).
    fn available_in_minimal(&self) -> bool {
        false
    }

    fn usage(&self) -> &str {
        "/jump"
>>>>>>> e3fdf3ed (Merge 2 (#4))
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::JumpShowPicker)
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

    #[test]
    fn jump_returns_show_picker_action() {
        let models = ModelState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Fullscreen,
<<<<<<< HEAD
            billing_surface_visible: true,
            usage_command_visible: true,
=======
>>>>>>> e3fdf3ed (Merge 2 (#4))
            pager_state: PagerLocalSnapshot::default(),
        };
        let result = JumpCommand.run(&mut ctx, "");
        assert!(matches!(
            result,
            CommandResult::Action(Action::JumpShowPicker)
        ));
    }
<<<<<<< HEAD
=======

    #[test]
    fn not_available_in_minimal() {
        // Native terminal scrollback replaces in-app scrolling in minimal.
        assert!(!JumpCommand.available_in_minimal());
    }
>>>>>>> e3fdf3ed (Merge 2 (#4))
}
