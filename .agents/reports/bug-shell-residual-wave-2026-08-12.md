# Shell residual wave — clusters 1–5 + live sample

**Date:** 2026-08-12
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer (onto residual mop)
**Inventory:** `.agents/reports/bug-shell-residual-inventory-2026-08-11.md`
**No git commit.**

## Already green (not re-opened)

- MCP reenable discovery (6/6) — `bug-shell-mcp-plan-residual-2026-08-11.md`
- Plan ask_user + exit_plan no-client — same report
- team_managed / dark / signed-policy, external auth headless, worktree/export
- Soft-interject + stream resumed + usage.jsonl catalog items

## Before → after (this wave)

| Cluster | Before (inventory / live) | After |
|---------|---------------------------|--------|
| **1. Prompt queue send-now / auto-send-now** (~4) | red / fixture drift | **4/4 green** |
| **2. Auth retry / dual-auth preflight** (~4) | red / ABRT under nextest | **4/4 green** (cargo test + `RUST_MIN_STACK`) |
| **3. Cancel / chat history** (~3) | non_ctrl_c FAIL; memory/mid_turn ABRT risk | **3/3 green** (+ full cancel suite sample) |
| **4. Recap / side-question** (~4) | 4 side-question contracts red | **4/4 green** (+ full recap_display_only module) |
| **5. Other shell --lib** | unsampled | sampled; **5 still red** (clusters 7–8 below) |

**Module re-enable:** residual actor suites that were behind undeclared `feature = "shell-half-merge-tests"` were restored under default `#[cfg(test)]` (prior turn + this wave fixtures). Prefer product + fixture restore over permanent disable.

### Aggregate filter re-verify

```text
cargo test -p xai-grok-shell --lib -- --test-threads=4 \
  queue_input_send_now queue_input_auto_send_now \
  auth_retry_budget pre_flight_keeps_console_primary refresh_persist_failure \
  cancel_running_task recap_display_only
→ 56 passed; 0 failed  (RUST_MIN_STACK=16777216)
```

---

## Product fixes (tests-as-spec)

### Cluster 1 — send-now (prior turn, re-verified)

- Mid-turn send-now requires `front_message_committed` before cancel is requested.
- Fixtures set `front_message_committed = true` for “running front” contracts (aligned with greened interject fixtures).
- Product path: `session/acp_session_impl/prompt_queue.rs` + `prompt_queue_actor_tests`.

### Cluster 2 — auth (prior turn, re-verified)

- **401 wire provenance:** sampler client maps UNAUTHORIZED through `auth_error_for_wire` / `SamplingError::auth()` so retry budget sees `SentCredential` correctly.
- **Dual-auth preflight:** `refresh_token_if_expired` keeps console primary when `session_identity_key` differs; only rotates identity key.
- **Refresh persist fault:** `refresh_persist_failure_is_transient_but_swaps_in_memory` uses `WRITE_FAULT_PATH`; storage `pub(crate)` for inject.
- ABRT under nextest: prefer `cargo test` + large stack (`RUST_MIN_STACK=16M`) for deep actor graphs.

### Cluster 3 — cancel / history (this turn)

| Test | Fix |
|------|-----|
| `non_ctrl_c_cancel_preserves_queued_task_wakes_and_does_not_arm_barrier` | **Product:** task-wake barrier + drop of queued `TaskCompleted` / `WorkflowCompleted` only on **`CancelTrigger::CtrlC`**, not every `is_stop_gesture` (Esc / mouse / client / None cancel the turn only). `tasks_cancel.rs` + `CancelOptions` doc. |
| `first_turn_memory_injection_disabled_does_not_persist_to_chat_history` | Already product-correct; stack overflow under default stack → **`RUST_MIN_STACK=16777216`**. |
| `mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id` | Live sample **green** with large stack. |

`is_stop_gesture` still true for Esc/Client (other cancel semantics / unit tests in `commands.rs`). Barrier arming is intentionally narrower (Ctrl+C hard stop), matching residual named contract + changelog “background after Ctrl+C”.

### Cluster 4 — side-question cache align (this turn)

`handle_side_question` rebuilt onto shared side-call plumbing (`side_call.rs`):

| Contract | Product behavior |
|----------|------------------|
| `prompt_cache_key` = parent session id | `parent_cached_request` |
| Session `reasoning_effort` | from `prepare_side_call` → `AuxCall` |
| Conv id `btw-*` when cache key forwarded; else parent session id | `forwards_prompt_cache_key()` branch |
| Req id still `xai-btw-*` | always |
| Main-turn tools + hosted tools | `side_call_request` / `turn_base_tool_specs` + `hosted_tools_for_turn` |
| Keep reasoning on Responses (prefix cache); strip only Messages | `setup.strip_reasoning` |
| Mid-turn orphan reasoning + in-flight tool call | `truncate_incomplete_tool_run` also pops trailing `Reasoning` after incomplete assistant/tool trim |

