use super::*;
#[test]
fn prompt_mode_from_session_mode_id_uses_acp_session_mode() {
    assert_eq!(
        PromptMode::Ask,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("ask"))
    );
    assert_eq!(
        PromptMode::Plan,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("plan"))
    );
    assert_eq!(
        PromptMode::Agent,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("default"))
    );
    assert_eq!(
        PromptMode::Agent,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("browser_use"))
    );
}
fn fn_def(name: &str) -> ToolDefinition {
    ToolDefinition::function(name, None::<&str>, serde_json::json!({"type": "object"}))
}
fn names(defs: &[ToolDefinition]) -> Vec<&str> {
    defs.iter().map(|d| d.function.name.as_str()).collect()
}
#[test]
fn cursor_filter_in_plan_mode_keeps_writes_and_shows_create_plan() {
    let defs = vec![
        fn_def("Read"),
        fn_def("Grep"),
        fn_def("Write"),
        fn_def("StrReplace"),
        fn_def("CreatePlan"),
        fn_def("SwitchMode"),
        fn_def("AskQuestion"),
    ];
    let filtered = filter_cursor_tools_by_plan_mode(defs, true);
    let kept = names(&filtered);
    assert!(kept.contains(&"Read"));
    assert!(kept.contains(&"Grep"));
    assert!(kept.contains(&"CreatePlan"));
    assert!(kept.contains(&"SwitchMode"));
    // Cursor AskQuestion is a different surface; only the grok questionnaire
    // names are stripped (see is_plan_mode_blocked_ask_user_tool_name).
    assert!(kept.contains(&"AskQuestion"));
    assert!(kept.contains(&"Write"));
    assert!(kept.contains(&"StrReplace"));
}
/// Plan mode must hard-strip `ask_user_question` from the advertised tool list.
/// Soft prompt bans alone left the tool available and models still opened
/// multi-choice plan questionnaires.
#[test]
fn plan_mode_tool_list_omits_ask_user_question() {
    let defs = vec![
        fn_def("read_file"),
        fn_def("search_replace"),
        fn_def("write"),
        fn_def("ask_user_question"),
        fn_def("AskUserQuestion"),
        fn_def("AskUser"),
        fn_def("enter_plan_mode"),
        fn_def("exit_plan_mode"),
    ];
    let in_plan = filter_cursor_tools_by_plan_mode(defs.clone(), true);
    let out_of_plan = filter_cursor_tools_by_plan_mode(defs.clone(), false);
    let in_names = names(&in_plan);
    assert!(
        !in_names.contains(&"ask_user_question"),
        "plan mode must not advertise ask_user_question: {in_names:?}"
    );
    assert!(!in_names.contains(&"AskUserQuestion"));
    assert!(!in_names.contains(&"AskUser"));
    assert!(in_names.contains(&"read_file"));
    assert!(in_names.contains(&"exit_plan_mode"));
    assert!(in_names.contains(&"search_replace"));
    // Outside plan mode the questionnaire stays available (non-plan use).
    let out_names = names(&out_of_plan);
    assert_eq!(out_names.len(), defs.len());
    assert!(out_names.contains(&"ask_user_question"));
}
#[test]
fn plan_mode_blocked_ask_user_name_matcher() {
    assert!(is_plan_mode_blocked_ask_user_tool_name("ask_user_question"));
    assert!(is_plan_mode_blocked_ask_user_tool_name("AskUserQuestion"));
    assert!(is_plan_mode_blocked_ask_user_tool_name("AskUser"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("exit_plan_mode"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("AskQuestion"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("read_file"));
}
/// Pins the `reconcile_plan_mode_with_prompt` transitions:
/// Plan → Pending, idempotent, non-plan modes exit cleanly.
#[test]
fn prompt_mode_plan_drives_tracker_into_pending_when_inactive() {
    use crate::session::plan_mode::{PlanModeState, PlanModeTracker};
    use std::path::PathBuf;
    fn reconcile(tracker: &mut PlanModeTracker, mode: PromptMode) {
        match mode {
            PromptMode::Plan => {
                tracker.enter_pending();
            }
            PromptMode::Agent | PromptMode::Ask => {
                if tracker.state() != PlanModeState::Inactive {
                    tracker.user_exit(false);
                }
            }
        }
    }
    let mut tracker = PlanModeTracker::new(PathBuf::from("/tmp/test"));
    assert_eq!(tracker.state(), PlanModeState::Inactive);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Agent);
    assert_eq!(tracker.state(), PlanModeState::Inactive);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Ask);
    assert_eq!(tracker.state(), PlanModeState::Inactive);
}
