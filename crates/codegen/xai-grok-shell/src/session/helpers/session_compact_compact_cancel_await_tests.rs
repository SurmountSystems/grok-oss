use super::*;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn pre_cancelled_token_skips_fut() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = await_unless_cancelled(&cancel, async {
        panic!("fut must not run when already cancelled");
    })
    .await
    .unwrap_err();
    assert!(matches!(err, CompactFailure::Cancelled));
}

#[tokio::test]
async fn cancel_aborts_pending_open() {
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel2.cancel();
    });
    let started = std::time::Instant::now();
    let err = await_unless_cancelled(&cancel, async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        0u8
    })
    .await
    .unwrap_err();
    assert!(matches!(err, CompactFailure::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stop must abort stream-open wait, elapsed {:?}",
        started.elapsed()
    );
}

/// Operator: Isolated Preview `/compact` queued while retry chrome said
/// `Retrying the model request (attempt 2): waiting for first token`
/// for 11m27s. Compact's first-token wait is the headers budget (120s).
#[test]
fn compact_first_token_wait_is_headers_budget_not_eleven_minutes() {
    let chrome = "Retrying the model request (attempt 2): waiting for first token";
    assert!(chrome.contains("Retrying the model request"));
    assert!(chrome.contains("waiting for first token"));
    let idle = Duration::from_secs(300);
    let budget = xai_grok_sampler::stream::first_token_wait(idle);
    assert!(
        budget < Duration::from_secs(11 * 60),
        "compact must not hang 11 minutes waiting for the first token, got {budget:?}"
    );
    assert!(
        budget <= idle,
        "compact first-token wait must not exceed post-token idle, got {budget:?}"
    );
    let deadline = std::time::Instant::now() + budget;
    let wait = compact_stream_wait(false, deadline, idle, std::time::Instant::now());
    assert!(
        wait <= budget,
        "pre-token compact wait must stay inside the first-token budget, got {wait:?}"
    );
    let err = compact_wait_expired(false, budget, idle, 0);
    let CompactFailure::Transient(acp_err) = err else {
        panic!("first-token compact miss must fail-open as Transient");
    };
    let msg = format!("{acp_err:?}");
    assert!(
        msg.to_ascii_lowercase().contains("first token"),
        "compact timeout must name the first token, got {msg}"
    );
}

#[test]
fn compact_responses_text_delta_is_first_token_not_scaffolding() {
    let text = ResponseStreamEvent::ResponseOutputTextDelta(
        async_openai::types::responses::ResponseTextDeltaEvent {
            sequence_number: 1,
            item_id: "item-1".into(),
            output_index: 0,
            content_index: 0,
            delta: "hello".into(),
            logprobs: None,
        },
    );
    assert!(!compact_responses_event_is_scaffolding(&text));
    assert!(compact_responses_event_is_token(&text));
    let empty = ResponseStreamEvent::ResponseOutputTextDelta(
        async_openai::types::responses::ResponseTextDeltaEvent {
            sequence_number: 2,
            item_id: "item-1".into(),
            output_index: 0,
            content_index: 0,
            delta: String::new(),
            logprobs: None,
        },
    );
    assert!(!compact_responses_event_is_token(&empty));
}
