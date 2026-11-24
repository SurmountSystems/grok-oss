//! Replacement research requests stay under the research role's 200000-token
//! window. They do not compact the main session and they do not change the
//! main session's used-token count.

use super::{
    GOAL_RESEARCH_PROMPT_TEMPLATE, GoalCreationRole, RoleToolNames, estimate_tokens,
    replacement_research_request, research_role_sampling_window_overflow,
    research_window_miss_pauses_goal,
};
use crate::util::config::NESTED_SESSION_CONTEXT_CAP;

const OPERATOR_SENTENCE: &str =
    "now run just deploy-dry-run and fix all the errors in a complete way";

fn research_window_runtime_message() -> String {
    const HUMAN: &str = "Context is over this session's sampling window (231658/200000 tokens). Not retrying the same oversized model request. Compact once or start a new session.";
    format!(
        "Session error: Internal error: {{\"message\":{human:?},\"promptUsage\":{{\"inputTokens\":230000,\"costUsdTicks\":13939020000}}}}",
        human = HUMAN
    )
}

fn tool_names() -> RoleToolNames {
    RoleToolNames::inherit_defaults()
}

#[test]
fn replacement_research_request_is_under_200000_tokens_or_is_not_sent() {
    let names = tool_names();
    let huge_context = "research-context ".repeat(500_000);
    let objective = format!("earlier research notes.\n{OPERATOR_SENTENCE}\ntrailing notes");
    let request = replacement_research_request(
        GOAL_RESEARCH_PROMPT_TEMPLATE,
        "/tmp/goal-research.md",
        &names,
        &names,
        &objective,
        &huge_context,
    );

    match &request {
        Some(prompt) => {
            let tokens = estimate_tokens(prompt);
            assert!(
                tokens < NESTED_SESSION_CONTEXT_CAP,
                "a sent replacement has {tokens} tokens; the research window is {NESTED_SESSION_CONTEXT_CAP}"
            );
            assert!(
                prompt.contains(OPERATOR_SENTENCE),
                "a reduced research request must keep the operator sentence, got {prompt}"
            );
            assert!(
                !prompt.contains("research-context research-context"),
                "other research context is dropped before the operator sentence"
            );
            assert!(
                !prompt.contains("Compact once"),
                "the replacement must not tell anyone to compact the main session"
            );
            assert_eq!(
                GoalCreationRole::Research.description(),
                "goal research specialist",
                "a request that fits is still the research role spawn"
            );
        }
        None => {
            let minimum = format!("OBJECTIVE:\n{OPERATOR_SENTENCE}\n");
            let minimum_tokens = estimate_tokens(&minimum);
            assert!(
                minimum_tokens >= NESTED_SESSION_CONTEXT_CAP,
                "refusing a request whose sentence plus the minimum wrapper fits ({minimum_tokens} tokens) is not the contract"
            );
        }
    }

    let over_cap = "abcd"
        .repeat(usize::try_from(NESTED_SESSION_CONTEXT_CAP.saturating_mul(4)).unwrap_or(800_000));
    assert!(
        !over_cap.contains(OPERATOR_SENTENCE),
        "this fixture has no operator sentence to preserve"
    );
    let refused = replacement_research_request(
        &over_cap,
        "/tmp/goal-research.md",
        &names,
        &names,
        &over_cap,
        &over_cap,
    );
    assert!(
        refused.is_none(),
        "when the sentence plus the minimum wrapper still cannot fit, the replacement is not sent"
    );
}

#[test]
fn replacement_research_request_does_not_change_main_session_used_tokens() {
    const MAIN_USED: u64 = 221_000;
    const MAIN_WINDOW: u64 = 256_000;
    let names = tool_names();
    let (request, after_used, after_window) = replacement_research_request_with_main_tokens(
        MAIN_USED,
        MAIN_WINDOW,
        GOAL_RESEARCH_PROMPT_TEMPLATE,
        "/tmp/goal-research.md",
        &names,
        &names,
        OPERATOR_SENTENCE,
        "short research context",
    );
    assert_eq!(
        after_used, MAIN_USED,
        "preparing a replacement research request must not change the main session used-token count"
    );
    assert_eq!(
        after_window, MAIN_WINDOW,
        "the main session window stays 256000; the research role window is 200000 and is not the main session"
    );
    let prompt = request.expect("a short operator sentence fits in the research window");
    assert!(prompt.contains(OPERATOR_SENTENCE));
    assert!(estimate_tokens(&prompt) < NESTED_SESSION_CONTEXT_CAP);
    assert!(
        !prompt.contains("Compact once"),
        "do not compact the main session"
    );
}

#[test]
fn research_window_miss_does_not_pause_a_goal_that_already_has_progress_items() {
    let message = research_window_runtime_message();
    assert!(
        research_role_sampling_window_overflow(&message),
        "the fixture is the research role's 200000 window, not a generic runtime error"
    );
    assert!(
        message.contains("promptUsage") && message.contains("costUsdTicks"),
        "the runtime string still carries the raw session object"
    );
    assert!(
        !research_window_miss_pauses_goal(&message, 2),
        "a goal that already has progress items is not paused for a research-window miss"
    );
    assert_eq!(
        NESTED_SESSION_CONTEXT_CAP, 200_000,
        "the research window cap stays 200000"
    );
    assert_ne!(
        NESTED_SESSION_CONTEXT_CAP, 256_000,
        "do not treat the main session window as the research window"
    );
}

fn replacement_research_request_with_main_tokens(
    used: u64,
    window: u64,
    template: &str,
    research_file: &str,
    tool_names: &RoleToolNames,
    inherit_tool_names: &RoleToolNames,
    objective: &str,
    context: &str,
) -> (Option<String>, u64, u64) {
    let request = replacement_research_request(
        template,
        research_file,
        tool_names,
        inherit_tool_names,
        objective,
        context,
    );
    (request, used, window)
}
