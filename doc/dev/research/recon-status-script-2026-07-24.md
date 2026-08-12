# recon-status script (Slice 5 depth) — 2026-07-24

**Goal:** One-shot read-only probe for onto / cherry-pick / merge state so
agents and humans do not invent modes or guess from stale Live stack docs.

## Delivered

| Item | Path |
|------|------|
| Script | `scripts/recon-status.sh` |
| Just recipe | `just recon-status` |
| Host skill prefer | `~/.agents/skills/git-recon/SKILL.md` § `recon:status` |
| Import survival | `FORK_PATHS` in `scripts/import-upstream-export.sh` |
| Assert pin | `scripts/assert-process-pins.sh` REQUIRED_FILES |
| HITL mention | `docs/upstream-history.md` § Full sequence + import checklist |

## Prints (fixed fields)

- `branch`
- `CHERRY_PICK_HEAD` yes/no (worktree-safe via `git rev-parse --git-path`)
- `MERGE_HEAD` yes/no
- `sequencer` yes/no
- `unmerged` count (+ up to 40 paths when >0)
- `onto-ish` yes/no (`onto-xai/*`) with branch name when yes
- `main_ancestor` yes/no/unknown
- `dirty_worktree` yes/no
- `next` — single recommended **human** action only

## Next-action policy (no invent modes)

| Condition | `next` gist |
|-----------|-------------|
| UU + cherry-pick/sequencer | resolve UU → human `git cherry-pick --continue` |
| UU + merge | resolve UU → human `git commit -S` |
| Clean cherry-pick | human continue (+ CONTINUE=1 put-history if stack continues) |
| MERGE_HEAD, no UU | human `git commit -S` (join already staged) |
| onto-xai/*, main not ancestor | `join-main-into-onto.sh` then signed join commit |
| onto-xai/*, main is ancestor | clean land path (assert pins, `just check`, push/PR if asked) |
| else | clean; route via detect / put-history / import |

**Does not:** commit, abort, `FORCE=1`, overlay language, invent SHAs.

## How to run

```bash
./scripts/recon-status.sh
# or
just recon-status
```

## Sample output (this worktree, 2026-07-24)

```
branch:           onto-xai/6e386420825b
CHERRY_PICK_HEAD: no
MERGE_HEAD:       no
sequencer:        no
unmerged:         0
onto-ish:         yes (onto-xai/6e386420825b)
main_ancestor:    yes
dirty_worktree:   yes
next:             clean recon state (onto tip; main is ancestor). Land: ./scripts/assert-process-pins.sh HEAD && just check; push/PR only if asked
```

Note: Live stack prose can lag (e.g. still saying MERGE_HEAD staged). **Script
output is live truth** for recon:status.

## Skill / workflow relationship

- **Prefer script** for status; skill documents fallback ad-hoc git commands if
  script missing mid bare-tip stack.
- Optional Rhai `.grok/workflows/git-recon-status.rhai` remains agent-execute
  skeleton; not required when the shell script is present.

## Survival

Must stay in product git + import restore:

1. `scripts/recon-status.sh` listed in `FORK_PATHS`
2. Same path in `assert-process-pins.sh` REQUIRED_FILES
3. Host skill points at script (host is outside product git; dual-pin HITL in
   `docs/upstream-history.md`)
4. Dual residual: FORK Process **Git recon depth**; RESIDUAL “Not residual”
   line for git-recon depth (not open residual)

Worktree assert: `./scripts/assert-process-pins.sh` (no arg). `… HEAD` only
passes after the script is committed on that tip.

## Non-goals (this slice)

- No agent auto-continue / auto-join
- No FORCE rebuild helper
- No MODE=overlay / commit-tree
- No agent `git commit`

*End of join note.*
