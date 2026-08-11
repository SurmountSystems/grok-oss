# Hourly residual loop — 2026-08-07 20:43

## Slice picked

**Dual SuperGrok poll soft edge:** demote sibling from automatic billing poll list
after N consecutive auth-class fails, without auto-deleting `auth.json` secrets.

(Multi-slot OIDC refresh before sibling poll left soft for a later fire.)

## Named contract

After `SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD` (3) consecutive auth-class SuperGrok
billing poll fails for one identity:

1. `should_skip_supergrok_billing_poll_for_auth_streak` is true.
2. `load_non_active_supergrok_billing_poll_targets` omits that sibling.
3. Secrets remain in `auth.json` (no auto-delete).
4. Network/other fails do not bump the auth streak.
5. A successful `remember_supergrok_billing_poll_ok` resets the streak and restores
   the sibling to the poll list.

## What changed (product)

| Area | Change |
|------|--------|
| `allowance_exhaust_from_billing.rs` | Process map `AUTH_FAIL_STREAK_BY_IDENTITY`; bump on auth fail; reset on Ok; clear with cache; filter non-active poll targets |
| `auth/mod.rs` | Export threshold + streak helpers |
| `upstream-regression-filters.md` §2c | Added `sibling_poll_skips_after_n` |
| `RESIDUAL.md` | N-fail demote shipped; multi-slot OIDC refresh still soft |

## Commands (exit 0)

```text
cargo test -p xai-grok-shell --lib sibling_poll_skips_after_n_consecutive_auth_fails_without_secret_delete
# 1 passed

cargo test -p xai-grok-shell --lib auth_failed_poll
# 1 passed

cargo test -p xai-grok-shell --lib non_active_poll_targets
# 2 passed

cargo test -p xai-grok-shell --lib order_live_prefers_poll_ok
# 1 passed

cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings
# ok
```

## Remaining residual (one-liner)

Next agent-doable: multi-slot OIDC refresh before sibling poll, or multi-track
also-guard short plan. Operator-gated: rebuild/dogfood, C4 ticket, live extras.
Do not invent SuperGrok free-period debit.

## Git

No `git add` / commit / push.
