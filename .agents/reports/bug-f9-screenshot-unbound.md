# Restore F9 TUI screenshot bind + plan auto-attach

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Slice:** after 1.0.3 restack, F9 was unbound. `/screenshot` and the encoder were already present.

SuperGrok is paid. This report does not discuss included SuperGrok period limits.

## Contract

1. F9 always binds `CaptureTuiScreenshot` (`When::Always`), even when other keymaps own the session.
2. When plan approval is open, capture auto-attaches the PNG to the plan composer.

## TDD red (before product edit)

Tests restored first, then run. Observed fail:

```
cargo test -p xai-grok-pager --lib --offline --no-run -- \
  capture_tui_screenshot_bound_to_f9_always \
  try_attach_tui_screenshot_for_plan_when_approval_open
```

| Test | Fail reason |
|------|-------------|
| `capture_tui_screenshot_bound_to_f9_always` | `ActionId::CaptureTuiScreenshot` not found (`actions/mod.rs` enum) |
| `try_attach_tui_screenshot_for_plan_when_approval_open` | `AgentView::try_attach_tui_screenshot_for_plan` not found |

That is the named contract missing. Encoder was not rebuilt.

## Product restore

Compared to Surmount `origin/main`. Encoder (`xai-grok-pager-render::tui_screenshot`) left alone.

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/actions/mod.rs` | `ActionId::CaptureTuiScreenshot`; named F9 test |
| `crates/codegen/xai-grok-pager/src/actions/defaults.rs` | F9 `When::Always` bind |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | `handle_global_action` maps F9 to `Action::CaptureTuiScreenshot` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `resolve_action` maps the same Action |
| `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` | Plan overlay bubbles F9 like Quit (`InputOutcome::Unchanged`) so Always dispatch still fires |
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | `try_attach_tui_screenshot_for_plan` + named attach/skip tests |
| `crates/codegen/xai-grok-pager/src/app/event_loop.rs` | After present: write PNG, then attach if plan approval is open |
| `crates/codegen/xai-grok-pager/src/views/dashboard/state.rs` | Exhaustive `ActionId` arm (None; global handle owns F9) |

## Green

```
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib --offline -- -D warnings   # exit 0
cargo test -p xai-grok-pager --lib --offline -- \
  capture_tui_screenshot_bound_to_f9_always \
  try_attach_tui_screenshot_for_plan_when_approval_open \
  try_attach_tui_screenshot_skips_when_no_plan_approval
```

3 passed. Same named tests as residual/FORK.

## Leftover

- Did **not** restore origin/main extras: `capture_tui_screenshot_arms_pending_flag`, `capture_tui_screenshot_global_key_maps_to_action`, `attach_screenshot_falls_through_to_parent_when_child_has_no_plan`. Helper + event-loop attach are in; those extra tests are not.
- User-guide still has no `/screenshot` / F9 page (guide is not in `FORK_PATHS`).
- Font raster still soft (simple ink marks). Operator can open the toast path and paste.
- Live TUI is the installed binary, not this source tree, until rebuild.

Stop.
