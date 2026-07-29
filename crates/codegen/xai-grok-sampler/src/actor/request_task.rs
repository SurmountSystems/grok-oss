//! Per-request streaming task.
//!
//! Spawned by the actor's `Submit` handler. Owns the retry loop and
//! consumes a Layer 2 stream from the matching backend transform.
//! Cancellation is cooperative via `CancellationToken`.

use std::pin::pin;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use xai_grok_sampling_types::{
    ConversationRequest, ConversationResponse, EmptyResponseContext, SamplingError,
    error::Result as SamplingResult,
};

use crate::client::{ApiBackend, SamplingClient};
use crate::config::{RetryPolicy, SamplerConfig};
use crate::events::{SamplingErrorInfo, SamplingErrorKind, SamplingEvent};
use crate::metrics::InferenceLatencyStats;
use crate::retry::{
    self as retry_mod, RetryDecision, classify_error, clone_error, resolve_max_retries,
};
use crate::stream::responses::stream_responses_tracked;
use crate::stream::{stream_chat_completions, stream_messages};
use crate::types::RequestId;

use grok_rate_limit::{ProviderKey, RateLimitMeta, SharedRateLimitStore, fingerprint_secret};

fn provider_key_for_config(config: &SamplerConfig) -> ProviderKey {
    match config.api_key.as_deref() {
        Some(k) if !k.is_empty() => {
            ProviderKey::from_base_url_and_key_fingerprint(&config.base_url, &fingerprint_secret(k))
        }
        _ => ProviderKey::from_base_url(&config.base_url),
    }
}

/// Before each HTTP attempt: honor any shared cross-process cooldown.
async fn wait_before_attempt(config: &SamplerConfig) {
    let store = SharedRateLimitStore::process_default();
    store
        .wait_if_limited(&provider_key_for_config(config))
        .await;
}

/// After a failed attempt: on 429 publish shared cooldown; always wait shared
/// then apply local backoff for non-429 (429 is fully covered by shared wait).
///
/// Returns `false` if `cancel_token` fires during the wait (caller should
/// treat as cancellation and stop retrying).
async fn sleep_for_retry(
    config: &SamplerConfig,
    err: &SamplingError,
    local_backoff: Duration,
    cancel_token: &CancellationToken,
) -> bool {
    let store = SharedRateLimitStore::process_default();
    let key = provider_key_for_config(config);
    if err.is_rate_limited() {
        let wait = err
            .retry_after()
            .map(Duration::from_secs)
            .unwrap_or(local_backoff);
        let meta = RateLimitMeta {
            status: Some(429),
            reason: Some(err.to_string()),
        };
        if let Err(e) = store.observe(&key, wait, meta) {
            tracing::debug!(error = %e, "shared rate limit observe failed");
        }
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => return false,
            _ = store.wait_if_limited(&key) => {}
        }
    } else {
        // Peers may still have a host-level cooldown; then local exp backoff.
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => return false,
            _ = store.wait_if_limited(&key) => {}
        }
        if !local_backoff.is_zero() && !sleep_or_cancel(local_backoff, cancel_token).await {
            return false;
        }
    }
    true
}

/// Default per-chunk idle timeout when neither config nor caller
/// supplies one. Matches the shell's session-level default
/// (5 minutes -- long enough for cold-start reasoning, short enough
/// to detect dead streams before the user gives up).
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;

/// Result type for the `submit_and_collect` oneshot. Carries the rich
/// `SamplingError` so callers can inspect retryability, status code,
/// etc., without losing information through the
/// `SamplingErrorInfo` round trip.
pub(crate) type CompletionResult =
    Result<(ConversationResponse, InferenceLatencyStats), SamplingError>;

/// Outcome of a single attempt within the retry loop.
enum AttemptOutcome {
    /// Stream emitted [`SamplingEvent::Completed`] with a non-empty
    /// response.
    Completed {
        response: Box<ConversationResponse>,
        metrics: InferenceLatencyStats,
    },
    /// Stream emitted [`SamplingEvent::Completed`] but the response
    /// was empty (no text, no tool calls). The retry loop treats this
    /// as a transient failure (the model returned reasoning-only or
    /// the stream was truncated). Metrics from the empty attempt are
    /// discarded; a successful retry produces fresh ones.
    Empty { context: EmptyResponseContext },
    /// Stream emitted [`SamplingEvent::Failed`]. The captured raw
    /// error is what the retry loop classifies; if no rich error was
    /// captured (e.g. the failure was synthesised inside the L2
    /// transform), `error` was reconstructed from the
    /// [`SamplingErrorInfo`].
    Failed { error: SamplingError },
    /// `cancel_token` fired mid-attempt. The retry loop bails out
    /// without further attempts.
    Cancelled,
    /// Failed to construct the underlying raw stream (e.g., HTTP
    /// connect error before any chunks arrive).
    InitFailed { error: SamplingError },
}

