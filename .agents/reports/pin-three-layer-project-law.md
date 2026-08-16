# Report: three-layer agent depth is project D1 law

Date: 2026-08-15
Scope: `/home/hunter/Projects/surmount/grok-build/AGENTS.md` only.

## Files changed

- `/home/hunter/Projects/surmount/grok-build/AGENTS.md`
- this report: `/home/hunter/Projects/surmount/grok-build/.agents/reports/pin-three-layer-project-law.md`

Host `AGENTS.md`, skills, `FORK.md`, `RESIDUAL.md`, and user-guide were not touched.

## Sections touched

- Hard constraint **3b** (effort ≥ 2 process mop): mop is L1 → L2 coordinator → L3 runs fmt/clippy/tests. L2 does not run those commands.
- **Subagents — parent is HITL UX only**: research, greps, edits, tests, and skill-body rewrites never run on L1 or L2. L2 spawns L3s.
- **Agent depth L1 / L2 / L3** (re-pinned 2026-08-15): always three layers. New Does / Does not table. Coordination tools only on L1/L2. L4 forbidden. Named as project D1 that must survive recon. Host dual-pin kept.
- Same-section bullets: CI/regression first turn; L1 and L2 may/must-not.
- **Never assume without checking**: first spawn is L1 → L2; L2 always fans out L3.
- **Survive recon**: this file’s agent-depth pin is listed as three layers always, not the old weaker rule.
- **Onto recovery**: multi-file conflicts go to L2 coordinators that spawn L3; L2 does not resolve files.

## Old weaker sentences replaced

These no longer stand as law:

1. L2 role: “Planning, research, implement, review, test work lives here.”
2. L3 role: “L2 **MUST** spawn specialists when the job is many greps/reads/edits. Keep a short window. Crossing about half the window: stop solo, fan out. Do not compact-and-continue on L2.”
3. “L1 coordinates; deeper layers do the heavy work.”
4. Parent-only “research and implementation never run in the parent” (now L1 and L2).
5. Parent-only “must not grep / implement” bullets (now L1 and L2; L3 does the work).
6. Process mop that itself “only runs fmt → clippy.”
7. “Multi-file conflicts → subagents on disjoint paths” with no L3.

The old softer law is named once in the depth section and once under Survive recon, both as **too weak / do not teach**.

## Leftover sites in this file only

Not teaching the old half-window rule. Worth knowing if a later pass wants even tighter wording:

- **Default loop** still says track → spawn → wait → read report. That is L1 coordination. It does not name L3. The depth section above it already requires L2 to spawn L3.
- **Additive asks** still says “spawn another subagent.” That is L1 spawning another L2 track.
- **Hard constraint 3a** still says “when you create or edit product code” run fmt/clippy/tests. Duty belongs to the L3 that edits.
- **Auth / keyring**: “One implementer owns TDD” still means one L2 track owns the work; L3 runs the tests.
- **Session-board L0 / L1 / L2** names are unchanged and still distinguished from agent depth.
- Host section titles cited with their existing names (`Hard stop — parent is coordinator only`).
