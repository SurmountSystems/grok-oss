# Process mop: dual-auth hop after included SuperGrok period limits are full

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Tag:** `[process-mop]`  
**Primary:** `.agents/reports/bug-dual-auth-spend-hop-restore-2026-08-13.md`

SuperGrok is a paid product. This report says **included SuperGrok period limits**, never "free SuperGrok."

Backup only. Re-ran clippy and the ten named hop tests. No product edits. No fmt pass (primary already ran `cargo fmt -p xai-grok-shell`, exit 0). Did not touch plan.rs, render.rs, settings, user-guide, welcome, Token Economy schema, or `sampling_identity`. Did not `/rebuild`. Did not spawn further agents.

## Environment

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-dual-auth-hop-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

`/tmp` tmpfs is full. Used the mop target dir, not the primary's.

## Commands

| Step | Command | Exit |
|------|---------|------|
| 1. clippy | `cargo --offline clippy -p xai-grok-shell --lib -- -D warnings` | **0** |
| 2. named tests | `cargo --offline test -p xai-grok-shell --lib --` plus the ten filters below | **0** |

Clippy: first run, cold compile, finished in 2m 55s. `Finished dev profile`. No warnings under `-D warnings`.

Tests: incremental on the same mop target, finished in 4m 29s compile + 0.02s run.

```
cargo --offline test -p xai-grok-shell --lib -- \
  sampling_config_auto_use_fills_console_hop_after_included_full \
  sampling_config_auto_use_omits_console \
  resolve_model_to_sampling_config_auto_use \
  sampling_config_auto_use_extras_keep_session_console_failover \
  sampling_config_auto_use_omits_console_while_supergrok_included_headroom \
  sampling_config_api_key_pin_keeps_console_primary \
  resolve_model_override_agent_config_auto_use_omits_console \
  resolve_model_override_api_key_pin_keeps_console_primary \
  resolve_model_override_config_missing_parent_supergrok_only_omits_console \
  resolve_credentials_sets_auth_type
```

```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 6566 filtered out; finished in 0.02s
```

| Test | Result |
|------|--------|
| `agent::config::tests::resolve_credentials_sets_auth_type` | ok |
| `agent::config::tests::resolve_model_to_sampling_config_auto_use` | ok |
| `agent::config::tests::sampling_config_auto_use_omits_console` | ok |
| `agent::config::tests::sampling_config_auto_use_fills_console_hop_after_included_full` | ok |
| `agent::config::tests::sampling_config_auto_use_extras_keep_session_console_failover` | ok |
| `agent::subagent::tests::resolve_model_override_api_key_pin_keeps_console_primary` | ok |
| `agent::subagent::tests::resolve_model_override_config_missing_parent_supergrok_only_omits_console` | ok |
| `agent::models::tests::sampling_config_auto_use_omits_console_while_supergrok_included_headroom` | ok |
| `agent::models::tests::sampling_config_api_key_pin_keeps_console_primary` | ok |
| `agent::subagent::tests::resolve_model_override_agent_config_auto_use_omits_console` | ok |

## Edits

None. Clippy and the named tests were already green. No fallout to mop.

## Leftovers (from primary; not this mop)

- Live TUI still needs a rebuild/install before dogfood shows the hop. This slice is unit tests only.
- `sampling_identity` SQLite column is still unused. Schema exists. Ingest still writes `None`. Token Economy schema was out of scope.

Stop. Clippy and named tests are green.
