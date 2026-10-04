use super::*;
use crate::session::helpers::prepared_compaction_history::build_compaction_chat_history;

#[test]
fn compact_failure_maps_onto_engine_error_classes() {
    let err = |msg: &str| acp::Error::internal_error().data(msg.to_string());

    // Overflow → ContextOverflow with the message preserved, so the flag
    // doesn't depend on message text.
    let mapped = compact_failure_to_sample_error(CompactFailure::Overflow(err(
        "compact failed: API error (status 413 Payload Too Large): Request failed (HTTP 413).",
    )));
    assert!(matches!(&mapped, CompactionSampleError::ContextOverflow(m)
        if m.contains("HTTP 413")));
    assert!(mapped.is_deterministic());

    let mapped =
        compact_failure_to_sample_error(CompactFailure::Deterministic(err("compact failed: 400")));
    assert!(matches!(&mapped, CompactionSampleError::Build(_)));

    let mapped =
        compact_failure_to_sample_error(CompactFailure::Transient(err("compact failed: blip")));
    assert!(matches!(&mapped, CompactionSampleError::Other(_)));
    assert!(!mapped.is_deterministic());
}

#[test]
fn sampler_state_keeps_exact_latest_prepared_items() {
    let mut state = SamplerState::default();
    let first =
        build_compaction_chat_history(vec![ConversationItem::user("first")], None, true, None, 0);
    let second =
        build_compaction_chat_history(vec![ConversationItem::user("second")], None, true, None, 0);
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

#[test]
fn acp_error_message_reads_string_and_object_shaped_data() {
    let string_err = acp::Error::internal_error().data("compact failed: boom");
    assert_eq!(
        crate::sampling::error::acp_error_message(&string_err),
        "compact failed: boom"
    );
    let object_err = acp::Error::internal_error()
        .data(serde_json::json!({"kind": "compact_cancelled", "message": "compact cancelled"}));
    assert_eq!(
        crate::sampling::error::acp_error_message(&object_err),
        "compact cancelled"
    );
    let no_data = acp::Error::internal_error();
    assert_eq!(
        crate::sampling::error::acp_error_message(&no_data),
        "Internal error"
    );
}
