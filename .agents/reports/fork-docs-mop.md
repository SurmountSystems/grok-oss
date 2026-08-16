# Process mop after FORK / docs / process write + review fix

**Date:** 2026-08-15  
**Role:** L3 process mop. No git add / commit / push. No rebuild or quit.

## Whether any `*.rs` changed in this work

**No.** This slice is markdown, skills, scripts, and `justfile` only.

Files this write actually touched (mtime 15:22–15:41, plus reports):

- `FORK.md`
- `doc/dev/upstream-regression-filters.md`
- `docs/upstream-history.md`
- `justfile`
- `scripts/assert-process-pins.sh`
- `scripts/import-upstream-export.sh`
- `scripts/recon-status.sh`
- host `~/.agents/skills/git-recon/SKILL.md` and
  `~/.agents/skills/upstream-export-import/SKILL.md` (outside product git)

No `*.rs` under `crates/` has mtime after 15:00 today. Dirty Rust in the
working tree is older product work, not this slice.

**Skipped:** `cargo fmt`, `cargo clippy`, and targeted cargo tests. Nothing
to mop on that path.

## Commands + exit codes

| Command | Exit | Notes |
|---------|------|--------|
| `./scripts/assert-process-pins.sh` | **0** | Worktree. Catalog titles present. |
| `./scripts/assert-process-pins.sh HEAD` | **1** | Committed catalog is the old pre-write file. No numbered class headings. |
| `just upstream-land-filters` | **0** | Worktree assert + reminder. No cargo. |
| `just upstream-land-filters HEAD` | **1** | Same HEAD catalog-title miss; recipe stops at the assert. |

Worktree assert stdout:

```
assert-process-pins: checking worktree at /home/hunter/Projects/surmount/grok-build
WARN:
  - AGENTS.md present but missing expected 'parent is coordinator' pin
OK: all required process-pin paths present (25 files + 5 dirs).
```

HEAD assert stdout (seven missing title markers):

```
assert-process-pins: checking tree HEAD
FAIL: process-pin paths missing (7):
  - doc/dev/upstream-regression-filters.md (missing land class title: ### 1. CLI identity)
  - … through ### 7. Product skills are not a Python runtime
```

`just upstream-land-filters` (worktree) printed the seven-class reminder and
did **not** invoke cargo. Recipe is assert then `@echo` only.

### HEAD fail: not a script / catalog mop

`HEAD` still has the previous catalog (product-filter sections, no
`### 1.`…`### 7.` land titles). The worktree catalog has those titles. The
new title sniff is doing what it should: a tree-ish without the seven
headings fails. Softening the assert or inventing a second inventory would
hide that. After this catalog is on the tree-ish being checked, `HEAD` /
onto tip should pass the same markers.

Did not rewrite `FORK.md`. No land-title mismatch on the worktree: class 7
still says a restack that installs non-excepted Python is a failed land.

### AGENTS warn (not a fail)

`AGENTS.md` wraps `parent is` / `coordinator only` across two lines, so the
single-line sniff does not match. That file was not part of this write
(mtime 12:21). Warn only. Left it.

## Catalog coherence

`doc/dev/upstream-regression-filters.md` (worktree) is coherent.

**Seven product class headings exist:**

1. `### 1. CLI identity`
2. `### 2. Config is a surface, not a field`
3. `### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)`
4. `### 4. DOGE / Surmount chrome (paint)`
5. `### 5. Dual-auth hop after included SuperGrok period limits are full`
6. `### 6. Last-session on start`
7. `### 7. Product skills are not a Python runtime`

Class 3 still **starts with** `### 3. grok-oss SQL extras`. Assert marker
`### 3. grok-oss SQL extras` still matches via `grep -F`.

**Operator cheat sheet class 5** includes:

- after-burner: `afterburner_does_not_skip_mark_when_sibling_has_included_remaining`
- Business / Team pick: `pick_prefers_business_included_before_personal_when_both_have_remaining`,
  `order_credentials_business_included_before_personal_when_both_have_room`
- flock / snapshot hub: `limits_snapshot_second_process_reads_file_and_does_not_http`,
  `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`,
  `limits_snapshot_never_writes_access_tokens`,
  `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http`
- combined remaining: `combined_included_remaining_sums_distinct_personal_and_business_pools`,
  `combined_included_remaining_does_not_double_count_unified_pool`
- `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`

**No-`fn` honesty leftovers are not required-land cargo.** They appear only
as honesty notes (product filter catalog + cheat-sheet name-check “do not
`rg`” list). Not in Required land tables. Not in cheat-sheet cargo blocks.

Named leftovers: `retry_chrome_soft_reconnects_when_retry_stream_starts`,
`stream_resumed_without_prior_retry_clears_activity`, `clip_retry_reason_*`,
`retrying_activity_label_*`, `retrying_label_shows_timeout_*`,
`shell_collision_contract_covers_every_pager_command_and_alias`,
`default_title_items_include_agents`, `title_escape_never_empty_payload`,
`title_updates_gated_only_by_title_enabled`.

## What was mopped

**Nothing to mop.** Worktree assert green. Catalog titles, class 3 prefix,
class 5 cheat sheet, and honesty leftovers are already correct. HEAD fail is
the old committed catalog, not a worktree mismatch. Did not touch `FORK.md`.
Did not run cargo.
