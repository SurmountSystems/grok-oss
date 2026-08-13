# CI 197: xai-grok-shell session/auth (2026-08-12)

**Status:** green on the listed filters. Default thread stack (no `RUST_MIN_STACK`).
**Crate:** `xai-grok-shell`
**Out of scope:** `team_managed_config` (another agent). No pager / worktree / export_github edits.

## Method

Ran one test from each cluster with `--nocapture`. ABRT was not a hang: debug
tests printed `has overflowed its stack` then SIGABRT at ~9-14s (compile plus
abort). `just ci` nextest uses the default 2MB stack. Tests-as-spec. Product
fix first. No test weakening.

---

## Cluster 1: ABRT (~9-14s) = debug stack overflow

**Tests**

- `auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries`
- `auth_retry_budget_tests::fail_closed_401_is_uncharged_and_turn_survives`
- `cancel_running_task_tests::first_turn_memory_injection_disabled_does_not_persist_to_chat_history`
- `chat_history_integrity_tests::mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id`

**Red (observed)**

```
thread '...mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id'
  has overflowed its stack
fatal runtime error: stack overflow, aborting
```

Same abort on the two auth-retry tests and the first-turn memory test before
the turn future was split. Not a 60s timeout.

**Root cause**

Debug `async fn` state machines for `handle_prompt`, `process_conversation_turn`,
`execute_tool_calls_batch`, and `prepare_tool_call` were larger than the default
2MB test stack. `Box::pin(self.huge_fn())` still *constructs* `huge_fn` on the
stack, then moves it to the heap. Nested unboxed `.await`s flatten child futures
into the parent. The chat-history test also held a `Vec` of eight dispatch
futures across `yield_now().await`, which sized the batch future by `8 * dispatch`.

Auth-retry (no tools) and memory (no eight-tool batch) fit after the first
splits. Chat-history still needed the turn loop split plus ingest extract.
Proof: same binary passed at `RUST_MIN_STACK=3145728` (3MB) before the last
ingest split; default 2MB failed. After ingest extract, default 2MB is green.

**Product files**

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/turn.rs`
  - Boxed `jsonschema::Validator`
  - `finish_handle_prompt` extracted and boxed
  - `process_conversation_turn` thinned: `turn_loop_before_sample`,
    `turn_loop_on_auth_resubmit`, `turn_loop_after_model_response`
  - `ingest_parsed_user_prompt` extracted and boxed from `handle_prompt`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`
  - Boxed `execute_tool_calls_batch`, `prepare_tool_call`, and large child awaits
  - Spawn the dispatch drainer *before* any await (do not hold `Vec<dispatch>`
    across `yield_now`)
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`
  - Boxed `handle_sampling_failure`

**Red → green**

| Filter | Red | Green |
|--------|-----|-------|
| `auth_retry_budget_tests` | SIGABRT default stack | 2 passed, 1.16s, default stack |
| `cancel_running_task_tests::first_turn_memory` | SIGABRT | 2 passed, 0.05s |
| `chat_history_integrity_tests` | SIGABRT | 1 passed, 0.15s |

---

## Cluster 2: Soft interject (FAIL units)

**Tests**

- `interjection_actor_tests::interject_contract_queued_prompt_images_ride_pending_interjections`
- `prompt_queue_actor_tests::interject_contract_idle_keeps_row_queued_no_cancel`
- `prompt_queue_actor_tests::interject_contract_queued_prompt_buffers_without_cancel`

**Root cause**

Onto hybrid left `handle_interject_queued_prompt` on send-now + cancel. Surmount
soft interject buffers mid-turn into `pending_interjections`, never cancels, and
keeps idle/bash rows queued (LWW edit only).

**File**

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/prompt_queue.rs`

**Green:** 11 + 45 tests in those two modules. The three named contracts pass.

---

## Cluster 3: usage.jsonl identity (FAIL units)

**Tests**

