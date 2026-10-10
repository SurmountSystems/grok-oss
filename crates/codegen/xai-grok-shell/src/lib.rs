#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
#![warn(unreachable_pub)]
#![deny(clippy::indexing_slicing)]
#[cfg(all(test, feature = "dhat-heap"))]
#[global_allocator]
static DHAT_ALLOC: dhat::Alloc = dhat::Alloc;
pub(crate) use xai_grok_telemetry::unified_log;
pub use xai_tracing_macros::{teprintln, timed, tprintln};
pub mod agent;
pub mod auth {
    pub use crate::agent::init::run_cli_logout;
    pub use crate::credential_factory::{
        build_bootstrap_otel_credentials, build_storage_client_for_proxy,
    };
    pub use xai_grok_login::config::{
        default_allow_spend_when_free_period_debit_unproven, default_auto_use_included_limits,
    };
    pub use xai_grok_login::harness_secrets::{
        DISABLE_SHARED_HARNESS_ENV, GROK_ZED_CONFIG_DIR_ENV,
    };
    pub use xai_grok_login::model::{
        SupergrokPrincipalListing, fingerprint_session_token, is_supergrok_session_mode,
        list_supergrok_principal_listings, multi_slot_scope_for_auth,
        supergrok_identity_id_from_auth, upsert_supergrok_session,
    };
    pub use xai_grok_login::openrouter::OPENROUTER_API_KEY_ENV;
    pub use xai_grok_login::xai_console::{
        XAI_CONSOLE_API_URL, XaiConsoleAuthError, add_console_api_key,
        console_inference_key_present, console_inference_key_present_default,
        fingerprint_console_key, list_console_api_key_fingerprints, load_stored_console_api_key,
        load_stored_console_api_keys, store_console_api_key,
    };
    pub use xai_grok_login::*;

    #[path = "allowance_exhaust_from_billing.rs"]
    pub mod allowance_exhaust_from_billing;
    #[path = "dual_auth_status.rs"]
    pub mod dual_auth_status;
    #[path = "free_period_debit_unproven_guard.rs"]
    pub mod free_period_debit_unproven_guard;
    #[path = "included_poll_history.rs"]
    pub mod included_poll_history;
    #[path = "limits_pins.rs"]
    pub mod limits_pins;
    #[path = "limits_snapshot_hub.rs"]
    pub mod limits_snapshot_hub;
    #[path = "supergrok_identity_rank.rs"]
    pub mod supergrok_identity_rank;
    #[path = "xai_management.rs"]
    pub mod xai_management;

