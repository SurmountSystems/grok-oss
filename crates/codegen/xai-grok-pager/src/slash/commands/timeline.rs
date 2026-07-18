<<<<<<< HEAD
//! `/timeline`: toggle the timeline sidebar (per-turn tick rail).
//!
//! Computes the new value itself and dispatches the typed `Action::SetTimeline(bool)`, mirroring `/timestamps`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use crate::slash::{ModeSupport, Remedy};
=======
//! `/timeline` -- toggle the timeline sidebar (per-turn tick rail).
//!
//! Computes the new value itself and dispatches the typed
//! `Action::SetTimeline(bool)`, mirroring `/timestamps`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
>>>>>>> e3fdf3ed (Merge 2 (#4))

pub struct TimelineCommand;

impl SlashCommand for TimelineCommand {
<<<<<<< HEAD
    slash_meta! {
        name: "timeline",
        description: "Toggle the timeline sidebar",
        usage: "/timeline",
        mode_support: ModeSupport::FullscreenOnly(Remedy::SwitchMode {
            why: "the timeline rail needs the interactive scrollback pane",
        }),
=======
    fn name(&self) -> &str {
        "timeline"
    }

    fn description(&self) -> &str {
        "Toggle the timeline sidebar"
    }

    /// Minimal mode has no interactive scrollback pane for the rail.
    fn available_in_minimal(&self) -> bool {
        false
    }

    fn usage(&self) -> &str {
        "/timeline"
>>>>>>> e3fdf3ed (Merge 2 (#4))
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let new = !crate::appearance::cache::load_show_timeline();
        CommandResult::Action(Action::SetTimeline(new))
    }
}
<<<<<<< HEAD
=======

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_available_in_minimal() {
        assert!(!TimelineCommand.available_in_minimal());
    }
}
>>>>>>> e3fdf3ed (Merge 2 (#4))