/// Run a single sampling request to completion (or final failure).
///
/// Returns the request id so the actor can clean it up from
/// `active_requests` via [`tokio::task::JoinSet::join_next`].
pub(crate) async fn run_request_task(
    request_id: RequestId,
    request: ConversationRequest,
    config: SamplerConfig,
    retry_policy: RetryPolicy,
    event_tx: mpsc::UnboundedSender<SamplingEvent>,
    cancel_token: CancellationToken,
    completion_tx: Option<oneshot::Sender<CompletionResult>>,
) -> RequestId {
    let mut completion_tx = completion_tx;
    let idle_timeout = Duration::from_secs(
        config
            .idle_timeout_secs
            .unwrap_or(DEFAULT_IDLE_TIMEOUT_SECS),
    );
    let configured_max_retries = config.max_retries.or(Some(retry_policy.max_retries));
    let max_retries = if configured_max_retries == Some(0) {
        0
    } else {
        resolve_max_retries(configured_max_retries)
    };

    // Build the initial client. Configuration errors here are fatal
    // (no point retrying with the same broken config).
    let mut client = match SamplingClient::new(config.clone()) {
        Ok(c) => c,
        Err(err) => {
            emit_failed(&event_tx, &request_id, &err);
            send_completion(&mut completion_tx, Err(err));
            return request_id;
        }
    };

    let sampling_span = crate::sampling_log::request_span(
        &request_id,
        &config.model,
        &format!("{:?}", client.api_backend()),
        &config.base_url,
        &client.auth_info(),
    );
    if let Some(eff) = config.reasoning_effort {
        sampling_span.record("reasoning_effort", eff.as_str());
    }

    let mut request = request;
    let mut config = config;
    let mut retry_count: u32 = 0;
    // Doom-loop recovery keeps its own resample budget, independent of the
    // transport/empty budget above.
    let doom_policy = (max_retries > 0)
        .then_some(config.doom_loop_recovery)
        .flatten();
    let doom_max_retries = doom_policy.map_or(0, |p| p.max_retries);
    let mut doom_retry_count: u32 = 0;
    let output_observed = Arc::new(AtomicBool::new(false));

    // If a prior turn already memoized this primary as out of allowance,
    // switch to the next live credential before burning an HTTP attempt.
    // Silent: already-memoized skip must not look like per-turn
    // "Retrying · Switched SuperGrok…" chrome. Shell also prefers live identity
    // at reconstruct_full_config so primary is often console already. Mid-request
    // credit switches still emit Retrying via apply_retry_decision.
    if let Some(hop_reason) = try_skip_memoized_exhausted_primary(&mut config, &mut client) {
        tracing::info!(
            target: crate::sampling_log::TARGET,
            %hop_reason,
            "skipped memoized exhausted primary before first attempt (silent)"
        );
    }

    loop {
        if cancel_token.is_cancelled() {
            handle_cancellation(&event_tx, &request_id, &mut completion_tx);
            return request_id;
        }

        // Cross-process rate-limit coordination (Grok OSS): wait until peers say open.
        wait_before_attempt(&config).await;
        if cancel_token.is_cancelled() {
            handle_cancellation(&event_tx, &request_id, &mut completion_tx);
            return request_id;
        }

        // Once the resample budget is spent, the attempt runs with the abort
        // disarmed so it can complete and be accepted as-is.
        let doom_check = doom_policy.filter(|_| doom_retry_count < doom_max_retries);
        let outcome = run_one_attempt(
            &client,
            request.clone(),
            request_id.clone(),
            idle_timeout,
            &event_tx,
            &cancel_token,
            doom_check,
            Arc::clone(&output_observed),
        )
        .instrument(sampling_span.clone())
        .await;

        let effective_max_retries =
            if retry_policy.retry_only_before_output && output_observed.load(Ordering::Relaxed) {
                0
            } else {
                max_retries
            };

        match outcome {
            AttemptOutcome::Completed {
                response,
                mut metrics,
            } => {
                metrics.attempts = retry_count + doom_retry_count + 1;
                if let Some(policy) = doom_policy {
                    let confident = policy.confident_triggers(&response.doom_loop_signals);
                    if !confident.is_empty() {
                        tracing::warn!(
                            target: crate::sampling_log::TARGET,
                            triggers = ?confident,
                            attempt = doom_retry_count + 1,
                            outcome = "accepted_after_budget",
                            "doom-loop recovery: resample budget spent; accepting as-is"
                        );
                    }
                }
                // Surface token usage on the sampling span alongside effort.
                if let Some(usage) = response.usage.as_ref() {
                    sampling_span.record("output_tokens", usage.completion_tokens);
                    sampling_span.record("reasoning_tokens", usage.reasoning_tokens);
                }
                // Emit Completed only after the loop succeeds; the L2
                // stream's terminal event was suppressed by
                // `run_one_attempt`.
                // Console-key success clears allowance memo (top-up recovery).
                // SuperGrok session success must **not** clear: extras can
                // still 200 while included weekly is 100%, which would put
                // SuperGrok back as primary and re-burn paid extras next turn.
                // Session recovery: billing usage drop
                // (`sync_allowance_exhaust_from_usage`) or TTL.
                clear_exhausted_after_success(&config);
                let _ = event_tx.send(SamplingEvent::Completed {
                    request_id: request_id.clone(),
                    response: response.clone(),
                    metrics: metrics.clone(),
                });
                send_completion(&mut completion_tx, Ok((*response, metrics)));
                return request_id;
            }
            AttemptOutcome::Empty { context } => {
                tracing::warn!(
                    target: crate::sampling_log::TARGET,
                    empty_response = true,
                    empty_reason = context.reason.as_str(),
                    had_reasoning = context.had_reasoning,
                    content_len = context.content_len,
                    tool_call_count = context.tool_call_count,
                    completion_tokens = context.completion_tokens.unwrap_or(0),
                    reasoning_tokens = context.reasoning_tokens.unwrap_or(0),
                    finish_reason = context.finish_reason_str(),
                    first_choice_seen = context.first_choice_seen,
                    model = %context.model,
                    "empty response from model: {reason} (retrying)",
                    reason = context.reason,
                );
                let err = SamplingError::EmptyResponse { context };
                if !apply_retry_decision(
                    &err,
                    &mut retry_count,
                    effective_max_retries,
                    &retry_policy,
                    &event_tx,
                    &request_id,
                    &mut request,
                    &mut client,
                    &mut config,
                    &cancel_token,
                    &mut completion_tx,
                )
                .await
                {
                    return request_id;
                }
            }
            AttemptOutcome::Failed { error } => {
                // Doom-loop resamples run on their own budget and never
                // consult the transport classifier, so no classifier change
                // can silently debit the transport budget for a doom failure.
                if let SamplingError::DoomLoopDetected { .. } = &error {
                    if retry_policy.retry_only_before_output
                        && output_observed.load(Ordering::Relaxed)
                    {
                        emit_failed(&event_tx, &request_id, &error);
                        send_completion(&mut completion_tx, Err(clone_error(&error)));
                        return request_id;
                    }
                    let backoff = retry_mod::doom_loop_backoff(doom_retry_count + 1);
                    doom_retry_count += 1;
                    tracing::warn!(
                        target: crate::sampling_log::TARGET,
                        reason = %error,
                        attempt = doom_retry_count,
                        max_retries = doom_max_retries,
                        outcome = "resampled",
                        "doom-loop recovery: discarding the poisoned attempt and resampling"
                    );
                    emit_retrying(
                        &event_tx,
                        &request_id,
                        doom_retry_count,
                        doom_max_retries,
                        &error,
                        &config,
                    );
                    if sleep_or_cancel(backoff, &cancel_token).await {
                        continue;
                    }
                    handle_cancellation(&event_tx, &request_id, &mut completion_tx);
                    return request_id;
                }
                if !apply_retry_decision(
                    &error,
                    &mut retry_count,
                    effective_max_retries,
                    &retry_policy,
                    &event_tx,
                    &request_id,
                    &mut request,
                    &mut client,
                    &mut config,
                    &cancel_token,
                    &mut completion_tx,
                )
                .await
                {
                    return request_id;
                }
            }
            AttemptOutcome::Cancelled => {
                handle_cancellation(&event_tx, &request_id, &mut completion_tx);
                return request_id;
            }
            AttemptOutcome::InitFailed { error } => {
                if !apply_retry_decision(
                    &error,
                    &mut retry_count,
                    effective_max_retries,
                    &retry_policy,
                    &event_tx,
                    &request_id,
                    &mut request,
                    &mut client,
                    &mut config,
                    &cancel_token,
                    &mut completion_tx,
                )
                .await
                {
                    return request_id;
                }
            }
        }
    }
}

use crate::prefer_live_primary::{is_session_identity, rotate_identity_config};

/// Pop the next distinct failover key and rebuild the client to use it.
///
/// Returns `Some(hop_reason)` when a key was applied (status/toast copy; no
/// secrets). For [`HopCause::CreditExhausted`], the active key is memoized
/// process-locally (1h) and dropped from the failover list so later turns
/// skip it. For [`HopCause::RateLimited`], the credit memo is **not** used
/// (throttle is temporary; shared `grok-rate-limit` cooldown covers the left
/// identity).
///
/// Dual-auth (SuperGrok session ↔ console key): when
/// [`SamplerConfig::failover_base_url`] / [`SamplerConfig::session_base_url`]
/// are set, also switches `base_url` and cli-chat-proxy headers. Hop-to-key
/// clears the live bearer (stash for reverse hop); hop-to-session reinstalls
/// a stashed resolver when present, else **live re-binds**
/// [`SamplerConfig::session_bearer_resolver`] (no prior stash required).
fn try_rotate_to_failover_key(
    config: &mut SamplerConfig,
    client: &mut SamplingClient,
    cause: crate::exhausted_identity::HopCause,
) -> Option<String> {
    let hop_reason = rotate_identity_config(config, cause)?;
    match SamplingClient::new(config.clone()) {
        Ok(fresh) => {
            *client = fresh;
            Some(hop_reason)
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to rebuild sampling client after key failover"
            );
            None
        }
    }
}