    pub use allowance_exhaust_from_billing::{
        SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD, SupergrokBillingPollOutcome,
        SupergrokBillingPollOutcomeKind, SupergrokBillingPollTarget, active_supergrok_identity_id,
        afterburner_skips_allowance_mark, afterburner_skips_allowance_mark_with_sibling,
        any_sibling_has_included_remaining, apply_billing_usage_to_session_exhaust,
        apply_billing_usage_to_session_exhaust_with_period, classify_supergrok_billing_poll_error,
        clear_included_billing_cache, consecutive_auth_fail_streak,
        demote_included_billing_on_auth_fail, ensure_fresh_access_token_for_supergrok_billing_poll,
        find_supergrok_auth_entry_for_billing, format_supergrok_billing_fail_note,
        included_billing_fields_snapshot, load_all_session_access_tokens,
        load_non_active_supergrok_billing_poll_targets, load_session_access_token,
        load_supergrok_billing_poll_targets, load_supergrok_session_candidates,
        persist_refreshed_supergrok_billing_auth, remember_active_supergrok_included_billing,
        remember_supergrok_billing_poll_failed, remember_supergrok_billing_poll_ok,
        remember_supergrok_build_usage, remember_supergrok_dollar_credits,
        remember_supergrok_included_billing, session_needs_oidc_refresh_before_billing_poll,
        should_skip_supergrok_billing_poll_for_auth_streak, supergrok_billing_poll_outcome,
        supergrok_billing_poll_outcomes_snapshot, supergrok_identity_last_poll_auth_failed,
        supergrok_identity_last_poll_ok, supergrok_out_of_allowance_with_console_ready,
    };
    pub use dual_auth_status::{
        DualAuthStatus, NOTE_SINGLE_SUPERGROK_SESSION_CANNOT_SEE_TEAM_PLAN,
        collect_dual_auth_status, collect_dual_auth_status_with,
    };
    pub use free_period_debit_unproven_guard::{
        ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN_ENV, FreePeriodHeadroomEvidence,
        FreePeriodUnprovenSpendGuard, allow_spend_when_free_period_debit_unproven_from_config,
        evaluate_free_period_unproven_spend_guard, free_period_headroom_evidence_live,
        free_period_headroom_from_usage_readings, free_period_unproven_spend_block_message,
        should_block_spend_when_free_period_debit_unproven,
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
    pub use limits_snapshot_hub::{
        LimitsSnapshotDocument, LimitsSnapshotIdentity, LimitsSnapshotManagement,
        LimitsSnapshotMode, LimitsSnapshotRole, POLL_OUTCOME_AUTH, POLL_OUTCOME_NETWORK,
        POLL_OUTCOME_NEVER, POLL_OUTCOME_OK, POLL_OUTCOME_OTHER, SNAPSHOT_FILE_NAME,
        SNAPSHOT_LOCK_FILE_NAME, SNAPSHOT_TTL, SNAPSHOT_TTL_SECS, apply_limits_snapshot,
        coordinate_limits_snapshot, fetch_management_into_snapshot, read_limits_snapshot_file,
        select_team_prepaid_meter, shared_limits_snapshot_disabled,
        should_also_call_management_prepaid_balance, snapshot_is_stale,
        snapshot_json_contains_secrets, write_limits_snapshot_file,
    };
    pub use supergrok_identity_rank::{
        AutoCredentialOrder, AutoSupergrokOrder, CombinedIncludedRemaining, IncludedBillingFields,
        IncludedPoolReading, PickSupergrokForAuto, SupergrokAccountRole, SupergrokIdentityHeadroom,
        SupergrokIdentityPin, SupergrokPrincipalSlot, SupergrokPrincipalSlotInput,
        SupergrokSessionCandidate, apply_included_billing_to_headroom,
        chrome_included_usage_from_combined, combined_included_remaining,
        enrich_candidates_with_included_billing, has_positive_supergrok_dollar_credits,
        included_remaining_from_usage_pct, list_supergrok_principal_slots,
        order_after_supergrok_included_exhaust, order_credentials_for_preferred_auto,
        order_credentials_for_preferred_auto_with_pin, order_live_supergrok_for_auto,
        pick_supergrok_identity_for_auto, pick_supergrok_identity_for_auto_with_pin,
        preferred_is_console_primary, preferred_uses_supergrok_auto_rank, principal_limits_label,
        ranked_free_period_primary_token, reset_at_from_period_end, role_from_session_fields,
        role_label, session_bearer_should_align_to_ranked_free_period_primary,
    };
    pub use xai_management::{
        CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS, CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS,
        ConsoleTeamPostpaidPreview, ConsoleTeamPrepaidMeter, ConsoleTeamUsageSeries,
        ConsoleTeamUsageSeriesRow, MANAGEMENT_API_BASE_URL, MANAGEMENT_CREDENTIAL_URL,
        MANAGEMENT_KEY_VALIDATION_PATH, ManagementAuthError, ManagementKeyValidateFailure,
        ManagementKeyValidateOutcome, ManagementKeyValidation,
        POSTPAID_INVOICE_PREVIEW_PATH_TEMPLATE, PostpaidBillingCycle, PostpaidCoreInvoice,
        PostpaidInvoiceLine, PostpaidInvoicePreviewResponse, PostpaidLineClass,
        PrepaidBalanceResponse, USAGE_ANALYTICS_PATH_TEMPLATE, USAGE_SERIES_DEFAULT_DAY_WINDOW,
        UsageAnalyticsDataPoint, UsageAnalyticsRequestBody, UsageAnalyticsRequestInner,
        UsageAnalyticsResponse, UsageAnalyticsTimeRange, UsageAnalyticsTimeSeries,
        UsageAnalyticsValueSpec, UsdCentsVal, XAI_MANAGEMENT_API_KEY_ENV,
        XAI_MANAGEMENT_TEAM_ID_ENV, billing_credits_card_from_prepaid_total,
        billing_credits_card_from_remaining_cents, billing_credits_remaining_cents_from_preview,
        cached_console_team_postpaid, cached_console_team_postpaid_default,
        cached_console_team_prepaid, cached_console_team_prepaid_cents_any,
        cached_console_team_prepaid_cents_default, cached_console_team_usage_series,
        cached_console_team_usage_series_default, cached_discovered_team_id,
        classify_postpaid_line, clear_console_team_billing_meter_caches,
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
        run_management_key_login, seed_console_team_postpaid_cache,
        seed_console_team_prepaid_cache, seed_console_team_usage_series_cache,
        store_management_api_key, usage_analytics_day_sum_by_description_request,
        usage_analytics_path, validate_management_key, validate_management_key_at,
        validate_management_key_outcome, validate_management_key_outcome_at,
    };
}
pub mod builtin;
pub mod shared_http_rate_limit;
pub use xai_grok_bundle as bundle;
pub mod claude_import;
pub mod claude_import_state;
pub mod cli_models;
pub mod config;
#[cfg(all(test, feature = "config-docs"))]
pub mod config_docs;
pub mod credential_factory;
pub use xai_grok_shell_base::cpu_profile;
pub use xai_grok_shell_base::env;
pub mod extensions;
pub use xai_grok_foreign_sessions as foreign_sessions;
pub mod heap_profile;
pub use xai_grok_http as http;
pub mod inspect;
pub mod instrumentation;
pub use xai_grok_telemetry::instrumentation_timer;
pub mod leader;
pub use xai_grok_cloud_config::managed_config;
pub mod mcp_doctor;
pub use xai_grok_models as models;
/// Uniquely grok-oss durable store (`$GROK_HOME/grok_oss.db`).
pub mod grok_oss;
pub mod plugin;
pub mod relay;
pub mod remote;
pub mod sampling;
pub mod session;
pub use xai_grok_shell_terminal as terminal;
#[cfg(test)]
pub(crate) mod test_support;
pub mod tier;
/// Token Economy: implement effort policy, period pacing, double-entry ledger.
pub mod token_economy;
pub mod tools;
pub mod upload;
pub mod util;
#[doc(hidden)]
pub mod waterfall;
