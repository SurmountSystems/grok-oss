# Included SuperGrok period limits language

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** leftover product language after the 1.0.3 restore wave

SuperGrok is a **paid** product. This report says **included SuperGrok period limits**, never "free SuperGrok."

Named contract: user-visible chrome, user-guide, FORK product bullets, doctor/limits copy, and `/spend` report strings use **included SuperGrok period limits** (or short UI **SuperGrok period limits** / **included SuperGrok period · N%**). Forbidden: "free SuperGrok", "free SuperGrok period", "free SuperGrok allowance."

This was a named language contract change (evidence: host/project `AGENTS.md` + implement prompt). Product strings changed first, then matching asserts. Hop logic, `/spend` ingest, plan chrome, and settings catalog rows were not changed. Wire/config key names were not changed.

## Before / after (one chrome string)

`/limits` **Active:** line, from `ActiveSpendDriver::as_human()`:

- Before: `Active: free SuperGrok period`
- After: `Active: included SuperGrok period limits`

Compact status was already `included SuperGrok period limits · N%` (cold `...%`). Active-line copy now uses the same meter name.

## Files touched

Product chrome / doctor / limits / spend:

- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` (`as_human`, compact/Active comments, matching asserts)
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` (`NOTE_*` copy, doctor dogfood block, matching asserts)
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` (Active-line assert + rustdoc)
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` (multipoll `P2` line + `activeDriverLabel` / human Active asserts)
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` (doctor Prefer / debit-unproven lines + tests)
- `crates/codegen/xai-grok-shell/src/auth/free_period_debit_unproven_guard.rs` (block toast/ACP copy + test)
- `crates/codegen/xai-grok-shell/src/auth/config.rs` (rustdoc next to `auto_use_included_limits` and `allow_spend_when_free_period_debit_unproven`; keys unchanged)
- `crates/codegen/xai-grok-shell/src/token_economy/period_pacing.rs` (`full_sentence`)
- `crates/codegen/xai-grok-shell/src/token_economy/reconcile.rs` (`/spend` double-entry headings)
- `crates/codegen/xai-grok-shell/src/token_economy/config.rs` (rustdoc on `show_period_pacing`)

User-guide / standing law / residual:

- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` (compact example matches chrome)
- `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md` (ban wording)
- `AGENTS.md` (meters vocabulary; SuperGrok is paid)
- `FORK.md` (operator-facing product bullets + dogfood compact string)
- `RESIDUAL.md` (vocabulary + 1.0.3 inventory honesty)
- `doc/dev/upstream-regression-filters.md` (compact string no longer says language is pending)

## Residual honesty

`RESIDUAL.md` open inventory no longer lists finished restore work as dropped:

- Dual-auth hop after included SuperGrok period limits are full is restored in source.
- Unread config fields, `/settings` rows, `/spend` ingest, leftover plan chrome are restored in source.

Honest leftovers kept open:

- `sampling_identity` unused
- host `~/.grok/docs` extract stale until the next product launch
- live TUI stays the old 1.0.3 binary until a successful rebuild/install
- C4 included SuperGrok period debit remains a server ticket (dogfood after rank-align)

## Leftover sites left on purpose

Wire / config / enum names (not renamed):

- JSON `activeDriver` value `supergrok_free_period`
- `[auth] allow_spend_when_free_period_debit_unproven`
- env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN`
- enum `ActiveSpendDriver::SuperGrokFreePeriod`
- helpers such as `classify_free_period_series`, `align_to_ranked_free_period_primary`

Comments and test *messages* still say "free SuperGrok period" in many internal rustdocs and assert messages (credit_bar footer tests, hop rank comments, sampler comments). Those are not operator chrome. One test still *bans* the old paint: `status_bar_pushes_credits_compact_included_supergrok_period_limits` / render `!text.contains("free SuperGrok period")`.

User-guide `02-authentication.md` and `04-slash-commands.md` still say automatic host hop after included SuperGrok period limits are full is **not** shipped. That hop was restored in source this wave. This pass did not rewrite hop behavior docs.

Host `~/.grok/docs` was not touched.

## Tests

Named language asserts were updated (not weakened). Hop tests were not edited.

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-limits-language-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
```

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 |
| `cargo clippy -p xai-grok-pager --offline --lib -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-shell --offline --lib -- -D warnings` | 0 |
| pager `--lib` chrome/honesty filters (8 tests: Active driver, Work C compact, honesty notes, limits JSON) | 0 (8 passed) |
| pager `--lib` compact + loading + status credits (3 tests) | 0 (3 passed) |
| pager `--lib` `format_supergrok_session_with_weekly_and_extras` + C6 + license honesty (3 tests) | 0 (3 passed) |
| shell `--lib` doctor + block message + `/spend` report + pacing (7 tests) | 0 (7 passed) |

Cold `--no-run` of both crates first died at 300s; incremental retry finished. Clippy used the same target dir.

No `git add` / commit / push. No `/rebuild`.
