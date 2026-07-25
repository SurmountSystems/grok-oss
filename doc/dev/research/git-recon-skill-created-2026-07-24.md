# git-recon skill created (2026-07-24)

**Mode:** durable host skill + optional status workflow + join note.  
**No product commit** (agent never commits). No overlay modes invented.

Implements the six capability bullets from
[`workflow-skill-git-recon-inventory-2026-07-24.md`](workflow-skill-git-recon-inventory-2026-07-24.md)
§7 as an operator skill, not a second competing direction doc.

---

## What landed

| Artifact | Path | Role |
|----------|------|------|
| Host skill | `~/.agents/skills/git-recon/SKILL.md` | SOP: status → route → conflict → stage → human-sign → land |
| Conflict ref | `~/.agents/skills/git-recon/references/conflict-fanout.md` | Preference table + ≤3-bucket fan-out + child prompt |
| Hand commands | `~/.agents/skills/git-recon/references/hand-commands.md` | Signed continue / join / feature-merge paste templates; signing boundary |
| Status workflow | `project/.grok/workflows/git-recon-status.rhai` | Optional probe only (`await`-free skeleton; agent execute) |
| Cross-link | `~/.agents/skills/upstream-export-import/SKILL.md` | Points mid-stack conflict labor at **git-recon** |

**Internal todo namespaces** (skill only; user-facing language stays plain):

`recon:status` · `recon:route` · `recon:conflict` · `recon:stage` ·
`recon:human-sign` · `recon:land`

---

## Policy encoded (must not drift)

- Agents **never** `git commit`; human `git commit -S` / `cherry-pick --continue` on TTY.
- **Every continue is a commit** under current hooks — “sign only final tip” fails (pre-commit refuse + post-commit soft-reset).
- No GPG bypass; no `MODE=overlay` / commit-tree.
- Parent HITL only; spawn-first on multi-file UU and post-pick CI.
- Dual-pin: process law changes update **branch** `AGENTS.md` / `FORK.md` /
  `docs/upstream-*` as well as host skill.

Canonical HITL: `docs/upstream-history.md` § *HITL runbook* + *Live stack*.

---

## FORK_PATHS / workflow survival (updated Slice 5 / W4)

| Path | In `FORK_PATHS` / assert? | Import behavior |
|------|---------------------------|-----------------|
| Host `~/.agents/skills/git-recon/**` | N/A (outside tree) | Safe |
| `doc/dev/research/**` (this note) | Yes via `doc/dev` | Restored on import |
| `scripts/recon-status.sh` | **Yes** (`FORK_PATHS` + `REQUIRED_FILES`) | Restored on import |
| Project `.grok/workflows/` (incl. `git-recon-status.rhai`) | **Yes** (dir in `FORK_PATHS` + `REQUIRED_DIRS`) | Restored when present on base |

**Status probe preference:** `./scripts/recon-status.sh` / `just recon-status`
first (skill § `recon:status`); Rhai workflow is optional agent-execute
skeleton only. Dual-pin: FORK Process **Git recon depth**,
`docs/upstream-history.md` HITL sequence, host skill.

Follow-up join for the shell probe:
[`recon-status-script-2026-07-24.md`](recon-status-script-2026-07-24.md).

---

## How to invoke

- Slash / skill: `/git-recon` (or natural language onto / put-history / join /
  cherry-pick continue).
- Status (prefer): `./scripts/recon-status.sh` or `just recon-status`.
- Workflow (when registered): `/git-recon-status` or workflow tool
  `name: "git-recon-status"` — status only; prefer shell script when present.
- Direction / import seams still: `/upstream-export-import`.

---

## Non-claims (skill-create change set; Slice 5 filled gaps)

- Skill create did not invent put-history modes or auto-continue.
- Slice 5 **did** add `scripts/recon-status.sh`, `just recon-status`,
  FORK_PATHS/assert pin, and dual residual FORK/RESIDUAL honesty.
- Live stack SHAs move; re-read Live stack / **script output** before land
  claims.
