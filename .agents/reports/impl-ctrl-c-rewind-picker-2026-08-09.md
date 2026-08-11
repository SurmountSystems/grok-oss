# Report: Ctrl+C dead on rewind turn picker (2026-08-09)

## Root cause

While `rewind_state` is set, `AgentView::handle_input` routes **all** key
presses into `handle_rewind_key` (exclusive overlay capture). In
`views/rewind.rs::handle_rewind_key`, **Esc** mapped to `RewindInput::Dismissed`
for the turn picker and other Esc-dismissible phases, but **Ctrl+C** fell
through `_ => RewindInput::Consumed`.

`Consumed` is painted as "handled" (`InputOutcome::Changed`) and never
reaches prompt clear, quit-arm, or plan-approval abandon paths. So with
"Rewind to which turn?" open, Ctrl+C did nothing visible. L1 stayed trapped
in exclusive capture until Esc.

Worse on **ModeSelect**: bare `KeyCode::Char('c')` selected "conversation
only" without checking modifiers, so Ctrl+C could wrongly advance that phase
instead of exiting.

Plan soft-park does **not** auto-open the rewind picker (no code path from
`exit_plan_mode` / soft park to `RewindShowPicker`). Rewind opens via
`/rewind` or idle **Esc Esc** (empty prompt + messages). Soft park toast and
rewind picker can both be up if the operator also Esc-Esc'd or ran `/rewind`.
Primary bug was Ctrl+C dead, not unexpected auto-open.

## Named product contract

**Ctrl+C dismisses the rewind overlay the same way Esc does** (except while
a rewind is **Executing**, where Esc is also swallowed).

- Does **not** quit the whole app on that press.
- Maps to `RewindInput::Dismissed` → `Action::RewindDismiss` (or
  `DismissError` on the error phase).
- Clears exclusive capture and returns control to the prompt.
- Bare `c` on ModeSelect still means conversation-only; **Ctrl+C does not**.
- Aligns with plan-approval empty-composer Ctrl+C abandon and the L1
  modal-free rule (no exclusive trap with no cancel chord).

## Red evidence (observed fail before product fix)

Tests added first, product early-return **not** yet applied:

```text
cargo test -p xai-grok-pager --lib -- ctrl_c_dismisses_
```

| Test | Fail reason |
|------|-------------|
| `views::rewind::tests::ctrl_c_dismisses_rewind_picker_like_esc` | `Ctrl+C on the rewind picker must dismiss (not Consumed)` |
| `views::rewind::tests::ctrl_c_dismisses_all_esc_dismissible_rewind_phases` | `Ctrl+C must dismiss phase Picker { ... }` |

## Green proof

Minimal product fix: at the top of `handle_rewind_key`, after Release
filter, if `key!('c', CONTROL)` matches:

| Phase | Result |
|-------|--------|
| Error | `DismissError` |
| Executing | `Consumed` (same as Esc) |
| All other phases | `Dismissed` |

```text
cargo test -p xai-grok-pager --lib -- ctrl_c_dismisses_
# 2 passed

cargo test -p xai-grok-pager --lib -- views::rewind::tests
# 20 passed

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# clean (lib). --all-targets has pre-existing failures outside this change.
```

## Files changed

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/rewind.rs` | Ctrl+C early dismiss in `handle_rewind_key`; unit tests for picker + all Esc-dismissible phases; ModeSelect bare-`c` vs Ctrl+C |

No git add/commit/push.

## Residual

- **Jump picker idle Ctrl+C** still swallows when no turn is running (only
  special-cases CancelTurn while a turn runs). Same trap class; out of scope
  unless operator asks.
- **Docs:** pager README key table lists Ctrl+C for prompt clear/cancel but not
  rewind dismiss. Optional one-line doc if wanted.
- **Plan soft-park + rewind coexistence:** not a product auto-open bug; no fix.
- **Running turn + rewind open:** Ctrl+C now dismisses overlay only (Esc
  parity). Second Ctrl+C on empty prompt still cancels the turn. No dual
  dismiss+CancelTurn wire-up like `/jump` (would diverge from CancelOffer Esc
  "let it finish").
