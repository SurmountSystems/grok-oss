# Process mop reverify: `/spend` ledger

**Date:** 2026-08-13
**Role:** `[process-mop]` only. No product edit.
**Prior mop:** [bug-process-mop-spend-ledger.md](bug-process-mop-spend-ledger.md)

Named product: `/spend` opens `grok_oss.db`, ingests session `usage.jsonl`, writes a `reconciliation_run`, and formats the live double-entry report. SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

`cfg.subagents.allow_worktree` is landed. This run was only fmt, clippy, and the two named tests.

## Result

**Green.** No spend-slice edit. Did not touch `welcome`, `settings_writes`, `actions.rs`, or `prompt_widget`.

## Commands and exit codes

Preferred `CARGO_TARGET_DIR=/tmp/grok-spend-reverify-target`. `/tmp` (45G tmpfs) filled during the first clippy/test batch (`No space left on device`). That target was removed. Later clippy and tests used `/home/hunter/.cache/grok-spend-reverify-target` on the home disk.

| Step | Command | Target | Exit |
|------|---------|--------|------|
| 1 | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | workspace default | **0** |
| 2 | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | `/tmp/grok-spend-reverify-target` | **0** |
| 3 | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | `/tmp/grok-spend-reverify-target` | **101** (ENOSPC writing `xai-grok-shell` rmeta) |
| 4 | `cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default` | `/tmp/grok-spend-reverify-target` | **101** (ENOSPC) |
| 5 | `cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation` | `/tmp/grok-spend-reverify-target` | **101** (ENOSPC) |
| 6 | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | home cache (first retry) | **101** mid-write: `PagerLocalSnapshot` missing fields in `dashboard.rs` and `prompt.rs` (not spend files; not mopped) |
| 7 | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | home cache | **0** |
| 8 | `cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default` | home cache | **0** (1 passed, 8824 filtered) |
| 9 | `cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation` | home cache | **0** (1 passed, 6569 filtered) |
| 10 | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | home cache (after snapshot fields landed) | **0** |

## Tests

- `app::dispatch::tests::status::show_spend_ingests_usage_jsonl_and_is_not_empty_default` **ok**
- `token_economy::spend_path_tests::spend_path_ingests_usage_jsonl_and_records_reconciliation` **ok**

## Slice mop

None. Failures were host ENOSPC on `/tmp`, then a short `PagerLocalSnapshot` mid-write in `dashboard.rs` / `prompt.rs`. Those are not spend-slice files (`status.rs`, `token_economy/mod.rs`, `ledger.rs`, `tests/status.rs`). Left them for the settings writers. After those fields landed, pager clippy and both named tests were green.

## Left alone

Did not edit:

- `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs`
- `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs`
- `actions.rs`, `prompt_widget`
- `dashboard.rs`, `prompt.rs`

## Residual for this mop

None. Named `/spend` fmt, clippy, and tests are green on the home-disk target.
