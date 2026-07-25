# Process docs landed (2026-07-24)

**Job:** pin HITL-parent / never-assume / skills multi-source / recon survival
into living product docs. Research inputs (read-only):

- `doc/dev/research/where-skills-come-from-2026-07-24.md`
- `doc/dev/research/skills-survive-upstream-recon-2026-07-24.md`
- `doc/dev/research/process-pin-targets-2026-07-24.md`

No git commit (human-only).

## Files changed

| Path | What landed |
|------|-------------|
| `AGENTS.md` | Parent = HITL UX only; never-assume / docs-lie; skills multi-source table; survive-recon pin list; kept hard stop / never-commit |
| `FORK.md` | Skills multi-source table; recon keeps/clobbers table; parent HITL pointer; dual-pin note for process vs host skill bodies |
| `docs/upstream-history.md` | Import checklist process pins + `FORK_PATHS` completeness; brief skills/process survival §; HITL-only + docs-lie under conflict HITL; subagent hard stop strengthened |
| `RESIDUAL.md` | Open item #6: import recon hardening (`FORK_PATHS` / post-import assert) |
| `crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md` | Short multi-source load note + process pins vs host skill bodies |
| `~/.grok/docs/user-guide/08-skills.md` | Mirror of product `08-skills.md` |

## Not done here (still residual / host)

- Host skill rewrites (`upstream-export-import` stale MODE text, skill-maintenance harness) — operator overlay, not this product-doc pass
- Global `~/.grok/AGENTS.md` A/B pins (cross-repo; out of this branch-doc scope unless asked)

## Verify later (human)

```bash
# process pins present (preferred)
./scripts/assert-process-pins.sh
# or: just upstream-assert-process-pins
```

## Close-out: residual #6 done (same day)

FORK_PATHS expansion + `scripts/assert-process-pins.sh` + `just upstream-assert-process-pins` already landed in product scripts (see `doc/dev/research/fork-paths-hardening-2026-07-24.md`). Residual open item **#6 (import recon hardening)** was therefore **removed** from `RESIDUAL.md` and recorded under *Not residual*; lasting truth lives in FORK § recon, AGENTS Survive recon (assert recipe), upstream-history import checklist + skills/process table, and the onto-log process-pin note. No new backlog invented.
