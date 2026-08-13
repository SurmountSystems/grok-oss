# StreamResumed runtime catalog fix

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior residual:** `.agents/reports/impl-upstream-shell-tests-compile-2026-08-11.md` (compile green; this test failed at runtime)
**Catalog:** `doc/dev/upstream-regression-filters.md` (stuck-retry / StreamResumed)

---

## Contract

`SamplingEvent::StreamStarted` must persist `RetryState::StreamResumed` so the pager clears sticky yellow Retrying chrome (soft-reconnect) instead of freezing attempt N across a live post-retry stream.

Named test:

```text
session::acp_session::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed
```

Command:

```bash
cargo test -p xai-grok-shell --lib stream_started_emits_retry_state_stream_resumed -- --nocapture
```

---

## Red (observed)

```text
thread 'session::acp_session::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed'
  panicked at .../replay_buffer_send_update_tests.rs:521:13:
StreamStarted must persist RetryState::StreamResumed for pager chrome clear
test ... FAILED
test result: FAILED. 0 passed; 1 failed
```

Cause: `handle_sampling_event` `StreamStarted` arm only updated streaming capture + `record_stream_start`. It never emitted the xAI notification.

---

## Fix (minimal product)

**File:** `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`
**Arm:** `SamplingEvent::StreamStarted`

After capture/`record_stream_start`, add:

```rust
self.send_xai_notification(XaiSessionUpdate::RetryState(
    crate::extensions::notification::RetryState::StreamResumed,
))
.await;
```

`send_xai_notification` already persists via `persistence_tx` (`PersistenceMsg::Update` / `SessionUpdate::Xai`) and forwards to the client. Matches how `Retrying` / `Failed` / `Exhausted` are emitted elsewhere.

No test rewrite. No broad refactor.

---

## Green (same filter)

```text
test session::acp_session::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6360 filtered out
```

Also: `cargo check -p xai-grok-shell --lib` green (warnings only, pre-existing).
`cargo fmt -p xai-grok-shell -- --check` clean.
`cargo clippy -p xai-grok-shell --lib --no-deps -- -D warnings` fails on pre-existing `unreachable_pub` (and similar), not on this edit.

---

## Residual

1. **Pager catalog filters** for StreamResumed soft-reconnect (`retry_chrome_soft_reconnect`, `stream_resumed_without_prior_retry`, …) not re-run here (pager tree may still be mid-dirt per prior mop).
2. Pre-existing shell `unreachable_pub` / tools dead_code clippy noise remains out of scope.
3. Stashes `recon-temp-work-b-wip-2026-08-10` and `recon-resume-local-dirt-2026-08-10` **not** dropped.
4. No git commit / push.

---

## Stashes / git

- No staging required for this one-file product fix unless recon asks.
- Agent does not commit.
