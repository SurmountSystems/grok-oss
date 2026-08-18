//! `/start` -- continue paused or interrupted work in the current session.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct StartCommand;

impl SlashCommand for StartCommand {
    fn name(&self) -> &str {
        "start"
    }

    fn description(&self) -> &str {
        "Start paused or interrupted work in this session"
    }

    fn usage(&self) -> &str {
        "/start"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::StartPausedOrInterruptedWork)
    }
}
