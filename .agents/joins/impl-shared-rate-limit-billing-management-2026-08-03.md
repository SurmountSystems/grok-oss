# Join: SuperGrok billing + Management API through shared rate limits

**Date:** 2026-08-03
**Status:** done
**Recon:** `.agents/joins/recon-pooled-rate-limit-ipc-2026-08-03.md`

## What shipped

Every product HTTP call on SuperGrok billing and Management API paths that can hit rate limits now uses the existing flock JSON store (`grok-rate-limit` under `$GROK_HOME/rate_limits/`). No Unix-socket daemon. No touch of exhausted-credit memo. No durable poll-history design work (parallel track).

### New module

`crates/codegen/xai-grok-shell/src/shared_http_rate_limit.rs`

| Helper | Role |
|--------|------|
| `billing_provider_key(proxy_base, access_token)` | Host + FNV fingerprint of SuperGrok session (never raw token in filenames) |
| `management_provider_key(base_url, management_key)` | Host + FNV fingerprint of Management API key |
| `wait_before_http` | `SharedRateLimitStore::wait_if_limited` before send |
| `observe_http_rate_limit` | On 429 always; on 403 only when `Retry-After` / `x-ratelimit-reset` present (bare invalid-key 403 must not poison peers) |
| `wait_from_rate_limit_headers` | Prefer `Retry-After` seconds; else reset epoch; fallback 60s |
| Test override | Thread-local store so hermetic tests avoid `process_default` OnceLock |

Kill switch `GROK_DISABLE_SHARED_RATE_LIMIT` still honored (store no-ops).

### Call sites wired

| Path | File | Wait + observe |
|------|------|----------------|
| SuperGrok credits `GET …/billing?format=credits` | `extensions/billing.rs` `fetch_credits_config_with_session` | yes |
| SuperGrok auto-topup `GET …/auto-topup-rule` | `extensions/billing.rs` | yes |
| Management key validation | `auth/xai_management.rs` `validate_management_key_outcome_at` | yes |
| Prepaid balance | `fetch_console_team_prepaid_balance_at` | yes |
| Postpaid invoice preview | `fetch_console_team_postpaid_preview_at` | yes |
| Usage series POST | `fetch_console_team_usage_series_at` | yes |

### Crate / deps

- `xai-grok-shell` depends on `grok-rate-limit`
- `grok_rate_limit::keys::XAI_MANAGEMENT` documented well-known host string
- `DISABLE_ENV` re-exported from `grok-rate-limit` root for callers/tests

## Tests (green)

```bash
cargo test -p xai-grok-shell --lib shared_http_rate_limit
# 9 passed

cargo test -p xai-grok-shell --lib prepaid_429_observes
# prepaid_429_observes_shared_rate_limit_store ok

cargo test -p xai-grok-shell --lib prepaid_403_without
# prepaid_403_without_retry_after_does_not_observe ok

cargo test -p grok-rate-limit
# 9 passed
```

Named contracts:

1. Provider keys are host+fingerprint; raw secrets never appear in key strings / filenames.
2. Two store handles on the same temp dir share Management- and billing-shaped cooldowns.
3. `observe_http_rate_limit` writes remaining wait; disable env makes further ops no-op.
4. `wait_before_http` sleeps until shared cooldown expires.
5. Hermetic Management prepaid mock returning 429 + Retry-After publishes remaining that a peer handle sees.
6. Bare 403 without Retry-After does **not** observe.

## Out of scope (as requested)

- Durable included poll history (`included_poll_history.rs` owned by parallel implementer)
- Exhausted-credit memo
- New IPC transport beyond flock store

### Incidental compile fix

While building shell tests, `included_poll_history::DurablePollHistoryStore::history_for` returned `VecDeque` where the signature expected `Vec`. Applied a one-line `.into_iter().collect()` so the crate compiles. Parallel durable-history work may replace this; no design change to poll history.

## Acceptance checklist

- [x] Billing + Management paths wait/observe shared store
- [x] Two processes (two store handles) share cooldown for same key
- [x] Tests green
- [x] Kill switch still honored
- [x] `cargo fmt -p xai-grok-shell -p grok-rate-limit`
