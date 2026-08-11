# Fix: plan present → decide workflow (stale / double-approve / view-only)

**Date:** 2026-08-10
**Package:** `xai-grok-pager`
**Board:** `bug:plan-workflow-broken`

## Result

**Fixed the remaining double-approve race** after prior CTA and stale-body work
(2026-08-04 / 08-09 / 08-10). One Approve (live soft-park or local idle) now
marks leaving plan mode so turn-end, draw self-heal, and `/view-plan` cannot
immediately re-park a second decision for the same plan. Status idle cue uses
the same effective plan-mode flag.

Stale body (B1), awaiting-decision CTAs (B3), and Revise re-arm (B4) were
already largely green from prior slices; this pass ties them together with the
approve race and documents the full lifecycle.

---

## Phase A — Lifecycle inventory

### Present paths

| Path | Entry | What parks | Body source |
|------|--------|------------|-------------|
| Live soft-park | Shell `exit_plan_mode` → `x.ai/exit_plan_mode` → `handle_exit_plan_mode` | `PlanApprovalViewState` + `response_tx` | Request snapshot; FileBacked SoT re-reads session `plan.md` on open/paint/CTA |
| Local idle decision | Turn-end (`prompt` / reconcile / `turn_completion`) → `surface_idle_plan_review_if_needed` | `for_idle_decision` (no `response_tx`) | `plan_body_for_preview` (disk / latest inline) |
| `/view-plan` / status / ShowPlan | `show_plan_preview` | Parks local idle **if** effectively in plan mode + body + no park yet | Same resolve; FileBacked re-read first |
| Draw self-heal | `sync_plan_viewer_approval_chrome` each paint | Parks local idle if idle turn + open casual panel + effectively in plan | Refreshes open FileBacked panel if disk diverged |

### Decide handlers

| CTA | Live reverse-request | Local idle |
|-----|----------------------|------------|
| **Approve** | ACP `approved`; shell runs tool / leaves plan mode | `SetPlanMode(Off)` + implement Interject |
| **Notes** | Focus prompt → approve + freeform | Same (via approve path after notes) |
| **Clarify** | ACP `questions`; stay in plan mode | Interject answer-only; ask re-`exit_plan_mode` |
| **Revise** | ACP `cancelled` immediately; stay in plan mode | Interject rewrite + re-`exit_plan_mode` |
| **Quit** | ACP abandon; leave plan mode | `SetPlanMode(Off)` + toast |

### Mode exit vs chrome clear

| Event | Plan mode | Approval chrome |
|-------|-----------|-----------------|
| Approve / Quit | `plan_mode_pending = Some(false)` immediately; shell `CurrentModeUpdate` later sets `plan_mode_active` / clears pending | Park + panel cleared on decision |
| Revise / Clarify | Stay in plan mode | Park cleared; turn-end may re-surface idle CTAs if no new reverse-request |
| `CurrentModeUpdate` leave | `plan_mode_active = false`, pending `None` | `clear_local_idle_plan_decision_if_any` only (live reverse-request untouched) |
| Turn-end | Unchanged | Keep live `response_tx` and local idle while effectively in plan; strip other leftovers |

### Paths that were still broken (this pass)

| Symptom | Root cause |
|---------|------------|
| **Double approve / re-park** | After Approve, park cleared but `plan_mode_active` stayed true until shell update. Decision paths used `plan_mode_active \|\| pending == Some(true)` (true while leaving). Live Approve did not set `plan_mode_pending = Some(false)` (local idle did). Turn-end / draw / `/view-plan` re-parked CTAs. Status still painted "Plan written. Click or /view-plan" via raw `plan_mode_active`. |
| **View-only while awaiting** | Prior 2026-08-10 work fixed `/view-plan` + draw self-heal; kept green. |
| **Stale body** | Prior FileBacked re-read + shell batch split; kept green. |
| **Revise stuck comment-only** | Prior decisive Revise + idle re-surface; confirmed re-arm after Revise. |