/// After a successful sample, clear credit-exhausted memo for the **active**
/// identity — except SuperGrok session JWT under dual-auth.
///
/// Named contract: Extra Usage Credits can authorize SuperGrok session 200s
/// while included weekly/monthly is fully used. Clearing the memo on that 200
/// would put SuperGrok back as primary and burn more extras on the next turn.
/// Console API keys still clear on success (true top-up recovery). Session
/// recovery is [`crate::exhausted_identity::sync_allowance_exhaust_from_usage`]
/// when usage drops, or the 1h TTL.
fn clear_exhausted_after_success(config: &SamplerConfig) {
    let Some(key) = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    if is_session_identity(config, key) {
        // Keep memo: extras-paid SuperGrok 200s are not recovery.
        return;
    }
    crate::exhausted_identity::clear_exhausted(&fingerprint_secret(key));
}

/// If the configured primary credential is already memoized exhausted and a
/// live failover remains, switch immediately so a subsequent turn does not
/// re-hit a dead key. Returns switch reason when a preemptive rotate applied.
///
/// Also treats SuperGrok session side as exhausted when `session_identity_key`
/// is memoized (OIDC refresh may change live `api_key` fingerprint).
fn try_skip_memoized_exhausted_primary(
    config: &mut SamplerConfig,
    client: &mut SamplingClient,
) -> Option<String> {
    if !crate::prefer_live_primary::primary_is_memoized_credit_exhausted(config) {
        return None;
    }
    try_rotate_to_failover_key(
        config,
        client,
        crate::exhausted_identity::HopCause::CreditExhausted,
    )
}

/// Apply a [`RetryDecision`]. Returns `true` if the loop should
/// continue, `false` if the request is finished (either fatal or
/// emit-to-session). Performs the side-effects of the decision:
/// sleeping, rebuilding the client, stripping images, emitting the
/// `Retrying` event.
#[allow(clippy::too_many_arguments)]
async fn apply_retry_decision(
    err: &SamplingError,
    retry_count: &mut u32,
    max_retries: u32,
    retry_policy: &RetryPolicy,
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    request: &mut ConversationRequest,
    client: &mut SamplingClient,
    config: &mut SamplerConfig,
    cancel_token: &CancellationToken,
    completion_tx: &mut Option<oneshot::Sender<CompletionResult>>,
) -> bool {
    // Credit exhaustion is fatal for one account but not for the request if
    // another key with balance is configured. Rotate before classify so we
    // do not surface a billing failure while failover keys remain.
    // Credit-worded 429 is also is_rate_limited(); credit path runs first.
    if err.is_credit_exhausted()
        && let Some(hop_reason) = try_rotate_to_failover_key(
            config,
            client,
            crate::exhausted_identity::HopCause::CreditExhausted,
        )
    {
        *retry_count += 1;
        emit_retrying_with_reason(
            event_tx,
            request_id,
            *retry_count,
            max_retries,
            err,
            config,
            hop_reason,
        );
        return true;
    }

    // Plain HTTP 429: hop to the next configured identity first (when any),
    // instead of sleeping forever on the same key. Observe shared cooldown
    // for the identity we leave so peers wait; do not sticky-memo as credit-dead.
    if err.is_rate_limited() {
        let left_key = provider_key_for_config(config);
        let local_backoff = retry_mod::retry_backoff_with_jitter(*retry_count);
        if let Some(hop_reason) = try_rotate_to_failover_key(
            config,
            client,
            crate::exhausted_identity::HopCause::RateLimited,
        ) {
            // Observe the identity we left (config already points at next).
            let store = SharedRateLimitStore::process_default();
            let wait = err
                .retry_after()
                .map(Duration::from_secs)
                .unwrap_or(local_backoff);
            let meta = RateLimitMeta {
                status: Some(429),
                reason: Some(err.to_string()),
            };
            if let Err(e) = store.observe(&left_key, wait, meta) {
                tracing::debug!(error = %e, "shared rate limit observe on hop failed");
            }
            *retry_count += 1;
            emit_retrying_with_reason(
                event_tx,
                request_id,
                *retry_count,
                max_retries,
                err,
                config,
                hop_reason,
            );
            return true;
        }
    }

    let rate_limit_threshold = if retry_policy.rate_limit_retry_threshold == 0 {
        retry_mod::RATE_LIMIT_RETRY_THRESHOLD
    } else {
        retry_policy.rate_limit_retry_threshold
    };
    let decision = classify_error(err, *retry_count, max_retries, rate_limit_threshold);

    // Connection-reset / broken-pipe on body upload often means nginx
    // rejected an oversized payload before responding 413. Strip
    // images proactively before any retry of those errors so we don't
    // burn budget re-uploading the same large body.
    if err.is_likely_body_rejected() {
        let stripped = request.strip_images();
        if stripped > 0 {
            tracing::warn!(
                stripped,
                "stripped {stripped} image(s) before retry (likely nginx 413 via connection reset)"
            );
        }
    }

    match decision {
        RetryDecision::Retry { backoff } => {
            *retry_count += 1;
            emit_retrying(event_tx, request_id, *retry_count, max_retries, err, config);
            if sleep_for_retry(config, err, backoff, cancel_token).await {
                true
            } else {
                handle_cancellation(event_tx, request_id, completion_tx);
                false
            }
        }
        RetryDecision::RetryWithBackoff { backoff, .. } => {
            *retry_count += 1;
            emit_retrying(event_tx, request_id, *retry_count, max_retries, err, config);
            if sleep_for_retry(config, err, backoff, cancel_token).await {
                true
            } else {
                handle_cancellation(event_tx, request_id, completion_tx);
                false
            }
        }
        RetryDecision::RetryWithImageStrip => {
            let stripped = request.strip_images();
            if stripped == 0 {
                // Nothing left to strip; upgrade to fatal.
                emit_failed(event_tx, request_id, err);
                send_completion(completion_tx, Err(clone_error(err)));
                return false;
            }
            *retry_count += 1;
            emit_retrying(event_tx, request_id, *retry_count, max_retries, err, config);
            true
        }
        RetryDecision::RetryWithClientRebuild { backoff } => {
            *retry_count += 1;
            emit_retrying(event_tx, request_id, *retry_count, max_retries, err, config);
            if !sleep_for_retry(config, err, backoff, cancel_token).await {
                handle_cancellation(event_tx, request_id, completion_tx);
                return false;
            }

            // Rebuild client with HTTP/1.1 fallback to escape poisoned
            // HTTP/2 connection pools.
            let mut http1_config = config.clone();
            http1_config.force_http1 = true;
            match SamplingClient::new(http1_config) {
                Ok(fresh) => {
                    *client = fresh;
                    tracing::info!("rebuilt sampling client with HTTP/1.1 fallback for retry");
                }
                Err(rebuild_err) => {
                    tracing::warn!(
                        error = %rebuild_err,
                        "failed to rebuild HTTP/1.1 client for retry; reusing existing client"
                    );
                }
            }
            true
        }
        RetryDecision::EmitToSession(emitted_err) => {
            emit_failed(event_tx, request_id, &emitted_err);
            send_completion(completion_tx, Err(emitted_err));
            false
        }
        RetryDecision::Fatal(fatal_err) => {
            // Emit only on true budget exhaustion (hit the retry / rate-limit
            // cap), mirroring `classify_error`'s Fatal conditions — NOT on a
            // server `x-should-retry: false` or a non-retryable error, which
            // are also Fatal but are not "exhausted".
            let next_attempt = (*retry_count).saturating_add(1);
            let server_said_stop = matches!(err.should_retry_header(), Some(false));
            // Unlimited (u32::MAX) never exhausts by budget.
            let budget_exhausted = !server_said_stop
                && !retry_mod::is_unlimited_retries(max_retries)
                && if err.is_rate_limited() {
                    let cap = max_retries.min(rate_limit_threshold);
                    !retry_mod::is_unlimited_retries(cap) && next_attempt >= cap
                } else {
                    err.is_retryable() && next_attempt >= max_retries
                };
            if budget_exhausted {
                let exhausted_span = tracing::info_span!(
                    "http.retries_exhausted",
                    total_attempts = next_attempt as i64,
                    model = %config.model,
                    error = %err,
                    status_code = tracing::field::Empty,
                );
                let status_code = match err {
                    SamplingError::Api { status, .. } => Some(status.as_u16()),
                    SamplingError::Http(e) => e.status().map(|s| s.as_u16()),
                    _ => None,
                };
                if let Some(status) = status_code {
                    exhausted_span.record("status_code", status as i64);
                }
                exhausted_span.in_scope(|| {});
            }
            emit_failed(event_tx, request_id, &fatal_err);
            send_completion(completion_tx, Err(fatal_err));
            false
        }
    }
}

