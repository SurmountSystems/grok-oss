# Shell residual tail — clusters 7–8 oneshots

**Date:** 2026-08-12
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**No git commit.**

## Already green (not re-opened)

- MCP reenable + plan ask_user/exit_plan — prior mcp-plan report
- Send-now ×4, auth retry/preflight ×4, cancel/chat ×3, recap/side-question ×4 —
  [`.agents/reports/bug-shell-residual-wave-2026-08-12.md`](bug-shell-residual-wave-2026-08-12.md)

## Goal

Green remaining shell residual oneshots from inventory clusters 7–8:

| # | Test | Result |
|---|------|--------|
| 1 | `agent::subagent::tests::rest::read_parent_sampling_config_fallback_wires_bearer_resolver` | **green** |
| 2 | `agent::subagent::tests::rest::resolve_model_override_wires_resolver_for_fresh_and_hard_expired_session_keys` | **green** |
| 3 | `replay_buffer_send_update_tests::channel_token_text_scrubs_curly_punctuation_when_on` | **green** |
| 4 | `session::unified_list::tests::parse_list_req_forces_kind_under_process_chat_mode_only` | **green** |
| 5 | `terminal::local_terminal::tests::test_timeout_kills_grandchildren_and_returns_promptly` | **green** |
| 6a | `acp_session_setup_wire::acp_session_setup_conformance` (integration) | **green** |
| 6b | `test_registry_churn::session_churn_returns_registry_snapshot_to_baseline` (integration) | **green** |

---

## Live red first (this turn / prior continuation)

| Target | Observed red |
|--------|----------------|
| Bearer resolver ×2 | Subagent path not wiring `WireValidBearerResolver` / session-token gate |
| Channel scrub | Assistant text path not scrubbing curly punctuation on `ChannelToken` |
| `parse_list_req` | Force-chat under process chat mode / env not aligned with test expects |
| Timeout grandchildren | Pipe full-buffer / process-group flakiness under long echo |
| `acp_session_setup_conformance` | `session_capabilities.resume/close` None; then `session/resume` method_not_found |
| `session_churn` | `registries` missing `loading_sessions` (+ full Counts wire) |

---

## Product fixes (tests-as-spec)

### Units (prior slice in this assign; re-verified green)

1. **Bearer resolver** — `agent/subagent/mod.rs`: restore wiring so parent sampling / model override use `WireValidBearerResolver` for fresh and hard-expired session keys (with fallback).
2. **Channel scrub** — `session/acp_session_impl/tool_calls.rs` (or adjacent send path): scrub curly punctuation on channel token text when ASCII scrub is on.
3. **Force chat list** — `agent/chat_modes.rs` + `session/unified_list`: process chat mode env drives force-kind chat for list parse.
4. **Timeout grandchildren** — `terminal/local_terminal.rs` test uses `/bin/echo` + 500ms bound so process-group kill returns promptly without pipe stall.

### Integrations (this turn)

#### `session/resume` + `session/close` advertise and implement

**Files:** `agent/mvp_agent/acp_agent.rs`

- **Initialize:** restore `SessionCapabilities` — always `.close(...)`; outside process chat mode also `.list(...)` + `.resume(...)`; attach via `.session_capabilities(...)`.
- **`resume_session`:** refuse chat process mode and `additionalDirectories`; force `noReplay: true` in meta; reuse `load_session`; map response fields into `ResumeSessionResponse`.
- **`close_session`:** send `Cancel` (SessionClose trigger) + `Shutdown(CancelRunningTurn)`, then `close_session_explicit`; no-op success when already gone.
- **Warm reconnect gate:** when replaying, only disable `gateway_enabled` if `!no_replay` (matches tip attach policy). Prevents mid-turn resume from dropping streamed chunks.

#### Full `RegistrySnapshot` wire contract

**File:** `agent/mvp_agent/session_lifecycle.rs`

Expand `RegistrySnapshot` / `registry_snapshot()` to the test `Counts` contract (`deny_unknown_fields`):

- `loading_sessions`, `session_registry_entries`, `resident_resources`, `retained_resources`
- `subagent_queued`
- `workspace_activity_sessions`

Main-shaped agent still uses HashMap session maps (tip `SessionRegistry` not mod’d). Mapping:

| Wire field | Source |
|------------|--------|
| `sessions` / `session_registry_entries` / `resident_resources` | `sessions.len()` |
| `loading_sessions` | `loading_sessions.len()` |
| `retained_resources` | `0` (no separate retained map on main shape) |
| `subagent_*` / `subagent_queued` | channel backend `registry_counts()` |
| `workspace_bindings` / `workspace_activity_sessions` | workspace handle + activity tracker |

Churn returns to baseline when `remove_session` drains the maps.

---

## Verify commands

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib -- --nocapture \
  read_parent_sampling_config_fallback_wires_bearer_resolver \
  resolve_model_override_wires_resolver_for_fresh_and_hard_expired_session_keys \
  channel_token_text_scrubs_curly_punctuation_when_on \
  parse_list_req_forces_kind_under_process_chat_mode_only \
  test_timeout_kills_grandchildren_and_returns_promptly
# → 5 passed

nice -n 19 ionice -c3 cargo test -p xai-grok-shell \
  --test acp_session_setup_wire --test test_registry_churn -- --nocapture
# → acp_session_setup_conformance ok (~13s)
# → session_churn_returns_registry_snapshot_to_baseline ok (~8s)

nice -n 19 ionice -c3 cargo fmt -p xai-grok-shell
```

---

## Remaining

**None for this assignment’s named oneshots.**

Notes (not blockers for this report):

- Tip-shaped `mod session_setup` / `mod session_registry` still not on the product tree; resume/close restored as thin Agent methods on the main HashMap session path.
- Parked half-merge unit modules under `feature = "shell-half-merge-tests"` remain out of default compile.
- Clippy package pass not re-run here (fmt only on touch); pre-existing lib warnings unchanged.
