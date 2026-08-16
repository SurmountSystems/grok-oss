# Restore empty-composer Ctrl+C as plan Quit

**Date:** 2026-08-13  
**Package:** `xai-grok-pager`  
**Host spawn:** this L2 session had no `spawn_subagent` tool. Work stayed on L2.

## Named contract

1. When plan approval chrome is open and the composer is empty, Ctrl+C
   abandons like `q` (Quit): dismiss approval, do not send a prompt, do not
   start a turn. `abandon_plan` still sets `plan_decision_resolved` (same as
   Quit).
2. When the composer has text (or images), Ctrl+C keeps the existing
   composer/interrupt behavior. First press clears the draft and leaves
   approval open. Second empty press then abandons.
3. Empty freeform Enter still never approves.
4. This is not a process-wide SIGINT to kill grok-oss.

## TDD

### Red (observed, tests restored first)

Catalog names restored from `origin/main` into
`plan.rs` `plan_approval_ctrl_c_tests`, plus sticky asserts that Quit already
uses.

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ctrl-c-abandon-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-pager --offline --lib -- \
  soft_park_empty_ctrl_c_abandons_plan_approval \
  plan_panel_empty_ctrl_c_abandons_plan_approval \
  plan_approval_ctrl_c_clears_draft_then_second_abandons
```

**Fail reason:** 0 passed, 3 failed.

- `soft_park_empty_ctrl_c_abandons_plan_approval`: `empty Ctrl+C must clear plan_approval_view (not soft-park no-op)`
- `plan_panel_empty_ctrl_c_abandons_plan_approval`: `panel empty Ctrl+C must clear plan approval`
- `plan_approval_ctrl_c_clears_draft_then_second_abandons`: first press cleared the draft, second empty press left `plan_approval_view` set

Plan overlay never called `abandon_plan` on empty-composer Ctrl+C. Preview
`handle_line_viewer_key` returned `Changed` and swallowed the chord.

### Green (same filters after product)

Same command: **3 passed**. Nearby also green:

- `empty_enter_on_revise_prompt_does_not_approve`
- `abandon_plan_optimistically_clears_plan_mode`
- `after_quit_current_mode_clears_pending_still_in_plan_does_not_repark`
- `a_on_empty_revise_prompt_approves`
- `s_on_empty_prompt_decisively_revises`
- `model_picker_during_plan_approval`
- `jump_picker_ctrl_c_cancels_compact`
- `ctrl_c_escalates_to_quit_while_wake_cancel_is_stuck`

`cargo clippy -p xai-grok-pager --offline --lib -- -D warnings`: exit 0.

Did not run `cargo fmt -p xai-grok-pager` (Clear finished writer live).

## Product changes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | Empty-composer Ctrl+C (no images) calls `abandon_plan`. Tests restored. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` | Overlay `CancelTurn` (Ctrl+C) routes into `handle_plan_feedback_key` so Preview does not swallow. F9 / Quit still bubble as `Unchanged`. |

`event_loop.rs` was not needed. `abandon_plan` already sets
`plan_decision_resolved` via `close_plan_review`. Sticky + Revising flags
were left alone.

## Leftover (not this slice)

- Direct `handle_line_viewer_key` still treats in-approval Ctrl+C as `Changed`
  if the overlay router is skipped. Live `handle_input` now intercepts first.
  `viewer.rs` was out of scope.
- Turn-row paint of Revising / Plan ready still lives in `render.rs`.
- Idle local-decision park, honest queue toast, and Revise barren-wait landing
  are still unrestored.

## Out of scope

- No `docs/user-guide/**`, `views/turn_status.rs`, todo pane, `render.rs`,
  settings, prompt_widget caret, spend, welcome, or F9 binds.
- No `git add` / `git commit`.
