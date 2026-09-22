//! `/uptime` prints the short local 15 minute and 24 hour reading.
//! It does not open a network connection and it does not send a request to xAI.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::uptime::{WindowPair, uptime_slash_output};

/// Show the local uptime reading.
pub struct UptimeCommand;

/// Text `/uptime` returns. `Some` formats windows already in hand.
/// `None` reads the local store. Neither path calls xAI.
pub fn render_uptime_slash(windows: Option<&WindowPair>) -> String {
    match windows {
        Some(windows) => uptime_slash_output(windows),
        None => live_uptime_slash_text(),
    }
}

fn live_uptime_slash_text() -> String {
    let home = xai_grok_config::grok_home();
    let dir = crate::uptime::uptime_dir(&home);
    let now_ms = chrono::Utc::now().timestamp_millis();
    crate::uptime::text_beside_status(&dir, now_ms)
}

impl SlashCommand for UptimeCommand {
    fn name(&self) -> &str {
        "uptime"
    }

    fn description(&self) -> &str {
        "Show the local 15 minute and 24 hour uptime reading"
    }

    fn usage(&self) -> &str {
        "/uptime"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(render_uptime_slash(None))
    }
}
