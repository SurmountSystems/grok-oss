# Auto-wake cancel-barrier tests

Date: 2026-08-15  
Crate: `xai-grok-shell` only. Did not touch `xai-grok-tools`. Did not implement ACP file lock. Did not start token economy.

## Named contract

1. After the operator cancels (cancel barrier up: `task_wake_suppressed` and `notifications_suppressed`), a background-task completion must not start a synthetic turn. Admission returns none / `false`, the fallback sits in pending notifications, the reservation stays, and `ReportedTaskCompletions` must not be created or filled. The model must not be told the completion was already reported. A later real user turn can still consume it.
2. With no cancel barrier, the same completion is admitted. Queue acceptance alone must not mark it reported. Only the actual synthetic turn start (`handle_prompt`) marks it reported and releases the reservation.

## Red (before product / harness edit)

Commands (env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`):

```
cargo test -p xai-grok-shell --lib auto_wake_suppression_tests -- --nocapture
```

| Test | Result | Fail reason |
|------|--------|-------------|
| `session::acp_session::auto_wake_suppression_tests::cancel_barrier_rejects_task_completion_wake_without_reporting_it` | FAIL | `declined admission must not report before user re-engagement` at `auto_wake_suppression_tests.rs:275` (`resources.get::<State<ReportedTaskCompletions>>()` was `Some`) |
| `session::acp_session::auto_wake_suppression_tests::task_completion_wake_is_admitted_without_cancel_barrier` | FAIL | `queue acceptance alone must not mark the completion reported` at `auto_wake_suppression_tests.rs:420` (same `get()` was `Some`) |

31 other tests in the module passed. `admit_task_completion_wake` already rejected the barrier case, stored the fallback, kept the reservation, and did not call `mark_completions_reported`. `queue_input` already did not mark `TaskCompleted` ids. `handle_prompt` already marks on synthetic turn start.

Root cause: every `create_test_actor` agent used a shared `state_path` of `/tmp/tool_state.json`. Finalize loads sibling `/tmp/resources_state.json`. That file existed on this host with:

```json
"grok_build.ReportedTaskCompletions": { "reported": [] }
```

So the empty reported set was already in resources before admit or queue. The tests treat resource presence as "already reported." That is why TRY 2 also failed: the leftover file survives the first attempt.

## Green (same commands, after edit)

```
cargo test -p xai-grok-shell --lib auto_wake_suppression_tests -- --nocapture
```

`ok. 33 passed; 0 failed` including both named tests.

Also covered by that run (exact names):

- `cancel_barrier_rejects_task_completion_wake_without_reporting_it`
- `task_completion_wake_is_admitted_without_cancel_barrier`

## Files touched

- `crates/codegen/xai-grok-shell/src/session/acp_session_tests/support.rs`  
  Each test agent now gets a unique `state_path` / session folder under `std::env::temp_dir()` (honors `TMPDIR`). No shared `/tmp/resources_state.json` load.

Did not change `admit_task_completion_wake`, `queue_input`, or `handle_prompt`. Did not change the two test assertions.

## Leftovers

- Host file `/tmp/resources_state.json` is still present (empty reported set plus an unrelated todo). Left it alone. Isolated test agents no longer load it.
- Other test helpers that still hardcode `/tmp/tool_state.json` (none under this helper after the edit) could still pick it up if they exist elsewhere.
- Unique dirs under `$TMPDIR/grok-test-agent-*` are not cleaned up. Empty and unused after the run.
- File-level clippy on `support.rs` as a standalone crate fails (`use super::*`). Expected for a `#[path]` test module. Not crate-wide clippy.
- ACP file lock and token economy were out of scope.
