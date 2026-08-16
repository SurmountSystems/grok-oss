# Report: pin leftover L1 helper steps onto L2-spawn-L3

Date: 2026-08-15
Worker: L3 specialist (no L4)
Scope: leftover implement-skill steps that still told L1 to run tools

## Files changed

- `/home/hunter/.agents/skills/implement/SKILL.md` (surgical pin only; loop/TDD/persona-protocol kept)

## Files left alone (and why)

- `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md`
  Rule 17 already keeps the allowlisted `python3 …/memory.py` CLI form (Grok intercepts to Rust).
  Rule 20 already states always-three-layers. It does not tell L1 to run memory.py,
  write JSON, merge reviews, or read personas. No edit needed.
- `RESIDUAL.md` (forbidden)
- Product crates (forbidden)
- `implement/scripts/memory.py` and `TestDocsConsistency.test_skill_md_default_header_matches`
  The `<!-- mirror-of: scripts/memory.py DEFAULT_HEADER -->` fenced block was not touched.

## Leftover L1-does-tools steps found

These told L1 to run tools. They are now L2-spawn-L3 (or L3 via that L2).

1. **Persona Injection.** L1 was told to `read_file` the three persona files and prepend bodies into spawn prompts.
2. **Step 0 Memory Retrieval.** L1 was told to `run_terminal_cmd` allowlisted `python3 …/memory.py snapshot`, parse JSON, and format the briefing.
3. **Step 3 effort >= 2 Merge.** L1 was told to `read_file` each individual review file and `write` the merged `review_file`.
4. **Step 6 Memory Flush.** Explicit: "The orchestrator performs this directly using its own tools — no subagent is needed." L1 was told to write the update JSON and run `memory.py update`.
5. **Cleanup.** L1 was told to run `agent-trash` itself.
6. **Setup `mkdir`.** L1 was told it may `mkdir -p` the scratch dir.

Supporting table/rules text still listed merge and memory flush as L1 work. That was pinned in the same file.

## Exact new wording (changed sections)

### Agent depth table and roles

```
| **L1** | Main thread | This orchestrator: status, spawn/wait L2, read short reports, board upsert, loop control | Grep, diagnose, implement, multi-file reads, CI logs, `memory.py`, write memory JSON, merge reviews, read persona files |
| **L2** | Subagents | Implementer, mop, general reviewers, specialists, setup/memory, merge, cleanup. Parallelize. Spawn L3s. Throw context away after a report. | Product work. Tool work. Greps. Edits. Tests. Skill body rewrites |
| **L3** | Specialists | All actual tools and work, in parallel. **Always.** | Spawn L4 (forbidden) |
```

```
| **Setup / memory** | Spawn L3 to confirm persona files exist, run allowlisted `python3 …/memory.py` snapshot/update (Grok intercepts to Rust), write briefing/flush reports | Spawn/wait; read those short reports |
| **Merge** (effort ≥ 2) | Spawn L3 to merge individual review files into `review_file` | Spawn/wait; read merged `review_file` |
| **Cleanup** | Spawn L3 to soft-delete scratch artifacts via agent-trash | Spawn/wait |
```

```
- **L1 must not:** CI log pulls, open failing tests, re-run nextest, product/code edits, re-do L2 greps "to be sure," run `memory.py`, write memory JSON, merge review files, or read persona files.
```

Intro line: `L1 does not read those files. If the setup L3 reports them missing, stop and report.`

### Persona Injection

```
Resolve these paths once at the start of the run (the system context gives you the absolute path to this SKILL.md). Store the three **paths** as orchestrator state. **Do not** `read_file` the persona files on L1 or L2.

When launching a subagent that needs a persona, **pass the matching persona file path** in the L2 prompt and tell L2: always spawn L3; have L3 read that persona file first and follow it. L1 and L2 do not read persona files.
```

Step 1 / Step 2 / specialist prompts now pass the path and say `Have L3 read that persona file first`. They do not prepend persona bodies.

### Setup mkdir

```
**`scratch_dir`:** `${TMPDIR:-/tmp}/grok-$(id -u)`. L1 does not `mkdir`. L3 creates the directory when writing the first artifact.
```

### Step 0 Memory Retrieval

```
Before launching the implementer, **spawn** an L2 `[memory]` helper. L1 does not run `memory.py`, write JSON, parse the snapshot, or read persona files. L2 always spawns L3 for those tools. L1 waits, then reads only the short `briefing_file`.
```

Helper path stays a string concat from the SKILL.md path the system already gave L1 (no tool):

```
memory_helper_path = dirname(<path-to-this-SKILL.md>) + "/scripts/memory.py"
```

