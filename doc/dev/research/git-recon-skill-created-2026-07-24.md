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

## FORK_PATHS / workflow survival (important)

| Path | In `FORK_PATHS` today? | Import behavior |
|------|------------------------|-----------------|
| Host `~/.agents/skills/git-recon/**` | N/A (outside tree) | Safe |
| `doc/dev/research/**` (this note) | Yes via `doc/dev` | Restored on import |
| Project `.grok/workflows/git-recon-status.rhai` | **No** | **Dropped** on import unless extended |

**If the status workflow must ride import as process infrastructure:**

1. Add `.grok/workflows` (or the specific `.rhai`) to `FORK_PATHS` in
   `scripts/import-upstream-export.sh`.
2. Extend `scripts/assert-process-pins.sh` REQUIRED list if it should be gated.
3. Dual-pin a one-line pointer in `docs/upstream-history.md` or `AGENTS.md`.

Until then: treat the Rhai file as **optional convenience on this machine /
when cherry-picked**; the **host skill** is the durable recon SOP.

Inventory §4.2 / §6 already predicted this gap — not fixed in product scripts
in this change set (out of scope).

---

## How to invoke

- Slash / skill: `/git-recon` (or natural language onto / put-history / join /
  cherry-pick continue).
- Workflow (when registered): `/git-recon-status` or workflow tool
  `name: "git-recon-status"` — status only.
- Direction / import seams still: `/upstream-export-import`.

---

## Non-claims

- Does not change put-history / join / import scripts.
- Does not add `recon-status.sh` product script (inventory optional follow-up).
- Does not edit branch `AGENTS.md` / `FORK.md` beyond this research note (skill
  dual-pin rule states when to touch them — process law already lives in
  upstream-history + AGENTS).
- Live stack SHAs move; re-read Live stack before land claims.
