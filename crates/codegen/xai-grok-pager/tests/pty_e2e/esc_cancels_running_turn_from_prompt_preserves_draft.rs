// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// **2× Esc from the PROMPT pane cancels a running turn even with a non-empty
/// draft, and the draft is PRESERVED** (unlike Ctrl+C, which clears the draft
/// first). First Esc arms "press again to cancel"; second Esc within the
/// confirm window fires cancel. The harness spawns with the default (non-vim)
/// config, so the Esc-cancel gate is on. Proves the real binary routes bare Esc
/// through `try_handle_esc_policy`'s turn-running arm before the idle
/// clear/rewind branches, and that cancel does not wipe the composer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn esc_cancels_running_turn_from_prompt_preserves_draft() {
    let content = ContentController::start().await.expect("start content");
    // Long paced stream so the turn is still visibly running when Esc lands.
    let long_response = format!(
        "{MOCK_RESPONSE_SENTINEL} {}",
        "streaming filler words for the cancellation window. ".repeat(120)
    );
    content.set_response(long_response);
    content.set_chunk_delay(Some(Duration::from_millis(50)));

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
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("stream started");

    // Type a draft into the prompt WHILE the turn streams (prompt stays focused
    // after submit). A distinctive single token avoids any wrapping ambiguity.
    let draft = "DRAFTKEEPME";
    harness.inject_keys(draft.as_bytes()).expect("type draft");
    harness
        .wait_for_text(draft, Duration::from_secs(10))
        .expect("draft renders in the composer");

    // 1× Esc arms cancel confirm (does not cancel yet; not idle clear).
    harness.inject_keys(keys::ESC).expect("press esc");
    harness
        .wait_for_text("press again to cancel", Duration::from_secs(5))
        .expect("first Esc must arm cancel confirm");
    assert!(
        !harness.contains_text("Turn cancelled by user"),
        "first Esc must not cancel yet\nscreen:\n{}",
        harness.screen_contents()
    );

    // 2× Esc within the confirm window cancels; draft is preserved.
    harness.inject_keys(keys::ESC).expect("press esc again");
    harness.update(Duration::from_millis(200));

    harness
        .wait_for_text("Turn cancelled by user", Duration::from_secs(15))
        .expect("turn cancelled marker");

    harness.update(Duration::from_millis(600));
    let screen = harness.screen_contents();

    // The draft must survive the cancel — Esc cancels, it does not clear.
    assert!(
        screen.contains(draft),
        "Esc cancel must preserve the draft (not clear it like Ctrl+C)\nscreen:\n{screen}"
    );
    assert!(
        !screen.contains("press again to clear"),
        "running-turn Esc must arm cancel, never the idle clear\nscreen:\n{screen}"
    );
    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
