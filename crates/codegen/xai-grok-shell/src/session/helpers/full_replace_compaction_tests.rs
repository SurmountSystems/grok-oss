use super::*;
use crate::session::helpers::prepared_compaction_history::build_compaction_chat_history;

#[test]
fn sampler_state_keeps_exact_latest_prepared_items() {
    let mut state = SamplerState::default();
    let first = build_compaction_chat_history(vec![ConversationItem::user("first")], None, true, 0);
    let second =
        build_compaction_chat_history(vec![ConversationItem::user("second")], None, true, 0);
    state.record_attempt(&first);
    state.record_attempt(&second);

    assert_eq!(
        serde_json::to_value(state.last_attempted_items.unwrap()).unwrap(),
        serde_json::to_value(second.items).unwrap()
    );
}

#[test]
fn compact_failure_credit_402_is_hop_eligible() {
    let err = acp::Error::internal_error()
        .data("compact failed: API error (status 402 Payment Required): Payment Required");
    assert!(compact_failure_is_credit(&CompactFailure::Deterministic(
        err
    )));
    let size = acp::Error::internal_error()
        .data("compact failed: The prompt is too long for this model's context window.");
    assert!(!compact_failure_is_credit(&CompactFailure::Deterministic(
        size
    )));
}

/// Named contract: compact HTTP 502 (even with Payment Required in the body)
/// must not hop as credit exhaust or recode as 402.
#[test]
fn compact_failure_http_502_is_not_credit_hop() {
    let err = acp::Error::internal_error()
        .data("compact failed: API error (status 502 Bad Gateway): Payment Required");
    assert!(
        !compact_failure_is_credit(&CompactFailure::Deterministic(err)),
        "HTTP 502 must not hop compact to console or SuperGrok dollar credits"
    );
    let transient = acp::Error::internal_error()
        .data("compact failed: Grok is temporarily unavailable. Please try again in a moment. (HTTP 502).");
    assert!(
        !compact_failure_is_credit(&CompactFailure::Transient(transient)),
        "HTTP 502 status_user_message must not be credit hop"
    );
}

/// Named contract: fail-open. Ambiguous "Payment Required" without HTTP 402
/// (or 400/403/429 plus credit wording) must not hop compact. A wrap that
/// names status 402 still hops (`compact_failure_credit_402_is_hop_eligible`).
#[test]
fn compact_failure_payment_required_without_status_is_not_credit_hop() {
    let err = acp::Error::internal_error().data("compact failed: Payment Required");
    assert!(
        !compact_failure_is_credit(&CompactFailure::Deterministic(err)),
        "bare Payment Required without a 402 must not hop compact"
    );
    let credits = acp::Error::internal_error().data("compact failed: out of credits");
    assert!(
        !compact_failure_is_credit(&CompactFailure::Deterministic(credits)),
        "out of credits without a 402 must not hop compact"
    );
    let forbidden = acp::Error::internal_error()
        .data("compact failed: API error (status 403 Forbidden): run out of credits");
    assert!(
        compact_failure_is_credit(&CompactFailure::Deterministic(forbidden)),
        "status 403 plus credit wording still hops"
    );
}
