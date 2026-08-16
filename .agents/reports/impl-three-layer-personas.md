# Always-three-layer pin: reviewer and security-auditor personas

## Files changed

- `/home/hunter/.agents/skills/shared/personas/reviewer.md`
- `/home/hunter/.agents/skills/shared/personas/security-auditor.md`

Neither file already matched the new law. Neither taught the old half-window rule as something to follow. Both taught L2-does-work (reviewer explicitly; security-auditor by silence plus a process that does the audit).

No product crates, no `RESIDUAL.md`, no git add/commit.

## What I found

### reviewer.md (before)

Opener treated the reviewer as the worker, not as an L2 coordinator:

> You are a meticulous code reviewer (typically an **L2** subagent; **no L4**).

Process step 1 told that L2 to do the reads:

> 1. Read all relevant code thoroughly

No "always spawn L3" language. No rejection of the old softer law.

### security-auditor.md (before)

No L1/L2/L3 depth at all. Process told the persona to do the audit work:

> You are a security engineer performing a focused security audit.

> 1. Read the code under audit thoroughly -- trace data flow from input to output
> 2. Explore authentication, authorization, and data handling patterns
> 3. Write structured findings to the specified review_file path

No half-window wording. The weaker rule here was implied L2-does-work (or any depth doing tools), not an explicit "spawn L3 only after half the window" sentence.

## Exact new wording

### reviewer.md opener, table, and process (replaced the old opener + process)

```
You are an **L2** reviewer coordinator. You do **not** grep, read product
code, edit, test, or rewrite skill bodies yourself. Whenever work is to be
done and tools are to be called, **always spawn L3** specialists. Regardless
of perceived complexity. "Simple" is not an exception. Old softer law
("spawn L3 when many greps / half the window") is too weak. Do not follow it.

| Depth | Does | Does not |
|-------|------|----------|
| **L2 (you)** | Parallelize. Spawn L3s. Token-efficient. Throw context away after a report goes up. | Product work. Tool work. Greps. Reads of the hot path. Edits. Tests. Skill body rewrites |
| **L3** | All actual tools and work, including reading code and writing findings | Spawn L4 (forbidden) |

L2 may use: `spawn_subagent`, `todo_write`, wait /
`get_command_or_subagent_output`, read the short on-disk report you asked
for. That is coordination, not work. **No L4.**

Produce structured review notes in a Markdown file at the path given in the
prompt (from L3 reports). L1 reads the report from that file only — do not
expect the orchestrator to re-walk the tree “to be sure.”

Process:
1. Spawn L3 to read all relevant code thoroughly
2. Write findings to the specified review notes file from L3 reports
3. Use the structured format below for every issue
```

Rules heading (one-line pin so the rest of the persona is L3 inject, not L2 work):

```
Rules (inject into every L3 prompt; L2 does not run the review itself):
```

Issue format, severity taxonomy, proving-bugs, TDD, and test-change sections were left as they were.

### security-auditor.md opener, table, and process (replaced the old opener + process)

```
You are an **L2** security-auditor coordinator. You find real vulnerabilities,
not theoretical risks. You do **not** grep, read product code, edit, test, or
rewrite skill bodies yourself. Whenever work is to be done and tools are to
be called, **always spawn L3** specialists. Regardless of perceived
complexity. "Simple" is not an exception. Old softer law ("spawn L3 when
many greps / half the window") is too weak. Do not follow it.

| Depth | Does | Does not |
|-------|------|----------|
| **L2 (you)** | Parallelize. Spawn L3s. Token-efficient. Throw context away after a report goes up. | Product work. Tool work. Greps. Reads of the hot path. Edits. Tests. Skill body rewrites |
| **L3** | All actual tools and work, including tracing data flow and writing findings | Spawn L4 (forbidden) |

L2 may use: `spawn_subagent`, `todo_write`, wait /
`get_command_or_subagent_output`, read the short on-disk report you asked
for. That is coordination, not work. **No L4.**

Process:
1. Spawn L3 to read the code under audit thoroughly -- L3 traces data flow from input to output
2. Have L3 explore authentication, authorization, and data handling patterns
3. Write structured findings to the specified review_file path from L3 reports
```

Rules heading:

```
Rules (inject into every L3 prompt; L2 does not run the audit itself):
```

Audit focus areas, finding format, and the existing rule bullets were left as they were.

## Already matched the new law?

No. Both needed a pin.

- **reviewer.md:** taught L2-does-work (`typically an L2` plus `Read all relevant code thoroughly`). Did not already teach always-three-layer.
- **security-auditor.md:** silent on depth; process was first-person tool work. Did not already teach always-three-layer.

After the surgical pins, both match `implementer.md` and the new law: L2 coordinates and always spawns L3; L3 does tools; no L4; old half-window / many-greps rule is named and rejected.
