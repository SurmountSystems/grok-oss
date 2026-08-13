# Shell residual inventory (non-pager CI 239 wave)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 explore (read-only). **No shell tool** in this worker: no fresh full `cargo test -p xai-grok-shell --lib`. Totals below are **CI baseline minus greened-report evidence**, plus **code-stub confirmation** for a few clusters.

**Sources:**
- CI wave list: `.agents/reports/bug-ci-239-test-cluster-2026-08-11.md` + session `prompt_436.txt` TRY 2 FAIL lines
- Greened: `bug-dark-signed-policy-cluster`, `bug-external-auth-headless-decline`, `bug-worktree-export-github-cluster`, catalog reverify / interject / usage / stream-resumed reports
- Code: `session_mode.rs` plan-tool filter stub; `managed_mcp.rs` reenable discovery stub

---

## Executive totals

| Bucket | Original CI | Greened (reports) | **Residual estimate** |
|--------|------------:|------------------:|----------------------:|
| **`xai-grok-shell` lib + integration** | **59** | **~32** (team_managed 30 + external_auth 2) | **~27 hard fails** |
| Shell flaky (not counted in 239) | 1 flaky | — | `close_pty_kills_a_background_grandchild` still reliability residual |
| **Non-pager other crates** | **31** | **~24** (config dark + worktree 12 + export 10) | **~7 oneshots** (see below) |
| **Pager** | **148** | many modules mopped; **dispatch residual ~45** | separate: `bug-pager-residual-inventory-2026-08-11.md` |

**Catalog shell items** (not all in the original 59) re-verified green: StreamResumed emit, 3 soft-interject contracts, 2 usage.jsonl identity writes (`impl-upstream-catalog-reverify-2026-08-11.md`).

**Bottom line:** After greening auth + dark/signed-policy + worktree/export, **shell session / plan / queue / recap / MCP reenable / setup** is the main non-pager residual (~27). Pager remains the larger fail mass.

---

## Greened (do not re-open without new red)

| Cluster | Count | Evidence |
|---------|------:|----------|
| `team_managed_config` integration | **30** | `bug-dark-signed-policy-cluster-2026-08-11.md` — **50/50** suite green (suite forced dark) |
| `external_auth_conforming_provider` | **1** | `bug-external-auth-headless-decline-2026-08-11.md` — product `auth/flow.rs` interactive `is_refresh=false` + lock drop |
| `external_auth_expired_credential` | **1** | Same product path; treated greened with conforming (no separate fail report since fix) |
| `claim_paths_are_inert_in_dark_build` (config) | **1** | same dark/signed report |
| `xai-fast-worktree` worktree + auto_gc | **12** | `bug-worktree-export-github-cluster-2026-08-11.md` |
| `xai-grok-workspace` `export_github` | **10** | same |
| Catalog shell extras | 6 | stream + interject×3 + usage×2 reverify **6/6** |

---

## Shell residual clusters (~27 from original CI list)

Named from `prompt_436` TRY 2 FAIL / ABRT. **None of these have a green implement report** (except soft-interject neighbors, which are different tests than the send-now four).

### 1. ACP session plan / edit gate / approval — **2**

| Test | Class |
|------|--------|
| `session::acp_session::plan_mode_edit_gate_tests::plan_mode_rejects_ask_user_question_before_ui` | FAIL |
| `session::acp_session::plan_approval_resume_tests::real_exit_plan_mode_no_client_executes_tool` | FAIL |

**Code signal (high confidence still red for ask_user):**
`filter_cursor_tools_by_plan_mode` in `session/acp_session_impl/session_mode.rs` is a **pass-through stub** (comment: “no toolset… plan-gated tool”).
`prepare_tool_call` only applies `plan_mode_edit_gate` on `AccessKind::Edit`; `ToolKind::AskUser` is treated read-only and is **not** hard-rejected.
Compile mop once claimed a real strip + `is_plan_mode_blocked_ask_user_tool_name`; current tree shows the stub again (half-merge / lost restore).

**Fix direction:** restore real plan-mode strip + hard reject of questionnaire tools in `prepare_tool_call` (Surmount plan-questionnaire ban); fix no-client `exit_plan_mode` execute path for resume test.

### 2. Prompt queue send-now / auto-send-now — **4**

| Test |
|------|
| `prompt_queue_actor_tests::queue_input_send_now_inserts_behind_running_front_and_requests_cancel` |
| `prompt_queue_actor_tests::queue_input_send_now_pins_front_on_running_task_identity` |
| `prompt_queue_actor_tests::queue_input_auto_send_now_when_wait_and_held_queue_empty` |
| `prompt_queue_actor_tests::queue_input_auto_send_now_during_foreground_subagent_await_window` |

