# FORK_PATHS hardening (2026-07-24)

**Status (2026-07-30):** historical implement note. **Live `FORK_PATHS` authority
is `scripts/import-upstream-export.sh` + matching `scripts/assert-process-pins.sh`**
(not the snapshot list below). Live list also restores `scripts/recon-status.sh`
and `.grok/workflows` (among others). Product cargo harden:
[`doc/dev/upstream-regression-filters.md`](../upstream-regression-filters.md).

**Implements P0 from:** [`skills-survive-upstream-recon-2026-07-24.md`](skills-survive-upstream-recon-2026-07-24.md)
**Authority for list:** `scripts/import-upstream-export.sh` (`FORK_PATHS`)
**Assertion:** `scripts/assert-process-pins.sh` · `just upstream-assert-process-pins`

---

## Why

Import does `read-tree -u --reset <xAI tree>` then restores only `FORK_PATHS`
from `BASE_REF`. Paths not on that list and not in the xAI export are
**deleted**. Process pins that lived only in Surmount git were fragile:

| Was at risk | Role |
|-------------|------|
| `AGENTS.md` | Project Hard stop / onto recovery / residual pointer |
| `RESIDUAL.md` | Open human-intent tracker |
| `README.md` | Grok OSS branding (xAI README would win) |
| `scripts/join-main-into-onto.sh` | Land path after put-history |
| `scripts/with-ci-hermetic-path.sh` | Local CI PATH hermeticity |
| `doc/dev/**`, `docs/dev/**` | Operator research + RCA that must survive compaction + import |
| `.github/workflows/ci.yml` | Surmount quality gate (no release package in GHA) |

Host skills (`~/.agents/**`) and `~/.grok/AGENTS.md` were never at risk from
import; product-tree process law was.

---

## What changed

1. **Expanded `FORK_PATHS`** with comments (why each path is required).
2. **`scripts/assert-process-pins.sh`** — fails if required paths missing in
   worktree or a given tree-ish; light content sniffs for AGENTS/FORK/README.
3. **Import calls the assert after restore** — fail closed before import commit
   if pins did not come back.
4. **`just upstream-assert-process-pins`** — same check for post-onto / manual.
5. **Short note** in `docs/upstream-onto-log.md` (avoid thrashing
   `docs/upstream-history.md` if concurrent editors).

Not done here (P1+ from recon research): host skill rewrite for
`upstream-export-import`, FORK.md “what recon keeps” table, full checklist
rewrite in `upstream-history.md` (import script comments + this note + onto-log
carry the pin-survival story for now).

---

## Final `FORK_PATHS` list (2026-07-24 snapshot — do not treat as live)

**Superseded for day-to-day work.** Read `FORK_PATHS` from
`scripts/import-upstream-export.sh` and `REQUIRED_*` from
`scripts/assert-process-pins.sh`. Known adds after this note: at least
`scripts/recon-status.sh`, `.grok/workflows`.

```text
# product identity / packaging
FORK.md
CONTRIBUTING.md
SECURITY.md
README.md
justfile
flake.nix
flake.lock
packaging

# process pins
AGENTS.md
RESIDUAL.md

# living recon docs + research roots
docs/upstream-history.md
docs/upstream-import-log.md
docs/upstream-onto-log.md
docs/git-workflow.md
docs/dev
doc/dev

# recon + hermeticity scripts
scripts/detect-upstream-export.sh
scripts/import-upstream-export.sh
scripts/sync-upstream.sh
scripts/put-history-on-xai.sh
scripts/replay-onto-upstream.sh
scripts/join-main-into-onto.sh
scripts/with-ci-hermetic-path.sh
scripts/assert-process-pins.sh
# live also: scripts/recon-status.sh

# workflows + Surmount-only crates
.github/workflows/upstream-export.yml
.github/workflows/ci.yml
# live also: .grok/workflows
crates/codegen/grok-rate-limit
```

**Still not restored (by design):** seams inside `xai-grok-*` (OpenRouter,
binary rename, sampler rate-limit, DOGE default, titles-on, stuck-retry, …) —
re-apply via cherry-pick / `git diff $BASE_REF -- …` and **cargo filters**
([`doc/dev/upstream-regression-filters.md`](../upstream-regression-filters.md)).
User-guide under pager is upstream-owned path; product sections need conflict
resolve / re-apply on onto, not FORK_PATHS wholesale (would pin an entire
shared tree to Surmount and block legitimate upstream doc updates).

---

## Operator use

```bash
# After any import (also runs inside import-upstream-export.sh post-restore)
./scripts/assert-process-pins.sh

# After onto tip lands / before PR
./scripts/assert-process-pins.sh HEAD
./scripts/assert-process-pins.sh onto-xai/<short>

just upstream-assert-process-pins
```

---

## Related

| Path | Role |
|------|------|
| `scripts/import-upstream-export.sh` | `FORK_PATHS` + post-restore assert |
| `scripts/assert-process-pins.sh` | Presence check |
| `docs/upstream-onto-log.md` | Short survival note |
| `docs/upstream-history.md` | Canonical recon law + review checklist (product filters + user-guide) |
| `doc/dev/upstream-regression-filters.md` | Durable cargo filter catalog for `xai-grok-*` seams |
| `doc/dev/research/skills-survive-upstream-recon-2026-07-24.md` | Full recon × skills research |

*End of hardening note.*
