# Slice 0a: pending plan must not hijack existing plan

Date: 2026-07-24  
Status: fixed (product code + tests; no commit from this work)

## Bug (user-facing)

User submits a plan workflow / `/plan <desc>` (or other plan-related text) as a
**pending** follow-up while work is already in flight or plan approval is open.
That pending intent used to drive or corrupt the **existing** plan task/mode
instead of waiting for a clean independent next turn after current work
settles.

## Root cause (verified in code)

Three cooperating holes, not one:

1. **Pager drain while approval open**  
   `maybe_drain_queue` held for non-idle turns but **not** for
   `plan_approval_view.is_some()`. On the idle resume re-park path the session
   is idle with approval open; a queued follow-up drained into a competing
   turn that could cancel/stale the in-flight `exit_plan_mode` decision.

2. **Shell promote while awaiting approval**  
   `SessionActor::maybe_start_running_task` blocked only on `running_task`.
   Plan approval parks without a running task (`awaiting_plan_approval` /
   parked plan-approval interaction). Server-side `pending_inputs` still
   promoted → new turn during approval.

3. **`/plan <desc>` mid-turn mode switch**  
   `dispatch_enter_plan_mode` set `plan_mode_pending` + emitted
   `SetSessionMode` (or queued a plain description without enter-plan
   semantics) even when a turn was running or approval was open. That
   flipped plan mode under the existing lifecycle. Already-in-plan
   `/plan <desc>` previously dropped the description.

Related (deliberately not redesigned): auto-implement after approve, abandon
kick, background-subagent hold — separate residual slices.

## Fix (surgical)

| Layer | Change |
|-------|--------|
| Pager drain | Block `maybe_drain_queue` when `plan_approval_view.is_some()` |
| Shell start | Block `maybe_start_running_task` when awaiting or parked plan approval |
| Deferred enter-plan | `QueuedPrompt.enter_plan_mode` + `enqueue_enter_plan_prompt`; drain emits `SetModeThenPrompt` |
| `/plan <desc>` busy | Queue deferred enter-plan row; **no** mid-flight `plan_mode_pending` / `SetSessionMode` |
| `/plan <desc>` already in plan | Same deferred row (idle drains immediately as next plan turn) |
| Abandon | If idle + local pending after abandon → `Action::DrainQueue` only (not on approve/revise — would race shell implement/revise) |
| Combine gate | Enter-plan rows treated as non-plain so they are not merged away |

## Files

### Product

- `crates/codegen/xai-grok-pager/src/app/agent.rs` — `enter_plan_mode` on `QueuedPrompt`; `enqueue_enter_plan_prompt`; combine treats enter-plan as non-plain
- `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` — drain gate `plan_approval_open`; drain path for `enter_plan_mode` → `SetModeThenPrompt`
- `crates/codegen/xai-grok-pager/src/app/dispatch/modes.rs` — deferred queue when busy / already in plan / approval open; no mid-turn mode hijack
- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` — abandon → `DrainQueue` when idle+pending; approve/revise leave drain to turn-end
- `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` — `QueuedPrompt` construction via `plain` + struct update for new field
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/notification_drain.rs` — block start while awaiting/parked plan approval
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs` — resume leave path then `maybe_start` (gate order preserved)

### Tests

- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/modes.rs`
  - `slash_plan_with_args_while_turn_running_queues_without_mode_switch`
  - `slash_plan_with_args_while_plan_approval_open_queues_without_hijack`
  - `slash_plan_with_args_already_in_plan_queues_deferred_enter_plan` (updated)
- `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` (inline tests)
  - `drain_blocked_when_plan_approval_open`
  - `deferred_enter_plan_row_drains_as_set_mode_then_prompt`
- `crates/codegen/xai-grok-shell/src/session/acp_session_tests/plan_approval_resume_tests.rs`
  - `maybe_start_blocked_while_awaiting_plan_approval`

### Verify (local)

```bash
cargo test -p xai-grok-pager --lib -- slash_plan_with_args drain_blocked_when_plan_approval deferred_enter_plan
cargo test -p xai-grok-shell --lib -- maybe_start_blocked_while_awaiting_plan_approval plan_approval
```

Both green at time of this note (6 pager + 17 shell filtered).

## Non-goals / residual

- Does **not** change approve-with-comments flush (separate research notes).
- Does **not** redesign auto-implement after plan approve.
- Does **not** change background-subagent queue hold (sibling research:
  `queue-hold-background-subagents-2026-07-24.md`).
- Human-only: signed commit when ready; agents do not commit.
