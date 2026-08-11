# Hourly residual loop — 2026-08-07 19:45

## Slice picked

**Phase R rate limits by API type** (inventory rank 1; residual still listed open).
Not a re-implement: join already claimed shipped 2026-08-03. This fire **verified**
unit contracts and **closed residual honesty**.

## What was done

1. Read `RESIDUAL.md` Open + Highest-value next and inventory
   `.agents/reports/residual-hourly-loop-inventory-2026-08-07.md`.
2. Confirmed product join
   `.agents/joins/impl-phase-r-rate-limits-by-api-type-2026-08-03.md` + FORK
   § Multi-session rate limits cover Imagine / video / voice / responses /
   chat+BYOK (sampler host+fingerprint) / billing / Management / GitHub.
3. Re-ran hermetic tests (all green, exit 0):

| Command | Result |
|---------|--------|
| `cargo test -p grok-rate-limit --lib` | 15 passed |
| `cargo test -p xai-grok-tools --lib shared_http_rate_limit` | 5 passed |
| `cargo test -p xai-grok-tools --lib imagine_429` | 1 passed (`imagine_429_observes_shared_rate_limit_store`) |
| `cargo test -p xai-grok-voice --lib voice_provider` | 1 passed |
| `cargo test -p xai-grok-tools --lib static_api_key_is_fallback` | 1 passed |
| `cargo test -p xai-grok-shell --lib shared_http_rate_limit` | 9 passed |

4. Updated `RESIDUAL.md` Open: Phase R marked **shipped**; rank table row 10
   struck as shipped + verified 2026-08-07; Highest-value prose no longer lists
   Phase R as open product work.
5. Refreshed inventory
   `.agents/reports/residual-hourly-loop-inventory-2026-08-07.md` (top agent-doable
   now dual SuperGrok poll soft edges, multi-track plan, etc.).

## Not done (next fires)

| Rank | Work |
|------|------|
| 1 | Dual SuperGrok poll soft edges (multi-slot OIDC refresh before sibling poll; N auth-fail demote without secret delete) |
| 2 | Multi-track also-guard product (short plan first if no plan file) |
| Operator | Rebuild/dogfood; C4 ticket; live extras-after-full |

## Remaining residual (one-liner)

Phase R closed; next agent-doable product slice is dual SuperGrok poll soft edges
(or multi-track plan). Operator-gated: rebuild/dogfood, C4 debit ticket, live
extras-after-full. Do not invent SuperGrok free-period debit.

## Git

No `git add` / commit / push (policy).
