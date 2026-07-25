# Canonical Hard stop + Regressions source text (2026-07-24)

Paste of the living pins used for skill / orchestrator reconcile checks.
Do not paraphrase when reconciling — match intent against these quotes.

Sources (read 2026-07-24):

- `~/.grok/AGENTS.md` — § *Regressions and deep diagnosis* + § *Hard stop*
- `~/Projects/surmount/grok-build/AGENTS.md` — § *Subagents — parent is coordinator only (hard)*

Deep companion (not pasted here):
`~/.agents/skills/shared/references/subagent-token-strategy.md`

---

## 1. Global — Regressions and deep diagnosis — never in the parent thread

From `/home/hunter/.grok/AGENTS.md`:

```
### Regressions and deep diagnosis — never in the parent thread

When the user reports a **regression**, a **product bug under investigation**,
a **CI failure**, or any task that needs multi-file greps, session logs,
config archaeology, long code walks, **or non-trivial implementation**:

1. **Do not** research or implement that work in the **main** (parent)
   thread. Parent context is expensive; **parent compaction is expensive**.
2. **Immediately** spawn **token-efficient, tightly scoped** subagents
   (prefer `explore` / `plan` for read-only; `general-purpose` only for edits).
   Fan out disjoint scopes in parallel. Parent holds: goal, acceptance,
   artifact paths, short join.
3. **Why:** each subagent has a **fresh** window and **does not need
   compaction** the way a long parent session does. Dumping diagnosis into the
   parent burns TPD (tokens / attention / price knee) and often forces a
   parent compact that destroys coordination quality.
4. **Hierarchy without nested spawn:** this host is **depth-1** (children
   cannot spawn children). “Hierarchical” means the **parent** layers work —
   inventory → root-cause → fix → verify — via sequential or parallel
   specialists, each with a tight prompt and an **on-disk** artifact. Do not
   invent nested-agent fantasies.
5. **Join on disk.** Children write short summary files; parent reads those
   only. Never re-run the child’s greps “to be sure” in the parent.
6. **User corrections survive compaction.** When the user states a process or
   product rule (including this section), **pin it the same turn** in this
   file or the relevant project `AGENTS.md` / living skill — chat-only memory
   is not enough. Keep pins short; link out for depth.

**Anti-pattern (forbidden):** regression report → parent solo marathon of
`rg` / log tails / multi-crate reads that fill the parent toward the economic
knee, then compact, then lose the plot.
```

---

## 2. Global — Hard stop — parent is coordinator only (pinned 2026-07-24)

From `/home/hunter/.grok/AGENTS.md`:

```
### Hard stop — parent is coordinator only (pinned 2026-07-24)

Hunter has re-flagged this **repeatedly**. Chat memory is not enough.

**Parent may:** set goals, spawn/wait children, read **short on-disk join
artifacts** children wrote, stage/hand human-only git commands, one-line
status to the user.

**Parent must not** (even “just a quick look”):
- Pull CI logs, open failing test files, or re-run nextest in the parent
- Multi-file `rg`/read loops for root cause
- Implement fixes, edit product/test code, or “help the child along”
- Wait for a child then re-do the child’s research in the parent

**First tool turn after a CI fail / regression / multi-file task:**
`spawn_subagent` (or parallel spawns) — not `grep`, not `gh run view`,
not `read_file` on the hot path. If you already broke this: stop parent
tools, spawn, join only on disk.

**Repeated failure mode to kill:** parent greps docs + fetches GHA logs +
locates the test file, *then* spawns. That is already the marathon. Spawn
first; children own fetch/read/fix.
```

---

## 3. Project (grok-build) — Subagents — parent is coordinator only (hard)

From `/home/hunter/Projects/surmount/grok-build/AGENTS.md`:

```
## Subagents — parent is coordinator only (hard)

Pinned after repeated parent marathons on CI / onto / conflict work.

- **CI fail, regression, multi-file diagnosis, non-trivial fix:** first action
  is `spawn_subagent` — not parent `grep` / `gh` log pull / test file reads.
- Parent may: goals, spawn/wait, read **short on-disk join notes**, hand signed
  git commands, brief user status.
- Parent must **not**: pull CI logs, open failing tests, re-run nextest, edit
  product code, or re-do the child’s greps “to be sure.”
- Full rule: `~/.grok/AGENTS.md` § *Regressions…* + § *Hard stop — parent is
  coordinator only*.
```

---

## Reconcile check (orchestrator skills)

When reconciling skills: verify orchestrators (`implement`, `plan`,
`check-work`, `review`, and similar) still carry **parent Hard stop** and
**spawn-first on CI / regression / multi-file diagnosis**. Link:
`shared/references/subagent-token-strategy.md`.
