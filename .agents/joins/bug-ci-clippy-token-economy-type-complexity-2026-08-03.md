# Join: CI clippy type-complexity on `latest_remote_sample`

**Date:** 2026-08-03
**Status:** green

## Problem

`cargo clippy -p xai-grok-shell --lib --locked -- -D warnings` failed:

```
error: very complex type used. Consider factoring parts into `type` definitions
 --> crates/codegen/xai-grok-shell/src/token_economy/ledger.rs:343:6
) -> Result<Option<(String, Option<String>, Option<String>, JsonValue)>, rusqlite::Error>
```

## Fix

Named struct instead of a 4-tuple (no `#[allow]`):

- Added `RemoteMeterSample` with fields `sampled_at`, `window_start`, `window_end`, `payload`
- `latest_remote_sample` now returns `Result<Option<RemoteMeterSample>, rusqlite::Error>`
- Test uses `got.payload[...]` instead of `got.3[...]`
- Re-exported from `token_economy::mod`

## Proof

```text
cargo clippy -p xai-grok-shell --lib --locked -- -D warnings
# Finished ok

cargo test -p xai-grok-shell --lib token_economy
# 30 passed; 0 failed
```

Also ran `cargo fmt -p xai-grok-shell`.

## Files

- `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs`
- `crates/codegen/xai-grok-shell/src/token_economy/mod.rs`

No git commit/add.