**Note:** Soft-interject contracts greened separately. These are **send-now / cancel** contracts on `queue_input` (`send_now: true` / wait-window auto), not soft buffer.

**Path:** `session/acp_session_impl/prompt_queue.rs` + `prompt_queue_actor_tests.rs`.

### 3. Auth retry / dual-auth preflight — **4** (2 ABRT)

| Test | Class |
|------|--------|
| `auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries` | ABRT ~6.6s |
| `auth_retry_budget_tests::fail_closed_401_is_uncharged_and_turn_survives` | ABRT ~6.6s |
| `auth_error_no_retry_tests::pre_flight_keeps_console_primary_when_session_identity_differs` | FAIL |
| `auth::manager::tests::refresh_persist_failure_is_transient_but_swaps_in_memory` | FAIL |

ABRT pattern = hang/timeout under nextest cut, not instant assert. Likely half-merge retry budget / uncharged 401 / dual-host primary.

### 4. Cancel / chat history integrity — **3** (2 ABRT)

| Test | Class |
|------|--------|
| `cancel_running_task_tests::non_ctrl_c_cancel_preserves_queued_task_wakes_and_does_not_arm_barrier` | FAIL |
| `cancel_running_task_tests::first_turn_memory_injection_disabled_does_not_persist_to_chat_history` | ABRT ~9.5s |
| `chat_history_integrity_tests::mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id` | ABRT ~7.8s |

### 5. Recap / side-question display-only — **4**

| Test |
|------|
| `recap_display_only_tests::auxiliary_calls_keep_the_main_turn_prefix` |
| `recap_display_only_tests::auxiliary_calls_send_the_session_reasoning_effort` |
| `recap_display_only_tests::side_question_request_rides_parent_prompt_cache` |
| `recap_display_only_tests::side_question_routes_on_the_session_id_when_the_key_is_not_forwarded` |

Onto hybrid `handle_side_question` / recap wiring drift (compile mop already aligned call shapes; runtime contracts still open).

### 6. MCP reenable discovery — **3** (stub, certainty high)

| Test |
|------|
| `util::config::mcp_reenable::tests::build_indexes_toml_enabled_false` |
| `util::config::mcp_reenable::tests::discover_contains_merge_when_nothing_disabled` |
| `util::config::mcp_reenable::tests::toml_duplicate_url_last_wins_matches_merge` |

**Code:** `session/managed_mcp.rs`:

```rust
pub fn discover_mcp_definitions_ignoring_disable(
    _inputs: &McpDiscoveryInputs<'_>,
) -> HashMap<String, McpServer> {
    HashMap::new()
}
```

Empty stub → all three fail. Restore real discovery (ignore personal disable list; match `merge_managed_mcp_servers` URL last-wins).

### 7. Subagent sampling config / bearer resolver — **2**

| Test |
|------|
| `agent::subagent::tests::rest::read_parent_sampling_config_fallback_wires_bearer_resolver` |
| `agent::subagent::tests::rest::resolve_model_override_wires_resolver_for_fresh_and_hard_expired_session_keys` |

Neighbor `subagent_override_auth_rank_flags_*` greened in compile mop; these two still on CI fail list.

### 8. Oneshots / terminal / list / setup / registry — **5**

| Test | Notes |
|------|--------|
| `replay_buffer_send_update_tests::channel_token_text_scrubs_curly_punctuation_when_on` | ASCII scrub on token channel |
| `session::unified_list::tests::parse_list_req_forces_kind_under_process_chat_mode_only` | process chat + client kind; feature `local-workspace` path |
| `terminal::local_terminal::tests::test_timeout_kills_grandchildren_and_returns_promptly` | group-kill grandchild (related to flaky pty) |
| `::acp_session_setup_wire::acp_session_setup_conformance` | integration wire |
| `::test_registry_churn::session_churn_returns_registry_snapshot_to_baseline` | registry churn ~1.7s |

### 9. Flaky (not in 239 hard fails)

| Test | Signal |
|------|--------|
| `terminal::pty_session::tests::close_pty_kills_a_background_grandchild` | TRY 1 FAIL 300s (`grandchild survived`); TRY 2 PASS 0.11s |

Same process-group kill family as local_terminal timeout test.

---

## Cluster table (shell residual only)

