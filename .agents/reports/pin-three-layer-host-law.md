# Report: always-three-layers host law (2026-08-15)

## Files changed

- `/home/hunter/.grok/AGENTS.md` only.
- No project `AGENTS.md`, skills, FORK, residual, or user-guide edits.

## Sections touched

- *Regressions and deep diagnosis — never in the parent thread*
- *Hard stop — parent is coordinator only* (HITL framing, *Agent depth*, default loop step 2, parent must-not, first tool turn, repeated failure mode)
- Nearby anti-pattern under *Project agent home*
- *Never assume — docs can lie* (verify step 1)
- *Context economics + strategic parallelism* (Spawn for depth row)
- *User-reported bugs & features* (item 3)
- *Post-impl verify* lead sentence and *Effort ≥ 2 process mop*

Self-improving feedback loop (*always remember* / *please remember* / *I hate repeating myself*) was left intact.

## Exact sentences replaced

### 1. Regressions item 4 (old weaker depth)

**Old:**

> **Agent depth max L3:** L1 → L2; **L2 that will do many greps/reads/edits MUST spawn L3 specialists** and keep a short window. Crossing about half the window: stop solo work, fan out. **Do not compact-and-continue** a product restore on L2. Parent spawn prompts must tell L2 to use L3.

**New:**

> **Always three layers (max L3):** L1 → L2 → L3. L2 **always** spawns L3 specialists for any tool work (greps, reads, edits, tests, skill rewrites), including implement loops. Do not do that work on L2. **Do not compact-and-continue** a product restore on L2. Parent spawn prompts must tell L2 to spawn L3 always.

Lead-in added: *Whenever work is to be done and tools are to be called, use three layers. Always. That includes implement loops and work that looks simple, not only the cases below.*

### 2. Hard stop / Agent depth (canonical standing law)

**Old HITL:**

> Research, diagnosis, multi-file inventory, and non-trivial implementation live in **subagents**.

**New HITL:**

> Research, diagnosis, multi-file inventory, implementation, greps, edits, and tests live in **L3 specialists**. L2 only parallelizes and throws context away after the report.

**Old depth (2026-08-13 weaker pin):**

> **L2 that will do many greps/reads/edits MUST spawn L3.** Crossing about half the window: stop solo, fan out. Do not compact-and-continue a product restore on L2. L1 spawn prompts must include that sentence.

**New depth (2026-08-15):**

> Whenever work is to be done and tools are to be called, agents are **three layers deep. Always.** Regardless of perceived complexity. Including implement loops. **"Simple" is not an exception.** The old softer law (L2 must spawn L3 only when there are many greps, or only when crossing about half the window) is too weak. Do not follow it.

Plus the L1 / L2 / L3 Does / Does not table, the compaction reason (work on L2 fills L2 and loses restack/skills work), and the coordination-only allowlist (`spawn_subagent`, `todo_write`, wait, read the short report).

Default loop step 2 now says each L2 always spawns L3. Added: *L2 has the same tool-work ban.*

### 3. First tool turn / failure mode

**Old:** First tool turn only after CI fail / regression / multi-file task. Parent greps then spawns is the failure.

**New:** First tool turn whenever work or tools are needed. L1 and L2 greps then spawn is the same failure. L3 owns fetch, read, and fix.

### 4. Never assume

**Old:** parent may do 1–2 targeted lookups only when already in scope.

**New:** L3 (spawned by L2) owns verification. L1 does not do targeted lookups. L1 and L2 only coordinate.

### 5. Context economics

**Old:** Many greps/reads/edits → subagent; 1–2 lookups → parent

**New:** Any tool work is L1 → L2 → L3. Always. No "1–2 lookups" on L1 or L2.

### 6. Bugs / TDD / mop

**Old:** Parent HITL coordinates; implementer owns red→green. Multi-file diagnosis still spawn-first.

**New:** Parent HITL coordinates. L2 spawns L3. The L3 implementer owns red→green. Any tool work is spawn-first (always three layers).

Mop is now an L2 coordinator that always spawns L3 for fmt, clippy, and tests.

## Leftover sites in this file

No leftover sentence still teaches L2 to do greps, edits, tests, or skill-body rewrites.

The only remaining "many greps / half the window" wording is the explicit rejection of that old law in *Agent depth*.

Kept on purpose (surrounding law, not L2 tool work):

- L1 may still do a *single* tiny same-turn process-law pin when the operator corrects process mid-turn. Larger pin novels still spawn.
- Host-emergency sections (*Host unusable*, keyring, supply-chain watch) still describe live restore steps. They are not product implement loops.
- Session-board L0–L2 remains a different namespace from agent depth L1/L2/L3.
- *Self-improving feedback loop* is unchanged.

Out of scope (not this file): project `AGENTS.md`, skill bodies, FORK, residual, user-guide. Those may still carry the weaker "L2 must spawn L3 when many greps" sentence.
