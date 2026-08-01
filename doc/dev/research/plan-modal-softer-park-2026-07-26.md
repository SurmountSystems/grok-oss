# Plan approval modal — softer park (UDAX residual)

Date: 2026-07-26  
Class: D3 design note.  
Track: `feat:plan-modal-softer-park` · `RESIDUAL.md` #2.

## Problem

When the agent enters plan mode, the **plan approval surface is modal**: it
takes over the decision chrome (four CTAs + quit after F residual). Operators
report that this **jars when unexpected** — especially when plan mode was not
explicitly requested, or when a parked plan reappears after an interject /
queue event.

F residual fixed the **CTA set** (approve / approve w/ comment / clarify /
revise / quit; no primary Comment). It deliberately **did not** redesign the
modal vs non-modal shell.

## Acceptance criteria (for a later deliberate pass)

1. **No surprise hard takeover** when the user did not expect plan mode —
   either soft chrome, toast, or a dismissible banner that parks without
   blocking the prompt.
2. **Four CTAs remain reachable** once the user engages the plan surface
   (do not regress F).
3. **Clarify (`?`) stays read-only** on the plan (ACP `"questions"`; plan stays
   Active) — unchanged wire.
4. **Park / abandon** still durable (plan file + session state); softer UX must
   not drop an approved plan.
5. Tests: pager plan-approval / focus_plan filters stay green; add at least one
   test for the new non-blocking park indicator if shipped.

## Options (not chosen this residual run)

| Option | Effort | Notes |
|--------|--------|-------|
| **A. Status chrome toast** | S | “Plan parked — press … to review” non-modal; approval still modal on demand |
| **B. Side panel / drawer** | M | Plan lives beside chat; CTAs in panel footer |
| **C. Inline plan card** | M–L | Card in transcript with CTAs; no full takeover |
| **D. Config: modal vs soft** | S+ | Default soft for unexpected; modal when `/plan` explicit |

Recommended first ship slice: **A** (toast + status line) with existing modal
reachable via key / click — smallest surprise reduction without redesigning
plan mode.

## Out of scope this pass

- Full non-modal redesign
- Changing ACP plan outcomes (`approved` / `cancelled` / `questions` / …)
- Renaming `send_now_*` symbols
- Onto / import / human commit work

## Status

**Option A shipped** (2026-07-26 implement residual): soft park on
`exit_plan_mode` — durable `plan_approval_view` + toast
(`PLAN_PARKED_TOAST`) + status label; no auto line-viewer; prompt preserved;
competing overlays left alone until `reopen_plan_approval`. Modal still on
demand (`/view-plan`, status click, `ShowPlan`). Tests:
`exit_plan_mode_soft_parks_with_toast_not_modal` and related plan_mode /
`plan_approval_status_label` filters.

**Still open residual:** options B/C/D and full non-modal plan redesign.

**Related (Wave 0b P1–P4, 2026-07-26):** selection → agent context shipped —
revise/clarify feedback carries `@plan.md:N` / `N-M` + quoted line text
(comments and freeform-with-selection, including multi-line). Screenshots on
the plan prompt ride Interject with revise/clarify/approve. User-guide
`19-plan-mode` documents all four.
