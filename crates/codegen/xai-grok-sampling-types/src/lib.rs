//! Pure data types for the xAI sampling / chat-completion API layer.
//!
//! API-agnostic conversation, chat-completion request/response, streaming, and error types used across the xAI agent stack.
//! It contains no I/O: no HTTP clients, no file system access.
//! Downstream crates like `xai-chat-state` can depend on it without pulling in the full `xai-grok-shell`.

#![deny(clippy::indexing_slicing)]

pub mod billing_credits_card;
pub mod conversation;
pub mod doom_loop;
pub mod error;
pub mod language_models;
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
    DEFAULT_EXACT_REPETITION_MIN_TOKENS, DOOM_LOOP_CHECK_EVENT_TYPE, DOOM_LOOP_CHECK_HEADER,
    DoomLoopPeek, DoomLoopRecoveryPolicy, DoomLoopSignal, DoomLoopSignalKind,
    EXACT_REPETITION_CHECK_HEADER, is_check_event, peek_doom_loop,
};
pub use self::error::{
    ApiErrorCode, EmptyReason, EmptyResponseContext, INVALID_IMAGE_ERROR_CODE,
    REPETITIVE_GENERATION_USER_MESSAGE, ResponseModelMetadata, Result, SamplingError,
    SentCredential, is_console_team_prepaid_message, is_context_length_error,
    is_credit_exhausted_compact_wrap, is_edge_outage_status, is_retryable_api_status,
    is_size_overflow_error_code, is_transient_api_status, outage_exhausted_user_message,
    parse_error_code, status_user_message, user_facing_api_error_message,
};
pub use self::language_models::{
    LanguageModelServing, LanguageModelsList, language_models_url_from_models_list_url,
    parse_language_models_json,
};
pub use self::tool_overrides::{
    ClearableField, MAX_WEB_SEARCH_DOMAINS, SearchDateBound, SearchDateBoundError, ToolOverrides,
    ToolOverridesUpdate, WebSearchOptions, WebSearchOptionsError, XSearchOptions,
};
pub use self::types::*;

pub use async_openai::types::responses as rs;
