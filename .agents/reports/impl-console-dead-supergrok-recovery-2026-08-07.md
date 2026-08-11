# Implement report: Console-dead recovery to SuperGrok free period

**Date:** 2026-08-07
**Plan:** session `plan.md` (Console-dead recovery to SuperGrok free period)
**Inventory:** `.agents/reports/plan-console-dead-supergrok-hop-inventory-2026-08-07.md`

## Outcome

Shipped the full vertical: rank recovery, credit hop, period-reset rank (already green, kept), terminal plain English, compact console-live honesty, user-guide note. Design A free-period-first preserved (console omitted while free SuperGrok period has room).

## Product changes

### 1. Rank (ExhaustedAll reverse only)

`order_credentials_for_preferred_auto` when free SuperGrok period is full and SuperGrok $ extras are 0/unknown:

- **Primary:** first console key (unchanged)
- **Failover:** remaining console keys, then **non-hard-expired SuperGrok JWTs as recovery tail**
- **`session_identity_key`:** first recovery SuperGrok token (was `None`)

Hard-expired SuperGrok is never recovery. While free period has headroom, console still omitted.

### 2. Mid-turn hop

`ensure_supergrok_recovery_after_console_credit_exhaust` (sampler):

- On **console** credit/spend death, if `session_identity_key` is set: clear preemptive SuperGrok included-full memo once and queue that JWT first in failover
- `apply_retry_decision` credit path calls this before rotate
- No hop invent when SuperGrok identity is absent / dead

### 3. Terminal copy

- `credit_exhausted_user_message` + `strip_api_error_status_prefix` (sampling-types)
- `map_sampling_err_to_acp` 403 credit path → plain string data
- `sampler_turn` terminal fail for credit 403/402 → plain English, not nested Internal error JSON envelope only
- Bare 403 policy/ZDR unchanged

### 4. Compact chrome

- `compact_meter_text_for_live_identity`: console live never returns bare SuperGrok `...%`
- Status bar already branches on console live; pure helper + unit test lock the contract

### 5. Docs

User-guide `02-authentication.md`: console team dead → SuperGrok recovery hop; meters stay distinct; plain fail when SuperGrok also dead.

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-sampling-types/src/error.rs` | Team 403 fixture; plain credit copy helpers |
| `crates/codegen/xai-grok-sampling-types/src/lib.rs` | Re-exports |
| `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | ExhaustedAll recovery failover + tests |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | Resolve comment + ExhaustedAll test contract |
| `crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs` | Recovery inject + unit tests |
| `crates/codegen/xai-grok-sampler/src/lib.rs` | Export recovery helper |
| `crates/codegen/xai-grok-sampler/src/actor/request_task.rs` | Credit path calls recovery |
| `crates/codegen/xai-grok-sampler/tests/test_actor.rs` | Console 403 hop / no-hop integration |
| `crates/codegen/xai-grok-shell/src/sampling/error.rs` | ACP map plain credit 403 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | Terminal plain credit path |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Compact console-live helper + test |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Recovery + meter honesty |

## RED → GREEN evidence

| Contract | Test | Result |
|----------|------|--------|
| Exact dogfood team 403 body | `credit_exhausted_detects_console_team_monthly_spending_limit_403` | GREEN (already matched via "spending limit"; fixture + plain copy asserts) |
| ExhaustedAll keeps SuperGrok recovery | `auto_exhausted_all_console_primary_keeps_supergrok_recovery_in_failover` | GREEN (new contract; old "no SuperGrok" revised) |
| Hard-expired not recovery | `auto_exhausted_all_hard_expired_supergrok_not_recovery` | GREEN |
| Console 403 hops SuperGrok | `console_team_credit_403_hops_to_supergrok_recovery` | GREEN |
| No hop when SuperGrok dead | `console_team_credit_403_no_hop_when_supergrok_also_dead` | GREEN |
| Memo clear + recovery queue | `ensure_supergrok_recovery_after_console_credit_clears_memo_and_queues` | GREEN |
| Period reset SuperGrok primary | `period_reset_clears_memo_and_ranks_supergrok_primary_without_console` (+ load path sibling) | GREEN (existing) |
| Terminal plain English | `terminal_credit_exhausted_403_is_plain_english_not_internal_error_json` | GREEN |
| Compact console honesty | `compact_status_console_live_does_not_imply_supergrok_drives_turn` | GREEN |

### Commands (post-impl)

```text
cargo test -p xai-grok-sampling-types --lib -- credit_exhausted
cargo test -p xai-grok-shell --lib -- auto_exhausted auto_both_included auto_after_included \
  resolve_auto_both_supergrok resolve_enforced_auto terminal_credit period_reset
cargo test -p xai-grok-sampler --lib -- ensure_supergrok
cargo test -p xai-grok-sampler --test test_actor -- credit
cargo test -p xai-grok-pager --lib -- compact_status_console
cargo fmt -p xai-grok-sampling-types -p xai-grok-sampler -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-sampling-types -p xai-grok-sampler --all-targets -- -D warnings
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

All targeted filters green. Clippy lib clean on touched packages. Pre-existing clippy noise on unrelated shell/pager **test** targets (`await_holding_lock` in ascii scrub / rate limit helpers) not introduced here.

## Residual

| Item | Notes |
|------|--------|
| Cold SuperGrok billing warm on first turn | Period-reset + enrich paths already clear memo when live % &lt; 100. No new forced warm poll; cold hop still one SuperGrok recovery attempt then plain fail if wire still dead. Operator dogfood with warm billing still recommended. |
| Multi-SuperGrok recovery host switch | Only `session_identity_key` (first recovery JWT) gets session host label on hop; secondary SuperGrok tokens in failover may not switch host (pre-existing multi-id gap). Dogfood is single SuperGrok. |
| FORK one-liner | Optional; not written (user-guide covers product behavior). |
| Rate-limit path | Still prefers next console before SuperGrok recovery (credit path injects SuperGrok first). |

## Operator dogfood checklist

```toml
[auth]
preferred_method = "oauth"
auto_use_included_limits = true
```

1. Warm SuperGrok billing → real free-period %, not stuck `...%` forever with live JWT.
2. Free period room → sample SuperGrok; console team 403 not primary path.
3. Free period full + console primary → team spend 403 → Retrying "out of allowance" to SuperGrok, or plain team admin English if SuperGrok dead.
4. Console Billing $ balance is not free SuperGrok period success.

## Git

No `git add` / commit (agent rule). Working tree dirty with product + test + docs only.
