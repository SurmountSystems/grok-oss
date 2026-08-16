# Report: always-three-layer pin (skill-rules + implement)

Pinned 2026-08-15. Isolated write scope only. No L4. No compile.

## Files changed

1. `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md`
   - Standing rule 20: always three layers. No exception for "simple" or implement loops.
   - Agent-depth table is now Does / Does not (L1 coordinate, L2 spawn L3s, L3 does tools).
   - Old softer law ("L2 must spawn L3 when many greps / half the window") is named and rejected.
   - Spawn-vs-not: any work that needs tools is L1 spawn L2, L2 always spawn L3.
   - Skill-authoring item 9 and Hard stop prose match the new law.

2. `/home/hunter/.agents/skills/implement/SKILL.md`
   - Agent-depth table and role table: L2 implementer/mop/reviewers spawn L3; they do not code.
   - Implementer, mop, reviewer, specialist, fix, and re-review prompts open with always-spawn-L3.
   - Implement loops are not an exception.

3. `/home/hunter/.agents/skills/shared/personas/implementer.md`
   - Needed an edit. It told L2 to implement code and used the weaker spawn rule.

## Implementer persona

**Edited.** Opening now: L2 is a coordinator. Always spawn L3. Do not implement, grep, edit, test, or rewrite skill bodies. TDD / fmt / clippy / test rules stay in the file as text L2 injects into every L3 prompt.

## Leftovers in these three files only

These still tell L1 or L2 to use tools beyond spawn / todo / wait / read the short report. Not rewritten (would be a full implement-loop restack, not a pin).

- **Implement SKILL Step 0 and Step 6:** L1 still runs the allowlisted `memory.py` helper (`run_terminal_cmd`) and writes the memory JSON. Step 6 says the orchestrator does this directly with its own tools.
- **Implement SKILL Step 3 merge, Cleanup, Final Report, Prepare reviewer focus:** L1 still reads and writes review/summary/merge files and runs the trash helper. Reading the asked-for report is allowed coordination. Writing the merged review and running cleanup are extra tools.
- **Implement SKILL Persona Injection:** L1 is still told to `read_file` all three persona files at the start of a run (multi-file reads).
- **Implementer persona body:** TDD, probe-hygiene, and claim-bool sections still say "you" (now labeled as L3 inject). Rare-escape still mentions `/tmp`.
- **Skill-rules rule 19:** still says "Implementer must fmt + clippy" without saying L3 runs those commands.
- **Skill-rules Hard stop table:** still says L1 may "Stage + hand human-only `git commit -S`" (older git-labor line; not the three-layer pin).

`reviewer.md` and `security-auditor.md` were outside this isolated write scope and were not edited.
