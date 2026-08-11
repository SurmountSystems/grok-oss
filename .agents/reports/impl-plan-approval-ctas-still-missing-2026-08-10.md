# Fix: plan panel still missing Approve / Revise after "Plan written"

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Board:** `bug:plan-approval-ctas-still-missing`

## Result

**Fixed.** The prior idle-decision park (2026-08-09) only armed CTAs when
`surface_idle_plan_review_if_needed` ran at turn-end. Dogfood still hit
**view-only** chrome because:

1. Status painted `Plan written. Click or /view-plan` whenever
   `plan_approval_view` was **None** (plan mode on + plan body).
2. That status invites **status click** / **`/view-plan`**, which called
   `show_plan_preview()` with **no** park → casual footer
   (`c comment | v select | y copy | Esc close`).
3. Lost-RPC **turn reconcile** never called surface/park (only the
   PromptResponse path did).

Product now parks a **local idle decision** (real Approve / Notes / Clarify /
Revise / Quit) on every open path that still needs a decision, and **self-heals
on draw** when an open plan panel is casual while plan mode is idle and a body
exists.

## Operator dogfood matched

| Symptom | Meaning |
|---------|---------|
| Status `Plan written. Click or /view-plan` | `plan_approval_view == None` (idle status branch) |
| Footer only `c comment` / bottom chrome comment keys | Casual viewer (`feedback_active == false`) |
| Side panel open with plan body | `show_plan_preview` without park |
| Right badge `· plan` | `plan_mode_active` still true |
| Transcript "Plan mode exited" / "Plan approved…" | Tool title/result copy from `exit_plan_mode` pipeline; **not** proof UI CTAs were live |

Prior report claimed done only for explicit `surface_idle_plan_review_if_needed`
call sites. It did **not** cover `/view-plan` / status click, draw self-heal, or
turn reconcile.

## Named product contract

| Situation | Behavior |
|-----------|----------|
| Live soft-park (`response_tx` open) | Unchanged: panel/strip CTAs |
| Plan mode on + plan body + no park + **`show_plan_preview`** (`/view-plan`, status, ShowPlan) | Park local idle decision **before** open; `feedback_active`; Approve/Revise/Quit |
| Plan mode on + plan body + **open casual panel** + turn idle | Draw `sync_plan_viewer_approval_chrome` parks + arms CTAs (no toast spam) |
| Turn-end PromptResponse **and** lost-RPC reconcile | `dismiss_stale` + `surface_idle_plan_review_if_needed` |
| Approve / Revise / Quit (local idle) | Unchanged: leave plan mode / Interject / abandon |
| Plan mode leaves (`CurrentModeUpdate`) | Clear local idle only (live reverse-request untouched) |
| Casual `/view-plan` when **not** in plan mode | Still view-only comment chrome (no false CTAs) |

View-only remains only after a real decision or plan mode abandoned with no
pending approval.

## Product changes

| File | Change |
|------|--------|
| `app/agent_view/plan.rs` | `park_local_idle_plan_decision_if_needed`; `show_plan_preview` parks first; `sync_plan_viewer_approval_chrome` self-heals open view-only when idle; `surface_idle` uses shared park helper |
| `app/dispatch/turn.rs` | Turn-end reconcile calls `dismiss_plan_approval_after_turn_if_stale` + `surface_idle_plan_review_if_needed` |

## TDD

**Red contracts encoded then greened:**

- `view_plan_while_plan_mode_awaiting_decision_parks_ctas_not_view_only` —
  exact dogfood open path (status / `/view-plan`): parks + paints Approve /
  Revise / Quit, not `c comment`
- `idle_plan_view_only_panel_draw_self_heals_to_approval_ctas` —
  panel already open casual with no park; draw self-heals CTAs
- Prior idle / soft-park / turn-end tests remain green

### Commands

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0

cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise \
  view_plan_while casual_view_plan
# 23 passed, exit 0
```

## Operator dogfood

1. Rebuild/install; quit old windows.
2. Enter plan mode; agent writes `plan.md` and finishes (with or without a live
   `exit_plan_mode` reverse-request).
3. If status is still `Plan written. Click or /view-plan`, click it or run
   `/view-plan` → expect footer **a approve · A notes · ? clarify · s revise ·
   q quit** (or compact keys), **not** only `c comment`.
4. If a casual panel was already open, next redraw should arm the same CTAs.
5. **Approve** → leave plan mode + implement Interject. **Revise** → rewrite
   Interject. **Quit** → leave plan mode.
6. True soft-park reverse-request still shows the same CTAs over a live waiter.

## Out of scope

- No git add/commit/push.
- Did not change shell intercept / reverse-request wire protocol.
- Did not invent freeform chat approval menus.
- Live reverse-request auto-approve / always-approve policy unchanged.
- Tool result text that says "plan has been approved" before the UI decision
  is separate model/tool copy; not reworked here.
