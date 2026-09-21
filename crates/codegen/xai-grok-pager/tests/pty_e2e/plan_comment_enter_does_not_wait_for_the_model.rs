#[allow(unused_imports)]
use super::common::*;

/// After Comment / Isolated Preview Human-box typing, Enter must not start
/// a Waiting-for-the-model Prompt. The critique rides Approve
/// (`PLAN_APPROVED_REVIEW_COMMENTS_LEAD`). Empty Enter never Approves;
/// see `plan_revise_empty_enter_does_not_approve`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn plan_comment_enter_does_not_wait_for_the_model() {
    let content = ContentController::start().await.expect("start content");
    content.set_response(format!("{MOCK_RESPONSE_SENTINEL} first turn done."));

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness = PtyHarness::spawn_with_content_env_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        &["--yolo", "--trust", "--no-leader"],
        &[],
        Some(content.home()),
    )
    .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    harness.inject_keys(b"go\r").expect("first turn");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(40))
        .expect("first turn streams");

    let dir = session_dir(&content, &mut harness);
    std::fs::write(dir.join("plan.md"), plan_body("CMT", 8)).expect("seed plan.md");

    let _expectation = expect_tool_turn(&content, "call_plan_cmt", "exit_plan_mode", "{}".into());
    harness
        .inject_keys(b"present the plan\r")
        .expect("submit plan prompt");
    harness
        .wait_for_text("Plan ready. Side panel open", Duration::from_secs(60))
        .unwrap_or_else(|e| {
            panic!(
                "plan approval never parked: {e}\nscreen:\n{}",
                harness.screen_contents()
            )
        });

    const CRITIQUE: &str = "keep the join order from the archive index";
    harness
        .inject_keys(CRITIQUE.as_bytes())
        .expect("type critique");
    for _ in 0..5 {
        harness.update(Duration::from_millis(100));
    }
    harness.inject_keys(b"\r").expect("send critique Enter");
    for _ in 0..15 {
        harness.update(Duration::from_millis(100));
    }
    let screen = harness.screen_contents();

    assert!(
        !screen.contains("Waiting for the model"),
        "Comment / Isolated Preview critique must not start a Prompt; screen:\n{screen}"
    );
    assert!(
        screen.contains("Plan ready. Side panel open"),
        "critique Enter must leave plan approval open; screen:\n{screen}"
    );
    assert!(
        screen.contains(CRITIQUE) || screen.contains("ride Approve"),
        "critique must stay on the parked plan for Approve; screen:\n{screen}"
    );
    assert!(
        !screen.contains("Enter:approve"),
        "footer must not advertise Enter as approve; screen:\n{screen}"
    );
    assert!(
        !screen.contains("panicked"),
        "pager panicked\nscreen:\n{screen}"
    );

    harness.quit().expect("clean quit");
}
