# Implement report: token economy proof multipoll harness (2026-08-08)

## Scope

Work A (multipoll evidence harness), Work B (economy proof checklist), Work C
(install + one live multipoll). Language pin: say **limits** not bare
"allowance"; say **credits** not bare "extras."

## Commands run

| Step | Command | Result |
|------|---------|--------|
| Hermetic multipoll TDD | `cargo test -p xai-grok-pager --lib -- multipoll` | 8 passed |
| Path checker still green | `cargo test -p xai-grok-pager --lib -- check_limits_first multipoll` | 19 passed, 1 ignored live |
| CLI parse | `cargo test -p xai-grok-pager --lib -- limits_parses_human_and_json limits_multipoll_parses` | 2 passed |
| fmt | `cargo fmt -p xai-grok-pager` | ok |
| clippy (lib) | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | ok |
| install | `just install` | `grok-oss 0.2.111` installed |
| Live multipoll | `just limits-multipoll` | exit 0 |

Clippy `--all-targets` on this branch still surfaces pre-existing test-only
lints in other modules (plan, render, session_startup, …). Touched product lib
path is clean under `-D warnings`.

## What shipped

### Work A — multipoll harness

- CLI: `grok-oss limits multipoll` (subcommand of existing `limits`)
  - `--samples` default **2**
  - `--sleep-secs` default **30** (matches `DEFAULT_MIN_WINDOW` flat detector)
  - `--out-dir` optional; default `.agents/reports/limits-multipoll-<utc>/`
- Just: `just limits-multipoll` (env `GROK_OSS_BIN`, `LIMITS_MULTIPOLL_*`)
- Pure classification (no network), names: `path_ok`, `free_period_stepped`,
  `FreePeriodSeriesClass::{Flat,Stepped,Insufficient}`,
  `classify_multipoll_samples`, `classify_free_period_series`
- Exit **0** when path OK/skipped; exit **non-zero only on path failure**.
  Free SuperGrok period flat never fails exit alone.
- Writes `samples.jsonl` (full limits JSON per line), `fields.jsonl` (compact
  evidence fields), `summary.json`.

### Work B — checklist docs

- User-guide [02-authentication.md](../../crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md)
  section **Token economy proof checklist** (complete sentences):
  1. `just check-limits-first-path`
  2. `just check-limits-first-live`
  3. multipoll before/after SuperGrok dogfood
  4. How to read P1 vs P2; flat 6% ≠ client off free SuperGrok period limits
- [04-slash-commands.md](../../crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md)
  documents `grok limits multipoll` / `just limits-multipoll`.
- `just check-limits-first-path` filter now includes `multipoll`.

### Language pin

- Project `AGENTS.md` hard constraint 4: limits/credits vocabulary + spend order.
- `RESIDUAL.md` open C4 bullet + meters speech note.

## Live multipoll result (Work C)

Ran after install:

```
just limits-multipoll
# samples: 2, sleep 30s between ends
# out: .agents/reports/limits-multipoll-20260808T102502Z/
# P1 path: OK
# P2 free SuperGrok period limits: flat (measurement only; not a path fail)
# flatPollUnprovenDebit (process history): true
```

**Do not claim free SuperGrok period burn is fixed.** Live multipoll measured
**flat** free SuperGrok period used % across the window. That is P2 unproven
debit evidence (measurement), not a client path failure. P1 path stayed OK
(SuperGrok session primary under free SuperGrok period headroom).

## Key paths

| Role | Path |
|------|------|
| Implementation | `crates/codegen/xai-grok-pager/src/limits_cmd.rs` |
| CLI parse tests | `crates/codegen/xai-grok-pager/src/app/cli.rs` |
| Just recipes | `justfile` (`limits-multipoll`, multipoll in path suite) |
| Live multipoll out | `.agents/reports/limits-multipoll-20260808T102502Z/` |
| This report | `.agents/reports/impl-token-economy-proof-harness-2026-08-08.md` |
