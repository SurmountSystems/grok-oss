# Report: status chrome messy team prepaid footer (2026-08-09)

Board: `bug:status-chrome-messy-team-prepaid`

## Operator fact

Screenshot (fixes-2 / grok-build session):

- **Top** correct: `free SuperGrok period · 27% · 60% behind linear burn`
- **Footer** messy: `not the active spend path: team prepaid remaining $340 · Grok Build class $1162.92 · Grok 4.5 (high) · always-approve`

Free SuperGrok period limits were the active primary meter with room. The long secondary team line dominated the prompt footer next to model name and always-approve.

## Root cause

`crates/codegen/xai-grok-pager/src/views/credit_bar.rs`:

- `usage_warning_for_session_with_identity_principal_gap_and_postpaid` (SuperGrok live path)
- `merge_supergrok_warning_with_team_meters` + `format_team_settlement_footer`
- Constant `TEAM_SECONDARY_METERS_LABEL = "not the active spend path"`

Prior Work C fix always attached secondary team prepaid / Grok Build class under that prefix whenever SuperGrok was live and team meters were known. Mid free SuperGrok period SuperGrok-native warnings are quiet (`supergrok_session_usage_warning` returns `None` below ~90%), so the **only** usage-warning string became the long team secondary line. Compact status was already correct free SuperGrok period %; footer still painted wallet settlement noise.

## Contract implemented

**While SuperGrok session is live and free SuperGrok period has room** (`included_usage_known && usage_pct < 100`):

- Prompt footer **does not** paint team prepaid remaining, Grok Build class, loading team prepaid, or the long `not the active spend path: …` string.
- SuperGrok free-period % / SuperGrok dollar credits warnings still fire when those alone would warn.
- Compact status still names free SuperGrok period (unchanged Design A).
- Full team meters stay on `/limits` / `grok limits`.

**After free SuperGrok period is full** (`usage_pct >= 100`):

- Secondary team meters may still attach under `not the active spend path:` (same label; not active SuperGrok dollar credits / not console live pay jargon).

**Console live** path unchanged: `Console key · team prepaid: $N` (+ optional team Grok Build class chip).

Meters stay distinct: free SuperGrok period limits ≠ SuperGrok dollar credits ≠ team prepaid remaining ≠ team Grok Build class.

## Code changes

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | `free_supergrok_period_has_room`; merge skips settlement while room; docs; tests |
| `…/docs/user-guide/02-authentication.md` | Surfaces + burn-order: quiet footer while free SuperGrok period has room |
| `…/docs/user-guide/04-slash-commands.md` | `/limits` footer bullets match new contract |
| `RESIDUAL.md` | Honesty: shipped footer behavior updated |
| `FORK.md` | Footer secondary team meters note updated |

## Named TDD

Primary operator contract:

- `operator_screenshot_free_period_primary_footer_not_long_team_prepaid`
  - free SuperGrok period 27% + team prepaid $340 + Grok Build class $1162.92
  - compact `free SuperGrok period · 27%`, `activeDriver = SuperGrokFreePeriod`
  - footer usage warning is **None** (no long not-active-spend team string)

Related:

- `footer_supergrok_live_with_management_prepaid_quiet_while_free_period_has_room`
- `work_c_free_period_headroom_intent_compact_and_quiet_footer`
- `work_c_settlement_footer_does_not_replace_free_period_intent`
- After free SuperGrok period full still shows secondary when appropriate
  (`footer_supergrok_live_after_free_period_full_shows_secondary_team_prepaid`, class/loading companions)

## Verify

```text
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib views::credit_bar::
# → 90 passed
```

`cargo clippy -p xai-grok-pager --all-targets -- -D warnings` fails on **pre-existing** dead code / private field in untouched files (`agent_view/queue.rs` `holds_queue_for_background`, `app/mouse.rs` `send_now`), not on this footer change.

## Dogfood

Rebuild / install the tree, open a SuperGrok-live session with free SuperGrok period under 100% and team Management meters warm. Footer should show model name / always-approve (and SuperGrok free-period warnings only when those fire), **not** the long team prepaid / Grok Build class disclaimer. `/limits` still shows team prepaid remaining and Grok Build class.
