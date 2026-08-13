# bug: clippy shell residual — greened (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Package:** `xai-grok-shell`
**No git commit.**

## Goal

```bash
cargo clippy -p xai-grok-shell --all-targets -- -D warnings
```

under max nice → **exit 0**.

## Results

| Check | Exit | Notes |
|-------|-----:|-------|
| `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` | **0** | Green (after fixes) |
| `cargo test -p xai-grok-shell --lib mcp_reenable -- --test-threads=1` | **0** | 6 passed |
| `cargo fmt -p xai-grok-shell` | **0** | Clean |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **101** | **Not** shell: dep `xai-grok-update` has 2 `disallowed_methods` (`auto_update.rs:706`, `rebuild.rs:676`). Shell itself is no longer the blocker. |

### Lint count

| | Count |
|--|------:|
| **Before** (error lines, excl. “could not compile”) | **~97** |
| **After** | **0** |

Host still prints the pre-existing build-script warning that `tokio::process::Command::spawn` is not a reachable path in `clippy.toml` (does not fail `-D warnings`).

## Before cluster (~97)

| Kind | ~N | Fix |
|------|---:|-----|
| `unreachable_pub` | 46 | Restrict to `pub(crate)` / `pub(super)` per clippy help |
| `unused_must_use` (WakeBarrier / interject bool / FlushAndAck Result) | 23 | `let _ = …` in tests |
| `await_holding_lock` | 9 | `#[allow]` on deliberate ENV_LOCK test serialization |
| `field_reassign_with_default` | 8 | Struct update syntax |
| `single_match` | 3 | `if let` |
| `disallowed_methods` (`Command::spawn`) | 2 | `#[allow]` + enroll/wait comments (auth helper / process group) |
| `items_after_test_module` | 2 | Move product items above `mod tests` |
| `unexpected_cfgs` (`shell-half-merge-tests`) | 1 | Declare empty feature in `Cargo.toml` |
| `private_interfaces` (`ModelByok` / `ModelAuthFacts`) | 1 | `pub(crate)` on facts + resolve helper |
| `needless_borrow` / `len_zero` | 2 | Drop `&`; use `!is_empty()` |

## Cargo.toml: test-support integration targets

After lib/unit clippy was green, bare `--all-targets` still tried to **compile** integration tests/benches that import `session::testkit` / `leader::in_process` (gated on feature `test-support`). Those targets now have `required-features = ["test-support"]` so cargo **skips** them without the feature:

- tests: `session_fork_replay_memory`, `session_load_perf`, `test_fork_copy_memory`, `test_leader_soak`, `test_session_load_memory`, `testkit_synth_roundtrip`
- bench: `fork_copy`

Also declared opt-in feature `shell-half-merge-tests = []` (parked half-merge tests; not in default CI).

With `--features test-support`, full all-targets (including those integration targets) was also clippy-clean before the required-features wiring.

## Touch list (high level)

| Area | Paths |
|------|--------|
| Visibility | `session/usage_log.rs`, `agent/subagent/mod.rs`, `auth/{flow,model}.rs`, `auth/oidc/protocol.rs`, handlers/extensions, `slash_commands` gates |
| Auth / BYOK API | `agent/config.rs` (`ModelAuthFacts`, `resolve_model_auth_facts_and_provider`) |
| Spawn allow | `auth/auth_provider.rs`, `auth/flow.rs` |
| Tests must_use | `acp_session_tests/{prompt_queue_actor,cancel_running_task}_tests.rs` |
| ENV_LOCK allows | `assistant_ascii_scrub.rs`, `replay_buffer_send_update_tests.rs`, `shared_http_rate_limit.rs` |
| Style | `models.rs`, `subagent/tests`, `auth/config.rs`, `manager_tests.rs`, `xai_management.rs`, `streaming_local_terminal.rs` |
| Module order | `slash_commands.rs` (`is_reserved_slash_name`), `util/subprocess.rs` (`shell_c`) |
| Manifest | `Cargo.toml` features + `[[test]]` / `[[bench]]` required-features |

## Residual

1. **Pager all-targets clippy** still red via **`xai-grok-update`** disallowed `Command::spawn` (2 sites). Separate package mop; not shell residual.
2. Integration targets that need testkit only build under `--features test-support` (intentional).
3. `shell-half-merge-tests` still parked (feature exists; tests not rewritten).
4. Host `clippy.toml` unreachable-path warning for `tokio::process::Command::spawn` remains.

## Commands used

```bash
nice -n 19 ionice -c3 cargo clippy -p xai-grok-shell --all-targets -- -D warnings
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib mcp_reenable -- --test-threads=1
cargo fmt -p xai-grok-shell
# optional check:
nice -n 19 ionice -c3 cargo clippy -p xai-grok-pager --all-targets -- -D warnings  # red on update dep
```