async fn sleep_or_cancel(duration: Duration, cancel_token: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => false,
        _ = tokio::time::sleep(duration) => true,
    }
}

/// Run a single attempt: build the raw stream, drive it through the
/// matching L2 transform, and forward all non-terminal events to
/// `event_tx`. Captures the rich `SamplingError` from the underlying
/// raw stream so the retry loop can classify it accurately.
///
/// `doom_check` is the doom-loop policy while the resample budget lasts;
/// `None` disarms the mid-stream abort and the terminal confidence check so
/// the attempt completes and its response can be accepted.
#[allow(clippy::too_many_arguments)]
async fn run_one_attempt(
    client: &SamplingClient,
    request: ConversationRequest,
    request_id: RequestId,
    idle_timeout: Duration,
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    cancel_token: &CancellationToken,
    doom_check: Option<xai_grok_sampling_types::DoomLoopRecoveryPolicy>,
    output_observed: Arc<AtomicBool>,
) -> AttemptOutcome {
    match client.api_backend() {
        ApiBackend::ChatCompletions => {
            let (raw, metadata) = match client.conversation_stream(request).await {
                Ok(pair) => pair,
                Err(e) => return AttemptOutcome::InitFailed { error: e },
            };
            let (teed, captured) = tee_errors(raw);
            let l2 = stream_chat_completions(teed, metadata, request_id.clone(), idle_timeout);
            drive_l2(
                l2,
                request_id,
                event_tx,
                cancel_token,
                captured,
                None,
                output_observed,
            )
            .await
        }
        ApiBackend::Responses => {
            let (raw, metadata, doom_loop) =
                match client.conversation_stream_responses(request).await {
                    Ok(parts) => parts,
                    Err(e) => return AttemptOutcome::InitFailed { error: e },
                };
            if doom_check.is_none()
                && let Some(collector) = &doom_loop
            {
                collector.disarm_abort();
            }
            let (teed, captured) = tee_errors(raw);
            let l2 = stream_responses_tracked(
                teed,
                metadata,
                request_id.clone(),
                idle_timeout,
                doom_loop,
                Arc::clone(&output_observed),
            );
            drive_l2(
                l2,
                request_id,
                event_tx,
                cancel_token,
                captured,
                doom_check,
                output_observed,
            )
            .await
        }
        ApiBackend::Messages => {
            let (raw, metadata) = match client.conversation_stream_messages(request).await {
                Ok(pair) => pair,
                Err(e) => return AttemptOutcome::InitFailed { error: e },
            };
            let (teed, captured) = tee_errors(raw);
            let l2 = stream_messages(teed, metadata, request_id.clone(), idle_timeout);
            drive_l2(
                l2,
                request_id,
                event_tx,
                cancel_token,
                captured,
                None,
                output_observed,
            )
            .await
        }
    }
}

/// Captured-error cell shared between the tee adapter and the
/// per-request task.
type ErrorCell = Arc<Mutex<Option<SamplingError>>>;

/// Wrap a raw chunk stream so its first error is captured into a
/// shared cell. The wrapped stream still yields the original
/// `Result<T, SamplingError>` items unchanged so the L2 transform sees
/// them and converts them to `SamplingErrorInfo` for events.
fn tee_errors<'a, T: Send + 'a>(
    raw: BoxStream<'a, SamplingResult<T>>,
) -> (BoxStream<'a, SamplingResult<T>>, ErrorCell) {
    let cell: ErrorCell = Arc::new(Mutex::new(None));
    let cell_clone = Arc::clone(&cell);
    let teed = raw
        .map(move |item| {
            if let Err(ref e) = item
                && let Ok(mut guard) = cell_clone.lock()
                && guard.is_none()
            {
                // Capture only the first error -- subsequent errors
                // on a torn-down stream are usually secondary effects
                // of the same disconnect.
                *guard = Some(clone_error(e));
            }
            item
        })
        .boxed();
    (teed, cell)
}

/// Drive an L2 event stream: forward non-terminal events to
/// `event_tx`, watch `cancel_token`, return `AttemptOutcome` based on
/// the terminal event (or cancellation). `doom_check`, when set, turns a
/// completed response carrying confident doom-loop signals into a
/// retryable failure (belt-and-braces behind the mid-stream abort).
#[allow(clippy::too_many_arguments)]
async fn drive_l2(
    l2: impl futures_util::Stream<Item = SamplingEvent>,
    request_id: RequestId,
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    cancel_token: &CancellationToken,
    captured: ErrorCell,
    doom_check: Option<xai_grok_sampling_types::DoomLoopRecoveryPolicy>,
    output_observed: Arc<AtomicBool>,
) -> AttemptOutcome {
    let mut l2 = pin!(l2);
    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                return AttemptOutcome::Cancelled;
            }
            next = l2.next() => match next {
                Some(SamplingEvent::Completed { response, metrics, .. }) => {
                    output_observed.store(true, Ordering::Relaxed);
                    // Doom outranks the truncation/empty classes: a confident
                    // loop poisons the attempt whatever else it looks like.
                    if let Some(policy) = doom_check {
                        let triggers = policy.confident_triggers(&response.doom_loop_signals);
                        if !triggers.is_empty() {
                            return AttemptOutcome::Failed {
                                error: SamplingError::DoomLoopDetected {
                                    triggers,
                                    aborted_at_chunk: None,
                                },
                            };
                        }
                    }
                    if response.stop_reason == Some(xai_grok_sampling_types::StopReason::Length) {
                        return AttemptOutcome::Failed {
                            error: SamplingError::MaxTokensTruncation,
                        };
                    }
                    // A content-filtered turn (Anthropic refusal, OpenAI
                    // content_filter stop reason) is legitimately content-less and
                    // deterministic — resampling it would retry-storm.
                    let content_filtered = response.stop_reason
                        == Some(xai_grok_sampling_types::StopReason::ContentFilter);
                    if !content_filtered && let Some(reason) = response.empty_reason() {
                        let context = build_empty_context(reason, &response);
                        return AttemptOutcome::Empty { context };
                    }
                    return AttemptOutcome::Completed { response, metrics };
                }
                Some(SamplingEvent::Failed { error: info, .. }) => {
                    let raw = captured
                        .lock()
                        .ok()
                        .and_then(|mut g| g.take());
                    let error = raw.unwrap_or_else(|| synthesize_from_info(&info));
                    return AttemptOutcome::Failed { error };
                }
                Some(other) => {
                    if matches!(
                        other,
                        SamplingEvent::FirstToken { .. }
                            | SamplingEvent::ChannelToken { .. }
                            | SamplingEvent::ToolCallDelta { .. }
                            | SamplingEvent::BackendToolCallStarted { .. }
                            | SamplingEvent::BackendToolCallCompleted { .. }
                    ) {
                        output_observed.store(true, Ordering::Relaxed);
                    }
                    let _ = event_tx.send(retag(other, &request_id));
                }
                None => {
                    // L2 streams always terminate with Completed or
                    // Failed; reaching None means the producer was
                    // dropped without termination -- treat as a
                    // synthetic transport error.
                    return AttemptOutcome::Failed {
                        error: SamplingError::EventStreamError(
                            "stream dropped without terminal event".to_string(),
                        ),
                    };
                }
            }
        }
    }
}

