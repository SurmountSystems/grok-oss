# Dual-auth hop after included SuperGrok period limits are full

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Branch:** `onto-xai/b13fa526f511`  
**Board:** `residual:dual-auth-spend-order-after-103`

SuperGrok is a paid product. This report says **included SuperGrok period limits**, never "free SuperGrok."

## Contract

While included SuperGrok period limits still have room, stay on the SuperGrok session. Do not make the console API key primary, and do not put it in the hop list.

After those included SuperGrok period limits are full:

1. SuperGrok dollar credits, when that identity has a known positive extras balance: SuperGrok session stays primary; console is failover only.
2. SuperGrok dollar credits 0 or unknown: console team prepaid / console API credits become primary; SuperGrok JWT stays on the chain as recovery failover.
3. `preferred_method=api_key` still pins console primary.

This is not last-session-on-start, not `/spend` ingest, not Settings rows.

## What was wrong

`[auth] auto_use_included_limits` was already read for rank, limits, and doctor. `order_credentials_for_preferred_auto` and AuthManager multi-slot pick were already in the tree.

The restack loss was the **hop list**, not an unread bool.

Ruled out:

- Rank empty. `resolve_credentials_preferring_with_supergrok_sessions` already filled `ResolvedCredentials.failover_api_keys` (extras red kept SuperGrok primary).
- Bool unread. `GrokComConfig.auto_use_included_limits` default true; rank keys off it.

The empty hop came from `sampling_config_for_model` stamping `failover_api_keys: Vec::new()` (and the same empty fields for host/identity). Live paths then used bare `resolve_credentials` (`auto_use=false`, classic session+console) or dropped the preferring flags.

I do not claim a deeper unknown cause. That stamp plus the un-wired preferring call sites is the evidence.

## TDD

Named red test: `sampling_config_auto_use_extras_keep_session_console_failover`.

Included remaining 0, SuperGrok dollar credits `$100.29`, console env key present. SuperGrok session must stay primary. Console must appear in `failover_api_keys`. The test does not accept an empty hop.

**Red** (stamp still empty; preferring already filled creds):

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-dual-auth-hop-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-shell --offline --lib -- \
  sampling_config_auto_use_fills_console_hop_after_included_full \
  sampling_config_auto_use_omits_console \
  resolve_model_to_sampling_config_auto_use \
  sampling_config_auto_use_extras_keep_session_console_failover
```

- `sampling_config_auto_use_extras_keep_session_console_failover` **FAILED**  
  `console must be failover while SuperGrok dollar credits remain: []`
- The other three passed. Included full + extras unknown makes **console primary**, so those asserts can pass with an empty failover list. They do not cover the extras hop.

**Green** (same filter plus ModelsManager / subagent siblings, after the stamp copy and live-path preferring wire):

```
cargo test -p xai-grok-shell --offline --lib -- \
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

`ok. 10 passed; 0 failed`

Post-impl: `cargo fmt -p xai-grok-shell` exit 0. `cargo clippy -p xai-grok-shell --offline --lib -- -D warnings` exit 0.

## Which meter hops to which

| Situation | Primary | Hop list |
|-----------|---------|----------|
| Included SuperGrok period limits still have room | SuperGrok session | Console omitted |
| Included full, SuperGrok dollar credits known positive | SuperGrok session | Console team prepaid / console API credits |
| Included full, extras 0 or unknown | Console team prepaid / console API credits | SuperGrok session as recovery |
| `preferred_method=api_key` | Console | SuperGrok session as failover when a session exists |

No new hop system. Rank + AuthManager already pick slots. This slice copies those fields onto `SamplerConfig` / chat-state credentials and passes live `auto_use_included_limits` into preferring.

## Product fill

1. Restore `resolve_credentials_preferring*` (already in tree from the red setup).
2. `sampling_config_for_model` copies `failover_api_keys`, `failover_base_url`, `session_base_url`, `session_identity_key`.
3. `ModelsManager::sampling_config` and `prepare_sampling_config_for_model` call `resolve_credentials_preferring_with_rank` with live preferred + auto_use. Prepare also `align_to_ranked_free_period_primary` when auto_use is on, and still surfaces a session under the api_key pin so console can keep SuperGrok as failover.
4. Mid-session model switch in `run_loop.rs` copies hop fields from `try_resolve_model_credentials` (that helper already used preferring).
5. Subagent model override uses preferring + parent/disk auto_use flags. After the stamp fill, bare `resolve_credentials` put console in failover while included SuperGrok period limits still had room; `resolve_model_override_agent_config_auto_use_omits_console` failed; preferring with rank made that sibling green. The test was not weakened.

## Files touched

- `crates/codegen/xai-grok-shell/src/agent/config.rs`
- `crates/codegen/xai-grok-shell/src/agent/config_tests.rs`
- `crates/codegen/xai-grok-shell/src/agent/models.rs`
- `crates/codegen/xai-grok-shell/src/agent/models/tests.rs`
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs`
- `crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/run_loop.rs`

## Leftovers

- **`sampling_identity` SQLite column is still unused.** Schema `local_usage_event.sampling_identity` exists in `grok_oss/mod.rs`. `UsageJsonlRow` has no such field. Ingest always writes `sampling_identity: None`. Not filled here. Token Economy schema was out of scope.
- **Host `GROK_HOME` OnceLock** can rank a real SuperGrok JWT in ModelsManager tests. Those tests assert "not the fixture console key," not a specific fixture token.
- **Empty `Vec::new()` hop remains** on `ResolvedCredentials::default`, the web-search default base when no sampling config is passed, and ACP test fixtures. Those are defaults/fixtures, not the restack stamp.
- **Language residual** in FORK / some doctor strings still says "free SuperGrok period." Not this slice.
- **Live TUI** still needs a rebuild/install before dogfood shows the hop. This slice is unit tests only. No `/rebuild`.

No product decision is parked. The hop chain is filled and the named tests are green.
