# Implement report: Free SuperGrok period always before credits

**Date:** 2026-08-07
**Plan:** Free SuperGrok period always before credits (complete vertical)
**Workspace:** `/home/hunter/Projects/surmount/grok-build`

## Outcome

Client vertical complete. Smoking-gun status paint fixed: sticky exhaust memo cannot force `console · $340` while live free SuperGrok period has headroom. Active driver on `limits --json` and human `/limits`. Settlement honesty strengthened. Rank/prefer_live regressions stay green. C4 server free-period debit remains residual honesty only.

## RED / GREEN table (named contracts)

| # | Contract | Test filter / name | RED | GREEN |
|---|----------|--------------------|-----|-------|
| 1 | SuperGrok live + included 6% + memo out + team prepaid → compact **`6%`**, not **`console · $340`** | `compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` | New pure helper did not exist; sticky pin ignored free-period headroom in `render.rs` | Pass after `status_sampling_identity_for_compact_meter` + paint wire |
| 1b | Paint path status bar same dogfood shape | `status_bar_free_period_headroom_not_console_prepaid_dollars` | Would paint console if sticky pin still forced ConsoleKey | Pass |
| 2 | Active driver free-period headroom even with extras + team prepaid known | `active_driver_free_period_headroom_even_with_extras_and_team_prepaid` | New API | Pass: `supergrok_free_period` |
| 3 | `limits --json` / human Active driver when free period room | `limits_json_active_driver_free_period_with_extras_on_account` | Field absent | Pass: `activeDriver=supergrok_free_period`, human `Active: free SuperGrok period` |
| 3b | After-burner: free period full + extras → SuperGrok extras | `active_driver_afterburner_extras_when_free_period_full`, `limits_json_active_driver_extras_afterburner` | New path | Pass: `supergrok_extras` |
| 4 | Rank: auto_use + free-period headroom → zero console primary | existing `auto_order_omits_console_while_any_supergrok_included_headroom`, `auto_with_included_headroom_still_omits_console`, `check_limits_first_*` | Already green (audit, not broken) | Kept green |
| 5 | Exhaust memo / out_of_allowance: live headroom blocks false console paint | `status_identity_sticky_console_when_free_period_full_and_memo_out` + sticky headroom test | Sticky pin always ConsoleKey on memo | Pass: headroom blocks; full/cold still sticky |
| 6 | Settlement: C6 + flat free period does not claim free period moved; not SuperGrok extras | `c6_team_usage_note_when_oauth_postpaid_dominates`, `branch_2b_stack_base_flat_and_c6_when_evidence` | C6 text weaker; no flat+settlement note | Pass: C6 names not free-period burn / not SuperGrok extras; flat+C6 adds `NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS` |
| 7 | Design A after-burner still works | `compact_status_supergrok_on_extras_*`, `status_bar_supergrok_on_extras_*` | Regression check | Pass |

### Commands run (post-impl)

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager -p xai-grok-shell --lib -- -D warnings
# exit 0

cargo test -p xai-grok-pager --lib -- \
  check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty \
  limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b \
  format_supergrok_session active_driver status_bar_free_period sticky_memo
# 57 passed; 1 ignored

cargo test -p xai-grok-shell --lib -- \
  auto_order_omits_console auto_order_keeps_supergrok auto_with_included_headroom \
  auto_after_included format_human_auto_use allowance_exhaust_from_billing \
  out_of_allowance_helper
# 33 passed

cargo test -p xai-grok-sampler --lib -- prefer_live exhausted
# 30 passed
```

## Product changes

### P1 smoking gun (sticky pin)

- **New pure helper** `status_sampling_identity_for_compact_meter` in `credit_bar.rs`:
  - Tracked console → console
  - Free period known and used % &lt; 100 → SuperGrok (blocks false sticky)
  - Else memo out + console ready → console
  - Else tracked
- **Wire** both sticky pin sites in `agent_view/render.rs` (status bar + footer) to use helper with live `credit_balance` free-period reading.

### P3/P5 active driver

- **`ActiveSpendDriver`** + `active_spend_driver(...)` (same Design A order as compact meter)
- `limits --json`: `activeDriver` + `activeDriverLabel`
- Human `/limits` / `grok limits`: second line **`Active: free SuperGrok period | SuperGrok extras | console key`**

### P2 settlement honesty

- C6 note strengthened: settlement rise is not free SuperGrok period burn proof and not SuperGrok dollar extras as live driver
- New note when flat free period + OAuth postpaid dominates: team Grok Build class can climb while free period stays flat; no invent debit; not SuperGrok extras

### P4 rank / memo / prefer_live

- Audit: existing rank omits console with headroom; memo mark only at ≥100%; enrich clears memo on headroom; prefer_live tests green
- Paint path was the real bug (memo true while live free period 6%)

### Docs

- `FORK.md`: shipped bullet free SuperGrok period before credits
- `RESIDUAL.md`: client TE chrome shipped; C4 server residual still open
- User-guide `02-authentication`: four-meter table + burn order + smoking-gun bug report note
- User-guide `04-slash-commands`: `activeDriver` / Active line
- Doctor free-period-first line points at `grok limits` Active driver and sticky chrome law

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Helpers + unit tests |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Sticky pin wire + paint test |
| `crates/codegen/xai-grok-pager/src/limits_cmd.rs` | `activeDriver` on JSON + tests |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | Human Active line |
| `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` | C6 strengthen + flat settlement note |
| `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` | Doctor free-period + Active pointer |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Meters + burn order |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | activeDriver |
| `FORK.md` | Shipped bullet |
| `RESIDUAL.md` | Client shipped / C4 open honesty |

## Non-goals (held)

- No invent free SuperGrok period debit (C4)
- No stop server dual-bill alone
- No license chart invent
- No mash meters into "credits"
- No git add / commit / push / stage

## Operator dogfood checklist (rebuild this tree)

1. Rebuild/install `grok-oss` from this tree.
2. Free SuperGrok period still ~6%, SuperGrok live: status compact shows **`6%`** (or free-period form), **not** `console · $340`.
3. `grok-oss limits --json`: `liveSampling` SuperGrok; `activeDriver` = `supergrok_free_period`; free period &lt; 100%; `console.isLive` false.
4. Team Grok Build class may still climb: C6 / settlement note present; product does not claim free period moved or SuperGrok extras primary.
5. Doctor: prefer free SuperGrok period yes; points at Active driver / no console · $ while free period has room.
6. When free period later hits 100% with extras: chrome may show SuperGrok extras $ (correct after-burner).

## Residual honesty

- **C4** free SuperGrok period debit under load: still open server residual (ticket package path unchanged).
- Client limits-before-credits chrome, rank, observe, settlement language: **complete**.
