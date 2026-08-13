# PR #36 local test-compile mop

Parent HITL. Goal: stop burning ~30-minute GitHub Actions `test-unit` cycles
on one compile error at a time. This mop found every remaining `--no-run`
compile error locally, fixed them, and pushed one recon-unsigned commit.

## Git / environment

| Item | Value |
|------|--------|
| Start HEAD | `82fa1794a8f1751045da6eb85b3e43d902972a69` |
| Branch | `onto-xai/b13fa526f511` (matched origin at start and before push) |
| rustc | 1.97.1 (8bab26f4f 2026-07-14) |
| rustfmt | 1.9.0-stable (8bab26f4f6 2026-07-14) |
| GitHub | read only. No comments, edits, reviews, or creates. |

## Commands + exits

| Command | Exit |
|---------|------|
| `git rev-parse HEAD` / branch check | 0, `82fa1794…` on `onto-xai/b13fa526f511` |
| `nice -n 19 cargo test --no-run --workspace --jobs 2 --locked` (first, start tip) | 101. 117 errors, only `xai-grok-shell` lib-test |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --no-run --jobs 2` (after product/fixture mop, before lock update) | 101. `--locked` blocked `ctor` lockfile add |
| `nice -n 19 cargo metadata --offline --format-version 1` | 0. `Cargo.lock` +1 line (`ctor` on shell) |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --no-run --jobs 2` | 0 after leftover `State` delete |
| `nice -n 19 cargo test -p xai-grok-shell --tests --locked --no-run --jobs 2` | 0 |
| `nice -n 19 cargo test -p xai-grok-pager --lib --locked --no-run --jobs 2` | 0 |
| `nice -n 19 cargo test -p xai-grok-pager --tests --locked --no-run --jobs 2` | 0 |
| `nice -n 19 cargo test --no-run --workspace --jobs 2 --locked` (second) | 101. 8 errors in `xai-grok-sampler` lib-test only (log killed mid wait-for-other-jobs) |
| `nice -n 19 cargo test -p xai-grok-sampler --lib --locked --no-run --jobs 2` | 0 after fixture fields |
| `nice -n 19 cargo test -p xai-grok-sampler --tests --locked --no-run --jobs 2` | 101 then 0 (`test_actor.rs` `SamplerConfig` + `EnvGuard` / `clear_all_including_durable` imports) |
| `nice -n 19 cargo test --no-run --workspace --jobs 2 --locked` (third) | **0** |
| `nice -n 19 cargo fmt -p xai-grok-shell` / `-p xai-grok-sampler` | 0 |
| `nice -n 19 cargo clippy -p xai-grok-shell -p xai-grok-sampler --lib --bins --locked -- -D warnings` | 0 (twice; after product plan-mode gates too) |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --jobs 2 plan_mode_ -- --test-threads=2` | 0. 29 passed |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --jobs 2 cancel_running_task -- --test-threads=2` | 0. 20 passed (after large-stack wraps) |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --jobs 2 align_to_ranked_free_period` | 0. 1 passed |
| `nice -n 19 cargo test -p xai-grok-shell --lib --locked --jobs 2 resolve_subagent_work_ulid` | 0. 1 passed |
| `nice -n 19 cargo test -p xai-grok-sampler --lib --locked --jobs 2 http_521_is_not_credit` | 0. 1 passed |
| `nice -n 19 cargo test -p xai-grok-shell --lib --tests --locked --no-run --jobs 2` (final) | 0 |
| `nice -n 19 cargo test -p xai-grok-sampler --lib --tests --locked --no-run --jobs 2` (final) | 0 |

Logs: `/tmp/pr36-mop/`.

## Compile errors found (first workspace `--no-run`)

117 errors, crate fail: `xai-grok-shell` (lib test). Codes: 45 E0063, 25 E0560, 24 E0425, 10 E0061, 7 E0599, 5 E0308, 1 E0433.

Classes:

- Missing crate `ctor`; missing `WORK_ULID_SESSION_FILE`, `resolve_subagent_work_ulid`, `subagent_override_auth_rank_flags`, `upsert_supergrok_session`, `OVERLOADED_USER_MESSAGE`, `openrouter_grok_45_default_entry`, `is_plan_mode_blocked_ask_user_tool_name`, `test_actor_inner`
- Missing methods `align_to_ranked_free_period_primary`, `session_wire_bearer_trace`; `BlockingWaitState::load` (use `.depth()`)
- Extra fields on `ResolvedCredentials` / `SubagentSpawnContext`; missing fields on `CompactionConfig`, `TokenUsage`, `SamplingError`, `TodoWriteSuccess`, `TodoItem`, `SamplerConfig`, `SamplingConfig`, `SessionActor` leftovers (`model_auth_facts`, `managed_mcp_expires_at`, `subagent_spawn_info`), `State`, `NotificationSender`, `InputItem`, `QueueEntryMeta`, `SessionPersistence`, `PersistenceHandle`, `SessionConfig`
- `cancel_running_task` arity (4 args vs `CancelOptions`); `handle_set_session_model` 6th arg; `base64::Engine`

Second workspace pass (shell already green):

- `xai-grok-sampler` lib-test: `SamplingErrorInfo.credential`, `SamplingError::Api.error_code`, `SamplerConfig.extra_response_includes`
- `xai-grok-sampler` integration `test_actor.rs`: `SamplerConfig` missing hop-host fields; `EnvGuard` / `clear_all_including_durable` not in scope

Pager lib + tests `--no-run` were already green.

## Files changed

Product / helpers:

- `crates/codegen/xai-grok-shell/Cargo.toml` (`ctor` workspace dev-dep)
- `Cargo.lock` (`ctor` on `xai-grok-shell`)
- `src/agent/config.rs` (`openrouter_grok_45_default_entry` + insert)
- `src/agent/subagent/mod.rs` (work ULID + rank-flags helpers)
- `src/auth/manager.rs` (`align_to_ranked_free_period_primary`, `session_wire_bearer_trace`, `new()` auto-align)
- `src/sampling/error.rs` (`OVERLOADED_USER_MESSAGE` + `is_overloaded()` early map)
- `src/session/acp_session_impl/session_mode.rs` (name matcher + real plan-mode strip)
- `src/session/acp_session_impl/tool_calls.rs` (prepare-time `ask_user_question` reject; headless `exit_plan_mode` honest leave)
- `src/session/acp_conversion.rs`, `src/session/usage_log.rs` (fixture/product fields)

Fixtures / tests:

- `src/agent/config_tests.rs`, `src/auth/manager_tests.rs`
- `src/session/acp_session_tests/auth_error_no_retry_tests.rs`
- `src/session/acp_session_tests/cancel_running_task_tests.rs` (`create_test_actor` + `CancelOptions`; large-stack wraps for debug `SessionActor` / `handle_prompt`)
- `src/session/acp_session_tests/tool_layer_images_bridge_tests.rs`
- `src/session/compaction_inline_auto_compact_flow_tests.rs`
- `src/session/helpers/session_compact_reasoning_compaction_regression_tests.rs`
- `src/session/persistence_tests.rs` (`test_actor_inner`)
- `src/test_support/lsp_runtime.rs`, `src/util/config/persist_tests.rs`
- `crates/codegen/xai-grok-sampler/src/{actor/request_task,client,events,retry}.rs`
- `crates/codegen/xai-grok-sampler/tests/test_actor.rs`

Left other untracked `.agents/reports/*` alone.

## TDD notes

- Compile-red was the CI `test-unit` `--no-run` step (117 then 8). Product helpers restored to the named test contracts; fixtures aligned to current types. Asserts not weakened.
- Runtime after compile-green:
  - `plan_mode_rejects_ask_user_question_before_ui` and `real_exit_plan_mode_no_client_executes_tool` failed at the named contract. Product `prepare_tool_call` now rejects the questionnaire in active plan mode (`Err(ToolLoop::Continue)` + text naming the tool, plan mode, and plan file / `exit_plan_mode`) and headless `exit_plan_mode` leaves plan mode with an honest no-panel result (not tool-body fall-through, not "approved" / "start coding"). Same tests then passed (29 `plan_mode_` tests).
  - Debug stack overflow on `first_turn_memory_injection_disabled_*`, persist-ack, and `handle_prompt_*` in `cancel_running_task_tests.rs`. Contract held under `RUST_MIN_STACK=32MiB`. Tests now run on a 16MiB dedicated thread. Asserts unchanged. 20 `cancel_running_task` tests passed.

## New tip + push

| Item | Value |
|------|--------|
| New tip | `c06c7e805c245ec34f427212b9fb82450a78d0bf` |
| Tree | `6814478ef18a083b236fa2df69e1c8e9c5f9a8ed` |
| Parent | `82fa1794a8f1751045da6eb85b3e43d902972a69` |
| Commit path | `git add` product files + this report, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `82fa1794..c06c7e80`, then report pin `c06c7e80..a036327e`. No force. No new branch. No new PR. |
| Branch tip after report pin | `a036327e6151398f7c46b79948256b24b2ae1832` |

## What this did not do

- No full nextest / `just ci` (hours).
- No GitHub writes.
- Did not invent five-CTA mouse buttons, dual-auth spend-order wiring, or PTY flake work.
- Nucleo `Some(2)`, FuzzySearchManager reuse-per-root, and Poll-must-not-write-`last_activity` were not touched.
- SuperGrok stays paid. Comments say included SuperGrok period limits, never "free SuperGrok".

## Remaining known runtime fails not fixed

None observed on the targeted filters above. Full nextest was not run. Other crates may still have runtime-only fails that compile.

Unused `WakeBarrier` / `Result` must-use warnings remain in `cancel_running_task_tests.rs` (warnings only; `--no-run` green).
