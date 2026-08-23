---
name: subagent
description: >
  Spawn one L2 coordinator with the user job as a self-contained
  prompt. The L1 main thread does not do the job. Use when the user
  runs /subagent, /subagent this, says they want this explicitly
  subagented, or asks to spawn an L2 coordinator. Not /polish. Not
  /implement. Not Hierarchical fast path on L1.
metadata:
  short-description: "Spawn one L2 coordinator for this job"
  argument-hint: "this <job> | <job>"
---

# Subagent

Spawn **one L2 coordinator** for the named job. The L1 main thread
does not diagnose, implement, or walk files for that job.

Not `/polish` (session polish pass). Not `/implement` (plan handoff).
Not the Hierarchical fast path on L1.

Product CLI is `grok-oss`. SuperGrok is a paid product. Never call
SuperGrok free. Complete American English thoughts. No nicknames.
Never bare child/children as agent names.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/subagent/` on startup. The live cache is not
the source. Do not add a project `.agents/skills/subagent` copy unless
the user asked for a project override.

## Steps

1. **Job text.** Take the rest of the slash line. `/subagent this ...`
   and `/subagent ...` are the same: strip a leading `this` token if
   present. That remainder is the job. If empty, ask once in
   freeform for the job. Do not invent it.
2. **Board.** Same turn `todo_write` merge upsert: short owed
   outcome. Bugs → `bug:<slug>`, features → `feat:<slug>`, other
   work → a namespaced id. Never `merge: false` wipe.
3. **Disk pointer.** When the contract must survive compaction,
   write a remaining-work pointer under `~/.agents/reports/` on
   this machine. Call it a report, not a join. Chat is not enough.
4. **Spawn one L2.** The product tool is `spawn_subagent`. The prompt
   is **self-contained**: the full job text, this tree, product CLI
   `grok-oss`, agent depth, report path, never `git add` / `git
   commit`. Do not tell L2 to "see the conversation above."
5. **Wait or other tracks.** Wait on that L2, or keep other
   healthy tracks running. Additive **also** / **btw** spawns
   another L2 (or queues same-file). Do not kill a healthy
   in-flight L2.
6. **Read the report.** L1 reads only the short on-disk L2 report
   under `~/.agents/reports/`. Do not re-do the L2 greps.
7. **Close out.** Complete the board item the same turn the
   substance lands. Cancel only with a recorded reason.
8. **Stop git.** Never `git add`. Never `git commit`. Never push.

## Sub-agents

| When | Owns | L1 keeps |
|------|------|----------|
| `/subagent` job | One L2 coordinator. L3 only if that L2 decides the problem is actually hard | Goal, board id, report path, wait |
| Additive disjoint job | Another L2 (do not kill the first) | Both report paths |
| Same-file race | Queue the new writer | First L2 stays live |
| Skill-body file writes | L3 (L2 must spawn) | Path, wait, read the report |

## Agent depth (not session-board L0-L2)

| Depth | Does | Does not |
|-------|------|----------|
| **L1 main** | Status. Board upsert. Spawn one L2. Wait or other tracks. Read the short report. Hierarchical fast path only for named one-liners that are not this job. | Do this job. Diagnose. Implement. Multi-file reads. CI logs |
| **L2 coordinator** | Do the job, or parallelize. Spawn L3 **only if the problem is actually hard**. Easy work can stay on L2. Write the report. Throw context away after. | Spawn L4. Show raw edits as if L1 |
| **L3 specialist** | Tools and work when spawned. Same agency as L2 except no spawn. | Spawn L4 |

**Hierarchical fast path** (L1 only): one-command host question; a
single known-path read already named; read and quote the asked-for
report; a single already-named one-line file edit. `/subagent`
means the job is **not** that path. Spawn L2.

No L4.

## Honesty

- Do not invent remaining SuperGrok.
- Name meters in complete thoughts: the included SuperGrok period
  limits for the current billing period (how much of that included
  quota is already used) vs SuperGrok dollar credits (prepaid
  top-ups on the SuperGrok account) vs console team prepaid /
  console API credits. Never call SuperGrok free.
- Do not call any pool used up unless the live product Usage view
  or `/limits` surface they can see agrees, or a named live fetch
  of that same named meter agrees. A subagent snapshot is not
  enough to override the user.
- grok-oss limits chrome is a client printout, not xAI billing truth.
- SuperGrok Heavy is a distinct weekly pool from standard SuperGrok.

## Hard rules

- No em dashes. No unicode ellipsis. Never bare child/children as
  agent nicknames.
- Wait times of a minute or more in minutes (`15m43s`, `1h2m`).
- Never assume. Docs can lie. Verify before claiming the job is
  done.
- One reviewer per slice unless the user asked for more.
- Behavior changes: red/green TDD (observed fail, then the same
  test green). After a structured `.rs` edit, file-level
  infer-from-path verify. Do not prove product work with crate-wide
  cargo via extra agents.
