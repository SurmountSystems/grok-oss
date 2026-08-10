# Fix: plan multi-approve still broken after claimed B2 fix

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Board:** `bug:plan-multi-approve-still-broken`
**Prior report:** `.agents/reports/impl-plan-workflow-broken-2026-08-10.md`

## Result

**Prior B2 fix was in tree and green** (`plan_mode_pending = Some(false)` on
Approve / Quit; `effectively_in_plan_mode()` gates park). That only covers the
race while pending stay holds. Dogfood still re-parked because every shell
`CurrentModeUpdate` **clears** `plan_mode_pending` to `None`. If plan mode is
still active (or the session never left plan mode after implement), effective
mode becomes true again and turn-end / draw / `/view-plan` re-arm Approve for
the same `plan.md` (including bodies that say "approved and implemented").

**This pass:** sticky session flag `plan_decision_resolved`. Set on decisive
Approve / Quit. Cleared only on a new `exit_plan_mode` soft-park present.
Decision chrome (local idle park, turn-end surface, draw self-heal, idle status
cue) uses `should_arm_plan_decision_chrome()` =
`effectively_in_plan_mode() && !plan_decision_resolved`.

## Verify: prior fix

| Check | Result |
|-------|--------|
| `approve_plan` / `abandon_plan` set `plan_mode_pending = Some(false)` | Present |
| `effectively_in_plan_mode()` used for park / surface / draw self-heal / status | Present |
| Tests `live_approve_does_not_repark_*`, `local_idle_approve_does_not_repark_*` | **Green** (re-ran) |
| Gap | Pending cleared by `detect_plan_mode_change` while `plan_mode_active` still true → re-park |

Honest dogfood note: if the operator was still on a binary built **before** the
prior B2 slice, that alone can show multi-approve. The ~9:06 symptoms also match
this **remaining code gap** after B2 (status `Plan ready. Side panel open` +
footer Enter:approve after agent already got "The user approved the plan...").
Rebuild/install is still required for either fix to show up in a live window.

## Root cause (remaining)

```
Approve → plan_mode_pending=Some(false), park cleared
  → implement turn / "Already done"
  → CurrentModeUpdate: plan_mode_pending=None, plan_mode_active may stay true
  → effectively_in_plan_mode() == true again
  → surface_idle / sync_plan_viewer / show_plan_preview park local idle CTAs
  → "Plan ready. Side panel open" + Enter:approve strip for same plan.md
```

Disk line `Workflow status: approved and implemented` does not block
`plan_preview_available()`; body presence alone was enough to re-park.

## Product changes

| File | Change |
|------|--------|
| `app/agent_view/mod.rs` | Field `plan_decision_resolved: bool` |
| `app/agent_view/session.rs` | Init `false` |
| `app/agent_view/plan.rs` | `should_arm_plan_decision_chrome()`; park / surface / draw self-heal / keep-local-idle use it; Approve + Quit set sticky true; tests |
| `app/agent_view/render.rs` | Idle "Plan written" status cue uses decision-chrome gate |
| `app/acp_handler/interactions.rs` | New `exit_plan_mode` present clears sticky (`plan_decision_resolved = false`) |

Revise / Clarify do **not** set sticky (still need re-arm while awaiting rewrite
or answers). New soft-park present re-arms.

## TDD

### New / extended

| Test | Contract |
|------|----------|
| `after_approve_current_mode_clears_pending_still_in_plan_does_not_repark` | Soft-park → Approve → pending cleared + still active → surface/draw/show: no park, no Plan ready status, no soft-park Approve strip |
| `approved_and_implemented_plan_body_does_not_repark_after_decide` | Body contains "approved and implemented" → after decide, idle surface must not re-park |
| `new_exit_plan_mode_present_clears_decision_resolved_and_parks` | New present after decide re-arms CTAs |
| Extended B2 live/local approve tests | Assert sticky + no status / CTA strip after draw |

### Commands (all exit 0)

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings

cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise view_plan_while \
  live_approve_does_not local_idle_approve_does_not after_revise_idle \
  after_approve_current approved_and_implemented new_exit_plan_mode \
  file_backed_plan soft_park_card_refreshes soft_park_revise
# 34 passed
```

## Operator dogfood

1. **Rebuild and install** this tree; quit every old Grok window (old binary
   will still multi-approve).
2. Plan mode → agent presents (`exit_plan_mode`) → side panel CTAs.
3. Approve **once** → panel clears; implement / agent reply runs.
4. After turn end and paint: **no** yellow `Plan ready. Side panel open`, **no**
   Approve/Revise/Quit strip, **no** `Enter:approve` soft-park footer for that
   plan. Status must not re-invite a second decision.
5. Optional: leave `Workflow status: approved and implemented` on disk; idle
   plan mode must still stay quiet until a **new** `exit_plan_mode` present.
6. New present (agent re-calls `exit_plan_mode`) → CTAs re-arm normally.
7. Revise still unparks and can re-surface idle CTAs while awaiting rewrite.

## Out of scope

- No git add/commit/push.
- Tool result copy that claims "plan approved" before UI decision
  (`bug:exit-plan-mode-false-approve`) unchanged.
- Always-approve / force-modal park settings unchanged.
