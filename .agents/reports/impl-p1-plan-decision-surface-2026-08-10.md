# P1: Plan decision surface (CTAs after present, no empty Enter approve)

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Board:** `impl:p1-plan-decision-surface`
**Plan slice:** Work P1 (plan workflow UX)

## Result

**Shipped.** Soft-park present already auto-opened the side panel with CTAs and
painted **Plan ready. Side panel open** while parked. Dogfood still had two
gaps this pass closed:

1. **Empty Enter on Prompt approved** when the side panel was open (accidental
   approve next to free typing). Q2: remove that path; keep mouse Approve and
   empty-prompt `a`.
2. **Contract tests** that live soft-park status is never **Plan written. Click
   or /view-plan** and that CTAs are armed without an extra open.

Sticky multi-approve, present ≠ Approve tool body, and always-approve ≠ plan
Approve were left unchanged (prior slices).

## Named product contracts

| Situation | Behavior |
|-----------|----------|
| Soft-park present (`exit_plan_mode` soft path) | Park + auto-open side panel + toast; status **Plan ready. Side panel open**; panel footer CTAs (or strip if too narrow) without extra click |
| Live park + draw | Frame must **not** paint **Plan written. Click or /view-plan** |
| Idle arm (plan mode + body, no park yet) | Status may still say idle click cue until surface/self-heal parks (unchanged) |
| Empty freeform Enter (Prompt focus, soft-park or panel) | **No-op** (not approve) when no text, no line comments, no images |
| Empty-prompt `a` with side panel open | **Approve** (also Preview path in line-viewer) |
| Empty-prompt `a` soft-park without panel | **Types** into composer (L1 modal-free; mouse strip owns decide) |
| Mouse Approve / strip / panel footer | Unchanged |
| Freeform Enter with text / comments / images | Still submits under intent (Revise default, Clarify, ApproveNotes) |
| Sticky Approve/Quit | Unchanged |

## Product changes

| File | Change |
|------|--------|
| `app/agent_view/plan.rs` | Empty freeform Enter never `approve_plan`; panel-open empty-prompt `a`/`A`/`?`/`s`/`q` still decide from Prompt focus after present |
| `app/agent_view/render.rs` | Footer: no `Enter:approve` on empty Prompt; panel empty shows `a:approve` + Tab/Esc |
| `docs/user-guide/19-plan-mode.md` | P1 contracts: auto-open CTAs, status copy, empty Enter does not approve |

## TDD

### Red (observed)

`panel_prompt_empty_enter_does_not_approve_but_a_still_does` failed before the
product edit: empty Enter with panel open cleared the park (approved).

`empty_enter_on_prompt_does_not_approve_under_questions_intent` and
`soft_park_present_status_is_plan_ready_not_click_or_view_plan` were already
green against prior soft-park no-op / status wiring; kept as permanent contracts.

### Green (same filters after fix)

| Test | Contract |
|------|----------|
| `panel_prompt_empty_enter_does_not_approve_but_a_still_does` | Panel Prompt empty Enter no-op; empty-prompt `a` approves |
| `empty_enter_on_prompt_does_not_approve_under_questions_intent` | Empty Enter never approve even under Questions intent |
| `soft_park_present_status_is_plan_ready_not_click_or_view_plan` | After soft-park present draw: no idle click ceremony; CTAs armed |

### Commands (exit 0)

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings

cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise view_plan_while \
  live_approve_does_not local_idle_approve_does_not after_revise_idle \
  after_approve_current approved_and_implemented new_exit_plan_mode \
  file_backed_plan soft_park_card_refreshes soft_park_revise \
  empty_enter_on_prompt panel_prompt_empty_enter soft_park_present_status \
  soft_park_empty soft_park_preview_empty soft_park_after_park \
  plan_panel_preview_enter plan_approval_opens_as_side \
  empty_enter_with_image
# 47 passed
```

## Operator dogfood

1. Rebuild/install; quit old Grok windows.
2. Plan mode → agent `exit_plan_mode` present.
3. Expect side panel open with Approve / Notes / Clarify / Revise / Quit **without**
   clicking status or `/view-plan`.
4. Status: **Plan ready. Side panel open** (not **Plan written. Click or /view-plan**).
5. Empty composer: bare **Enter** does nothing. **Mouse Approve** or empty-prompt
   **`a`** approves. Typing freeform then Enter revises by default.
6. Narrow terminal: strip CTAs still clickable; no silent zero chrome.
7. After one Approve: no re-arm of decision chrome until a new present.

## Out of scope (P2+)

- Full revise-in-flight chrome / state machine (P2).
- Dead `require_plan_approval` wire-up.
- Large toast/status/card progressive-disclosure redesign.
- No git add/commit/push.
