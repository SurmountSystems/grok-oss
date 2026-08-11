# Fix two hard test fails (2026-08-09)

## Summary

Both hard fails from the full suite paste are green after targeted product/test
updates. Flaky ACP load test was re-run once and still failed; left alone (no
clear red→green without drive-by).

## Fail 1: `shell_collision_contract_covers_every_pager_command_and_alias`

| | |
|--|--|
| **Package** | `xai-grok-pager` |
| **Red** | `unreserved pager key rebuild` |
| **Root cause** | New `/rebuild` builtin was registered without adding `rebuild` to the static `SHELL_RESERVED` collision table in the contract test. Every pager slash name/alias must be reserved so shell and pager do not collide. |
| **Fix** | Add `"rebuild"` to `SHELL_RESERVED` (alphabetical). Product command already correct. |
| **File** | `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs` |

### Green

```bash
cargo test -p xai-grok-pager --lib slash::commands::tests::shell_collision_contract_covers_every_pager_command_and_alias
# ok
```

## Fail 2: `new_scope_takes_precedence_over_legacy`

| | |
|--|--|
| **Package** | `xai-grok-shell` |
| **Red** | `left: "legacy-key" right: "new-key"` — new OAuth scope should win over legacy store key |
| **Root cause** | `AuthManager::new` (default `auto_use_included_limits = true`) calls `align_to_ranked_free_period_primary()`. That loads free SuperGrok period candidates from all SuperGrok-session-mode entries. When both OAuth base and legacy `https://accounts.x.ai/sign-in` held tokens with empty `user_id` / `team_id`, identity_id fell back to the **store scope string**, so they looked like two principals. Equal headroom + lex identity_id ranked legacy first; align `hot_swap`ped the intentional new-scope primary onto `legacy-key`. |
| **Fix (product)** | In `load_supergrok_session_candidates`, **skip `LEGACY_SCOPE`**. Legacy is pre-OIDC storage fallback only, not a dual SuperGrok principal (personal vs team multi-slots remain the real dual case). |
| **Regression test** | `load_supergrok_candidates_skips_legacy_scope_when_oauth_base_present` |
| **Files** | `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` |

### Green

```bash
cargo test -p xai-grok-shell --lib new_scope_takes_precedence_over_legacy
cargo test -p xai-grok-shell --lib load_supergrok_candidates_skips_legacy
# also still green:
cargo test -p xai-grok-shell --lib align_to_ranked_free_period_primary_switches
cargo test -p xai-grok-shell --lib auth_manager_new_auto_use_aligns
cargo test -p xai-grok-shell --lib legacy_scope_fallback_reads_old_auth_json
```

## Flaky (not fixed)

```
xai-grok-pager app::acp_handler::tests::queue_and_adoption::session_loaded_with_synthetic_running_prompt_id_stays_idle
```

Re-ran once after the hard fixes:

```
synthetic non-scheduler running prompt must not be adopted on load
```

Still failed in this session. Operator paste already marked it flaky (2/2 pass
on retry in full suite). No clear isolated red→green without broader cancel-resume
/ session-load work; left as residual flake.

## Verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager -p xai-grok-shell` | done |
| `cargo clippy -p xai-grok-shell --lib -- -D warnings` | clean |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | clean |
| `--all-targets` clippy on those packages | pre-existing fails elsewhere (not from this change) |

No git add / commit / push.
