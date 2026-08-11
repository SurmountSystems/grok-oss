# Report: status chrome bare `intent` label (2026-08-09)

## Bug

Top-right status chrome painted **`intent · 24% · 60% behind linear burn`**.
Bare **intent** is decoder-ring. The percent is free SuperGrok period used %;
the label must name that meter.

## Named contract

Compact status meter names the **real spend-order meter**, not the word
`intent`:

- Free SuperGrok period room / cold / full-no-extras →
  `free SuperGrok period · N%` or `free SuperGrok period · ...%`
- SuperGrok dollar credits after free period full → `SuperGrok extras · $N`
  (unchanged)
- Console live → `console · $N` (unchanged)
- Never invent free SuperGrok period % on the client

## Root cause

Work C "paying intent" chrome in
`crates/codegen/xai-grok-pager/src/views/credit_bar.rs` used the prefix
`intent ·` in:

- `credit_bar_loading_line`
- `compact_meter_text_for_live_identity_with_active_poll` (SuperGrok free-period
  branches)

`ActiveSpendDriver::as_human()` already returned **`free SuperGrok period`** for
`/limits` **Active:**; compact status did not use that plain name.

## Fix

1. Added `free_supergrok_period_compact_meter(pct_display)` that formats
   `{ActiveSpendDriver::SuperGrokFreePeriod.as_human()} · {pct_display}` so
   compact status stays aligned with the human driver label.
2. Replaced every SuperGrok free-period compact paint of `intent · …` with that
   helper.
3. Updated unit tests (credit_bar + status-bar paint path) and user-guide
   (`02-authentication.md`, `04-slash-commands.md`).
4. Residual shipped-chrome bullet now says `free SuperGrok period · N%`.
   Did **not** edit `FORK.md` (other agents may own that tree this session).

## TDD

**Contract:** compact status must not contain bare paying-path label `intent ·`
or lone word `intent`; must name free SuperGrok period.

| Step | Evidence |
|------|----------|
| Red contract | Prior product/tests asserted `intent · 24%` etc. (operator screenshot + tree). New test `compact_status_names_free_supergrok_period_not_bare_intent` encodes the plain-name contract. |
| Green product | Helper + free-period branches use `free SuperGrok period · …` |
| Green re-run | See commands below |

### Commands (all exit 0)

```bash
cargo test -p xai-grok-pager --lib compact_status_names_free_supergrok_period_not_bare_intent
cargo test -p xai-grok-pager --lib views::credit_bar   # 87 passed
cargo test -p xai-grok-pager --lib status_bar_         # 12 passed (incl. credits meter paint)
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings    # clean
```

Note: `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` still hits
unrelated pre-existing failures outside this change (integration test /
bench / other modules). Lib clippy for the touched crate is clean.

## Example paint after fix

`free SuperGrok period · 24% · 60% behind linear burn`

(Percent still comes only from a known free SuperGrok period reading; cold stays
`free SuperGrok period · ...%`.)

## Files touched

- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` (product + tests)
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` (status paint tests)
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md`
- `RESIDUAL.md` (shipped chrome wording only)

## Out of scope / left alone

- `/limits` honesty notes that still say "intent chrome" as the *concept*
  (spend-order vs settlement) — not the status compact paying-path label.
- `FORK.md` historical checkbox text still mentions `intent ·` (avoided race).
- No free SuperGrok period % invented; only label string change.

## Done when

Operator rebuilds and top-right meter reads free SuperGrok period (or SuperGrok
extras / console), never bare `intent ·`.
