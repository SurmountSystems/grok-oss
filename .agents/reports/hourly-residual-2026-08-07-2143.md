# Hourly residual loop — 2026-08-07 21:43

## Slice picked

**Dual SuperGrok multi-slot OIDC refresh before sibling billing poll** (inventory
rank 1 soft edge remaining after N-fail demote).

## Named contract

When a non-active SuperGrok multi-slot JWT is past the early-invalidation buffer
and still has OIDC refresh credentials (`refresh_token` + issuer + client_id):

1. `session_needs_oidc_refresh_before_billing_poll` is true.
2. `ensure_fresh_access_token_for_supergrok_billing_poll` exchanges the RT and
   returns the new access token.
3. Refreshed auth is written only to that multi-slot scope (does not clobber
   active base of another principal).
4. Secrets are never auto-deleted.
5. Refresh failure falls back to the stored token so poll honesty / N-fail demote
   still work.
6. Sibling poll path (`poll_and_remember_non_active_supergrok_included_billing`)
   calls ensure_fresh before credits HTTP.

## What changed (product)

| Area | Change |
|------|--------|
| `allowance_exhaust_from_billing.rs` | Pure needs-refresh; find multi-slot entry; persist refreshed scope; async ensure_fresh via `oidc_token_exchange` |
| `extensions/billing.rs` | Sibling poll uses ensure_fresh before credits GET |
| `auth/mod.rs` | Export new helpers |
| `upstream-regression-filters.md` §2c | New filters |
| `RESIDUAL.md` | Dual SuperGrok poll soft edges closed |

## Commands (exit 0)

```text
cargo test -p xai-grok-shell --lib session_needs_oidc_refresh
# 1 passed

cargo test -p xai-grok-shell --lib find_and_persist_refreshed
# 1 passed

cargo test -p xai-grok-shell --lib ensure_fresh_refreshes_expired
# 1 passed (hermetic mock IdP)

cargo test -p xai-grok-shell --lib sibling_poll_skips
# 1 passed

cargo test -p xai-grok-shell --lib non_active_poll_targets
# 2 passed

cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings
# ok
```

## Remaining residual (one-liner)

Next agent-doable: multi-track also-guard **short plan** (not full invent), or
Management cache polish only with dogfood evidence. Operator-gated: rebuild/
dogfood, C4 ticket, live extras. Do not invent SuperGrok free-period debit.

## Git

No `git add` / commit / push.
