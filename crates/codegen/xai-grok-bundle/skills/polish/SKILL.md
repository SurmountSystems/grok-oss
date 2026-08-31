---
name: polish
description: >
  Run a polish pass: inventory problems in this session, write a
  remaining-work report so compaction cannot drop the list, then fan
  out disjoint fixes until things work well. Use when the user runs
  /polish, asks to polish, says make things work well, or wants every
  problem from this session documented and fixed. Not /finish
  (post-mortem). Not /reports (checkpoint). Not /dream. Not /recap.
metadata:
  short-description: "Polish pass: document and fix this session's problems"
  argument-hint: "[optional focus]"
---

# Polish

A polish pass. Work continues. Document every problem the user reported
this session, then fix disjoint slices. Do not claim the product is
working until those slices are actually fixed.

Not `/finish` (post-mortem). Not `/reports` (checkpoint). Not `/dream`.
Not `/recap`.

Product CLI is `grok-oss`. SuperGrok is a paid product. Never call
SuperGrok free. Complete American English thoughts. No nicknames.

Grok OSS screenshots from any current working directory are this
product. Do not treat another grok-oss window as out of scope.

This is a default Grok OSS skill. Grok installs it into
`~/.grok/bundled/skills/polish/` on startup. The live cache is not the
source. Do not add a project `.agents/skills/polish` copy unless the
user asked for a project override.

## Steps

1. **Inventory.** Collect every user-reported problem this session from
   chat and Grok OSS screenshots. Mentioned work is in scope. Do not
   park it. Do not ask "say if you want that."
2. **Board.** Same turn `todo_write` merge upsert: bugs → `bug:<slug>`,
   features → `feat:<slug>`. Short owed outcome, not a chat dump.
   Never `merge: false` wipe.
3. **Disk pointer.** Write or update a remaining-work report under
   `~/.agents/reports/` on this machine so compaction cannot drop the
   list. Call it a report, not a join. Chat is not enough.
4. **Fan out.** For each disjoint slice, L1 spawns L2. L2 spawns L3
   only if the problem is actually hard. Easy work can stay on L2. No
   L4. The product tool is `spawn_subagent`.
5. **Fix.** Behavior changes: red/green TDD (observed fail, then the
   same test green). After a structured `.rs` edit, file-level
   infer-from-path verify. Do not prove product work with crate-wide
   cargo via extra agents. One reviewer per slice. Effort is
   thoroughness, not reviewer count.
6. **Hunt nits** screenshots often hide. See
   [Incident classes](references/incident-classes.md).
7. **Honesty in status.** Do not claim working. A live binary is not
   the tree until rebuild. Name meters in complete thoughts: the
   included SuperGrok period limits for the current billing period (how
   much of that included quota is already used) vs SuperGrok dollar
   credits (prepaid top-ups on the SuperGrok account) vs console team
   prepaid / console API credits.
8. **Close out.** Complete board items the same turn the substance
   lands. Cancel only with a recorded reason. Never wipe the board.
9. **Stop git.** Never `git add`. Never `git commit`. Never push.

## Sub-agents

| When | Owns | L1 keeps |
|------|------|----------|
| Inventory too large for L1 | L2 (spawn L3 only if the inventory is actually hard) | Goal, remaining-report path, wait |
| Disjoint polish slice (bug or feat) | L2 implementer; L3 only if hard | Slice goal, report path, board id |
| Skill-body file writes | L3 (L2 must spawn) | Path, wait, read the report |
| Review after a slice | One L2 reviewer; L3 only if hard | One reviewer unless the user asked for more |

Prompts are self-contained. Each L2 writes a short report under
`~/.agents/reports/` on this machine. L1 reads that file only.

## Agent depth (not session-board L0-L2)

| Depth | Does | Does not |
|-------|------|----------|
| **L1 main** | Status. Spawn L2. Wait. Read short reports. Board upsert. Hierarchical fast path. | Diagnose, implement, multi-file reads, CI logs |
| **L2 subagent** | Parallelize. Spawn L3 **only if the problem is actually hard**. Easy work can stay on L2. Throw context away after the report. | Spawn L4. Show raw edits as if L1. |
| **L3 specialist** | Tools and work when spawned. Same agency as L2 except no spawn. | Spawn L4 |

**Hierarchical fast path** (L1 only): one-command host question; a
single known-path read already named; read and quote the asked-for
report; a single already-named one-line file edit. Not a license to
diagnose or implement.

L1 stays lean: remaining-work pointers, live L2 ids, last short report
path. Additive **also** / **btw** spawns another L2 (or queues
same-file). Do not kill a healthy in-flight L2.

## Honesty

- Do not invent remaining SuperGrok.
- Do not call any pool used up unless the live product Usage view or
  `/limits` surface they can see agrees, or a named live fetch of that
  same named meter agrees. A subagent snapshot is not enough to
  override the user.
- grok-oss limits chrome is a client printout, not xAI billing truth.
- Fail-open: a client 100% / remaining 0 / SuperGrok dollar credits $0
  printout must not mark SuperGrok used up or hop to console so this
  session cannot self-fix. Real SuperGrok HTTP 402 after that request
  failed can still leave SuperGrok.
- Matching nextReset is not proof of a shared pool.
- SuperGrok Heavy is a distinct weekly pool from standard SuperGrok.

## Hard rules

- No em dashes. No unicode ellipsis. Never bare child/children as
  agent nicknames.
- Wait times of a minute or more in minutes (`15m43s`, `1h2m`).
- Never assume. Docs can lie. Verify before claiming a slice is fixed.
