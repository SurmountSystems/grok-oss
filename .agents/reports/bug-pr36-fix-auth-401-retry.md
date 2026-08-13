# PR36: authenticated 401 retry budget exhaust (hermetic)

Branch: `onto-xai/b13fa526f511`
Old HEAD: `75356b2060feaa0b78d59dce2368aeb5987e37bf`

## Verdict

Flake, not a broken retry-budget contract. Send-now kind routing in
`prompt_queue.rs` did not change 401 charging. The product still exhausts
after `AuthRetrySchedule::MAX_RETRIES` (3) credentialed 401s. The GHA
fail was the test's `tokio::time::timeout(60s)` under `start_paused`.

## Red (observed)

Command:

```
cargo test -p xai-grok-shell --lib auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries -- --nocapture
```

First run: PASS (0.10s). Loop of 10: run 4 FAIL in 0.11s wall:

```
thread 'auth-401-exhaust' panicked at auth_retry_budget_tests.rs:233:6:
turn must finish within timeout: Elapsed(())
```

That matches CI: TRY 2 FAIL in 0.185s (not a real 60s hang). MCP had no
assertion body because this is a timeout expect, not an `assert_eq`.

Cause: `block_on_local(true)` plus `tokio::time::timeout(60s)` around
`handle_prompt`. When the mock HTTP server leaves the runtime idle,
auto-advance jumps to that 60s timer and aborts a healthy turn. The
sampler's stream idle bound (`tokio::time::timeout` on `stream.next()`)
is the same class of landmine.

## Product / test change

File:
`crates/codegen/xai-grok-shell/src/session/acp_session_tests/turn/auth_retry_budget_tests.rs`

`authenticated_401s_still_exhaust_after_three_retries` now uses a live
tokio clock (`block_on_local(false)`). Assertions unchanged: turn fails,
message names authenticated rejections, 4 authenticated `/responses`
sends (initial + 3 retries), terminal `retryState` type `auth`.

No change to `AuthRetrySchedule`, `RetryPolicy::edge_client`, or
send-now routing.

## Green

Same filter:

```
cargo test -p xai-grok-shell --lib auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries -- --nocapture
```

PASS in 7.11s (real 1s/2s/4s backoff). Five more loops: 5/5 PASS.

Also:

```
cargo test -p xai-grok-shell --lib -- auth_retry_budget_tests queue_send_now_during_goal_routes_by_kind queue_send_now_never_cancels_uncommitted_front -- --nocapture
```

4 passed (full `auth_retry_budget_tests` + both send-now filters).

```
cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib --bins -- -D warnings
```

Both clean.

## Land

Product commit (this report is a follow-up tree):

- HEAD: `f1dbf925025e786c551214dbd5a45a2d50ac30c4`
- Tree: `c9b6d1489ceb2a4e99d9558e1df2007b9b546e33`
- Parent: `75356b2060feaa0b78d59dce2368aeb5987e37bf`
- Ancestors: `e5fd4816`, `origin/main`, previous tip, and `origin/onto-xai/b13fa526f511` all still ancestors.

Push: `git push origin HEAD:onto-xai/b13fa526f511` (no force) after the report pin.
