# Report: always-three-layers pin (2026-08-15)

L3 specialist only. No L4. Isolated write scope honored.

## Files changed

1. `/home/hunter/.agents/skills/hierarchically-structured-subagents/SKILL.md`
2. `/home/hunter/.agents/skills/shared/references/subagent-token-strategy.md`

No other files touched.

## What weaker text was replaced

Old standing law in both files: L2 must spawn L3 when the job is many
greps, reads, or edits; crossing about half the window, stop solo work
and fan out. "Simple" 1-2 lookups could stay on L1 or L2. L3 was
optional. Implement loops were not named as always-L3.

New standing law (operator 2026-08-15): whenever work is to be done and
tools are to be called, agents are three layers deep. Always. Including
implement loops. Simple is not an exception.

| Depth | Does | Does not |
|-------|------|----------|
| L1 main | Status. Spawn L2. Wait. Read short reports. Board upsert. | Grep, diagnose, implement, multi-file reads, CI logs |
| L2 subagent | Parallelize. Spawn L3s. Stay token-efficient. Throw context away after the report. | Product work, tool work, greps, edits, tests, skill body rewrites |
| L3 specialist | All actual tools and work, in parallel | Spawn L4 |

Reason pinned in both files: L1 stays cheap for HITL. L2 exists so
context can be discarded after the report. Work on L2 fills L2 and
causes compaction. That is how restack and skills work was lost.

L1 and L2 may still use `spawn_subagent`, `todo_write`,
`get_command_or_subagent_output` / wait, and read the short on-disk
report they asked for. That is coordination, not work.

### SKILL.md (whole short file rewritten)

Replaced:

- Frontmatter: "L2 MUST spawn L3 for many greps, reads, or edits.
  Crossing about half the window: stop solo work, fan out."
- Depth table that listed L2 as implement/explore/plan/review.
- Hard rules 2-3 (many greps must spawn L3; half-window fan-out).
- Sub-agents table row that allowed "1-2 targeted lookups already in
  scope" on the caller.

Now: always-three-layers table, L2 orchestrator only, L3 owns every
tool, coordination-tools-only list, no simple-job exception.

### Token strategy (surgical section replacements)

Replaced:

- Agent-depth table: L3 "MUST when L2 will do many greps/reads/edits."
- Default stance: spawn L3 only on large/log-heavy/multi-file work.
- Hard stop: "research + implementation in L2/L3" and "that is L2 work."
- Never-assume closer: "1-2 targeted lookups may stay L1-local."
- Regressions item 4: half-window / many-greps L2-must-spawn-L3.
- Ownership line: "L2 (and optional L3) own greps / CI / fixes."
- When-to-spawn / do-not-spawn: skip spawn for 1-2 tool calls; L1 may
  `read_file` / `grep` a known path.
- Decision micro-flow: "trivial (1-2 tools)? do it in L1."
- Skill-author item 7: "diagnosis and fix own L2 (optional L3)."
- Several leftover "L2 owns fetch/read/fix" and "spot-check disputed
  lines in L1" sentences.

Now those sections say always three layers; L2 orchestrates only; L3
does all tools; throw L2 context away after the report.

## Leftovers in these two files only

These are not the old threshold as live law. They are remaining
soft spots in the same two files.

1. **SKILL.md** hard rule 3 still says "Keep a short window" for L2.
   That is hygiene, not a spawn threshold. The spawn rule is already
   always-L3.

2. **Token strategy** still quotes the old "many greps / half the
   window" wording in three places, each as **too weak / not enough**.
   That is deliberate contrast, not the live rule.

3. **Token strategy** "Required rule pins" and several "Canonical:
   `~/.grok/AGENTS.md`" pointers still describe the older L1-only
   Hard stop. Those host files were out of this write scope. Inside
   this file, the body now contradicts those pin blurbs.

4. **After compaction** still says L1 may `read_file` the latest
   `grok-impl-summary-*` / `grok-review-*` / plan. That matches the
   allowed "read the short report you asked for." It does not
   re-open a half-window exception.

5. **After compaction** "spawn a tight implementer" and "Parallel
   explore only for new unknown areas" do not repeat L1 → L2 → L3
   on those two lines. Nearby sections do.

6. **Anti-patterns** still say "child" in several older rows
   (parent/child table language). The Language section still allows
   structural depth terms. Those rows do not restore the old
   half-window rule.

7. One Do/Don't row still says "Hand signed git commands after
   stage." That is pre-existing git-handoff prose, not a tool-work
   exception for product greps.

No compile. No git. Isolated scope only.
