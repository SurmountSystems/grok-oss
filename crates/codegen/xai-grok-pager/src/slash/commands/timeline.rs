//! `/timeline` -- toggle the timeline sidebar (per-turn tick rail).
//!
//! Computes the new value itself and dispatches the typed
//! `Action::SetTimeline(bool)`, mirroring `/timestamps`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::{ModeSupport, Remedy};

pub struct TimelineCommand;

impl SlashCommand for TimelineCommand {
    fn name(&self) -> &str {
        "timeline"
    }

    fn description(&self) -> &str {
        "Toggle the timeline sidebar"
    }

    /// Minimal mode has no interactive scrollback pane for the rail.
    fn mode_support(&self) -> ModeSupport {
        ModeSupport::FullscreenOnly(Remedy::SwitchMode {
            why: "the timeline rail needs the interactive scrollback pane",
        })
    }

    fn usage(&self) -> &str {
        "/timeline"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let new = !crate::appearance::cache::load_show_timeline();
        CommandResult::Action(Action::SetTimeline(new))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_only_in_minimal() {
        assert!(
            !TimelineCommand
                .mode_support()
                .supports(crate::app::ScreenMode::Minimal)
        );
        assert!(
            TimelineCommand
                .mode_support()
                .supports(crate::app::ScreenMode::Fullscreen)
        );
    }
}