```
L1 first tool this step: `spawn_subagent`. Not `run_terminal_cmd`. Not `write`. Not `read_file` on personas.

- `description`: `"[memory] Snapshot past issues"`
```

L2 prompt (excerpt): always spawn L3; confirm persona paths exist (do not copy bodies back); have L3 run:

```
python3 "<memory_helper_path>" snapshot
```

Then L3 writes `briefing_file`. L1 reads only that file.

Grok intercept kept: allowlisted `python3 …/implement/scripts/memory.py …` is hot-wired to embedded `util/implement_memory`. Do not invent a replacement.

Legacy migration is labeled a **human** one-time copy, not an L1 `/implement` tool step.

### Step 3 effort >= 2 Merge

```
After all reviewers complete, **spawn** an L2 `[merge]` helper. L1 does not read the individual review files or write the merged `review_file`. L2 always spawns L3 for those tools.

- `description`: `"[merge] Merge review files"`
```

L2 prompt: have L3 read individuals, merge with source tags, write `review_file`. L1 then reads only the merged `review_file` (coordination).

### Step 6 Memory Flush

```
After the loop terminates with 0 open issues, **spawn** an L2 `[memory]` helper to update the workspace memory file. L1 does not write memory JSON, run `memory.py`, or parse helper stdout. L2 always spawns L3 for those tools. L1 waits, then reads only `<flush_report>`.

- `description`: `"[memory] Flush review patterns"`
```

L3 owns Steps 6a-6d, including `write` of the JSON spec and:

```
python3 "${MEMORY_HELPER}" update < ${scratch_dir}/grok-mem-${IMPL_ID}.json
```

The old "orchestrator performs this directly / no subagent is needed" line is gone.

### Cleanup

```
After Step 6 (Memory Flush) and the Final Report, **spawn** an L2 `[cleanup]` helper. L1 does not run agent-trash. L2 always spawns L3.

- `description`: `"[cleanup] Trash implement scratch"`
```

L3 runs the existing `agent-trash` bash helper. No new Python.

### Rules bullets (helpers)

```
- **Pass persona paths, do not read them on L1** — … L2 always has L3 read that file. L1 and L2 do not `read_file` persona files.
- **Always three layers** — L1 coordinates only. Every L2 (implementer, mop, reviewer, specialist, setup/memory, merge, cleanup) always spawns L3 for tools. … No L4.
- **Read the merged review_file** after each review … L1 does not merge individual review files.
- **Effort>=2 uses individual files + merge** — … Spawn L2 `[merge]`; L3 merges them into `review_file`. L1 reads only the merged file.
- **Always go through the `memory.py` helper, on L3.** … L1 derives `MEMORY_HELPER` from the SKILL.md path … (path string only; no tool) and passes it to L2. L3 runs the allowlisted CLI (Grok intercepts to Rust). … L1 does not run `memory.py` or write the update JSON.
- **Use the `snapshot` subcommand for reads, not `read`.** L3 runs `python3 "${MEMORY_HELPER}" snapshot` and writes structured fields into `<briefing_file>`. L1 reads that short report only.
```

## What was left alone (why)

Surgical pin only. These are loop control or allowed coordination reads of reports L1 already asked for:

- Todo scaffold, effort parse, specialization decision, role-swap, tool-call discipline
- Orchestrator gates that `read` `summary_file` / merged `review_file` / `briefing_file` / `flush_report` (the short reports L1 requested)
- Effort=1: reviewer writes `review_file` directly; L1 reads that one file (no merge)
- Stalemate compare of the already-read merged review vs prior snapshot
- Final Report coordination reads
- Injecting already-loaded `past_issues_briefing` text into later L2 prompts
- Helper protocol, JSON shape, exit codes, `DEFAULT_HEADER` mirror
- Human one-time legacy migration (now labeled human, not L1)

## Leftovers (out of this slice)

- Plan Alignment L2 prompt still says "If a design document… is referenced by file path… read it in full before starting your review." That is an L2 prompt leftover, not an L1 helper. L2 is already told to spawn L3 for all reads. Not rewritten.
- Step 3a still says "use the appropriate ask/question tool if available." HITL escalation, not a memory/merge/persona helper. Out of scope.
- `/home/hunter/.agents/skills/execute-plan/SKILL.md` still says the orchestrator does memory flush directly. Different skill. Out of scope.
- No new Python. Allowlisted `memory.py` CLI form kept for the existing Rust intercept.

## Constraints honored

- No `git add` / `git commit`
- No invented Python scripts
- No product crate edits
- No `RESIDUAL.md` edit
