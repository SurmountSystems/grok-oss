# bug: external auth headless decline → interactive sign-in

**Date:** 2026-08-11
**Status:** done
**Crate:** `xai-grok-shell`
**Test:** `external_auth_conforming_provider::a_provider_that_declines_the_headless_run_can_still_sign_the_user_in`

## Contract

When an external conforming `auth_provider_command` declines the headless run
(`GROK_AUTH_EXPIRED=1`, exit non-zero), sign-in must re-run the same binary on
the **interactive** contract (`GROK_AUTH_EXPIRED` unset), not fall through to
real browser OAuth (`auth.x.ai`).

## Red (observed)

```text
just cargo-ci cargo test -p xai-grok-shell --test external_auth_conforming_provider --locked -- --nocapture
# exit 101
SSO session lapsed; cannot mint without the user
Signing in with browser instead...
Signing in with Grok...
Open this URL to sign in: https://auth.x.ai/oauth2/authorize?...
thread panicked at external_auth_conforming_provider.rs:127:10:
the sign-in must reach the provider's interactive branch, not the browser login: Elapsed(())
```

## Root cause (two product bugs in `auth/flow.rs`)

### 1. Interactive external call re-armed headless env

In `run_auth_flow_inner`, the external-provider block used:

```rust
let is_refresh = reauth || auth_manager.is_expired();
```

After a headless decline, external-binary permanent failure is non-sticky
`Other` and **retains** the expired credential. So `is_expired()` stayed true,
`GROK_AUTH_EXPIRED=1` was set again on the "sign-in" call, the conforming
binary declined again, and the flow printed `Signing in with browser instead...`
then hit live loopback OAuth.

README contract (Auth Binary with Refresh Support): escalation after a declined
headless probe is a **sign-in** with the variable **unset**.

### 2. Self-contention on `auth.json.lock` (~45s)

The expired path held an advisory lock across `auth_manager.auth()`.
`refresh_chain` acquires the same lock with `REFRESH_LOCK_TIMEOUT` (45s), so
this process waited on itself (~47s). The integration test guards that with
`NO_SELF_CONTENTION` (20s).

## Fix

File: `crates/codegen/xai-grok-shell/src/auth/flow.rs`

1. Interactive external call always uses `is_refresh = false` (never sets
   `GROK_AUTH_EXPIRED` in this block). Comment documents the headless vs
   sign-in split.
2. Disk peek under the short lock only; **drop** the lock before `auth()` so
   `refresh_chain` can acquire cleanly.

No test changes. CI no longer reaches live `auth.x.ai` for this contract when
the provider mints on interactive.

## Green

| Command | Exit |
|---------|------|
| `just cargo-ci cargo test -p xai-grok-shell --test external_auth_conforming_provider --locked -- --nocapture` | **0** (0.04s) |
| `just cargo-ci cargo test -p xai-grok-shell --lib --locked auth::flow::` | **0** (31 passed) |
| `just cargo-ci cargo fmt -p xai-grok-shell` | **0** |
| `just cargo-ci cargo clippy -p xai-grok-shell --all-targets --locked -- -D warnings` | **0** |

## Files changed

- `crates/codegen/xai-grok-shell/src/auth/flow.rs` only

## Done / not done

| Done | Not done / out of scope |
|------|-------------------------|
| Product fix for headless-decline → interactive | No harness-only workaround |
| Self-lock contention on expired path | Broader ensure_authenticated refresher wiring (separate) |
| Red observed, green same filter | Git commit/push (forbidden) |
| fmt + clippy + related unit tests | |
