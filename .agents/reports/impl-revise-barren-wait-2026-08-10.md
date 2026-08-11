# Revise barren-wait landing (R1–R6)

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Branch:** `fixes-2`
**Report path:** `.agents/reports/impl-revise-barren-wait-2026-08-10.md`

## Result

Shipped. After decisive **Revise** / **Clarify**, the operator is no longer
left in a barren “Waiting + Enter:queue” surface with no human line and no
agent activity chrome.

## Root cause

Three product gaps stacked on top of P2 (`plan_feedback_in_flight`):

1. **Exclusive status while busy.** While `plan_feedback_in_flight` was set,
   the turn-status branch always painted only **“Revising plan...”** and
   cleared cancel/pause/activity hits. When the agent was actually rewriting
   (thinking/tools), that chrome was hidden. Felt like forever Waiting with
   nothing happening.

2. **Ghost composer draft → Enter:queue.** Revise restored the pre-panel
   `stashed_prompt` (often “original chat” after force-modal / reopen). That
   non-empty draft made `can_send()` true while the rewrite turn was busy, so
   the footer showed **Enter:queue** even though the operator had not typed a
   follow-up. Soft-park empty stash avoided this only sometimes.

3. **No human line on empty Revise.** Scrollback only pushed a user line in
   minimal mode when freeform was non-empty. Bare mouse Revise left a barren
   transcript until the agent spoke.

P2 itself (no idle “Plan written. Click or /view-plan” re-arm mid-rewrite)
stayed correct and is preserved.

## Named product contracts

| Id | Contract | Behavior after fix |
|----|----------|--------------------|
| **R1** | No barren wait after decisive Revise | Human line always; empty composer; idle chip or busy activity chrome |
| **R2** | Freeform-first revise path | Product remains **decisive** Revise (notes optional). Composer is for normal chat after unpark, not plan-feedback queue mode |
| **R3** | Immediate kick + honest busy chrome | Live ACP `cancelled` when channel open; local idle / dead channel **Interject** rewrite; busy turn paints real turn status (tools/cancel); generic Waiting overlays **Revising plan...** |
| **R4** | P2 in-flight | `plan_feedback_in_flight` still blocks idle CTA re-arm; re-present clears flag |
| **R5** | Caret residue | Composer cleared after revise (`set_text("")`); empty insertion caret is Human green block only (P3 still green). No ghost draft letter mid-line |
| **R6** | Re-present | Unchanged: new `exit_plan_mode` clears in-flight and arms CTAs |

P1 (empty Enter never approves; present ≠ approve; sticky multi-approve) unchanged.

## Product changes

| File | Change |
|------|--------|
| `views/plan_approval_view.rs` | `PLAN_REVISE_HUMAN_LINE`, `PLAN_CLARIFY_HUMAN_LINE` constants |
| `app/agent_view/plan.rs` | Always push human scrollback line; clear composer (drop stash); Interject when local idle **or** ACP send failed; tests for R1/R3 |
| `app/agent_view/render.rs` | Idle in-flight → Revising chip; busy in-flight → real `render_turn_status` (+ Revising overlay on generic Waiting); `plan_status_cue` includes in-flight |
| `docs/user-guide/19-plan-mode.md` | Continuous revise loop: human line, clear composer, busy activity chrome |
| `FORK.md` | Ship bullet + report link |

## TDD (red → green)

### New / updated tests (named contracts)

| Test | Contract |
|------|----------|
| `after_revise_empty_always_pushes_human_scrollback_line` | R1 human line + empty composer / not sendable |
| `after_revise_busy_turn_keeps_cancel_activity_chrome` | R3 busy chrome, no idle Plan written, no Approve re-arm |
| `after_revise_clears_composer_no_ghost_stash_draft` | R1 no ghost stash → no Enter:queue bait |
| `after_revise_dead_channel_interjects_rewrite` | R3 kick when no live channel |
| `send_plan_feedback_with_screenshot_returns_interject_images` | Updated: composer empty after revise (was restored “original chat”) |
| Prior P2: `after_revise_status_is_revising_not_plan_written_click_or_view`, `after_revise_in_flight_surface_does_not_rearm_idle_ctas`, `re_present_after_revise_clears_in_flight_and_arms_ctas` | R4/R6 still green |

### Red evidence (intent)

Before product edits, the barren contracts were encoded against prior behavior:

- Restoring stash left non-empty `prompt` after revise with `install_plan_approval` stash (“original chat”) → `can_send()` true mid-rewrite.
- Exclusive in-flight status branch set `hit_cancel_button = None` while `TurnRunning`.
- Empty revise did not push `UserPrompt` outside minimal mode.

### Green commands (exit 0)

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings

cargo test -p xai-grok-pager --lib -- \
  after_revise empty_always_pushes busy_turn_keeps clears_composer dead_channel \
  soft_park_revise_cta send_plan_feedback_with_screenshot after_revise_status \
  re_present_after_revise plan_feedback_queue idle_plan_revise
# 14 passed

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
  empty_enter_with_image re_present_after_revise plan_feedback_queue \
  send_plan_feedback_with_screenshot send_plan_questions_with
# 56 passed
```

## Operator dogfood

1. Rebuild/install; quit old Grok windows.
2. Plan mode → `exit_plan_mode` present → click **Revise** (empty freeform).
3. Expect: toast “Revision sent…”, human line **“Revise the plan”** in scrollback,
   empty composer (no Enter:queue), status **Revising plan...** until the turn
   is busy, then thinking/tools/cancel chrome (not exclusive barren wait).
4. Agent re-presents: CTAs arm once (**Plan ready. Side panel open**).
5. Empty Enter still never approves (P1).

## Residual

- Pre-panel draft is **not** restored after Revise/Clarify (tradeoff vs Enter:queue
  ghost). Approve still restores stash. If dogfood wants draft recovery after
  rewrite finishes, park a soft residual to restore stash only on re-present.
- Agent-written `plan.md` freeform “reply approve” menus remain soft residual
  (product chrome does not invent them).
- No git add/commit/push.