/// Re-tag a forwarded event with the canonical request_id. The L2
/// transform tags events with the id we passed in, so this is
/// usually a no-op; keeping the helper makes the data-flow explicit.
fn retag(event: SamplingEvent, _request_id: &RequestId) -> SamplingEvent {
    event
}

/// Reconstruct a [`SamplingError`] from a [`SamplingErrorInfo`] when
/// the L2 transform fired a synthesised Failed event (idle timeout,
/// `ResponseFailed`, server error event) and there is no captured raw
/// error in the cell.
fn synthesize_from_info(info: &SamplingErrorInfo) -> SamplingError {
    match info.kind {
        SamplingErrorKind::IdleTimeout => SamplingError::IdleTimeout {
            elapsed_secs: info
                .message
                .split_whitespace()
                .find_map(|tok| tok.strip_suffix('s').and_then(|n| n.parse::<u64>().ok()))
                .unwrap_or(0),
        },
        SamplingErrorKind::Auth => SamplingError::Auth(info.message.clone()),
        // Must stay Serialization: EventStreamError is retryable, and a
        // response-parse failure is deterministic on retry. `info.message`
        // is the variant's rendered Display, so rebuild via the constructor
        // that owns the prefix-stripping.
        SamplingErrorKind::Serialization => {
            SamplingError::serialization_from_rendered(&info.message)
        }
        SamplingErrorKind::Http => SamplingError::EventStreamError(info.message.clone()),
        SamplingErrorKind::Api | SamplingErrorKind::RateLimited => {
            let status = info
                .status_code
                .and_then(|c| reqwest::StatusCode::from_u16(c).ok())
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            SamplingError::Api {
                status,
                message: info.message.clone(),
                model_metadata: info.model_metadata.clone(),
                retry_after_secs: info.retry_after_secs,
                should_retry: None,
            }
        }
        SamplingErrorKind::EmptyResponse => {
            if let Some(ctx) = &info.empty_response_context {
                SamplingError::EmptyResponse {
                    context: ctx.clone(),
                }
            } else {
                SamplingError::EventStreamError(info.message.clone())
            }
        }
        SamplingErrorKind::MaxTokensTruncation => SamplingError::MaxTokensTruncation,
        SamplingErrorKind::DoomLoopDetected => SamplingError::DoomLoopDetected {
            triggers: info.doom_loop_triggers.clone().unwrap_or_default(),
            aborted_at_chunk: info.doom_loop_aborted_at_chunk,
        },
    }
}

/// Build an [`EmptyResponseContext`] from a completed-but-empty response.
fn build_empty_context(
    reason: xai_grok_sampling_types::EmptyReason,
    response: &ConversationResponse,
) -> EmptyResponseContext {
    let had_reasoning = response
        .reasoning_items()
        .any(|r| !r.summary.is_empty() || r.content.is_some() || r.encrypted_content.is_some());
    let (content_len, tool_call_count, model, first_choice_seen) = match response.assistant() {
        Some(a) => (
            a.content.len(),
            a.tool_calls.len(),
            a.model_id.clone().unwrap_or_default(),
            // If model_id is set, the L2 saw at least one choice.
            a.model_id.is_some(),
        ),
        None => (0, 0, String::new(), false),
    };

    let finish_reason = response.stop_reason.map(|sr| sr.as_str().to_owned());
    let (completion_tokens, reasoning_tokens, prompt_tokens) = response
        .usage
        .as_ref()
        .map(|u| {
            (
                Some(u.completion_tokens),
                Some(u.reasoning_tokens),
                Some(u.prompt_tokens),
            )
        })
        .unwrap_or((None, None, None));

    EmptyResponseContext {
        reason,
        had_reasoning,
        content_len,
        tool_call_count,
        finish_reason,
        completion_tokens,
        reasoning_tokens,
        prompt_tokens,
        model,
        first_choice_seen,
    }
}

fn emit_failed(
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    err: &SamplingError,
) {
    let info = SamplingErrorInfo::from(err);
    let _ = event_tx.send(SamplingEvent::Failed {
        request_id: request_id.clone(),
        error: info,
    });
}

fn emit_retrying(
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    attempt: u32,
    max_retries: u32,
    err: &SamplingError,
    config: &SamplerConfig,
) {
    let info = SamplingErrorInfo::from(err);
    let mut reason = err.to_string();
    if err.is_rate_limited() {
        let key = provider_key_for_config(config);
        let rem = SharedRateLimitStore::process_default().remaining(&key);
        if let Some(secs) = err.retry_after() {
            reason = format!("{reason} · wait {secs}s (shared across grok-oss processes)");
        } else if !rem.is_zero() {
            reason = format!("{reason} · shared wait {}s", rem.as_secs().max(1));
        } else {
            reason = format!("{reason} · coordinating with other grok-oss sessions");
        }
    }
    emit_retrying_reason(event_tx, request_id, attempt, max_retries, &info, reason);
}

/// Identity-failover hop: surface dual-auth status chrome (no raw keys).
fn emit_retrying_with_reason(
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    attempt: u32,
    max_retries: u32,
    err: &SamplingError,
    _config: &SamplerConfig,
    reason: String,
) {
    let info = SamplingErrorInfo::from(err);
    emit_retrying_reason(event_tx, request_id, attempt, max_retries, &info, reason);
}

fn emit_retrying_reason(
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    attempt: u32,
    max_retries: u32,
    info: &SamplingErrorInfo,
    reason: String,
) {
    let _ = event_tx.send(SamplingEvent::Retrying {
        request_id: request_id.clone(),
        attempt,
        max_retries,
        kind: info.kind,
        reason,
        doom_loop_triggers: info.doom_loop_triggers.clone(),
        doom_loop_aborted_at_chunk: info.doom_loop_aborted_at_chunk,
    });
}

fn handle_cancellation(
    event_tx: &mpsc::UnboundedSender<SamplingEvent>,
    request_id: &RequestId,
    completion_tx: &mut Option<oneshot::Sender<CompletionResult>>,
) {
    // No status code, no upstream API error -- this is a client-side
    // termination. Use kind=Api so consumers that switch on kind have
    // a sensible default; the message clearly identifies it.
    let info = SamplingErrorInfo {
        kind: SamplingErrorKind::Api,
        status_code: None,
        message: "request cancelled".to_string(),
        is_retryable: false,
        retry_after_secs: None,
        model_metadata: None,
        empty_response_context: None,
        doom_loop_triggers: None,
        doom_loop_aborted_at_chunk: None,
    };
    let _ = event_tx.send(SamplingEvent::Failed {
        request_id: request_id.clone(),
        error: info,
    });
    send_completion(
        completion_tx,
        Err(SamplingError::Auth("request cancelled".to_string())),
    );
}

