# Synthesis: always-three-layer spawn copy

The three implementers finished. The waiter confirmed all three reports exist and are complete (not stubs). This file only synthesizes those reports.

## L3 ids

| Role | L3 id |
|------|--------|
| Product / builder | `01a006ad-0352-7193-adc8-68d0808392c1` |
| Implement skill helpers | `01a006ad-0352-7193-adc8-68e66f91212d` |
| Personas | `01a006ad-0352-7193-adc8-6909a9d30502` |
| Waiter (`not_found` wait workaround) | `01a006ad-4903-7832-bea5-ce556738083e` |

## Files changed

From the product/builder report:

- `crates/codegen/xai-grok-agent/src/builder.rs` (rustdoc on `CHILD_TASK_DESCRIPTION`, the spawn-tool description string, and `builder::tests::child_task_description_is_concise`)

From the implement-skill helpers report:

- `/home/hunter/.agents/skills/implement/SKILL.md`

From the personas report:

- `/home/hunter/.agents/skills/shared/personas/reviewer.md`
- `/home/hunter/.agents/skills/shared/personas/security-auditor.md`

The waiter did not edit product or skill files. None of the three implementers edited `RESIDUAL.md` or `templates/subagent_prompt.md`.

## Exact new spawn-tool wording

L2 now sees this on the spawn tool (quoted from the builder report): "Launch a specialist (L3) for an independent sub-task that needs its own context. You are L2. Whenever work is to be done and tools are to be called, agents are three layers deep. Always. Including implement loops. You MUST always spawn L3 for any tools/work. L2 never does greps, edits, or tests. Do not compact-and-continue a product restore on L2. No L4." The usage lines after that still name subagent type, description, prompt, and the background/return-id behavior.

## Whether tests now assert always-three-layer

Yes. The same test (`builder::tests::child_task_description_is_concise`) now requires `three layers deep. Always`, `MUST always spawn L3 for any tools/work`, `L2 never does greps, edits, or tests`, and `Including implement loops`. It still forbids compact-and-continue, `Agent types:`, and `<example>`, still requires `len() < 700`, and now rejects the old `many greps` / `half the window` wording.

## Commands + exit codes from L3 A (builder)

Env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-three-layer-spawn-copy-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`, rustc 1.97.1 (8bab26f4f 2026-07-14). Filter: `cargo test -p xai-grok-agent --lib child_task_description_is_concise -- --nocapture`.

| Step | Exit |
|------|------|
| Observe old wording (old test green on weak copy) | 0 |
| Red after stronger asserts (`child description must teach three layers always`) | 101 |
| First product copy over-length (`got 727 chars`) | 101 |
| Green after compact copy | 0 |
| `cargo fmt -p xai-grok-agent` | 0 |
| `cargo clippy -p xai-grok-agent --all-targets -- -D warnings` | 0 |
| `cargo test -p xai-grok-agent --lib builder::tests -- --nocapture` | 0 (42 passed) |

## Leftovers

- `templates/subagent_prompt.md` is still a shared L2/L3 worker prompt. Teaching "L2 never uses tools" there would also bind L3. Left alone on purpose.
- The phrase "Regardless of perceived complexity." is not in `CHILD_TASK_DESCRIPTION` so the string stays under the 700-character bound. The test still requires three layers always, implement loops, always-spawn-L3, and no half-window / many-greps teaching.
- Crate search: no remaining `many greps` / `half the window` product copy in `xai-grok-agent` except the test's negations.
- Implement skill: Plan Alignment L2 prompt still says to read a referenced design document in full. L2 is already told to spawn L3 for all reads. Not rewritten.
- Implement skill Step 3a still says to use the ask/question tool if available. HITL escalation, not a helper pin.
- `/home/hunter/.agents/skills/execute-plan/SKILL.md` still says the orchestrator does memory flush directly. Different skill. Out of this slice.
- Allowlisted `python3 …/memory.py` CLI form is kept (Grok intercepts to Rust). No new Python.

## What the three slices now teach

Product spawn copy, implement-skill helper steps, and the reviewer / security-auditor personas all say the same thing: L1 coordinates only, every L2 always spawns L3 for tools, L3 does the work, and there is no L4. Memory snapshot/flush, review merge, persona file reads, setup mkdir, and cleanup are L2-spawn-L3, not L1 tool steps. The old many-greps / half-window rule is named and rejected in the product test and both personas.
