# Fix: plan mode stuck after planning (no CTAs / dead end)

**Date:** 2026-08-08
**Package:** `xai-grok-pager`
**Branch:** fixes-2

## Result

**Fixed (discoverability + idle dead-end).** Soft-park reverse-request chrome was already green when `exit_plan_mode` parks. The dogfood stuck state was **plan mode still on, turn idle, no `plan_approval_view`**, so footer showed only Shift+Tab / Ctrl+e / Ctrl+. with no Approve/Revise/Quit and no clear way to open the panel.

## Root cause

Two layers:

1. **Product soft-park works only after a live reverse-request** (`x.ai/exit_plan_mode` → `handle_exit_plan_mode`). Agent freeform like "Waiting on your plan panel decision…" does **not** open approval CTAs. Mode badge stays **plan**; turn can finish ("Worked for …"); composer is idle normal hints.

2. **Idle turn status row was zero height**, so even a "plan written" status chip could not paint after the turn ended. Soft-park status works while the tool is mid-await (turn not idle). After freeform turn-end, there was no status row and no auto-open.

Real Approve/Revise/Quit still require the agent to call `exit_plan_mode` (shell intercept + reverse-request). This fix stops the **dead end**: auto-open review panel, toast, and clickable status.

## Named contracts

| Situation | Product behavior |
|-----------|------------------|
| Live soft-park (`plan_approval_view` + open `response_tx`) | Unchanged: side panel CTAs / strip CTAs; turn-end does **not** wipe |
| Idle plan mode + plan body + no reverse-request | Auto-open side panel, Prompt stays focused, toast, clickable status "Plan written. Click or /view-plan" |
| Live soft-park | Idle-review surface is a no-op (does not replace CTAs/toast) |
| Stale approval cleared on turn-end, plan mode still on | After clear, idle-review re-opens panel + toast |

## Product changes

| File | Change |
|------|--------|
| `views/plan_approval_view.rs` | `PLAN_IDLE_REVIEW_TOAST`, `PLAN_IDLE_REVIEW_STATUS` |
| `app/agent_view/plan.rs` | `surface_idle_plan_review_if_needed`; `plan_preview_available` pub(crate) |
| `app/dispatch/prompt.rs` | After `dismiss_plan_approval_after_turn_if_stale`, call surface |
| `app/turn_completion.rs` | Same dismiss + surface on viewer finalize |
| `app/agent_view/render.rs` | Force turn-status row when plan approval parked **or** idle plan + plan body; paint idle review status chip |

## TDD

**Red (observed):** `idle_plan_mode_draw_paints_clickable_review_status` failed until turn-status height included plan status cue (row was 0 when idle).

**Green:** same tests pass.

### New tests

- `idle_plan_mode_without_approval_surfaces_review_panel`
- `idle_plan_review_surface_skips_when_approval_parked`
- `turn_end_stale_clear_then_surfaces_idle_plan_review`
- `idle_plan_mode_draw_paints_clickable_review_status`
- toast/status string contracts on `plan_approval_status_label_distinguishes_empty`

### Commands

```bash
cargo test -p xai-grok-pager --lib -- \
  idle_plan_mode surface_idle soft_park_draw exit_plan_mode_soft \
  turn_end_preserves turn_end_clears turn_end_stale plan_approval_status_label

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
just install
```

## Out of scope / not done

- No git add/commit/push.
- Did not invent fake Approve CTAs without a reverse-request (would lie about shell intercept).
- Did not change Escape double-confirm (parallel agent); stayed off event_loop key race paths.
- Agent freeform inventing "use plan panel CTAs" without calling `exit_plan_mode` remains process soft residual (product now surfaces review + how to leave).