Files: `recap.rs` (`handle_side_question`), `helpers/side_question.rs` (`truncate_incomplete_tool_run`).

---

## Named tests green (priority)

### Queue send-now / auto-send-now

- `queue_input_send_now_inserts_behind_running_front_and_requests_cancel`
- `queue_input_send_now_pins_front_on_running_task_identity`
- `queue_input_auto_send_now_when_wait_and_held_queue_empty`
- `queue_input_auto_send_now_during_foreground_subagent_await_window`

### Auth

- `auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries`
- `auth_retry_budget_tests::fail_closed_401_is_uncharged_and_turn_survives`
- `pre_flight_keeps_console_primary_when_session_identity_differs`
- `refresh_persist_failure_is_transient_but_swaps_in_memory`

### Cancel / history

- `non_ctrl_c_cancel_preserves_queued_task_wakes_and_does_not_arm_barrier`
- `first_turn_memory_injection_disabled_does_not_persist_to_chat_history`
- `mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id`
- Neighbor cancel suite sample (Ctrl+C wake drop, interrupt reminder, queue broadcast, etc.) green under same filter

### Side-question / recap

- `auxiliary_calls_send_the_session_reasoning_effort`
- `side_question_routes_on_the_session_id_when_the_key_is_not_forwarded`
- `side_question_request_rides_parent_prompt_cache`
- `auxiliary_calls_keep_the_main_turn_prefix`
- `side_question_trims_reasoning_orphaned_by_mid_turn_truncation` (extra guard)
- Full `recap_display_only_tests` module green in the 56-filter run

---

## Remaining residual (live sample, not fixed this wave)

Inventory clusters **7–8** still red under default features:

| Test | Failure sketch |
|------|----------------|
| `read_parent_sampling_config_fallback_wires_bearer_resolver` | `bearer_resolver` is `None` |
| `resolve_model_override_wires_resolver_for_fresh_and_hard_expired_session_keys` | session-jwt resolver wire-up |
| `channel_token_text_scrubs_curly_punctuation_when_on` | streaming capture keeps curly quotes/em dash; expect ASCII scrub |
| `parse_list_req_forces_kind_under_process_chat_mode_only` | empty kind array not forced to chat |
| `test_timeout_kills_grandchildren_and_returns_promptly` | background pid not echoed before kill (process-group / timing) |

Flaky neighbor (not hard residual): `close_pty_kills_a_background_grandchild`.

Integration oneshots from inventory (`acp_session_setup_conformance`, `session_churn_returns_registry_snapshot_to_baseline`) not re-run this wave.

**Clippy:** `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` fails in **dependency** `xai-grok-tools` (pre-existing dead_code + disallowed `Command::spawn`), not in shell sources touched here. Shell fmt applied: `cargo fmt -p xai-grok-shell`.

---

## Commands used

```bash
# Format
nice -n 19 ionice -c3 cargo fmt -p xai-grok-shell

# Priority residual filters (large stack for deep actor tests)
nice -n 19 ionice -c3 env RUST_MIN_STACK=16777216 \
  cargo test -p xai-grok-shell --lib -- --test-threads=4 \
  queue_input_send_now queue_input_auto_send_now \
  auth_retry_budget pre_flight_keeps_console_primary refresh_persist_failure \
  cancel_running_task recap_display_only
# → 56 passed

# Cluster 7–8 sample
nice -n 19 ionice -c3 env RUST_MIN_STACK=16777216 \
  cargo test -p xai-grok-shell --lib -- --test-threads=4 \
  mid_turn_user_injection_must_not_duplicate \
  read_parent_sampling_config_fallback_wires_bearer \
  resolve_model_override_wires_resolver \
  channel_token_text_scrubs_curly \
  parse_list_req_forces_kind \
  test_timeout_kills_grandchildren
# → mid_turn green; 5 inventory oneshots still red
```

## Key product files touched

| Path | Change |
|------|--------|
| `session/acp_session_impl/recap.rs` | `handle_side_question` → `prepare_side_call` + `side_call_request` |
| `session/helpers/side_question.rs` | trailing `Reasoning` in incomplete-run trim |
| `session/acp_session_impl/tasks_cancel.rs` | Ctrl+C-only task-wake suppress |
| `session/commands.rs` | `CancelOptions` barrier doc |
| (prior) sampler client / sampling-types auth wire, dual-auth preflight, prompt_queue fixtures, half-merge test mod re-enable | clusters 1–2 |

## Next implementer (optional)

1. Subagent bearer resolver wiring (`agent/subagent` rest tests).
2. Streaming token ASCII scrub on capture path (`replay_buffer_send_update_tests`).
3. Unified list process-chat kind force when kind is empty array.
4. Local terminal process-group kill / grandchild timeout reliability.
5. Full `cargo test -p xai-grok-shell --lib -- --test-threads=4` with `RUST_MIN_STACK` when CI budget allows (long).
