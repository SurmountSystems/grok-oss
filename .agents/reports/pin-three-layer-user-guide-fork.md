# Pin: always three agent layers (user-guide + FORK)

Date: 2026-08-15
Scope: only `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` and `FORK.md`.

## Files

| File | Action |
|------|--------|
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | Edited |
| `FORK.md` | Edited (process inventory bullet now states always three layers) |

AGENTS.md, skills, and residual were not touched.

## Weaker text found and replaced

### User-guide Token efficiency

Was:

> Keep the main thread thin. An L2 that will do many greps, reads, or edits must spawn L3 specialists and keep a short window. Crossing about half the window: stop solo work, fan out, write a short report on disk. Do not compact-and-continue a long walk on L2.

Now: always three layers when work needs tools. Table for L1 / L2 / L3. L1 and L2 may only coordinate (spawn, board, wait, read the short report). The older softer rule is named as replaced.

### User-guide "When not to use" / "When to Use"

Was:

> Simple tasks that the parent can handle directly

> Researching a codebase while the parent continues other work

> Running tests in parallel while the parent implements changes

The simple-parent exception is gone. Use-case bullets no longer say the parent implements. The remaining "when not to use" line still allows skipping extra Isolated Agents for tight back-and-forth or setup cost, and it now says the main thread still does not take over the tools.

### FORK process bullet

Was:

> **Parent = HITL only** — main thread (agent **L1**) goals/spawn/reports/human git; research + implementation in subagents (**L2**); **L2 MUST spawn L3** on many greps/reads/edits (keep a short window; do not compact-and-continue on L2; **L3 max**, no deeper).

Now: **Parent = HITL only; always three layers (2026-08-15).** L1 coordinates only. L2 fans out and discards context. L3 does all tools and work. No L4. Simple and implement loops are not exceptions. Older weaker law named as replaced.

## Leftovers in these two files only

- **User-guide Depth Limits** still describes product nesting (`max_depth`, L1 can spawn L2, L2 can spawn L3, no L4). It does not restate the old "when many greps / half the window" rule. It still offers `[subagents] max_depth = 1` if only the main thread should spawn. That is a product config knob, not the old process exception.
- **User-guide "How Subagents Work"** still says the main agent identifies work to delegate and the spawned session gets tools. Product spawn mechanics, not the weaker L2-solo rule.
- **User-guide** still says "child session" / "child's output" in older sections. Unrelated to depth law; left alone.
- **FORK** still mentions L1 modal-free typing and short on-disk L2 reports in other product bullets. Those do not restate the weaker spawn-when-many-greps rule.

No compile. No git.
