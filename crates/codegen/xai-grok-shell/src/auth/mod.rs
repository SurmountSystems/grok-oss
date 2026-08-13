pub(crate) mod api_key_probe;
pub mod allowance_exhaust_from_billing;
pub(crate) mod attribution;
mod auth_provider;
mod config;
pub mod credential_provider;
pub mod credentials_store;
#[path = "devbox_login_stub.rs"]
pub(crate) mod devbox_login;
pub(crate) mod device_code;
pub mod dual_auth_status;
pub mod error;
mod external_auth;
mod flow;
pub mod free_period_debit_unproven_guard;
pub mod harness_secrets;
pub mod included_poll_history;
mod jwt;
pub(crate) mod manager;
mod model;
pub mod oidc;
pub mod openrouter;
pub(crate) mod recovery;
pub(crate) mod refresh;
pub mod secret_entry;
pub mod secret_store_progress;
pub(crate) mod single_flight;
mod storage;
pub mod supergrok_identity_rank;
mod token_output;
pub(crate) mod token_type;
pub(crate) use api_key_probe::{
    DEFAULT_PROBE_TIMEOUT, first_party_env_key_allows_advertise, should_probe_first_party_env_key,
};
pub mod xai_console;
pub mod xai_management;
pub use auth_provider::{AuthProviderConfig, AuthProviderRef};
pub(crate) use auth_provider::{
    PROVIDER_TIMEOUT_CEILING_SECS, PROVIDER_TOKEN_EXPIRY_SKEW_SECS, ProviderRefreshOutcome,
};
#[cfg(test)]
pub(crate) use auth_provider::{test_backdate_provider_mint, test_counting_provider};
pub(crate) use config::LEGACY_AUTH_SCOPE;
pub use config::{
    ForceLoginTeam, GrokComConfig, OAuth2ProviderConfig, OidcAuthConfig, PreferredAuthMethod,
    XAI_OAUTH2_ISSUER, default_allow_spend_when_free_period_debit_unproven,
    default_auto_use_included_limits, is_xai_oauth2_issuer, xai_oauth2_issuer,
};
// free_period_debit_unproven_guard re-exports are below (after module decl).
pub(crate) use external_auth::{parse_output, refresh_with_command};
pub(crate) use flow::{
    AuthChannels, mint_session_noninteractive, run_auth_flow, run_auth_flow_with_stderr_bridge,
    try_noninteractive_auth_no_mint,
};
pub use flow::{
    AuthUrlInfo, AuthUrlMode, LoginTransportOverride, LogoutResult, ensure_authenticated,
    ensure_authenticated_or_noninteractive, ensure_authenticated_with_override, perform_logout,
    run_cli_login, run_cli_logout, try_ensure_fresh_auth,
};
pub use jwt::{is_jwt_expired_or_near, parse_jwt_expiration};
mod meta;
pub use allowance_exhaust_from_billing::{
    SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD, SupergrokBillingPollOutcome,
    SupergrokBillingPollOutcomeKind, SupergrokBillingPollTarget, active_supergrok_identity_id,
    afterburner_skips_allowance_mark, apply_billing_usage_to_session_exhaust,
    apply_billing_usage_to_session_exhaust_with_period, classify_supergrok_billing_poll_error,
    clear_included_billing_cache, consecutive_auth_fail_streak,
    demote_included_billing_on_auth_fail, ensure_fresh_access_token_for_supergrok_billing_poll,
    find_supergrok_auth_entry_for_billing, format_supergrok_billing_fail_note,
    included_billing_fields_snapshot, load_all_session_access_tokens,
    load_non_active_supergrok_billing_poll_targets, load_session_access_token,
    load_supergrok_billing_poll_targets, load_supergrok_session_candidates,
    persist_refreshed_supergrok_billing_auth, remember_active_supergrok_included_billing,
    remember_supergrok_billing_poll_failed, remember_supergrok_billing_poll_ok,
    remember_supergrok_build_usage, remember_supergrok_dollar_extras,
    remember_supergrok_included_billing, session_needs_oidc_refresh_before_billing_poll,
    should_skip_supergrok_billing_poll_for_auth_streak, supergrok_billing_poll_outcome,
    supergrok_billing_poll_outcomes_snapshot, supergrok_identity_last_poll_auth_failed,
    supergrok_identity_last_poll_ok, supergrok_out_of_allowance_with_console_ready,
};
pub use dual_auth_status::{
    DualAuthStatus, collect_dual_auth_status, collect_dual_auth_status_with,
};
pub use error::{AuthError, RefreshTokenError, RefreshTokenFailedReason};
pub use free_period_debit_unproven_guard::{
    ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN_ENV, FreePeriodHeadroomEvidence,
    FreePeriodUnprovenSpendGuard, allow_spend_when_free_period_debit_unproven_from_config,
    evaluate_free_period_unproven_spend_guard, free_period_headroom_evidence_live,
    free_period_headroom_from_usage_readings, free_period_unproven_spend_block_message,
    should_block_spend_when_free_period_debit_unproven,
};
pub use harness_secrets::{
    DISABLE_SHARED_HARNESS_ENV, GROK_ZED_CONFIG_DIR_ENV, SharedKeySource,
    probe_shared_openrouter_key, probe_shared_openrouter_key_default,
};
pub use included_poll_history::{
    DEFAULT_MIN_POLLS, DEFAULT_MIN_WINDOW, DURABLE_SUBDIR, FlatPollEvidence,
    IncludedPollHistoryStore, IncludedPollSample, clear_included_poll_history,
    clear_process_included_poll_history_only, flat_poll_evidence_for_samples,
    flat_poll_evidence_from_history, flat_poll_evidence_from_history_with,
    flat_poll_unproven_debit_from_history, flat_poll_unproven_debit_from_history_with,
    included_debit_unproven, included_poll_history_for, record_included_poll_now,
    record_included_poll_sample,
};
pub use manager::{AuthManager, shared_api_key_provider};
pub(crate) use manager::{AuthRemedy, SilentRefresh};
pub use meta::{AuthMeta, GateInfo};
pub use model::{
    AuthMode, GrokAuth, SupergrokPrincipalListing, fingerprint_session_token,
    list_supergrok_principal_listings, lookup_auth, multi_slot_scope_for_auth,
    supergrok_identity_id_from_auth, upsert_supergrok_session,
};
pub(crate) use model::{TOKEN_TTL, UserInfo, default_coding_data_retention_opt_out, is_expired};
pub(crate) use refresh::DiagnosticUploader;
pub use harness_secrets::{
    DISABLE_SHARED_HARNESS_ENV, GROK_ZED_CONFIG_DIR_ENV, SharedKeySource,
    probe_shared_openrouter_key, probe_shared_openrouter_key_default,
};
pub use openrouter::{
    OPENROUTER_API_KEY_ENV, OPENROUTER_API_KEYS_ENV, OPENROUTER_API_URL,
    OPENROUTER_GROK_45_CATALOG_ID, OpenRouterAuthError, OpenRouterCreditsData,
    OpenRouterCreditsResponse, clear_openrouter_api_key, fetch_openrouter_credit_balance_cents,
    fetch_openrouter_credit_balance_cents_with_key, has_openrouter_api_key,
    is_openrouter_catalog_id, load_openrouter_api_key, load_openrouter_api_key_default,
    openrouter_balance_usd_from_credits, run_openrouter_login, run_openrouter_logout,
    store_openrouter_api_key, usd_to_cents,
};
pub use secret_entry::{
    API_KEY_STDIN_SENTINEL, CliApiKeyError, is_argv_api_key_secret, materialize_cli_api_key,
    materialize_cli_api_key_with, prompt_api_key_no_echo, read_api_key_from_stdin,
};
pub use storage::{clear_api_key, read_api_key, read_auth_json, store_api_key};
pub use supergrok_identity_rank::{
    AutoCredentialOrder, AutoSupergrokOrder, IncludedBillingFields, PickSupergrokForAuto,
    SupergrokAccountRole, SupergrokIdentityHeadroom, SupergrokPrincipalSlot,
    SupergrokPrincipalSlotInput, SupergrokSessionCandidate, apply_included_billing_to_headroom,
    enrich_candidates_with_included_billing, has_positive_supergrok_dollar_extras,
    included_remaining_from_usage_pct, list_supergrok_principal_slots,
    order_after_supergrok_included_exhaust, order_credentials_for_preferred_auto,
    order_live_supergrok_for_auto, pick_supergrok_identity_for_auto, preferred_is_console_primary,
    preferred_uses_supergrok_auto_rank, principal_limits_label, ranked_free_period_primary_token,
    reset_at_from_period_end, role_from_session_fields, role_label,
    session_bearer_should_align_to_ranked_free_period_primary,
};
pub use xai_console::{
    XAI_CONSOLE_API_URL, XaiConsoleAuthError, add_console_api_key, clear_console_api_key,
    console_inference_key_present, console_inference_key_present_default,
    credential_url as xai_console_credential_url, fingerprint_console_key,
    list_console_api_key_fingerprints, load_stored_console_api_key, load_stored_console_api_keys,
    run_list_console_api_keys, run_xai_console_login, store_console_api_key,
};
/// Outcome of applying SuperGrok included-usage % to the out-of-allowance memo.
pub use xai_grok_sampler::AllowanceExhaustAction;
pub use xai_management::{
    CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS, CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS,
    ConsoleTeamPostpaidPreview, ConsoleTeamPrepaidMeter, ConsoleTeamUsageSeries,
    ConsoleTeamUsageSeriesRow, MANAGEMENT_API_BASE_URL, MANAGEMENT_CREDENTIAL_URL,
    MANAGEMENT_KEY_VALIDATION_PATH, ManagementAuthError, ManagementKeyValidateFailure,
    ManagementKeyValidateOutcome, ManagementKeyValidation, POSTPAID_INVOICE_PREVIEW_PATH_TEMPLATE,
    PostpaidBillingCycle, PostpaidCoreInvoice, PostpaidInvoiceLine, PostpaidInvoicePreviewResponse,
    PostpaidLineClass, PrepaidBalanceResponse, USAGE_ANALYTICS_PATH_TEMPLATE,
    USAGE_SERIES_DEFAULT_DAY_WINDOW, UsageAnalyticsDataPoint, UsageAnalyticsRequestBody,
    UsageAnalyticsRequestInner, UsageAnalyticsResponse, UsageAnalyticsTimeRange,
    UsageAnalyticsTimeSeries, UsageAnalyticsValueSpec, UsdCentsVal, XAI_MANAGEMENT_API_KEY_ENV,
    XAI_MANAGEMENT_TEAM_ID_ENV, cached_console_team_postpaid, cached_console_team_postpaid_default,
    cached_console_team_prepaid, cached_console_team_prepaid_cents_default,
    cached_console_team_usage_series, cached_console_team_usage_series_default,
    cached_discovered_team_id, classify_postpaid_line, clear_console_team_billing_meter_caches,
    clear_console_team_postpaid_cache, clear_console_team_prepaid_cache,
    clear_console_team_usage_series_cache, clear_discovered_team_id_cache,
    clear_management_api_key, clear_management_billing_process_caches,
    console_team_postpaid_from_response, console_team_postpaid_setup_note,
    console_team_prepaid_from_response, console_team_prepaid_setup_note,
    console_team_usage_series_from_response, console_team_usage_series_setup_note,
    fetch_console_team_postpaid_preview, fetch_console_team_postpaid_preview_at,
    fetch_console_team_postpaid_preview_default, fetch_console_team_prepaid_balance,
    fetch_console_team_prepaid_balance_at, fetch_console_team_prepaid_balance_default,
    fetch_console_team_usage_series, fetch_console_team_usage_series_at,
    fetch_console_team_usage_series_default, fingerprint_management_key,
    format_management_key_validate_failure, has_management_api_key_env,
    load_stored_management_api_key, management_api_base, management_api_key_from_env,
    management_credential_url, management_team_id_from_env, postpaid_invoice_preview_path,
    prepaid_balance_path, prepaid_remaining_cents_from_total_val, resolve_management_api_key,
    resolve_management_api_key_default, resolve_management_team_id,
    resolve_management_team_id_default, resolve_management_team_id_with_discovery,
    run_management_key_login, store_management_api_key,
    usage_analytics_day_sum_by_description_request, usage_analytics_path, validate_management_key,
    validate_management_key_at, validate_management_key_outcome,
    validate_management_key_outcome_at,
};
