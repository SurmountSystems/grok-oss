# Bug fix: Ctrl+C during plan approval soft-park

**Date:** 2026-08-07
**Package:** `xai-grok-pager`
**Board:** `bug:plan-approval-ctrl-c`

## Result

**Fixed.** Empty-composer Ctrl+C while plan approval is open (soft-park or side panel) now **abandons** plan approval (same outcome as panel `q` / soft-park mouse Quit). Non-empty Ctrl+C still clears the draft first; a second empty Ctrl+C then abandons.

## Root cause

Two sinks swallowed the chord:

1. **Soft-park** (`handle_plan_feedback_key`): empty Ctrl+C reached `prompt.handle_key` → `PromptEvent::Ignored` → mapped to `InputOutcome::Changed`, so it never bubbled to global quit and never abandoned the park.
2. **Side panel Preview** (`handle_line_viewer_key`): explicit branch
   `if in_plan_approval { return InputOutcome::Changed; }` for Esc/`q`/Ctrl+C **no-op swallowed** Ctrl+C (empty `q` was already handled earlier as CTA; Ctrl+C had no path).

`Esc` stays focus step-back (not abandon). Soft-park bare `q` still types into the composer (mouse Quit / empty panel `q` abandon).

## Named contract

While plan approval is active:

| Composer | Ctrl+C |
|----------|--------|
| Empty (no images) | Abandon plan approval (`outcome: "abandoned"`) |
| Non-empty draft | Clear draft; keep park; second empty Ctrl+C abandons |

## Product change

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | Early empty Ctrl+C → `abandon_plan()` in `handle_plan_feedback_key` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` | Panel path: Ctrl+C routes to `handle_plan_feedback_key` instead of no-op |

## TDD

**Red (observed):** three new tests failed — park remained open on empty Ctrl+C (soft-park + panel); second empty after clear also failed to abandon.

**Green:** same tests pass.

### New tests

- `soft_park_empty_ctrl_c_abandons_plan_approval`
- `plan_panel_empty_ctrl_c_abandons_plan_approval`
- `plan_approval_ctrl_c_clears_draft_then_second_abandons`

### Commands

```bash
# Red→green filter
cargo test -p xai-grok-pager --lib -- \
  soft_park_empty_ctrl_c_abandons \
  plan_panel_empty_ctrl_c_abandons \
  plan_approval_ctrl_c_clears_draft

# Broader regression (88 passed)
cargo test -p xai-grok-pager --lib -- \
  plan_approval soft_park abandon_plan plan_panel_empty soft_park_empty_ctrl

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
# --all-targets has pre-existing fails outside this change (benches/other tests)
```

## Out of scope / not done

- No git add/commit/push.
- Did not map Ctrl+C to app-level double-press quit while park is open (prefer quit-the-park like panel `q`).
- Soft-park bare `q` remains non-capturing by design.
