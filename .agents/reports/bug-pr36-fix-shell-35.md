# PR #36 `xai-grok-shell` nextest: five + team_managed (35)

**Date:** 2026-08-13
**Agent:** L2 implementer (product only, `xai-grok-shell`)
**Names:** `.agents/reports/bug-pr36-ci-2174fd75-fails.md`
**GHA:** run 31680531078, SHA `2174fd75`

## Status: green

The prior local mop that called `team_managed_config` (~30) and some auth tests
"likely GHA-clean" (operator `GROK_HOME` OnceLock) was **wrong**. Those 30
failed on a clean GitHub Actions workspace. This slice fixes product +
hermetic test-support so the named contracts pass without operator `~/.grok`.
No skip-on-CI. No weaker asserts.

## Red (observed before product edit)

rustc 1.97.1 (8bab26f4f 2026-07-14).

```
cargo nextest run -p xai-grok-shell --locked --test-threads=2 --build-jobs 2 \
  -E 'test(team_sync_writes_files)'
```

- `FAIL` 62ms. `team_managed_config.rs:344` `assert!(wrote)`:
  `expected team config to be written`.

```
cargo nextest run -p xai-grok-shell --locked --test-threads=2 --build-jobs 2 \
  -E 'test(resolve_credentials_openrouter_does_not_use_xai_session) or
      test(resolve_model_override_api_key_pin_keeps_console_primary) or
      test(authenticated_401s_still_exhaust_after_three_retries) or
      test(queue_send_now_never_cancels_uncommitted_front) or
      test(parse_list_req_forces_kind_under_process_chat_mode_only)'
```

| Test | Red |
|------|-----|
| `resolve_credentials_openrouter_does_not_use_xai_session` | `left: SessionToken` / `right: ApiKey` (`config_tests.rs:1530`) |
| `resolve_model_override_api_key_pin_keeps_console_primary` | `got Some("tok-session-under-pin")` expected `Some("console-pin-primary")` |
| `queue_send_now_never_cancels_uncommitted_front` | order `["m1"]` expected `["m1", "m2"]` |
| `parse_list_req_forces_kind_under_process_chat_mode_only` | empty kind `Some([])` vs expected `None` ("must still force chat") |
| `authenticated_401s_still_exhaust_after_three_retries` | **already PASS** 0.167s. No product change. |

## Root cause: why GHA fails the 30 `team_managed_config` tests

Not 30 unique product bugs. Two shared setup holes. The 1.0.3 response
shape already matches `team_config_body()`. Not API drift.

### 1. Suite is dark. Product default is armed.

Product embeds the prod `v1` pubkey. `verification_active()` is true unless
a test-support override clears keys. Integration tests compile
`xai-grok-config` **without** `cfg(test)`, so the signing seam is only
visible when the crate feature `test-support` is on.

Unsigned mock bodies then hit `apply_fetched` → `verify_signed_envelope` →
`SignatureRejected`. `wrote=false` in tens of milliseconds. That is the
local leftover (`assert!(wrote)`), and it is also the GHA leftover.

Armed/signed contracts live in `xai-grok-config` unit tests. This suite
must stay keyless.

### 2. `grok_home()` OnceLock + no integration ctor

`xai_grok_config::grok_home()` caches the first value for the process.
The lib `#[cfg(test)]` ctor does **not** compile into the integration
binary. Without a pre-main pin:

- GHA has no operator `~/.grok`.
- Tests that write `auth.json` into a temp dir while product `sync()`
  reads a different (default / empty) home get `NoPrincipal` and
  `wrote=false`.

A first `grok_home()` read without `$GROK_HOME` sticks for every later
test in the binary. Shared OnceLock, missing test-home ctor, setup order:
that is the hermetic-tree failure, not "CI is special."

Fix: crate `test-support` enables `xai-grok-config/test-support`. A
`#[ctor::ctor]` in `tests/team_managed_config.rs` creates a temp
`GROK_HOME`, pins `grok_home()`, and `set_embedded_keys(Some(&[]))` so
`!verification_active()`. Process-global dark (not thread-local
`with_dark`) so tokio workers stay dark. After tamper,
`bootstrap_fails_closed_when_managed_policy_compromised` retargets at a
5xx mock so bootstrap cannot re-serve policy.

## Five non-team: product (or test-support) to make the named contract true

### OpenRouter never uses the xAI session JWT

