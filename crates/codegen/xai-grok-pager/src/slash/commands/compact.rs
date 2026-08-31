//! `/compact` -- compact conversation history.
//!
//! Takes an optional context argument. Stays on the existing queue pipeline:
//! returns `CommandResult::QueueCommand` so the dispatch layer enqueues it
//! as `QueueEntryKind::Command`.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::queue_schedule::{
    compact_command_text, queue_later_command, split_schedule_token,
};

/// Compact the conversation history, optionally with a focus context.
pub struct CompactCommand;

impl SlashCommand for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Compact conversation history"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn usage(&self) -> &str {
        "/compact [queue|later] [compaction instructions]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    /// Args are optional -- `/compact` with no args is valid.
    fn args_required(&self) -> bool {
        false
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("compaction instructions")
    }

    fn aliases(&self) -> &[&str] {
        &["compaction"]
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let (hold, rest) = split_schedule_token(args);
        let text = compact_command_text(rest);
        if hold {
            queue_later_command(text)
        } else {
            CommandResult::QueueCommand(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compaction_is_alias_of_compact() {
        assert_eq!(CompactCommand.aliases(), &["compaction"]);
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "compact"),
            "expected /compact in builtins, got {names:?}"
        );
        let reg = crate::slash::registry::CommandRegistry::new(
            crate::slash::commands::builtin_commands(),
        );
        assert!(
            reg.get("compaction").is_some(),
            "/compaction must resolve to the compact builtin"
        );
    }
}
