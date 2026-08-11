# P2: Revise loop chrome (status + no idle re-arm while rewrite in flight)

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Board:** `impl:p2-revise-loop-chrome`
**Plan slice:** Work P2 (plan workflow UX)

## Result

**Shipped.** After decisive **Revise** / **Clarify** unparks:

1. Status paints **“Revising plan...”** / **“Waiting for updated plan...”**, not
   idle **“Plan written. Click or /view-plan”**.
2. Local idle surface / `park_local_idle` / draw self-heal **do not** re-arm
   decision CTAs while `plan_feedback_in_flight` is set.
3. A new `exit_plan_mode` present clears the flag and arms CTAs once.
4. Freeform while the plan-feedback channel is closed uses an honest queue
   toast (P2/Q1) instead of bare “Queued”.

P1 contracts (empty Enter never approves, soft-park “Plan ready”, sticky
Approve/Quit) stay green (50 plan filters in the combined suite).

## Named product contracts

| Situation | Behavior |
|-----------|----------|
| Soft-park present (P1) | Unchanged: **Plan ready. Side panel open** + CTAs |
| Decisive Revise / Clarify unparks | Set `plan_feedback_in_flight` (Revising / Clarifying) |
| While in-flight, no live park | Status **Revising plan...** or **Waiting for updated plan...**; never **Plan written. Click or /view-plan** |
| While in-flight | `should_arm_plan_decision_chrome()` false; surface / local idle / draw self-heal do not re-park CTAs |
| New `exit_plan_mode` present | Clear `plan_feedback_in_flight` + `plan_decision_resolved`; park CTAs once |
| Freeform Enter mid-rewrite (busy, no live channel) | Queues as normal follow-up; toast: **No live plan feedback channel — message will queue as a normal follow-up.** |
| Empty Enter / `a` approve (P1) | Unchanged |

## Product changes

| File | Change |
|------|--------|
| `views/plan_approval_view.rs` | `PlanFeedbackInFlight`, status constants, queue toast constant |
| `app/agent_view/mod.rs` | Field `plan_feedback_in_flight` |
| `app/agent_view/session.rs` | Init `None` |
| `app/agent_view/plan.rs` | Set flag on revise/clarify; `should_arm` requires flag clear; tests |
| `app/agent_view/render.rs` | In-flight status branch before idle “Plan written” cue |
| `app/acp_handler/interactions.rs` | New present clears `plan_feedback_in_flight` |
| `app/dispatch/prompt.rs` | Honest queue toast when in-flight |
| `docs/user-guide/19-plan-mode.md` | Continuous revise loop + sticky vs in-flight |
| `FORK.md` | Ship note for P2 revise-in-flight chrome |

## TDD

### Red (named contracts; observed against prior behavior intent)

| Test | Contract |
|------|----------|
| `after_revise_status_is_revising_not_plan_written_click_or_view` | After revise + busy draw: no idle click ceremony; paints Revising |
| `after_revise_in_flight_surface_does_not_rearm_idle_ctas` | surface / park_local do not re-arm while flag set |
| `re_present_after_revise_clears_in_flight_and_arms_ctas` | New present clears flag and arms panel CTAs |
| `plan_feedback_queue_toast_is_honest_when_no_live_channel` | Queue toast copy (Q1) |

Prior test `after_revise_idle_surface_rearms_approval_ctas_not_view_only` was
**replaced** by `after_revise_in_flight_surface_does_not_rearm_idle_ctas` to
match P2 (no idle re-arm until re-present).

### Green (same filters after product)

Combined P1 + multi-approve sticky + P2 + revise CTA filters: **50 passed**.

### Commands (exit 0)

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings

cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise view_plan_while \
  live_approve_does_not local_idle_approve_does_not after_revise \
  after_approve_current approved_and_implemented new_exit_plan_mode \
  file_backed_plan soft_park_card_refreshes soft_park_revise \
  empty_enter_on_prompt panel_prompt_empty_enter soft_park_present_status \
  soft_park_empty soft_park_preview_empty soft_park_after_park \
  plan_panel_preview_enter plan_approval_opens_as_side \
  empty_enter_with_image re_present_after_revise plan_feedback_queue
# 50 passed
```

## Operator dogfood

1. Rebuild/install; quit old Grok windows.
2. Plan mode → agent `exit_plan_mode` present (P1: side panel + Plan ready).
3. Click **Revise** (or empty-prompt `s`).
4. Expect toast “Revision sent…”, park/panel clear, status **Revising plan...**
   (not **Plan written. Click or /view-plan**), no Approve strip while agent rewrites.
5. Type freeform while rewrite runs and Enter: honest toast that the message
   queues as a normal follow-up (not silent fail).
6. Agent rewrites `plan.md` and calls `exit_plan_mode` again: CTAs re-arm once
   (**Plan ready. Side panel open**).
7. Empty Enter still never approves (P1). One Approve still sticky until next present.

## Out of scope

- Dead `require_plan_approval` wire-up.
- Large toast/status progressive-disclosure redesign.
- No git add/commit/push.