| Cluster | Count | Confidence still red | Suggested implementer scope |
|---------|------:|----------------------|-----------------------------|
| MCP reenable discovery stub | **3** | **Certain** (empty fn) | `managed_mcp` + `mcp_reenable` only |
| Plan ask_user hard block + exit_plan no-client | **2** | **High** (filter pass-through; no prepare reject) | `session_mode` + `tool_calls` prepare |
| Prompt queue send-now / auto | **4** | High (no green report) | `prompt_queue` only |
| Recap / side-question | **4** | High | recap display-only path |
| Auth retry budget + preflight + refresh persist | **4** | High (2 ABRT) | auth retry + dual primary |
| Cancel / chat history | **3** | High (2 ABRT) | cancel + injection integrity |
| Subagent bearer / model override | **2** | Medium | `agent/subagent` rest |
| Scrub / list kind / terminal timeout / setup / registry | **5** | Medium | oneshots each |
| **Shell residual total** | **~27** | | |
| Flaky pty close_pty | **1** | Known flaky | reliability only |

---

## Non-shell non-pager residual (~7 oneshots)

| Package | Original | Greened? | Residual |
|---------|---------:|----------|----------|
| `xai-grok-config` dark claim | 1 | **yes** | 0 |
| `xai-fast-worktree` | 12 | **yes** | 0 |
| `xai-grok-workspace` export_github | 10 | **yes** | 0 |
| `xai-grok-tools` is_read_only + contract snapshot | 2 | **no report** | **2** |
| `xai-grok-agent` encrypted templates stale | 1 | **no report** | **1** |
| `xai-grok-hooks` hook_child_cannot_open_dev_tty | 1 | **no report** | **1** (env-ish) |
| `xai-grok-pager-minimal` dim thinking rail | 1 | **no report** | **1** |
| `xai-grok-sampler` status_user_message_matrix | 1 | **no report** | **1** (catalog sampler subset was green; this name not re-run) |
| `xai-grok-update` install-internal smoke | 1 | **no report** | **1** |
| `xai-grok-pager-render` auto dark → DOGE | 2 | **likely green** (catalog DOGE filters PASS) | **0–2** treat as re-check once |

**Non-pager non-shell residual working estimate: ~7** (tools 2 + agent + hooks + minimal + sampler + update), pending one sample filter each.

---

## Recommended fix order (shell-first fan-out)

Parallel-safe, disjoint files:

1. **MCP reenable** (`managed_mcp` discovery) — 3, certain, small
2. **Plan questionnaire hard block** (`session_mode` + `prepare_tool_call`) — 2 + any list-omit neighbors
3. **Prompt queue send-now** — 4
4. **Recap display-only** — 4
5. **Auth retry / dual preflight** — 4 (ABRT needs longer timeout / careful reproduce)
6. **Cancel + chat integrity** — 3
7. **Subagent bearer** — 2
8. **Oneshots** — scrub, list kind, terminal, setup wire, registry
9. **Non-shell oneshots** — tools / agent templates / hooks / update / sampler matrix
10. Flaky pty — last (reliability)

Do **not** burn parent tokens re-grepping; implementers run:

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib -- --test-threads=8 2>&1 | tee /tmp/shell-lib-resample.txt | tail -80
# then cluster filters, e.g.:
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib mcp_reenable -- --test-threads=1
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib plan_mode_rejects_ask_user queue_input_send_now
```

---

## Honesty / limits

1. **No live full shell `--lib` suite** in this explore worker (no shell). Residual **~27** is CI-list arithmetic after greened clusters, not a new nextest summary.
2. Some of the ~27 may have greened as side effects of catalog mops; **MCP stub + plan filter stub** prove at least **5** still red without running tests.
3. Full `just check` still expected red mostly on **pager (~45 dispatch residual)** + this shell ~27 + ~7 oneshots.
4. Pager detail: `.agents/reports/bug-pager-residual-inventory-2026-08-11.md`.

---

## 10-line summary

1. Original CI: shell **59**, pager **148**, other **~31**.
2. Greened non-pager: team_managed **30**, external auth **2**, dark config, worktree **12**, export **10**.
3. Shell residual **~27** = original session/plan/queue/recap/auth/mcp/setup list.
4. MCP reenable: discovery is **empty stub** → 3 fails certain.
5. Plan ask_user: tool filter is **pass-through stub**; prepare does not hard-reject AskUser.
6. Soft interject / StreamResumed / usage.jsonl catalog: **green** (separate names).
7. Send-now queue ×4, recap ×4, auth retry ×4, cancel/history ×3 still open.
8. Flaky: `close_pty` grandchild kill (not in hard 239).
9. Non-shell oneshots still open: tools×2, agent templates, hooks tty, sampler matrix, update, pager-minimal.
10. Next: implementers on MCP + plan gate first; parent should one-shot full shell lib when a worker with shell is available.
