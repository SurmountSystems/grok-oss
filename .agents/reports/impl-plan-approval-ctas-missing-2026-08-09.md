# Fix: plan panel missing Approve / Revise CTAs (idle dead end)

**Date:** 2026-08-09
**Package:** `xai-grok-pager`
**Board:** `bug:plan-approval-ctas-missing`

## Result

**Fixed.** When plan mode is still on with a plan body but there is **no live** `exit_plan_mode` reverse-request, the product now parks a **local idle decision** surface: side panel with real **Approve / Notes / Clarify / Revise / Quit** CTAs (same footer chrome as soft-park). View-only keys (`c comment | v select | y copy | Esc close`) no longer replace decision CTAs in that stuck state.

Live soft-park reverse-request path is unchanged.

## Root cause

Operator symptoms matched **idle plan review**, not live soft-park:

| Symptom | Meaning |
|---------|---------|
| Status `Plan written. Click or /view-plan` | `plan_approval_view` was `None` (idle chip) |
| Footer `c:comment \| v:select \| y:copy \| Esc:close` | Casual plan viewer hints |
| Panel bottom only `c comment` | `feedback_active == false` |

Prior work (2026-08-08) auto-opened the side panel on idle plan mode so operators were not stuck on a bare mode badge, but **intentionally** left Approve/Revise off until a live reverse-request. Dogfood after plan write still hit freeform-without-`exit_plan_mode` (or a cleared reverse-request): panel open, prose claiming footer buttons, **no way to approve or revise**.

Soft-park card legend (`Footer buttons: Approve…`) can remain in scrollback from an earlier park, which made the dead end look worse.

## Named product contract

| Situation | Behavior |
|-----------|----------|
| Live soft-park (`response_tx` open) | Unchanged: panel/strip CTAs; turn-end does not wipe |
| Plan mode on + plan body + no reverse-request | **Local idle decision park**: `plan_approval_view` with `is_local_idle_decision`, panel `feedback_active`, Approve/Revise/Quit painted |
| **Approve** (local idle) | `SetPlanMode(Off)` + Interject implement `plan.md` |
| **Revise** (local idle) | Interject: rewrite `plan.md`, then call `exit_plan_mode` again |
| **Quit** (local idle) | `SetPlanMode(Off)` + toast |
| **Clarify** (local idle) | Interject answer-only; ask re-`exit_plan_mode` if still ready |
| Turn-end while local idle + plan mode on | Keep decision park (do not strip CTAs) |
| Plan mode leaves (`CurrentModeUpdate`) | Clear local idle park only (live reverse-request untouched) |
| Real `exit_plan_mode` reverse-request while idle park open | Replaces park as before (stale-cancel of local is a no-op channel) |

No freeform chat "reply approve" menus.

## Product changes

| File | Change |
|------|--------|
| `views/plan_approval_view.rs` | `is_local_idle_decision`, `for_idle_decision()`, `IDLE_PLAN_DECISION_TOOL_CALL_ID`; idle toast names CTAs |
| `app/agent_view/plan.rs` | `surface_idle_plan_review_if_needed` parks local decision + panel CTAs; dismiss preserves local idle; approve/revise/quit/clarify local paths; tests |
| `app/acp_handler/session_notification.rs` | Leaving plan mode clears local idle decision park |

## TDD

**Red contract encoded then greened:**

- `idle_plan_decision_draw_paints_approve_and_revise_ctas` — panel paints Approve + Revise + Quit, not casual `c comment`
- `idle_plan_mode_without_approval_surfaces_review_panel` — parks local decision with `feedback_active` (updated from old "must not invent reverse-request only" open-panel-only contract)
- `idle_plan_approve_leaves_plan_mode_and_interjects_implement`
- `idle_plan_revise_interjects_rewrite_request`
- `turn_end_preserves_local_idle_decision_park`
- Existing soft-park + turn-end reverse-request tests still green

### Commands

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise
# 19 passed

cargo test -p xai-grok-pager --lib -- approve_plan_flush soft_park_revise plan_panel_click
# 101 passed (filter overlap)
```

All of the above: **exit 0**.

## Operator dogfood

1. Rebuild/install and quit old windows.
2. Enter plan mode, have the agent write `plan.md` and finish **without** (or after losing) `exit_plan_mode` reverse-request chrome.
3. Expect: side panel open, footer **a approve · A notes · ? clarify · s revise · q quit** (or compact keys), toast names Approve/Revise/Quit.
4. Click **Approve** → leave plan mode + implement turn. **Revise** → agent rewrite Interject. **Quit** → leave plan mode.
5. True `exit_plan_mode` soft-park still shows the same CTAs over a live reverse-request.

## Out of scope

- No git add/commit/push.
- Did not change shell intercept / reverse-request wire protocol.
- Did not invent freeform chat approval menus.
- Live reverse-request auto-approve / always-approve policy unchanged.
