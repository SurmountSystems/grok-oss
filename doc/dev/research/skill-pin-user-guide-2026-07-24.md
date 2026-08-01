# User-guide pin — Token efficiency + parent coordinator

**Date:** 2026-07-24  
**Scope:** product user-guide `16-subagents` (+ light onto history bullet)  
**Closes:** inventory gap — AGENTS/help/implement linked `16-subagents.md` § *Token efficiency* but the section was missing.

## Edits

| Path | in_repo? | Change |
|------|----------|--------|
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | **Y** | Added § **Token efficiency** (parent coordinator, spawn-first, join on disk, depth-1, soft quality band, parallel without waste). Extended **When to Use** with CI/regression bullet. |
| `~/.grok/docs/user-guide/16-subagents.md` | N (install/runtime mirror) | Same content as in-repo product source. |
| `docs/upstream-history.md` | **Y** | One hard **spawn-first** anti-pattern bullet for multi-file conflict + post-pick CI; one-line hard stop under subagents section. |

## § Token efficiency — bullets shipped

- Parent coordinates; children own heavy research/fix (multi-file, CI logs, root cause).
- **Spawn first** on CI failure / regression / multi-file diagnosis — no parent log pull or failing-test open first.
- Join on short on-disk summaries only; no re-grep after child summary.
- Depth is one (children cannot spawn); hierarchical = parent layers specialists.
- Soft quality band: parent context is expensive; keep parent as coordinator budget.
- Parallelism without waste (disjoint scopes; no identical fan-out).
- Product-generic note: stricter operator hard stop lives in project/host `AGENTS.md` (no hard dependency on `~/.grok` path in product prose).

## Anchor for links

Heading is exactly `## Token efficiency` so existing pointers remain valid:

- `~/.grok/AGENTS.md` → `~/.grok/docs/user-guide/16-subagents.md` § *Token efficiency*
- help / implement skills that link the same section

## Not done here (other inventory ranks)

Skills under `~/.agents/skills/**` (deep guide, `_SKILL_RULES`, pr-babysit, implement Hard stop bullets, etc.) are **out of this task** — home-dir skill git, not grok-build product.

## Return

- Product guide (tracked): `$REPO/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md`
- Home mirror: `~/.grok/docs/user-guide/16-subagents.md`
- Onto history: `$REPO/docs/upstream-history.md`
- This note: `$REPO/doc/dev/research/skill-pin-user-guide-2026-07-24.md`
