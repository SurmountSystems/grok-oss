# bug: CI clippy `manual_range_patterns` (502|503|504)

**Date:** 2026-08-04
**Crate:** `xai-grok-sampler`
**Status:** fixed

## Problem

CI clippy (`-D warnings`) failed on `clippy::manual_range_patterns`:

- `crates/codegen/xai-grok-sampler/src/actor/request_task.rs:1108`
  - `matches!(status.as_u16(), 502 | 503 | 504)` → use `502..=504`
- `crates/codegen/xai-grok-sampler/src/retry.rs:334`
  - `matches!(code, 502 | 503 | 504)` → use `502..=504`

## Change

Surgical pattern rewrites to inclusive ranges:

| File | Before | After |
|------|--------|--------|
| `src/actor/request_task.rs` | `matches!(…, 502 \| 503 \| 504)` | `matches!(…, 502..=504)` |
| `src/retry.rs` (exhausted outage check) | same | `502..=504` |
| `src/retry.rs` (status hint arm) | `#[allow(clippy::manual_range_patterns)]` + `502 \| 503 \| 504 =>` | `502..=504 =>` (allow removed) |

Same-crate scan: no remaining `502 | 503 | 504` or-patterns under `xai-grok-sampler`.

## Verify

```text
cargo fmt -p xai-grok-sampler
cargo clippy -p xai-grok-sampler --lib --locked -- -D warnings
  → ok (exit 0)

cargo test -p xai-grok-sampler --lib -- stream_521 is_retryable cf_edge
  → 3 passed (classify_event_stream_error_is_retryable,
     classify_521_is_retryable_with_client_rebuild,
     classify_stream_error_is_retryable)
```

## Git

No `git add` / `git commit` (operator-owned).
