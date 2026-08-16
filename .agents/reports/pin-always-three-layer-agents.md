# Always three-layer agents (L2 synthesis)

Date: 2026-08-15. L2 coordinated only. L3s did every file walk and every law edit.

## Whether this host could spawn L3

Yes. Seven write specialists and two waiters ran. Their reports landed on disk.

`get_command_or_subagent_output` and `wait_commands_or_subagents` could not see those L3 ids (`not_found` / "No background tasks or subagents exist in this session"). Coordination used a blocking (`background: false`) waiter L3 that only checked the named report paths. That wait-id gap is a host/session fact, not an unpinnable law file. `RESIDUAL.md` was left untouched because every named pin path exists.

## L3s spawned (ids)

| Id | Slice |
|----|--------|
| `01a006a6-fcdf-79d1-8fb6-9062aa11097f` | Host law (`~/.grok/AGENTS.md`) |
| `01a006a6-fcdf-79d1-8fb6-9076782d1738` | Project law (`AGENTS.md`) |
| `01a006a6-fcdf-79d1-8fb6-908fa9828898` | Hierarchical skill + token-strategy |
| `01a006a6-fce0-7401-b10c-ecd79f994fb1` | Skill-rules, implement skill, implementer persona |
| `01a006a6-fce0-7401-b10c-ece8d828a1eb` | User-guide `16-subagents.md` + `FORK.md` |
| `01a006a6-fce0-7401-b10c-ecf1f72fa244` | Residual only if a pin could not land |
| `01a006a6-fce0-7401-b10c-ed10473f3f5a` | In-tree bundled skill walk |
| `01a006a7-dc7b-7092-ae94-ecdf09e76e43` | Background waiter (wait tool never attached) |
| `01a006a8-3472-7c83-9771-279c65fe39d3` | Blocking waiter (confirmed all six write reports exist) |

No L4.

## Files changed

- `/home/hunter/.grok/AGENTS.md`
- `/home/hunter/Projects/surmount/grok-build/AGENTS.md`
- `/home/hunter/.agents/skills/hierarchically-structured-subagents/SKILL.md`
- `/home/hunter/.agents/skills/shared/references/subagent-token-strategy.md`
- `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md`
- `/home/hunter/.agents/skills/implement/SKILL.md`
- `/home/hunter/.agents/skills/shared/personas/implementer.md`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md`
- `/home/hunter/Projects/surmount/grok-build/FORK.md`

Not edited: `RESIDUAL.md`. No in-repo product skill body existed to pin.

Slice reports: `.agents/reports/pin-three-layer-*.md` and `pin-three-layer-wait-status.md`.

## Exact new rule (plain English)

Whenever work is to be done and tools are to be called, agents are three layers deep. Always. That includes work that looks simple. That includes implement loops.

- **L1 main** tells the operator what is happening, spawns L2, waits, reads the short report, and updates the session board. L1 does not grep, diagnose, implement, do multi-file reads, or pull CI logs.
- **L2 subagent** fans out L3s in parallel, stays token-efficient, and throws its context away after the report goes up. L2 does not do product work, tool work, greps, edits, tests, or skill-body rewrites.
- **L3 specialist** does all actual tools and work, in parallel. L3 does not spawn L4.

L1 and L2 may still use `spawn_subagent`, `todo_write`, wait / `get_command_or_subagent_output`, and read the short on-disk report they asked for. That is coordination, not work.

The old softer law (L2 must spawn L3 only when there are many greps, or only after about half the window) is too weak. Do not follow it.

Reason: L1 stays cheap for a human in the loop. L2 exists so that context can be discarded after the report. Work done on L2 fills L2 and causes compaction. That is how restack and skills work was lost.

## Leftovers that are real, not guesses

1. **Product spawn-tool copy still teaches the old weaker rule.** The bundled-skills walker found no in-tree `SKILL.md`. It did find `crates/codegen/xai-grok-agent/src/builder.rs` (`CHILD_TASK_DESCRIPTION`, about lines 1242–1258) still telling L2 to spawn L3 only when the work needs many greps, reads, or edits, and to stop solo after about half the window. Tests around 1545–1561 assert that wording. That file is product source, not a skill body, so this wave did not edit it.

2. **Shared subagent template is L2 and L3 together.** `crates/codegen/xai-grok-agent/templates/subagent_prompt.md` tells every subagent it is a focused worker that should use tools. Pinning "L2 never uses tools" there would also bind L3. Left alone on purpose.

3. **Implement skill still has L1 tool steps beyond coordination.** `_SKILL_RULES` / `implement/SKILL.md` still tell L1 to run the allowlisted `memory.py` helper, write memory JSON, merge review files, and read persona files at the start of a run. The implementer-persona L3 labeled that as a full implement-loop restack, not this pin.

4. **`reviewer.md` and `security-auditor.md`** were outside the implementer-persona write scope and were not edited.

5. **This session could spawn L3 but could not wait on L3 ids.** Reports were the proof. Residual was not opened for that.

Self-improving feedback loop on the host file was left intact.

Never committed. Never staged.
