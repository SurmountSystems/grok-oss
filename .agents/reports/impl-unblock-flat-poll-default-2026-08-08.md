# Unblock flat free SuperGrok period debit gate (dogfood default)

**Date:** 2026-08-08
**Branch:** `fixes-2`
**Why:** After rebuild, every sampler turn failed in ~0.5s with the free SuperGrok period flat-poll block ("Blocked: free SuperGrok period limits are not debiting…"), framed as yellow **Internal error**. Default-block was wrong for dogfood while server C4 free SuperGrok period debit stays unproven.

## Product change

| Item | Before | After |
|------|--------|--------|
| Default | `allow_spend_when_free_period_debit_unproven = false` → **block** turns | **`true` → allow** turns |
| Opt-in hard block | set allow = true to work | set allow = **false** (or env `=0`) to block |
| Honesty | block message only | flat-poll `/limits` notes + doctor dual-auth line + turn-start **warn** when unproven + headroom |
| ACP framing when blocked | `internal_error().data(msg)` → UI "Internal error: …" | `Error::new(code, msg)` product message as primary (no Internal error chrome) |

Config key name kept for minimal churn:
`[auth] allow_spend_when_free_period_debit_unproven`

Env: `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN`
- **unset** → config / default **true** (allow)
- **truthy** → allow
- **falsy when set** (`0` / `false` / `off` / `no`) → hard block

## Opt-in hard block (operator)

```toml
[auth]
allow_spend_when_free_period_debit_unproven = false
```

Or:

```bash
export GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0
```

## Files touched

- `crates/codegen/xai-grok-shell/src/auth/config.rs` — default true; serde omit default true
- `crates/codegen/xai-grok-shell/src/auth/free_period_debit_unproven_guard.rs` — docs, env falsy, honesty helper, tests
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` — allow path warn; block without Internal error
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` — status copy
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` — blocked note = opt-in hard block; honesty-without-block test
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `RESIDUAL.md`, `FORK.md` — standing text matches default allow

## TDD (observed green contracts)

Named contracts:

1. Default config + flat unproven + free SuperGrok period headroom → **does not** block
2. `allow = false` same conditions → **does** block
3. Honesty still surfaces when flat unproven without turns-blocked note

Commands:

```text
cargo test -p xai-grok-shell --lib free_period_debit_unproven
  → 18 passed (incl. multipoll_six_percent_flat_unproven_does_not_block_by_default,
     multipoll_six_percent_flat_unproven_blocks_when_allow_false,
     default_allow_spend_is_true_dogfood)

cargo test -p xai-grok-shell --lib auth::config::tests
  → 15 passed (allow_spend…_default_true_opt_in_block_false)

cargo test -p xai-grok-pager --lib turns_blocked_note
  → 2 passed

cargo test -p xai-grok-pager --lib flat_unproven_honesty_without_turns_blocked
  → 1 passed
```

## Verify

- `cargo fmt -p xai-grok-shell -p xai-grok-pager`
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` (ok)
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` (ok)
- Note: `clippy --all-targets` on shell still hits **pre-existing** test-only `await_holding_lock` / related elsewhere (not this gate)
- `just install` → `grok-oss 0.2.111 (c87f66a61d94) [stable]` at `~/.cargo/bin/grok-oss`

## Operator action

Restart / re-run dogfood binary (`grok-oss` / pager install path). No multipoll re-run required for this client default flip. C4 server debit remains open (ticket / multipoll evidence unchanged).

## Git

No `git add` / `git commit` (agent policy).
