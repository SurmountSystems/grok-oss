# Report: API retry graceful on xAI outage (HTTP 521)

**Date:** 2026-08-04
**Board:** `bug:retry-xai-outage-521`
**Repo:** `/home/hunter/Projects/surmount/grok-build`

## Operator evidence

- Dogfood failure after ~3m15s:
  `Retry failed: API error (status 521 <unknown status code>): Connection to Grok timed out or was interrupted. Please try again. (HTTP 521).`
  `Turn failed … Internal error: { "message": "API error (status 521 …", "http_status": 521 }`
- Operator: retry path does not handle xAI being down gracefully.
- Related: https://x.com/cryptoquick/status/2084546115396079959

## Root cause

1. **`SamplingError::is_retryable` only listed `520` among Cloudflare edge codes.**
   521 (Web Server Is Down), 522–527, and 530 were **non-retryable**, so the first 521 became `RetryDecision::Fatal` immediately (still labeled “Retry failed” for non-retryable errors). Meanwhile `status_user_message` already treated 520–524 as edge outages.

2. **Display used raw `StatusCode` formatting.**
   IANA does not define 521, so Display was `521 <unknown status code>`, which landed in RetryFailed chrome and ACP `Internal error` JSON.

3. **No plain-English exhaust path** for edge/gateway outages on the terminal surface (shell `handle_sampling_failure` passed through full Display).

4. **Dual-auth (already correct path, guarded by tests):** hop/memo only runs for `is_credit_exhausted` and plain 429. 521 never matched those; risk was only false “unknown / fatal” UX, not economics hop. Confirmed with unit tests.

## Inventory (classification)

| Kind | Retry? | Notes |
|------|--------|--------|
| 429 | Yes | `Retry-After` preferred; hop to failover when configured (not credit memo) |
| 500, 502, 503, 504 | Yes | Backoff; first retry rebuilds HTTP/1.1 client |
| **520–527, 530** | **Yes (fixed)** | Cloudflare edge / origin outages; same backoff path |
| 501 | No | Not implemented |
| Transport timeout / connect / stream | Yes | Existing |
| IdleTimeout | No | Model stuck |
| Credit / allowance bodies | Hop | `is_credit_exhausted` only; **not** 521 |

Shared helpers (sampling-types):

- `is_transient_api_status`
- `is_edge_outage_status`
- `http_status_label` / `format_http_status` (no `<unknown status code>`)
- `outage_exhausted_user_message` → `xAI connection failed after N try/tries (HTTP 521). Try again shortly.`
- `status_user_message(521)` → origin-down copy

Retry budget: still default-unlimited (`GROK_MAX_RETRIES` / model cap); backoff 2s→… capped 60s with jitter; honors `Retry-After` when present. User cancel stops the loop (no infinite silent spin without feedback: footer shows `xAI unavailable (HTTP 521) · next try in Ns`).

## Product changes

| Area | Change |
|------|--------|
| `xai-grok-sampling-types` | Transient map, status labels, Display via `format_http_status`, 521 status copy |
| `xai-grok-sampler` `retry` | Docs; `format_sampling_error` plain English when retry_count set for edge/502–504; classify tests |
| `xai-grok-sampler` actor | Footer reason for edge/gateway: `xAI unavailable (HTTP N)` |
| `xai-grok-shell` sampler_turn | Terminal RetryFailed / ACP data rewrite for 502–504 and edge outages |

## Tests (red→green)

**`xai-grok-sampling-types` (lib):**

- `cloudflare_edge_outage_statuses_are_retryable`
- `http_521_display_uses_known_label_not_unknown_status`
- `outage_exhausted_message_is_plain_english`
- `format_http_status_keeps_iana_reason_for_standard_codes`
- `non_transient_5xx_like_501_not_retryable`
- Full lib: **287 passed**

**`xai-grok-sampler` (lib + integration):**

- `classify_521_is_retryable_with_client_rebuild`
- `classify_521_honors_retry_after_when_present`
- `classify_521_exhausted_budget_is_fatal`
- `format_521_exhausted_is_plain_english`
- `format_521_without_retry_count_keeps_status_hint`
- `cloudflare_edge_range_is_transient`
- `http_521_is_not_credit_or_rate_limit_hop` (dual-auth: no credit/429 hop class)
- `cf_edge_error_message` including `stream_521_html_is_retryable_and_labeled`
- Full lib: **214+ passed**; cf_edge: **7 passed**

**`xai-grok-shell`:** `sampling::error` suite green after recompile (26 filtered).

## Commands run

```bash
cargo fmt -p xai-grok-sampling-types -p xai-grok-sampler -p xai-grok-shell
cargo test -p xai-grok-sampling-types --lib
cargo test -p xai-grok-sampler --lib
cargo test -p xai-grok-sampler --test cf_edge_error_message
cargo test -p xai-grok-shell --lib -- sampling::error
```

No `git add` / `git commit`. No full `just check`.

## Files touched

- `crates/codegen/xai-grok-sampling-types/src/error.rs`
- `crates/codegen/xai-grok-sampling-types/src/lib.rs`
- `crates/codegen/xai-grok-sampler/src/retry.rs`
- `crates/codegen/xai-grok-sampler/src/actor/request_task.rs`
- `crates/codegen/xai-grok-sampler/tests/cf_edge_error_message.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`

## Residual / follow-ups

- Shell terminal path uses `outage_exhausted_user_message(..., 1)` because attempt count is not on `SamplingErrorInfo` at failure; retry loop footer still shows real attempt N while soft-retrying. Optional: plumb attempt count into Failed events for exact N in the toast.
- Default retry budget remains unlimited for all transient errors (existing product). Cap via `GROK_MAX_RETRIES` if operators want hard stop after N outage tries.
- 525–527 SSL/railgun treated as transient edge (same as connect-class); if product wants SSL hard-fail, narrow `is_transient_api_status` later.
