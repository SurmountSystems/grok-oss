# Report: two unit test fails (billing None balance + shell `/spend`)

Date: 2026-08-03
Repo: `/home/hunter/Projects/surmount/grok-build`
Crate: `xai-grok-pager`

## Failures (red)

```
FAIL xai-grok-pager app::dispatch::tests::billing::billing_fetched_none_balance_clears_cached
FAIL xai-grok-pager slash::commands::tests::shell_collision_contract_covers_every_pager_command_and_alias
```

### Assert messages

| Test | Panic |
|------|--------|
| `billing_fetched_none_balance_clears_cached` | `None balance should disable billing polling` at `app/dispatch/tests/billing.rs:900` |
| `shell_collision_contract_covers_every_pager_command_and_alias` | `unreserved pager key spend` (then after partial fix: `double-entry`) at `slash/commands/mod.rs` |

## Diagnosis

### 1. None balance left `billing_poll_wanted` true

Named contract (test + product comment above the clear path): a `BillingFetched` with `balance: None` means the response carried **no billing config** (parse/transport failures go to `BillingError`). That path must clear cached balance **and** disable billing polling so the status bar matches "No billing data."

Product path `app/dispatch/billing.rs` already set `app.credit_balance = balance.clone()` (so None cleared cache), but poll wanted was:

```rust
.unwrap_or(true) // No config yet: keep polling so chrome can warm.
```

That disagreed with the contract. Recent limits chrome work also OR-in `!b.included_usage_known` for Some balances; that is correct for known-but-unknown-included meters, and was not the None failure mode.

### 2. `/spend` missing from shell collision allowlist

New Token Economy slash command `spend` (aliases `double-entry`, `ledger`) was registered in `builtin_commands()` but not listed in the `SHELL_RESERVED` table that the shell-collision contract checks so pager keys do not collide with shell names.

## Fix (minimal product)

1. **`crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs`**
   - `unwrap_or(true)` → `unwrap_or(false)` when balance is None.
   - Comment updated to match clear-polling contract.
   - OpenRouter / console team prepaid still force poll via existing `||` clauses.

2. **`crates/codegen/xai-grok-pager/src/slash/commands/mod.rs`**
   - Add reserved keys: `spend`, `double-entry`, `ledger` to `SHELL_RESERVED` in the collision contract test table.

No test expectation rewrites. No git add/commit.

## Proof (green)

```bash
cargo fmt -p xai-grok-pager

cargo test -p xai-grok-pager --lib -- billing_fetched_none_balance --nocapture
# ok: billing_fetched_none_balance_clears_cached
# ok: billing_fetched_none_balance_shows_no_data_message

cargo test -p xai-grok-pager --lib -- shell_collision --nocapture
# ok: shell_collision_contract_covers_every_pager_command_and_alias

cargo test -p xai-grok-pager --lib -- billing_fetched_ --nocapture
# ok: 13 passed (includes high/low usage poll enable/disable)
```

## Files touched

- `crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs`
- `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs`
- `.agents/reports/bug-ci-two-unit-fails-2026-08-03.md` (this report)
