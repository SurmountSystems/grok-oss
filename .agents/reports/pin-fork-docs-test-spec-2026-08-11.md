# Report: dual-pin tests-as-spec + confirm max niceness (2026-08-11)

## Status

Done. Minimal inserts only; no product code, no git.

## Operator intent covered

1. Keep FORK / AGENTS / residual / product docs honest when behavior or CI contracts move.
2. Tests are part of the spec; do not reshape expectations to broken product code.
3. Confirm max-niceness dual-pin near cargo-ci (already present; no second rewrite).

## What was already true (no edit)

| Pin | Where | Notes |
|-----|--------|--------|
| **Max niceness** | `FORK.md` after PATH hermeticity | **`Max niceness (local heavy work):`** — `cargo-ci` / `nix_retry` via `scripts/run-nice.sh` (`nice -n 19` + `ionice -c3`), escape `GROK_NO_NICE=1`, dual-pin AGENTS **3a** + justfile header. Matches prior report `.agents/reports/pin-fork-max-nice-2026-08-11.md` and feat `feat-max-nice-cargo-nix-just-2026-08-11.md`. |
| **Post-impl max-nice** | `AGENTS.md` hard constraint **3a** | Prefer `just cargo-ci cargo …` / `just test`; cites `scripts/run-nice.sh`. |
| **TDD / do not fit tests** | `AGENTS.md` **14** / **15** | Red→green named contract; do not reshape tests without evidence (host dual-pin). |
| **Regression filters** | `FORK.md` § *Upstream regression filters* | Product seams “stay honest through **cargo tests**”; catalog + cheat sheet already there. |

## Files changed

### 1. `/home/hunter/Projects/surmount/grok-build/FORK.md`

**Anchor:** § *Upstream regression filters*, immediately after the “stay honest through cargo tests / after recon run assert + filters” paragraph, before “Full filter catalog…”.

**Insert (one short block, ~4 sentences):**

- Heading phrase: **`Tests encode product contracts.`**
- CI/unit tests are part of the spec; prefer product fix over reshaping expectations.
- Cross-ref `AGENTS.md` hard constraints **14** / **15**.
- When product intent truly changes, update tests **and** FORK / residual / user-guide honesty in the same wave so a fork pin does not lie.

**Not bulk-rewritten:** surrounding product-seams list, filter catalog link, operator cheat sheet, PATH hermeticity, max niceness block, and rest of FORK untouched.

### 2. `/home/hunter/Projects/surmount/grok-build/AGENTS.md`

**Anchor:** hard constraint **15** (*Do not fit tests to code*), appended after the existing Host *Test intent* line.

**Insert (one short sub-bullet, ~4 sentences):**

- **`CI/unit tests encode product contracts`** (pinned 2026-08-11).
- Prefer product fix; only change a test when the named intended contract changed with evidence.
- When product intent changes, update tests **and** FORK / residual / user-guide honesty in the same wave if a fork pin would otherwise lie.
- Dual-pin: `FORK.md` § *Upstream regression filters*.

**Not bulk-rewritten:** constraints 3a / 14 and the rest of AGENTS untouched.

## Confirmation checklist

| Check | Result |
|-------|--------|
| FORK max niceness present (no new niceness paragraph) | Yes — pre-existing block dual-pins AGENTS 3a + `scripts/run-nice.sh` |
| FORK not bulk-rewritten | Yes — single short paragraph insert only |
| AGENTS not bulk-rewritten | Yes — append under existing **15** only |
| Tests-as-spec dual-pin FORK ↔ AGENTS | Yes — mutual cross-refs |
| Git commit/add/push | Not done |

## Out of scope (intentional)

- No residual Open edits (no open product slice; process dual-pin only).
- No user-guide changes (no behavior/UI string change this turn).
- No CI recipe / justfile / `run-nice.sh` code changes.