fn send_completion(
    completion_tx: &mut Option<oneshot::Sender<CompletionResult>>,
    result: CompletionResult,
) {
    if let Some(tx) = completion_tx.take() {
        let _ = tx.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    #[test]
    fn synthesize_idle_timeout_extracts_elapsed_secs() {
        let info = SamplingErrorInfo {
            kind: SamplingErrorKind::IdleTimeout,
            status_code: None,
            message: "inference idle timeout after 240s with no chunks".to_string(),
            is_retryable: false,
            retry_after_secs: None,
            model_metadata: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
        };
        let err = synthesize_from_info(&info);
        match err {
            SamplingError::IdleTimeout { elapsed_secs } => assert_eq!(elapsed_secs, 240),
            other => panic!("expected IdleTimeout, got {other:?}"),
        }
    }

    #[test]
    fn synthesize_api_500_round_trips() {
        let info = SamplingErrorInfo {
            kind: SamplingErrorKind::Api,
            status_code: Some(500),
            message: "boom".to_string(),
            is_retryable: true,
            retry_after_secs: None,
            model_metadata: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
        };
        let err = synthesize_from_info(&info);
        match err {
            SamplingError::Api {
                status, message, ..
            } => {
                assert_eq!(status.as_u16(), 500);
                assert_eq!(message, "boom");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn synthesize_rate_limited_preserves_retry_after() {
        let info = SamplingErrorInfo {
            kind: SamplingErrorKind::RateLimited,
            status_code: Some(429),
            message: "slow down".to_string(),
            is_retryable: true,
            retry_after_secs: Some(7),
            model_metadata: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
        };
        let err = synthesize_from_info(&info);
        match err {
            SamplingError::Api {
                status,
                retry_after_secs,
                ..
            } => {
                assert_eq!(status.as_u16(), 429);
                assert_eq!(retry_after_secs, Some(7));
            }
            other => panic!("expected Api(429), got {other:?}"),
        }
    }

    #[test]
    fn synthesize_serialization_stays_serialization() {
        // Round-trip a REAL error's Display so a Display-template rewording
        // cannot silently reintroduce double-prefixing.
        let original = SamplingError::Serialization(
            serde_json::from_str::<i32>("missing field `delta`").unwrap_err(),
        );
        let info = SamplingErrorInfo::from(&original);
        let err = synthesize_from_info(&info);
        assert!(
            matches!(err, SamplingError::Serialization(_)),
            "expected Serialization, got {err:?}"
        );
        assert!(!err.is_retryable());
        assert_eq!(
            err.to_string(),
            info.message,
            "rebuilt Display must round-trip without double-prefixing"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn retry_sleep_returns_immediately_on_cancellation() {
        let cancel_token = CancellationToken::new();
        let sleeper = sleep_or_cancel(Duration::from_secs(120), &cancel_token);
        tokio::pin!(sleeper);

        cancel_token.cancel();
        assert!(!sleeper.await);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_decision_cancellation_emits_terminal_cancel() {
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (completion_tx, completion_rx) = oneshot::channel();
        let mut completion_tx = Some(completion_tx);
        let mut retry_count = 0;
        let mut request = ConversationRequest::default();
        let mut config = SamplerConfig {
            base_url: "http://localhost".into(),
            model: "test-model".into(),
            ..Default::default()
        };
        let mut client = SamplingClient::new(config.clone()).expect("test client");
        let error = SamplingError::EventStreamError("retry me".into());

        let should_continue = apply_retry_decision(
            &error,
            &mut retry_count,
            2,
            &RetryPolicy::default(),
            &event_tx,
            &RequestId::from("cancel-backoff"),
            &mut request,
            &mut client,
            &mut config,
            &cancel_token,
            &mut completion_tx,
        )
        .await;

        assert!(!should_continue);
        assert!(matches!(
            event_rx.recv().await,
            Some(SamplingEvent::Retrying { .. })
        ));
        assert!(matches!(
            event_rx.recv().await,
            Some(SamplingEvent::Failed { .. })
        ));
        assert!(completion_rx.await.expect("completion sent").is_err());
    }

    #[tokio::test]
    async fn tee_captures_first_error_only() {
        let items: Vec<SamplingResult<u32>> = vec![
            Ok(1),
            Err(SamplingError::EventStreamError("first".into())),
            Err(SamplingError::EventStreamError("second".into())),
        ];
        let raw = stream::iter(items).boxed();
        let (mut teed, cell) = tee_errors(raw);
        while teed.next().await.is_some() {}
        let captured = cell.lock().unwrap().take().expect("error captured");
        match captured {
            SamplingError::EventStreamError(msg) => assert_eq!(msg, "first"),
            other => panic!("expected EventStreamError, got {other:?}"),
        }
    }

    #[test]
    fn rotate_failover_key_pops_next_distinct_key() {
        crate::exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("key-a".into()),
                failover_api_keys: vec!["key-a".into(), "key-b".into(), "key-c".into()],
                failover_base_url: None,
                session_base_url: None,
                session_identity_key: None,
                base_url: "https://openrouter.ai/api/v1".into(),
                model: "x-ai/grok-4.5".into(),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_some()
            );
            assert_eq!(config.api_key.as_deref(), Some("key-b"));
            // Exhausted primary duplicate dropped; only key-c remains.
            assert_eq!(config.failover_api_keys, vec!["key-c".to_string()]);
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_some()
            );
            assert_eq!(config.api_key.as_deref(), Some("key-c"));
            assert!(config.failover_api_keys.is_empty());
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_none()
            );
        });
    }

    #[test]
    fn credit_exhausted_without_failover_does_not_rotate() {
        crate::exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("only".into()),
                failover_api_keys: vec![],
                failover_base_url: None,
                session_base_url: None,
                session_identity_key: None,
                base_url: "https://openrouter.ai/api/v1".into(),
                model: "x-ai/grok-4.5".into(),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_none()
            );
            assert_eq!(config.api_key.as_deref(), Some("only"));
        });
    }

    /// D2: session → console key hop clears live bearer so AuthManager cannot re-inject
    /// the exhausted SuperGrok JWT mid-request.
    #[test]
    fn rotate_session_to_console_key_clears_bearer_resolver() {
        use crate::config::{BearerResolver, SharedBearerResolver};
        use std::sync::Arc;

        crate::exhausted_identity::with_memo_lock(|| {
            #[derive(Debug)]
            struct StaticBearer(&'static str);
            impl BearerResolver for StaticBearer {
                fn current_bearer(&self) -> Option<String> {
                    Some(self.0.to_owned())
                }
            }

            let mut config = SamplerConfig {
                api_key: Some("session-jwt".into()),
                failover_api_keys: vec!["console-biz-key".into()],
                failover_base_url: None,
                session_base_url: None,
                session_identity_key: Some("session-jwt".into()),
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                bearer_resolver: Some(Arc::new(StaticBearer("session-jwt")) as SharedBearerResolver),
                stashed_bearer_resolver: None,
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let reason = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::CreditExhausted,
            )
            .expect("session→key hop");
            assert_eq!(config.api_key.as_deref(), Some("console-biz-key"));
            assert!(
                config.bearer_resolver.is_none(),
                "hop session→key must clear bearer_resolver"
            );
            assert!(config.failover_api_keys.is_empty());
            assert!(
                reason.contains("SuperGrok session") && reason.contains("console key"),
                "hop reason labels session→key: {reason}"
            );
            assert!(
                reason.contains("out of allowance"),
                "allowance hop: {reason}"
            );
            assert!(crate::exhausted_identity::is_credential_hop_reason(&reason));
        });
    }

    /// D2: console key → session JWT string hop (key-primary dual-auth ordering).
    #[test]
    fn rotate_console_key_to_session_jwt() {
        crate::exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("console-biz-key".into()),
                failover_api_keys: vec!["session-jwt".into()],
                failover_base_url: None,
                session_base_url: None,
                session_identity_key: Some("session-jwt".into()),
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let reason = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::CreditExhausted,
            )
            .expect("key→session hop");
            assert_eq!(config.api_key.as_deref(), Some("session-jwt"));
            assert!(config.failover_api_keys.is_empty());
            assert!(config.bearer_resolver.is_none());
            assert!(
                reason.contains("console key") && reason.contains("SuperGrok session"),
                "hop reason labels key→session: {reason}"
            );
        });
    }

    /// Dual-host: session on cli-chat-proxy → console key switches to api.x.ai and drops proxy headers.
    #[test]
    fn rotate_session_to_console_key_switches_host_and_headers() {
        use crate::config::{BearerResolver, SharedBearerResolver};
        use indexmap::IndexMap;
        use std::sync::Arc;

        crate::exhausted_identity::with_memo_lock(|| {
            #[derive(Debug)]
            struct StaticBearer(&'static str);
            impl BearerResolver for StaticBearer {
                fn current_bearer(&self) -> Option<String> {
                    Some(self.0.to_owned())
                }
            }

            let proxy = "https://cli-chat-proxy.example.x.ai/v1";
            let console = "https://api.x.ai/v1";
            let mut headers = IndexMap::new();
            headers.insert("X-XAI-Token-Auth".into(), "xai-grok-cli".into());
            headers.insert(
                "x-authenticateresponse".into(),
                "authenticate-response".into(),
            );
            headers.insert("x-grok-client-mode".into(), "interactive".into());
            headers.insert("X-Custom".into(), "keep-me".into());

            let mut config = SamplerConfig {
                api_key: Some("session-jwt".into()),
                failover_api_keys: vec!["console-biz-key".into()],
                failover_base_url: Some(console.into()),
                session_base_url: Some(proxy.into()),
                session_identity_key: Some("session-jwt".into()),
                base_url: proxy.into(),
                model: "grok-4".into(),
                extra_headers: headers,
                bearer_resolver: Some(Arc::new(StaticBearer("session-jwt")) as SharedBearerResolver),
                stashed_bearer_resolver: None,
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_some()
            );
            assert_eq!(config.api_key.as_deref(), Some("console-biz-key"));
            assert_eq!(config.base_url, console);
            assert!(config.bearer_resolver.is_none());
            assert!(config.stashed_bearer_resolver.is_some());
            assert!(!config.extra_headers.contains_key("X-XAI-Token-Auth"));
            assert_eq!(
                config.extra_headers.get("X-Custom").map(String::as_str),
                Some("keep-me"),
                "non-proxy headers must survive host switch"
            );
        });
    }

    /// Dual-host reverse: console → session restores proxy host, proxy headers, and stashed bearer.
    #[test]
    fn rotate_console_key_to_session_restores_host_headers_and_bearer() {
        use crate::config::{BearerResolver, SharedBearerResolver};
        use std::sync::Arc;

        crate::exhausted_identity::with_memo_lock(|| {
            #[derive(Debug)]
            struct StaticBearer(&'static str);
            impl BearerResolver for StaticBearer {
                fn current_bearer(&self) -> Option<String> {
                    Some(self.0.to_owned())
                }
            }

            let proxy = "https://cli-chat-proxy.example.x.ai/v1";
            let console = "https://api.x.ai/v1";
            let resolver: SharedBearerResolver = Arc::new(StaticBearer("session-jwt"));

            let mut config = SamplerConfig {
                api_key: Some("console-biz-key".into()),
                failover_api_keys: vec!["session-jwt".into()],
                failover_base_url: Some(console.into()),
                session_base_url: Some(proxy.into()),
                session_identity_key: Some("session-jwt".into()),
                base_url: console.into(),
                model: "grok-4".into(),
                stashed_bearer_resolver: Some(resolver),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            assert!(
                try_rotate_to_failover_key(
                    &mut config,
                    &mut client,
                    crate::exhausted_identity::HopCause::CreditExhausted,
                )
                .is_some()
            );
            assert_eq!(config.api_key.as_deref(), Some("session-jwt"));
            assert_eq!(config.base_url, proxy);
            assert!(config.bearer_resolver.is_some());
            assert!(config.stashed_bearer_resolver.is_none());
            assert_eq!(
                config
                    .extra_headers
                    .get("X-XAI-Token-Auth")
                    .map(String::as_str),
                Some("xai-grok-cli")
            );
        });
    }

    /// Live re-bind hop-to-session without prior stash (key-primary dual-auth).
    #[test]
    fn rotate_console_key_to_session_live_rebinds_without_prior_stash() {
        use crate::config::{BearerResolver, SharedBearerResolver};
        use std::sync::Arc;

        crate::exhausted_identity::with_memo_lock(|| {
            #[derive(Debug)]
            struct LiveSessionBearer;
            impl BearerResolver for LiveSessionBearer {
                fn current_bearer(&self) -> Option<String> {
                    Some("session-jwt-live".into())
                }
            }

            let live: SharedBearerResolver = Arc::new(LiveSessionBearer);
            let mut config = SamplerConfig {
                api_key: Some("console-biz-key".into()),
                failover_api_keys: vec!["session-jwt-live".into()],
                session_identity_key: Some("session-jwt-live".into()),
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                stashed_bearer_resolver: None,
                session_bearer_resolver: Some(live),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let reason = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::CreditExhausted,
            )
            .expect("key→session hop");
            assert_eq!(config.api_key.as_deref(), Some("session-jwt-live"));
            assert!(
                config.bearer_resolver.is_some(),
                "must live re-bind session_bearer_resolver without prior stash"
            );
            assert_eq!(
                config
                    .bearer_resolver
                    .as_ref()
                    .and_then(|r| r.current_bearer())
                    .as_deref(),
                Some("session-jwt-live")
            );
            assert!(
                config.stashed_bearer_resolver.is_none(),
                "stash remains empty when hop used durable live re-bind"
            );
            assert!(
                config.session_bearer_resolver.is_some(),
                "durable session resolver is not consumed"
            );
            assert!(
                reason.contains("console key") && reason.contains("SuperGrok session"),
                "hop reason: {reason}"
            );
        });
    }

    /// D3: after a hop, exhausted primary fingerprint is memoized so a later
    /// rotate skips re-selecting it (and preemptive skip hops without API fail).
    #[test]
    fn rotate_memos_exhausted_fingerprint_and_skips_on_next_turn() {
        crate::exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("dead-key".into()),
                failover_api_keys: vec!["live-key".into(), "also-live".into()],
                failover_base_url: None,
                session_base_url: None,
                session_identity_key: None,
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let reason = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::CreditExhausted,
            )
            .expect("hop");
            assert_eq!(config.api_key.as_deref(), Some("live-key"));
            assert!(crate::exhausted_identity::is_credential_hop_reason(&reason));
            assert!(reason.contains("out of allowance"), "{reason}");
            let dead_fp = fingerprint_secret("dead-key");
            assert!(
                crate::exhausted_identity::is_exhausted(&dead_fp),
                "exhausted primary must be memoized"
            );

            // Simulate next turn: resolve rebuilds list with dead primary first.
            config.api_key = Some("dead-key".into());
            config.failover_api_keys = vec!["live-key".into(), "also-live".into()];
            let hop = try_skip_memoized_exhausted_primary(&mut config, &mut client)
                .expect("preemptive skip of memoized dead key");
            assert_eq!(config.api_key.as_deref(), Some("live-key"));
            assert!(crate::exhausted_identity::is_credential_hop_reason(&hop));

            // Memoized dead key must not be re-selected from failover either.
            config.api_key = Some("live-key".into());
            config.failover_api_keys = vec!["dead-key".into(), "also-live".into()];
            // Mark live-key exhausted and hop — must skip dead-key in list.
            crate::exhausted_identity::mark_exhausted(&fingerprint_secret("live-key"));
            let hop2 = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::CreditExhausted,
            )
            .expect("skip dead");
            assert_eq!(
                config.api_key.as_deref(),
                Some("also-live"),
                "memoized dead-key must be skipped in failover list"
            );
            assert!(crate::exhausted_identity::is_credential_hop_reason(&hop2));
        });
    }

    /// Billing usage 100% + dual-auth: mark SuperGrok out of allowance → switch
    /// to console key before the next request (no HTTP 402 required).
    #[test]
    fn billing_allowance_exhaust_skips_session_before_request() {
        use grok_rate_limit::fingerprint_secret;

        crate::exhausted_identity::with_memo_lock(|| {
            let session = "supergrok-session-jwt";
            let console = "console-biz-key";
            assert_eq!(
                crate::exhausted_identity::sync_allowance_exhaust_from_usage(
                    100.0,
                    Some(session),
                    true,
                ),
                crate::exhausted_identity::AllowanceExhaustAction::Marked
            );
            assert!(crate::exhausted_identity::is_exhausted(
                &fingerprint_secret(session)
            ));

            let mut config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![console.into()],
                base_url: "https://cli-proxy.x.ai/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-proxy.x.ai/v1".into()),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let hop = try_skip_memoized_exhausted_primary(&mut config, &mut client)
                .expect("must leave SuperGrok without a prior 402");
            assert_eq!(config.api_key.as_deref(), Some(console));
            assert!(crate::exhausted_identity::is_credential_hop_reason(&hop));
            assert!(
                hop.contains("out of allowance"),
                "billing-driven switch uses allowance cause: {hop}"
            );
            assert!(
                hop.contains("console key"),
                "prefer console key after SuperGrok weekly 100%: {hop}"
            );
        });
    }

    /// Named contract: after prefer-live already made console primary
    /// (shell reconstruct path), preemptive skip is a no-op — first attempt
    /// is console without another SuperGrok→console switch ceremony.
    #[test]
    fn memoized_exhaust_first_request_already_console_no_second_hop() {
        use grok_rate_limit::fingerprint_secret;

        crate::exhausted_identity::with_memo_lock(|| {
            let session = "seamless-session-jwt";
            let console = "seamless-console-key";
            crate::exhausted_identity::mark_exhausted(&fingerprint_secret(session));

            // Shell reconstruct_full_config prefers live identity first.
            let mut config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![console.into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            let preferred =
                crate::prefer_live_primary::prefer_live_identity_after_credit_exhaust(&mut config)
                    .expect("must flip to console before first request");
            assert_eq!(config.api_key.as_deref(), Some(console));
            assert!(
                config.base_url.contains("api.x.ai"),
                "console host before HTTP: {}",
                config.base_url
            );
            assert!(crate::exhausted_identity::is_credential_hop_reason(
                &preferred
            ));

            // Request task safety net: already on live console → no switch.
            let mut client = SamplingClient::new(config.clone()).expect("client");
            assert!(
                try_skip_memoized_exhausted_primary(&mut config, &mut client).is_none(),
                "second switch must not fire when primary is already console"
            );
            assert_eq!(config.api_key.as_deref(), Some(console));

            // Simulate next turn: resolve re-pins SuperGrok (prepare_sampler_for_turn).
            config.api_key = Some(session.into());
            config.failover_api_keys = vec![console.into()];
            config.base_url = "https://cli-chat-proxy.grok.com/v1".into();
            config.extra_headers.clear();
            // Prefer live again at reconstruct — seamless, no SuperGrok HTTP.
            let again =
                crate::prefer_live_primary::prefer_live_identity_after_credit_exhaust(&mut config)
                    .expect("each turn re-pin still leaves SuperGrok when out of allowance");
            assert_eq!(config.api_key.as_deref(), Some(console));
            assert!(again.contains("console key"), "{again}");
            assert!(
                try_skip_memoized_exhausted_primary(&mut config, &mut client).is_none(),
                "request path silent after prefer-live apply"
            );
        });
    }

    /// Named contract: SuperGrok session 200 while weekly is 100% is often
    /// **Extra Usage Credits**, not recovery. Clearing the allowance memo on
    /// that 200 re-enables session next turn and burns more extras.
    /// Console-key success still clears (true top-up path).
    #[test]
    fn session_success_does_not_clear_allowance_exhaust_memo() {
        use grok_rate_limit::fingerprint_secret;

        crate::exhausted_identity::with_memo_lock(|| {
            let session = "session-jwt-extras-still-pay";
            let console = "console-after-hop";
            assert_eq!(
                crate::exhausted_identity::sync_allowance_exhaust_from_usage(
                    100.0,
                    Some(session),
                    true,
                ),
                crate::exhausted_identity::AllowanceExhaustAction::Marked
            );
            let session_fp = fingerprint_secret(session);
            assert!(crate::exhausted_identity::is_exhausted(&session_fp));

            // Slip-through: sample still used the session (pre-switch missed or
            // mid-flight refresh) and got HTTP 200 paid by extras.
            let config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![console.into()],
                session_identity_key: Some(session.into()),
                ..Default::default()
            };
            clear_exhausted_after_success(&config);
            assert!(
                crate::exhausted_identity::is_exhausted(&session_fp),
                "extras-paid SuperGrok 200 must not erase allowance exhaust memo"
            );

            // Console success still clears its own mark (top-up recovery).
            let console_fp = fingerprint_secret(console);
            crate::exhausted_identity::mark_exhausted(&console_fp);
            let console_cfg = SamplerConfig {
                api_key: Some(console.into()),
                failover_api_keys: vec![session.into()],
                session_identity_key: Some(session.into()),
                ..Default::default()
            };
            clear_exhausted_after_success(&console_cfg);
            assert!(
                !crate::exhausted_identity::is_exhausted(&console_fp),
                "console-key success must still clear memo for recovery"
            );
            // Session memo untouched by console success clear.
            assert!(crate::exhausted_identity::is_exhausted(&session_fp));
        });
    }

    /// Rate-limit switch reuses rotate mechanics but does **not** memoize the
    /// left identity as credit-dead (return-to-primary when cool).
    #[test]
    fn rate_limit_rotate_does_not_memoize_credit_exhausted() {
        crate::exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("throttled-key".into()),
                failover_api_keys: vec!["backup-key".into()],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                ..Default::default()
            };
            let mut client = SamplingClient::new(config.clone()).expect("client");
            let reason = try_rotate_to_failover_key(
                &mut config,
                &mut client,
                crate::exhausted_identity::HopCause::RateLimited,
            )
            .expect("rate-limit hop");
            assert_eq!(config.api_key.as_deref(), Some("backup-key"));
            assert!(
                reason.contains("rate limited"),
                "distinct hop reason: {reason}"
            );
            assert!(
                !reason.contains("out of allowance"),
                "must not claim allowance: {reason}"
            );
            assert!(crate::exhausted_identity::is_credential_hop_reason(&reason));
            let left_fp = fingerprint_secret("throttled-key");
            assert!(
                !crate::exhausted_identity::is_exhausted(&left_fp),
                "rate-limit hop must not use 1h credit memo"
            );
            // Preemptive credit skip must not fire for a rate-limited-only hop.
            config.api_key = Some("throttled-key".into());
            config.failover_api_keys = vec!["backup-key".into()];
            assert!(
                try_skip_memoized_exhausted_primary(&mut config, &mut client).is_none(),
                "no credit memo → no preemptive skip"
            );
            assert_eq!(config.api_key.as_deref(), Some("throttled-key"));
        });
    }
}