`resolve_credentials` fell through to `session_key` when the OpenRouter
env key was unset, so auth type became `SessionToken` and the SuperGrok
session JWT was sent to OpenRouter.

After own credential / auth provider, OpenRouter `base_url` now returns
`(None, openrouter_url, ApiKey)` and does not fall through.

### `preferred_method=api_key` pin keeps console primary

`resolve_model_override_to_config` always passed the live SuperGrok
session into `resolve_credentials`. Session outranked `XAI_API_KEY` even
when `agent_config` pinned `preferred_method = api_key` (and
`auto_use_included_limits` was true).

Product now calls `subagent_override_auth_rank_flags`. When
`preferred_is_console_primary`, `session_key` is `None` so resolve uses
the console key. Did **not** invent hop failover in
`sampling_config_for_model`.

### Send-now / interject must spare an uncommitted front

`handle_interject_queued_prompt` pulled `m2` into the running turn while
`front_message_committed` was false. Queue became `["m1"]`.

Same guard as send-now cancel: if `front_awaiting_commit`, leave the
queued row in place.

### Process chat mode: tests are spec

`process_chat_mode_enabled()` was hardcoded `false` (Surmount release
intent). `parse_list_req` only forces `kind=["chat"]` when that flag is
on. The test already checks the **off** path (client `kind=build`
untouched). The **on** path sets `GROK_CHAT_MODE=1` and names the
force-chat contract.

Debug builds now honor `$GROK_CHAT_MODE` (`1` / `true` / `yes`). Release
stays hard-off. Did not restore desktop `GROK_SESSION_LIST_CONVERSATIONS`
(`conversations_lane_enabled()` stays false). Did not invent process-chat
product beyond the test.

On-path expects were leftover / inverted:

- `kind=build` under chat mode: honor client kind only with
  `local-workspace`, else `Some(["chat"])`.
- Absent / empty / null / unknown kind: `Some(["chat"])`, not `None`.
  Stronger than the leftover `None`. Matches the comments ("must still
  force chat").
- Sibling `conversations_lane_active_truth_table`: last assert flipped
  to `assert!(conversations_lane_active())` so it matches its own
  message ("process chat mode must enable the lane").

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/Cargo.toml` | crate + dep `test-support` includes `xai-grok-config/test-support` |
| `crates/codegen/xai-grok-shell/tests/team_managed_config.rs` | pre-main hermetic `GROK_HOME` + dark keys; bootstrap 5xx after tamper |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | OpenRouter skips xAI session JWT |
| `crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs` | api_key pin: do not pass session into resolve |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/prompt_queue.rs` | interject leaves row when front uncommitted |
| `crates/codegen/xai-grok-shell/src/agent/chat_modes.rs` | debug env for `GROK_CHAT_MODE`; release hard-off |
| `crates/codegen/xai-grok-shell/src/session/unified_list/mod.rs` | restore force-chat expects; lane-active truth table |

Did not edit pager, sampler, pager-bin, pty-harness, Nucleo,
`sampling_config_for_model` hop failover, or the 401 retry path.

## Green

```
cargo fmt -p xai-grok-shell                         # exit 0
cargo clippy -p xai-grok-shell --lib --bins --locked -- -D warnings  # exit 0
```

After fmt + clippy, re-run:

```
# five + team_sync
Summary [0.218s] 6 tests run: 6 passed, 6889 skipped

# --test team_managed_config
Summary [1.861s] 50 tests run: 50 passed, 0 skipped
```

All 30 GHA-named team_managed tests are in that 50.

`authenticated_401s_still_exhaust_after_three_retries` stayed green
(0.181s) with no code change (16MiB `run_on_large_stack` already in tree).

## Leftovers (not this slice)

- Pager / pager-bin / pty-harness / sampler GHA fails: other fixers.
- `conversations_lane_enabled()` remains hardcoded false (desktop env
  lane). Only process chat mode (debug env) turns the combined
  `conversations_lane_active()` predicate on.
- `resolve_model_override_to_config` passes `disk_flags = None`. The
  named pin test supplies `agent_config`. Disk-only preferred_method
  without agent_config is not this contract.
- Pre-existing `unused_must_use` warnings in
  `cancel_running_task_tests.rs` (lib-test only; clippy `--lib --bins`
  is clean).

Stop. Parent commit-tree + push.
