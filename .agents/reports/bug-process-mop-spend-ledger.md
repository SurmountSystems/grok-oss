# Process mop: `/spend` ledger restore

**Date:** 2026-08-13
**Role:** `[process-mop]` only. No new product work.
**Implementer report:** [bug-spend-ledger-restore-2026-08-13.md](bug-spend-ledger-restore-2026-08-13.md)

Named product: `/spend` again opens `grok_oss.db`, ingests session `usage.jsonl`, writes a `reconciliation_run`, and formats the live double-entry report (not the empty default). SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| 1 | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | **0** |
| 2 | `cargo clippy -p xai-grok-pager --lib -- -D warnings` (first) | **101** |
| 3 | `cargo clippy -p xai-grok-shell --lib -- -D warnings` (first, parallel with 2) | **101** |
| 4 | `cargo clippy -p xai-grok-shell --lib -- -D warnings` (retry) | **101** |
| 5 | `cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default` | **101** |
| 6 | `cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation` | **101** |
| 7 | `cargo clippy -p xai-grok-pager --lib -- -D warnings` (second retry) | **101** |

Steps 2 through 7 all stop at the same compile error before clippy lints or tests run:

```
error[E0609]: no field `subagents` on type `&mut util::config::mcp::Config`
 --> crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs:233:29
    |
233 |     update_config(|cfg| cfg.subagents.allow_worktree = value).await
    |                             ^^^^^^^^^ unknown field
```

`Config` in `crates/codegen/xai-grok-shell/src/util/config/mcp.rs` has `cli`, `models`, `ui`, `harness`, `skills`, and seven other fields. It has no `subagents` field.

## Slice mop

No spend-slice edit. The failure is not in the `/spend` files.

Read the listed spend files. The named contracts are present (`dispatch_show_spend`, `run_spend_double_entry`, `local_usage_event_exists`, `count_reconciliation_runs`, both named tests). Nothing in those four files needs a mop from this run.

## Left alone (not this slice)

Did not edit:

- `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs` (the compile break)
- `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs` (implementer one-line compile unblock)
- settings/, title.rs, session_startup, builder.rs, session_notification

Those belong to the branding and config writers.

## Tests

Neither named test compiled. Exit 101 is the settings `Config.subagents` break, not a spend-contract fail.

## Residual for this mop

Clippy and the named `/spend` tests cannot run until the config writer lands `Config.subagents` (or stops writing `cfg.subagents.allow_worktree`). After that lands, re-run steps 2 through 6. This mop did not change product code.
