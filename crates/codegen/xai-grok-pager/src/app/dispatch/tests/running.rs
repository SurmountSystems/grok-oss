//! `/running` dispatch: transcript report, not a dashboard.

use super::*;
use crate::app::actions::Action;
use crate::app::agent::AgentId;

#[test]
fn show_running_sessions_no_active_agent_is_noop() {
    let mut app = test_app();
    let effects = dispatch(Action::ShowRunningSessions, &mut app);
    assert!(
        effects.is_empty(),
        "ShowRunningSessions without an agent is a no-op"
    );
}

#[test]
fn show_running_sessions_commits_transcript_block() {
    let mut app = test_app_with_agent();
    let before = agent_scrollback_len(&app);
    let effects = dispatch(Action::ShowRunningSessions, &mut app);
    assert!(effects.is_empty(), "got: {effects:?}");
    assert_eq!(agent_scrollback_len(&app), before + 1);
    let text = last_system_text(&app, AgentId(0));
    let lower = text.to_ascii_lowercase();
    assert!(
        lower.contains("running grok-oss"),
        "slash dispatch must commit a running grok-oss report; got {text:?}"
    );
}
