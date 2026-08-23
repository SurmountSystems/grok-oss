//! Pure data types for the xAI sampling / chat-completion API layer.
//!
//! This crate contains the API-agnostic conversation types, chat completion
//! request/response types, streaming types, and error types used across the
//! xAI agent stack.  It intentionally contains **no I/O** (no HTTP clients,
//! no file system access) so it can be depended on by downstream crates
//! (e.g., `xai-chat-state`) without pulling in the full `xai-grok-shell`.

pub mod billing_credits_card;
pub mod conversation;
pub mod doom_loop;
pub mod error;
pub mod messages;
pub mod provider_error;
pub mod serde_helpers;
pub mod tool_overrides;
pub mod types;

pub use self::billing_credits_card::{
    BILLING_CREDITS_CARD_NAMED_FIELD, BillingCreditsCard,
    billing_credits_card_from_supergrok_prepaid_balance,
    billing_credits_cents_from_core_invoice_prepaid_remaining,
    billing_credits_usd_from_core_invoice_prepaid_remaining,
    billing_credits_usd_from_included_period_percent, billing_credits_usd_from_named_json_field,
    current_billing_credits_usd, prefer_live_documented_usd_over_stored,
};
pub use self::conversation::*;
pub use self::doom_loop::{
    DOOM_LOOP_CHECK_EVENT_TYPE, DOOM_LOOP_CHECK_HEADER, DoomLoopPeek, DoomLoopRecoveryPolicy,
    DoomLoopSignal, DoomLoopSignalKind, is_check_event, peek_doom_loop,
};
pub use self::error::{
    ApiErrorCode, COMPACT_CREDIT_BLOCK_ADD_CREDITS_LIE, EmptyReason, EmptyResponseContext,
    INVALID_IMAGE_ERROR_CODE, ResponseModelMetadata, Result, SamplingError, SentCredential,
    compact_credit_block_user_message, console_team_prepaid_stay_on_supergrok_user_message,
    credit_exhausted_user_message, credit_exhausted_user_message_for_included_period,
    format_http_status, http_status_label, is_compact_credit_block_add_credits_lie,
    is_console_team_prepaid_message, is_context_length_error, is_credentials_rejected_message,
    is_credit_exhausted_compact_wrap, is_credit_exhausted_message, is_edge_outage_status,
    is_retryable_api_status, is_server_or_gateway_outage_status, is_transient_api_status,
    message_names_server_or_gateway_outage, outage_exhausted_user_message, parse_error_code,
    status_user_message, strip_api_error_status_prefix, user_facing_api_error_message,
};
pub use self::tool_overrides::{
    ClearableField, SearchDateBound, SearchDateBoundError, ToolOverrides, ToolOverridesUpdate,
    WebSearchOptions, XSearchOptions,
};
pub use self::types::*;

// Re-export async-openai crate Responses API types under `rs` namespace
pub use async_openai::types::responses as rs;
