# Restore `/spend` SQLite product wire (2026-08-13)

SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

Diagnosis was already on disk (`fork-gaps-sql-features-2026-08-13.md`). This turn restored the dropped `/spend` writer. Schema stays v1. No last-session / cancel-resume / todos / settings-registry work.

## Named contract

`/spend` (aliases `/double-entry` / `/ledger`) again:

1. Opens `grok_oss.db` fail-open (`try_open_from_token_economy_config`).
2. Ingests session `usage.jsonl` into `local_usage_event` (`refresh=true` on the spend path).
3. Persists Management prepaid / postpaid cache samples into `remote_meter_sample` when a management key and process cache are present.
4. Persists a `reconciliation_run` row.
5. Formats the real double-entry report (not `DoubleEntryReport::default()`).
6. `/limits` spend section still opens the DB with `refresh_from_sessions=false`.

## TDD

### Red (observed before product restore)

```bash
cargo test -p xai-grok-pager --lib show_spend_ingests_usage_jsonl_and_is_not_empty_default -- --nocapture
```

**Fail:** `assertion left != right failed: /spend must format the live ledger, not DoubleEntryReport::default()`

The dispatcher formatted `DoubleEntryReport::default()` and never opened the store.

### Green (same filter after restore)

```bash
cargo test -p xai-grok-pager --lib show_spend_ingests_usage_jsonl_and_is_not_empty_default -- --nocapture
```

**Pass.** The test plants a fixture `usage.jsonl` under process `grok_home()`, points live Token Economy at an isolated `grok_oss.db`, dispatches `Action::ShowSpend`, then checks:

- formatted body is not the empty default
- fixture `event_ulid` is in `local_usage_event`
- `reconciliation_run` has at least one row

Hermetic companion (temp grok home, no process home walk):

```bash
cargo test -p xai-grok-shell --lib spend_path_ingests_usage_jsonl_and_records_reconciliation -- --nocapture
```

**Pass.**

## What changed

`dispatch_show_spend` matches Surmount `main` again:

- Fail-open open via Token Economy config.
- Remote book from latest `management_usage_series` sample plus prepaid/postpaid process cache (inserts `remote_meter_sample` when a management key is on file).
- Included SuperGrok period context from the live credit balance.
- `run_spend_double_entry(..., grok_home)` with **refresh=true** (ingest + summarize + `reconciliation_run`).
- Formats that report into scrollback.

`/limits` still calls `build_double_entry_report` (refresh false).

## Files touched

- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs` — restore `/spend` writer
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/status.rs` — red/green dispatch test
- `crates/codegen/xai-grok-shell/src/token_economy/mod.rs` — `run_spend_double_entry`, ingest-under-home, persist helper, hermetic test
- `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs` — `local_usage_event_exists`, `count_reconciliation_runs`
- `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs` — one-line compile unblock (`footer` binding) so pager lib tests could run; not spend logic

## Post-impl

- `cargo fmt -p xai-grok-shell -p xai-grok-pager`
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` (ok)
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` (ok)
- Targeted tests above (ok)
- `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` still fails on **unrelated** in-flight files (subagent tests, auth config, ascii-scrub, rate-limit). Not introduced by this restore.

## Leftovers

- Operator-facing spend copy still says "free SuperGrok period" in `format_double_entry_report`. Language residual from the diagnosis, not a SQL table.
- `local_usage_event.sampling_identity` is still `None` on ingest (same as `main`).
- Prepaid/postpaid `remote_meter_sample` rows only write when a management key is present **and** the process cache is warm. No management-key fixture in these tests.
- Dispatch test plants under process `grok_home()` (OnceLock). Isolated DB path is temp. Guard removes the plant dir. The hermetic shell test is the one that does not walk a developer home.
- `/limits` still does not ingest. Left as specified.
- No schema v2. No last-session / cancel-resume / todos in SQL.
