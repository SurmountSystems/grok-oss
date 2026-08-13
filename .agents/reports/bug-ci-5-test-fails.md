# Report: five `xai-grok-shell` just-ci unit fails

Implementer report. Tests treated as spec. No test asserts loosened.

## Red (before product edit)

Command:

```bash
cargo test -p xai-grok-shell --lib -- --test-threads=1 \
  post_auth_settings_xai_upgrades_writeback_emits_and_opens_gate \
  post_auth_settings_non_xai_keeps_local_but_still_emits \
  post_auth_settings_failure_resolves_gate_onto_local_policy \
  settings_not_cached_when_identity_logs_out_during_fetch
```

All four failed (0.40s):

| Test | Panic |
|------|--------|
| `post_auth_settings_failure_resolves_gate_onto_local_policy` | `an exhausted fetch is a definitive answer: open on local policy` |
| `post_auth_settings_non_xai_keeps_local_but_still_emits` | `a settings response must open the gate regardless of auth kind` |
| `post_auth_settings_xai_upgrades_writeback_emits_and_opens_gate` | `left: Local, right: Writeback` |
| `settings_not_cached_when_identity_logs_out_during_fetch` | `settings fetched for a logged-out identity must not be cached` |

Temporary debug in `maybe_fetch_post_auth_settings` (reverted before the fix):

```
DBG maybe_fetch: early return remote_settings already Some writeback=None
```

Host had a live `~/.grok/auth.json`. `GROK_HOME` unset.

`test_timeout_kills_grandchildren_and_returns_promptly` passed once in isolation, then failed with `background pid should have been echoed before the kill` under load (same machine as just-ci). That is the 300ms timeout firing before `bash -lc` finished the operator login profile and printed `bgpid=`.

## Root cause

### Settings family (one path)

`MvpAgent::new` -> `bootstrap` -> `ensure_remote_settings_side_effects` joined an early prefetch that built a **second** `AuthManager` on the process grok home (`start_early_prefetch` / `start_early_prefetch_settings_only`). That used the operator disk session and the startup proxy, not the agent's temp-dir `AuthManager` or mock proxy.

Result at construction:

- `cfg.remote_settings` was already `Some` (prod snapshot, `writeback_enabled = None`)
- storage stayed `Local` (precondition of the writeback test still held)
- tests then suppressed the external-OTEL gate
- `maybe_fetch_post_auth_settings` saw settings present and returned without fetching the mock or opening the gate
- `refresh_remote_settings` after `clear_in_memory` did not drop the leftover snapshot

So the four tests never drove the post-auth mock path they specify.

### Timeout

Two product holes vs the test contract (kill grandchildren, return promptly, `bgpid` echoed first):

1. `LocalTerminalRunner` spawned `bash -lc`. Login profile work is charged against the request timeout. Under just-ci load, 300ms is not enough for `echo bgpid=$!`.
2. `killpg` on the shell group misses job-control grandchildren (own PGID, same session after `setsid`). Those hold the pipes; `join_bounded` then sits on `KILL_REAP_TIMEOUT` (5s), which is past the test's 4s prompt bound.

Streaming local terminal and the computer local runner already use non-login `-c`.

## Product change

Did not edit `cancel_running_task_tests.rs`, credentials, keyring, or fuzzy-search.

### 1. Bootstrap prefetch uses the agent's credential

`crates/codegen/xai-grok-shell/src/agent/init.rs`

`ensure_remote_settings_side_effects` now takes `&AuthManager` and calls `start_early_prefetch_with_auth` / `start_early_prefetch_with_auth_gated` with `auth_manager.current()`. Isolated agents no longer inherit the operator disk session.

`start_early_prefetch_with_auth_gated` is `pub(crate)` so the pre-gate (no managed sync) call can share that path.

TUI startup that has no agent yet still uses `start_early_prefetch` (grok-home) from the pager/bin.

### 2. Logout drops cached settings

`fetch_settings_resolving_gate` in `agent_ops.rs`: if `current_or_expired()` is `None` after the fetch, clear `cfg.remote_settings` and return `None`. Account switch (a different live identity) still keeps the old snapshot until the new fetch lands.

### 3. Local terminal timeout contract

`crates/codegen/xai-grok-shell/src/terminal/local_terminal.rs`

- Unix spawn is `bash -c` (non-login), matching streaming local terminal.
- After `start_kill` + `ProcessGroup::kill`, sweep `/proc` and `SIGKILL` every member of the shell's session (`kill_unix_session_members`). Job-control grandchildren in another PGID die and drop the pipes.

## Green

```bash
cargo test -p xai-grok-shell --lib --offline -- --test-threads=1 \
  post_auth_settings_xai_upgrades_writeback_emits_and_opens_gate \
  post_auth_settings_non_xai_keeps_local_but_still_emits \
  post_auth_settings_failure_resolves_gate_onto_local_policy \
  settings_not_cached_when_identity_logs_out_during_fetch \
  terminal::local_terminal::tests::test_timeout_kills_grandchildren_and_returns_promptly
```

6/6 ok (plus neighbors `same_credential_refresh_does_not_flap_resolved_gate`, `settings_self_heal_refetches_after_token_rotation`, `storage_mode_self_corrects_to_writeback_when_settings_arrive`).

Timeout test: 15/15 isolated runs, each ~0.32s. `kill: No such process` on stderr is the test's `kill -0` after the grandchild is already dead.

## Verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-shell` | clean |
| `cargo clippy -p xai-grok-shell --lib --bins -- -D warnings` | clean (forced recheck of touched files, 23.69s) |
| Named filters | green |

## Flaky title test (leftover)

`session::persistence::durable_update_tests::reset_title_to_auto_then_generated_title_is_adopted`

- Not the same root (persistence actor + mock `save_session_data`, not settings prefetch or PTY kill).
- No cheap hermetic lock: it already polls disk and the mock for several seconds.
- 8/8 isolated runs passed here (~0.10s).
- Left untouched. Just-ci 2/2 flake is a timing race between unpin, fallback title, and the mock PUT/POST. Separate slice if it stays noisy.
