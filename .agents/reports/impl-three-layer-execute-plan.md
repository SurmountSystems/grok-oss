# Report: execute-plan leftover three-layer memory flush

**Verdict:** Edited. The leftover was real. It is pinned now.

Named leftover (from `.agents/reports/impl-three-layer-spawn-copy.md`): `/home/hunter/.agents/skills/execute-plan/SKILL.md` told the orchestrator to do memory flush directly. That violated always-three-layer law (L1/L2 coordinate only; memory snapshot/flush is tool work for L2-spawn-L3).

This turn verified the working-tree skill against `HEAD` in `~/.agents/skills` and confirmed the leftover phrases are gone. No further churn.

## Paths touched

- Edited: `/home/hunter/.agents/skills/execute-plan/SKILL.md`
- This report: `/home/hunter/Projects/surmount/grok-build/.agents/reports/impl-three-layer-execute-plan.md`

Did not rewrite the whole skill. Did not touch product crates, implement skill, personas, or `RESIDUAL.md`.

## Exact leftover phrases (before → after)

Before quotes are from `git -C ~/.agents/skills show HEAD:execute-plan/SKILL.md`. After quotes are the working-tree file.

### 1. Agent depth table taught optional L3

**Before:**

```
| **L1** | Main thread | This orchestrator: DAG/state, branch prep, stack assembly, wait/join, hand signed git |
| **L2** | Subagents | `[implementer]` and `[reviewer]` per PR |
| **L3** | Specialists | Optional from L2 only; **max depth; no L4** |
```

No `[memory]` L2. No always-three-layer paragraph.

**After:**

```
**Always three layers** when work is to be done and tools are to be called.
Regardless of perceived complexity. Including this execute-plan loop. "Simple"
is not an exception. Old softer law ("L2 must spawn L3 when many greps /
half the window") is too weak. Do not use it.

| **L1** | Main thread | This orchestrator: status, spawn/wait L2, read short reports, board upsert, DAG/state, branch prep, stack assembly, hand signed git. No greps, no memory snapshot/flush, no tool work. |
| **L2** | Subagents | `[implementer]` and `[reviewer]` per PR; `[memory]` for snapshot/flush. Parallelize. Spawn L3s. Stay token-efficient. Throw context away after the report. No product/tool work. |
| **L3** | Specialists | All actual tools and work (including memory snapshot/flush). **Always.** No L4. |
```

Plus a `[memory]` role row: spawn L3 to run allowlisted `python3 …/memory.py` snapshot/update (Grok intercepts to Rust); L1 only spawn/wait and reads the short reports.

Hard stop now also says L1 must not run `memory.py`, write memory JSON, or parse snapshot/flush helper stdout.

### 2. Step 0 told L1 to run snapshot itself

**Before:**

> Before launching any implementers, attempt to load past issue patterns from the workspace memory file.

and

> 1. Run `python3 "${MEMORY_HELPER}" snapshot` via `run_terminal_cmd` and capture stdout.

Also: "Substitute this absolute path directly into every helper invocation" (orchestrator runs the CLI).

**After:**

> Before launching any implementers, **spawn** an L2 `[memory]` helper ... L1 does not run `memory.py`, write JSON, or parse the snapshot. L2 always spawns L3 for those tools. L1 waits, then reads only the short `briefing_file`.

> L1 first tool this step: `spawn_subagent`. Not `run_terminal_cmd`. Not `write`.

L3 runs:

```
python3 "<memory_helper_path>" snapshot
```

### 3. Step 10 was the named leftover (orchestrator does flush)

**Before:**

> After cleanup, update the workspace memory file with patterns from this run. The orchestrator performs this directly using its own tools -- no subagent is needed.

and

> Use the `write` tool to create `${scratch_dir}/grok-exec-mem-${PLAN_ID}.json` with the JSON spec above, then invoke:

**After:**

> After cleanup, **spawn** an L2 `[memory]` helper to update the workspace memory file with patterns from this run. L1 does not write memory JSON, run `memory.py`, or parse helper stdout. L2 always spawns L3 for those tools. L1 waits, then reads only `<flush_report>`.

L3 still uses the same allowlisted update form:

```
python3 "${MEMORY_HELPER}" update < ${scratch_dir}/grok-exec-mem-${PLAN_ID}.json
```

Steps 10a–10d are labeled L3. L1 does not write the JSON or run the helper.

### 4. Rules / tags / cleanup

**Before:**

> **Use the implement skill's memory.py helper** -- derive the absolute path from the implement skill's SKILL.md path announced in the system context (see Step 0 for derivation), not from `$(pwd)`.

Role tags listed only `[implementer]` and `[reviewer]`.

**After:**

> **Always three layers** -- L1 coordinates only. Every L2 (implementer, reviewer, memory) always spawns L3 for tools. L2 does not grep, edit, test, review product code, run `memory.py`, or write memory JSON. No L4. Implement loops are not an exception.

> **Always go through the implement skill's `memory.py` helper, on L3.** L1 derives the absolute path ... and passes it to L2. L3 runs the allowlisted CLI (Grok intercepts to Rust). ... L1 does not run `memory.py` or write the update JSON.

`[memory]` is a required description tag for Step 0 snapshot and Step 10 flush.

Step 9 trash list now includes `grok-exec-mem-briefing-<PLAN_ID>.md` and `grok-exec-mem-flush-<PLAN_ID>.md`.

Implementer / reviewer / fix / re-review prompts now open with L2-spawn-L3 (surgical opener only).

## Python / memory.py

Did not invent Python. Did not add a new helper. Kept the allowlisted CLI forms already in the skill:

- `python3 "<memory_helper_path>" snapshot`
- `python3 "${MEMORY_HELPER}" update < ${scratch_dir}/grok-exec-mem-${PLAN_ID}.json`

Grok intercepts those forms to `util/implement_memory`. Host still ships `memory.py` for non-Grok hosts.

## Git

No `git add`. No `git commit`. Host skill remains unstaged vs `~/.agents/skills` `HEAD`.

## Out of this slice (left on purpose)

Not rewritten, because the assignment was leftover helper/tool steps (especially memory flush), not a full skill rewrite:

- Persona Injection still says L1 `read_file` the persona files and prepend bodies (line 202).
- Setup still has L1 write the state file; Step 0.5 still has orchestrator `gt`/`gh` probes; Step 1 still has L1 `read_file` the design doc.
- Step 9 agent-trash is still shown as an orchestrator shell block (not an L2 `[cleanup]` spawn).
- Git, DAG, branch prep, stack assembly, and conflict resolution stay orchestrator-owned (skill law).
- Implementer prompt body still says "Implement the changes" after the L2 opener (same pattern the implement-skill leftover left alone).

"Half the window" appears only as the rejected old law.

## Standing law now in the leftover wording

Whenever work needs tools, agents are three layers always, including implement loops:

- L1: status, spawn L2, wait, read short reports, board upsert. No greps, no memory flush, no tool work.
- L2: parallelize, spawn L3s, stay token-efficient, throw context away after the report. No product/tool work.
- L3: all actual tools and work, including memory snapshot/flush. No L4.
