# FORK docs handoff for dogfood wave (2026-08-09)

**Scope:** docs only (`FORK.md`, short `RESIDUAL.md` Open pointer). No product code.
**No git add/commit/push.**

## Goal

Next task / recon / dogfood session can open FORK and know what this wave
shipped, what is still open, and which reports to read first.

## Verified before claiming shipped

| Item | Evidence | Status in docs |
|------|----------|----------------|
| Plan Revise decisive (ACP cancel) | `impl-plan-revise-stuck`, `impl-plan-panel-revise-test`; `request_plan_revise` in tree | Shipped |
| Same-batch plan write + `exit_plan_mode` | `impl-plan-stale-after-exit-plan-mode`; `split_tool_batch_before_exit_plan_mode` | Shipped |
| OAuth 403 bad-credentials → auth | `impl-oauth-403-bad-credentials`; `is_credentials_rejected_message` | Shipped |
| Rewind skip missing intermediate checkpoints | `impl-rewind-compaction-checkpoint-missing`; replay test | Shipped |
| Ctrl+C dismisses rewind like Esc | `impl-ctrl-c-rewind-picker` | Shipped |
| Pause/stop chrome `[pause]`/`[stop]` | `impl-work-b-pause-stop-chrome`, `impl-pause-stop-verify-or-fix` | Shipped |
| Soft stop chord-only (no button) | Work B reports | Shipped (chord); button not shipped |
| Composer Enter cue | `impl-work-a-composer-enter-cue`; `enter_prompt_mode` | Shipped |
| Meters `intent · N%` + Team settlement | `impl-work-c-meters-chrome` (already in FORK free-period bullet) | Shipped |
| Auto-resume after error terminal | **No** report at `impl-rebuild-auto-resume-after-error-2026-08-09.md` | **In flight / expected only** |

## Sections touched

### `FORK.md`

| Location | Change |
|----------|--------|
| Product · Soft interject | Expanded with Work A Enter cue honesty + report link |
| Product · Plan approval CTAs | Decisive Revise + report links |
| Product · new bullet | Same-batch plan write + exit_plan_mode |
| Product · Fearless global pause | Status `[pause]`/`[resume]` + Work B reports |
| Product · Soft stop | Split into hard `[stop]` + soft-stop chord; no soft-stop button |
| Product · new bullets | Ctrl+C rewind; rewind missing checkpoints; OAuth 403 bad-credentials |
| Product · new subsection | **Dogfood / next session handoff (2026-08-09)** — install gate, shipped list, in-flight, residual opens, wave regression filters |

### `RESIDUAL.md`

| Location | Change |
|----------|--------|
| Open (top) | Short **Dogfood / next-session gate** pointer to FORK handoff + D0 checklist; marks auto-resume-after-error in flight; soft-stop button / mid-sample freeze not shipped |

## What next-session should read first

1. [`FORK.md`](../../FORK.md) § **Dogfood / next session handoff (2026-08-09)**
2. [`.agents/reports/d0-dogfood-checklist-2026-08-09.md`](d0-dogfood-checklist-2026-08-09.md) (install + full quit/reopen)
3. [`.agents/reports/impl-remaining-plan-wave-2026-08-09.md`](impl-remaining-plan-wave-2026-08-09.md) (A/B/C/E package map)
4. Per-feature reports linked from FORK product bullets when debugging one seam
5. [`RESIDUAL.md`](../../RESIDUAL.md) Open for post-dogfood process features + C4

## Explicit non-goals

- Did not claim auto-resume-after-error as shipped
- Did not invent mid-sample freeze or soft-stop button as product
- Did not dump recon diaries into D1
- Did not race product implementer on auto-resume code
- Process “no bad metaphors” stays in AGENTS (pointer only in FORK handoff)

## Done when

- [x] FORK integrated handoff + ship bullets with report links
- [x] RESIDUAL Open points at dogfood gate honestly
- [x] This report written
