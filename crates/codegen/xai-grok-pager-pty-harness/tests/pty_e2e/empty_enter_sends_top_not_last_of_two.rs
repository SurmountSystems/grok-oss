// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// With two mid-turn queued rows, empty Enter sends the **top** (first) row now, not the most recently typed one.
/// The running turn is cancelled silently and alpha runs as its own next turn, with the interjection preamble.
/// Bravo stays queued and promotes afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn empty_enter_sends_top_not_last_of_two() {
    let content = ContentController::start().await.expect("start content");
<<<<<<< HEAD:crates/codegen/xai-grok-pager-pty-harness/tests/pty_e2e/empty_enter_sends_top_not_last_of_two.rs
    let mut turn_one = content.expect_agent_turn_blocked(
        "running turn before top-row send-now",
        slow_turn_text("TURNONE"),
    );
    let mut turn_two =
        content.expect_agent_turn("top queued row", "TURNTWO top-row send-now acknowledged.");
    let mut turn_three = content.expect_agent_turn(
        "remaining queued row",
        "TURNTHREE remaining queue promoted.",
=======
    let mut turn_one = content.expect_response_blocked(
        "running turn before top-row send-now",
        InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
        ScriptedResponse::sse(chat_completions_message_events(&slow_turn_text("TURNONE"))),
    );
    let mut turn_two = content.expect_response(
        "top queued row",
        InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
        ScriptedResponse::sse(chat_completions_message_events(
            "TURNTWO top-row send-now acknowledged.",
        )),
    );
    let mut turn_three = content.expect_response(
        "remaining queued row",
        InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
        ScriptedResponse::sse(chat_completions_message_events(
            "TURNTHREE remaining queue promoted.",
        )),
>>>>>>> e3fdf3ed (Merge 2 (#4)):crates/codegen/xai-grok-pager/tests/pty_e2e/empty_enter_sends_top_not_last_of_two.rs
    );

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");
    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text("TURNONE", Duration::from_secs(45))
        .expect("turn 1 streaming");
    tokio::time::timeout(Duration::from_secs(10), turn_one.wait_blocked())
        .await
        .expect("turn 1 reached completion barrier");

    harness
        .inject_keys(b"queue-alpha-top\r")
        .expect("queue alpha");
    harness
        .wait_for_text("queue-alpha-top", Duration::from_secs(20))
        .expect("alpha visible");
    harness
        .inject_keys(b"queue-bravo-later\r")
        .expect("queue bravo");
    harness
        .wait_for_text("queue-bravo-later", Duration::from_secs(20))
        .expect("bravo visible");

    harness
        .inject_keys(b"\r")
        .expect("empty Enter send-now top");
    turn_one.release();
<<<<<<< HEAD:crates/codegen/xai-grok-pager-pty-harness/tests/pty_e2e/empty_enter_sends_top_not_last_of_two.rs
    // Alpha (the promoted TOP row) then bravo drain back-to-back after the completion release. Waiting
    // on any on-screen marker is thus racy: a flaky observation, not a real failure, same rationale as
    // `removed_queued_prompt_never_sent`.
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    while !all_user_messages(&content)
        .iter()
        .any(|u| u.contains("queue-bravo-later"))
    {
        assert!(
            std::time::Instant::now() < deadline,
            "queued rows never drained through to the final turn\nscreen:\n{}",
            harness.screen_contents()
        );
        harness.update(Duration::from_millis(100));
    }
=======
    // Alpha (the promoted TOP row) then bravo drain back-to-back. Each
    // promoted "❯ …" block and the intermediate TURNTWO reply is scrolled
    // above the viewport by the next turn's start-adoption before a 100ms poll
    // can observe it, so gating on those transient markers is inherently racy.
    // Gate only on the FINAL reply (stable at the viewport head) and prove the
    // top-row order + send-now silence via the recorded wire below, which is
    // not subject to scrolling.
    harness
        .wait_for_text("TURNTHREE", Duration::from_secs(90))
        .expect("all queued turns drained through to the final reply");
>>>>>>> e3fdf3ed (Merge 2 (#4)):crates/codegen/xai-grok-pager/tests/pty_e2e/empty_enter_sends_top_not_last_of_two.rs
    tokio::time::timeout(Duration::from_secs(10), turn_two.wait_satisfied())
        .await
        .expect("top queued row expectation satisfied");
    tokio::time::timeout(Duration::from_secs(10), turn_three.wait_satisfied())
        .await
        .expect("remaining queued row expectation satisfied");

    assert!(
        !harness.contains_text("Turn cancelled by user"),
        "send-now cancel must not render a cancelled marker\nscreen:\n{}",
        harness.screen_contents()
    );

    let users = all_user_message_blobs(&content);
    let alpha = users
        .iter()
        .find(|u| u.contains("queue-alpha-top"))
        .unwrap_or_else(|| panic!("top row never on wire: {users:#?}"));
    assert!(
        alpha.contains(INTERJECTION_WIRE_PREFIX),
        "send-now must use the interjection preamble: {alpha}"
    );
    assert!(
        alpha.contains("<user_query>"),
        "send-now must wrap the steered text in user_query: {alpha}"
    );

    // The final request's user sequence proves the order: prompt, then the TOP row (alpha), then bravo
    let bodies = content.request_bodies();
    let last = bodies.last().expect("final request recorded");
    let finals: Vec<String> = last["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .filter(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_str()
                    .is_some_and(|c| c.contains("<user_query>"))
        })
        .map(|m| m["content"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(3, finals.len(), "expected 3 user messages: {finals:#?}");
    assert!(finals[0].contains(PROMPT), "first: {finals:#?}");
    assert!(
        finals[1].contains("queue-alpha-top"),
        "second must be the TOP row: {finals:#?}"
    );
    assert!(
        finals[2].contains("queue-bravo-later") && !finals[2].contains(INTERJECTION_WIRE_PREFIX),
        "third must be the naturally drained bravo with no send-now preamble: {finals:#?}"
    );

    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );
    harness.quit().expect("clean quit");
}
