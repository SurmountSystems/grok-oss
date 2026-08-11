# P4: Docs so plan decision UX + revise loop + green caret survive forks

**Date:** 2026-08-10
**Board:** `impl:p4-docs-fork-survive`
**Plan slice:** Work P4 (docs / recon survival)
**Scope:** docs-only (no product code)

## Result

**Shipped.** Aligned FORK, residual honesty, user-guide, and standing AGENTS
pins with the P1–P3 product contracts. No inventing behavior: every claim
traces to P1–P3 reports and already-shipped code/tests.

## Sources of truth (read for this pass)

| Report | Contracts documented |
|--------|----------------------|
| [`.agents/reports/impl-p1-plan-decision-surface-2026-08-10.md`](impl-p1-plan-decision-surface-2026-08-10.md) | Soft-park auto CTAs; **Plan ready. Side panel open**; empty Enter never approves; empty-prompt `a` with panel |
| [`.agents/reports/impl-p2-revise-loop-chrome-2026-08-10.md`](impl-p2-revise-loop-chrome-2026-08-10.md) | Revising / Waiting status; no idle CTA re-arm; re-present clears in-flight; honest queue toast |
| [`.agents/reports/impl-p3-green-letter-caret-2026-08-10.md`](impl-p3-green-letter-caret-2026-08-10.md) | Mid-letter empty half `text_primary`; solid reverse plate; no neon letter ink |

## Files touched

| Path | Change |
|------|--------|
| `FORK.md` | Product bullet for plan CTAs expanded with P1/P2; new **Plan decision surface (P1)** bullet; soft-park status string exact; P3 labeled on caret bullet + `text_primary`; dogfood handoff + regression filters include P1–P3 |
| `RESIDUAL.md` | Open plan-approval items mark P1–P3 shipped honesty; still-soft remains agent freeform menus only |
| `crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md` | Fixed stale line that said empty Enter still plain-approves; screenshot table matches no-op Enter |
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | Plan approval key cheatsheet; always-approve ≠ plan Approve; caret note names `text_primary` |
| `crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md` | Caret table + prose: empty half is `text_primary` |
| `AGENTS.md` | Surgical: empty Enter never approves; revise wait / no idle re-arm; caret empty half not neon green |

## Contracts mirrored (fork inventory)

1. **Present ≠ Approve** — `exit_plan_mode` / “Plan ready” soft-park is review only. Always-approve is tool permissions only.
2. **P1 decision surface** — auto-open CTAs; status **Plan ready. Side panel open**; empty freeform Enter no-op; mouse Approve or empty-prompt `a`.
3. **Sticky after Approve/Quit** — no re-arm until new present.
4. **P2 revise loop** — **Revising plan...** / **Waiting for updated plan...**; `plan_feedback_in_flight` blocks idle re-arm; honest queue toast; re-present arms once.
5. **P3 caret** — empty half mid-letter = `text_primary`, not neon green ink.
6. **Never freeform chat approve** — product CTAs only (agent `plan.md` freeform remains soft residual).

## Doc bug fixed

User-guide `19-plan-mode` had a leftover sentence:

> Empty Enter with no text, comments, or images still means plain approve.

That contradicted P1 and earlier paragraphs in the same file. Replaced with the
no-op contract and correct Approve paths.

## Out of scope

- Product code / tests (P1–P3 already green).
- Dead `require_plan_approval` wire-up.
- Agent freeform `plan.md` menus (still soft residual).
- Git add / commit / push.

## Verify (docs only)

No `cargo` required. Spot-check:

- FORK plan bullets name P1–P3 and link reports.
- `19-plan-mode` has no “empty Enter … plain approve.”
- Residual open plan items list P1–P3 as shipped; soft = agent freeform only.