---

## Phase B/C — Contracts and product fix

### Named contracts

| ID | Contract | Status |
|----|----------|--------|
| **B1** | Open / re-present shows current disk `plan.md` for FileBacked (not forever-frozen snapshot) | Green (prior + re-verified) |
| **B2** | One Approve leaves approval chrome; no immediate second park without new present | **Fixed this pass** |
| **B3** | While effectively in plan mode and a body exists, decision CTAs not comment-only | Green (prior + re-verified) |
| **B4** | Revise unparks decisively; idle re-surface re-arms CTAs (not stuck comment-only) | Green (prior + new re-arm test) |

### Product changes

| File | Change |
|------|--------|
| `app/agent_view/plan.rs` | `effectively_in_plan_mode()` = `plan_mode_pending.unwrap_or(plan_mode_active)`. Park / surface / keep-local / draw self-heal use it. **Every** `approve_plan` sets `plan_mode_pending = Some(false)`. New B2/B4 tests. |
| `app/agent_view/render.rs` | Idle status cue uses `effectively_in_plan_mode()` so post-Approve does not re-invite "Plan written…" |

Shared idea (as requested): one effective-mode helper for "ensure approval chrome" gates; Approve is the shared "dismiss after decision / leaving mode" signal; FileBacked re-read on open/paint remains the disk SoT path.

---

## TDD evidence

### New (this pass)

| Test | Contract |
|------|----------|
| `live_approve_does_not_repark_decision_while_plan_mode_clearing` | B2 live: after Approve, surface / show / draw do not re-park while `plan_mode_active` still true |
| `local_idle_approve_does_not_repark_while_plan_mode_clearing` | B2 local idle: same race window |
| `after_revise_idle_surface_rearms_approval_ctas_not_view_only` | B4: after Revise, idle surface re-arms CTAs |

### Re-verified

`view_plan_while_plan_mode_awaiting_decision_parks_ctas_not_view_only`,
`idle_plan_view_only_panel_draw_self_heals_to_approval_ctas`,
`idle_plan_*`, `turn_end_*`, `soft_park_draw_*`, `file_backed_plan_*`,
`soft_park_card_refreshes_*`, `soft_park_revise_*`, `exit_plan_mode_soft_*`

### Commands

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0

cargo test -p xai-grok-pager --lib -- \
  idle_plan soft_park_draw exit_plan_mode_soft turn_end_preserves \
  turn_end_clears turn_end_stale plan_approval_status_label \
  idle_plan_decision idle_plan_approve idle_plan_revise view_plan_while \
  live_approve_does_not local_idle_approve_does_not after_revise_idle \
  file_backed_plan soft_park_card_refreshes soft_park_revise
# 31 passed, exit 0
```

---

## Operator dogfood

1. Rebuild/install; quit every old Grok window.
2. Plan mode → agent writes `plan.md` and presents (`exit_plan_mode` or freeform finish).
3. Expect side panel with **Approve / Notes / Clarify / Revise / Quit**, not only `c comment`.
4. Click **Approve once** → panel clears, implement starts; **no** immediate second "Plan ready" park or Approve strip for the same plan.
5. If status still said "Plan written…" before this build, after Approve it must not reappear until a new present.
6. **Revise** → toast "Revision sent…"; after rewrite / idle without present, CTAs re-arm (or new soft-park from `exit_plan_mode`).
7. Rewrite `plan.md` while FileBacked park is open → panel body tracks disk on paint (B1).

---

## Out of scope

- No git add/commit/push.
- Shell reverse-request wire / batch split unchanged this pass (already green).
- No freeform chat "reply approve" menus.
- Tool result copy that says "plan has been approved" before UI decision remains soft residual (`bug:exit-plan-mode-false-approve`) if still noisy.
- Always-approve / force-modal park settings unchanged.
