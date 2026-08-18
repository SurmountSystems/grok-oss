//! `/running` (alias `/windows`) -- list live grok-oss TUI windows on this machine.
//!
//! Transcript table, refresh on open. Not Agent Dashboard, not `/sessions`,
//! not `/tasks`, not `/resume`, and not `/start`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// List running grok-oss sessions on this machine.
pub struct RunningCommand;

impl SlashCommand for RunningCommand {
    fn name(&self) -> &str {
        "running"
    }

    fn aliases(&self) -> &[&str] {
        &["windows"]
    }

    fn description(&self) -> &str {
        "List running grok-oss sessions on this machine"
    }

    fn usage(&self) -> &str {
        "/running"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::ShowRunningSessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn running_command_name_and_windows_alias() {
        let cmd = super::RunningCommand;
        assert_eq!(cmd.name(), "running");
        assert_eq!(cmd.aliases(), &["windows"]);
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        assert!(matches!(
            cmd.run(&mut ctx, ""),
            crate::slash::command::CommandResult::Action(
                crate::app::actions::Action::ShowRunningSessions
            )
        ));
    }

    #[test]
    fn running_slash_lists_sibling_fixture_row() {
        let dir = tempfile::tempdir().unwrap();
        let self_pid = std::process::id();
        let mut sibling = spawn_live_grok_named();
        let sibling_pid = sibling.id();
        let mut not_grok = spawn_live_not_grok();
        let not_grok_pid = not_grok.id();

        let fixture = format!(
            r#"[
  {{
    "session_id": "sibling-fixture-row",
    "pid": {sibling_pid},
    "cwd": "/tmp/running-slash-sibling-cwd",
    "opened_at": "2026-08-16T12:00:00Z",
    "title": "fixture summary title",
    "activity": "idle",
    "activity_line": "turn paused",
    "prompt": "SECRET_PLEASE_IMPLEMENT_THE_LOGIN_FLOW",
    "tool_arguments": {{"cmd": "cat /etc/shadow"}}
  }},
  {{
    "session_id": "this-window-sess",
    "pid": {self_pid},
    "cwd": "/tmp/running-slash-this-cwd",
    "opened_at": "2026-08-16T12:01:00Z"
  }},
  {{
    "session_id": "dead-window-must-not-list",
    "pid": 2000000000,
    "cwd": "/tmp/dead-must-not-list",
    "opened_at": "2026-08-16T12:00:00Z"
  }},
  {{
    "session_id": "not-grok-must-not-list",
    "pid": {not_grok_pid},
    "cwd": "/tmp/not-grok-must-not-list",
    "opened_at": "2026-08-16T12:00:00Z"
  }}
]"#
        );
        let path = dir.path().join("active_sessions.json");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(fixture.as_bytes()).unwrap();
        file.flush().unwrap();

        let text = crate::running_sessions::slash_report_in(dir.path(), Some(self_pid));
        let _ = sibling.kill();
        let _ = sibling.wait();
        let _ = not_grok.kill();
        let _ = not_grok.wait();

        assert!(
            text.contains("sibling-fixture-row") || text.contains("sibling-"),
            "slash report must list the planted sibling session id; got {text:?}"
        );
        assert!(
            text.contains("/tmp/running-slash-sibling-cwd"),
            "slash report must list the sibling cwd; got {text:?}"
        );
        assert!(
            text.contains(&sibling_pid.to_string()),
            "slash report must list the sibling pid; got {text:?}"
        );
        assert!(
            text.to_ascii_lowercase().contains("this window"),
            "slash report must mark this window; got {text:?}"
        );
        assert!(
            !text.contains("dead-window-must-not-list")
                && !text.contains("/tmp/dead-must-not-list"),
            "dead pid must not appear; got {text:?}"
        );
        assert!(
            !text.contains("not-grok-must-not-list")
                && !text.contains("/tmp/not-grok-must-not-list"),
            "live non-grok pid must not appear; got {text:?}"
        );
        assert_forbidden_claim_absent(&text);
    }

    fn assert_forbidden_claim_absent(text: &str) {
        let lower = text.to_ascii_lowercase();
        for needle in [
            "secret_please_implement_the_login_flow",
            "tool_arguments",
            "cat /etc/shadow",
            "bearer ",
        ] {
            assert!(
                !lower.contains(needle),
                "report must omit private fields and prompt text (case-insensitive); \
                 found {needle:?} in {text:?}"
            );
        }
    }

    fn spawn_live_grok_named() -> std::process::Child {
        use std::os::unix::process::CommandExt;
        std::process::Command::new("sleep")
            .arg0("grok-oss-sibling")
            .arg("60")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn grok-named sleep as a sibling pid")
    }

    fn spawn_live_not_grok() -> std::process::Child {
        std::process::Command::new("sleep")
            .arg("60")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn sleep as a live non-grok pid")
    }
}