- `record_response_token_usage_tests::main_usage_jsonl_keeps_main_identity`
- `record_response_token_usage_tests::subagent_usage_jsonl_uses_agent_turn_identity`

**Root cause**

Main vs agent-turn (`explore` + `work_ulid`) identity was not written through
`usage_jsonl_identity` on the hub path.

**File**

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`

**Green:** 7 tests in `record_response_token_usage_tests`, including both named
identity tests.

---

## Cluster 4: StreamResumed (FAIL unit)

**Test**

- `replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed`

**Root cause**

`SamplingEvent::StreamStarted` did not emit `RetryState::StreamResumed`, so the
pager's sticky retry chrome could stay after a live stream.

**File**

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`
  (`handle_sampling_event`)

**Green:** 18 tests in `replay_buffer_send_update_tests`, including StreamResumed.

---

## Cluster 5: External auth integrations (FAIL)

**Tests**

- `external_auth_conforming_provider::a_provider_that_declines_the_headless_run_can_still_sign_the_user_in`
- `external_auth_expired_credential::expired_external_credential_routes_to_the_provider_login_flow`

**Red (expired, observed)**

Message was SelfHealing ("no need to run /login") instead of Acme SSO + `/login`.
Live external token + failed headless run recorded `Other` (does not
`blocks_unattended_retry`), so `auth_remedy` stayed SelfHealing. Terminal 401
applied `auth_remedy()` not `auth_remedy_after_retries`.

**Product files**

- `crates/codegen/xai-grok-shell/src/auth/refresh/external_refresher.rs`
  - `record_failure` is `ProviderInteractiveRequired` (non-sticky, blocks
    unattended retry)
- `crates/codegen/xai-grok-shell/src/auth/manager/remedy.rs`
  - `AuthManager::auth_remedy_after_retries`: SelfHealing + external provider
    refresh authority → ProviderLogin
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`
  - both terminal 401 paths use `auth_remedy_after_retries`
- `crates/codegen/xai-grok-shell/src/auth/flow.rs`
  - drop auth lock before `auth()`; interactive external `is_refresh=false`

The external-refresher unit now expects `ProviderInteractiveRequired`, not
`Other`. Still asserts `!is_sticky`. That is a tighter product contract, not a
weaker test.

**Green**

- conforming: 1 passed, 0.03s
- expired: 1 passed, 0.71s

---

## Verify (this run, default stack)

```
cargo test -p xai-grok-shell --lib session::acp_session::interjection_actor_tests
  # 11 passed
cargo test -p xai-grok-shell --lib session::acp_session::prompt_queue_actor_tests
  # 45 passed
cargo test -p xai-grok-shell --lib session::acp_session::record_response_token_usage_tests
  # 7 passed
cargo test -p xai-grok-shell --lib session::acp_session::replay_buffer_send_update_tests
  # 18 passed
cargo test -p xai-grok-shell --lib session::acp_session::auth_retry_budget_tests
  # 2 passed, 1.16s
cargo test -p xai-grok-shell --lib session::acp_session::cancel_running_task_tests::first_turn_memory
  # 2 passed
cargo test -p xai-grok-shell --lib session::acp_session::chat_history_integrity_tests
  # 1 passed, 0.15s
cargo test -p xai-grok-shell --test external_auth_conforming_provider
  # 1 passed, 0.03s
cargo test -p xai-grok-shell --test external_auth_expired_credential
  # 1 passed, 0.71s
```

Post-impl: `cargo fmt -p xai-grok-shell` and
`cargo clippy -p xai-grok-shell --all-targets -- -D warnings` (exit 0).

No workspace `RUST_MIN_STACK`. No `nextest.toml` change.

## Prior reports (verified)

- `.agents/reports/bug-external-auth-headless-decline-2026-08-11.md`
- `.agents/reports/impl-upstream-interject-contracts-2026-08-11.md`
- `.agents/reports/impl-upstream-usage-jsonl-identity-2026-08-11.md`
- `.agents/reports/impl-upstream-stream-resumed-runtime-2026-08-11.md`
