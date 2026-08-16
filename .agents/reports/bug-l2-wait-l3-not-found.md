# bug: L2 wait L3 not found

Status: fixed. Named fixture is green.

L3 id = `01a0087b-6a63-7841-a17e-60b5dfcc2841`

## Contract

An agent that just spawned a subagent must be able to wait on the id the spawn tool returned. `not_found` is wrong while that subagent is still running.

## Red (before product edit)

- Test: `implementations::grok_build::task::coordinator::tests::spawner_can_wait_on_the_id_it_just_received_while_the_task_is_live`
- Command:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-tools --lib spawner_can_wait_on_the_id_it_just_received_while_the_task_is_live -- --nocapture
```

- Fail reason: L2 bound with `ChannelBackend::for_session(..., "l2")` spawned `"l3"`. Coordinator reparented the child so `parent_session_id` became `"parent"`. L2 `query("l3")` returned `None`. The test panicked:

`spawner must find its live child by the spawn id; not_found is wrong`

That is the observed product miss: spawn returned an id, then wait on that same id was `not_found` while the child was still live.

## Green (same test after the product fix)

- Same test name.
- Same command as above.
- Result: `ok`. `1 passed; 0 failed; 0 ignored; 0 measured; 3249 filtered out`.

Nearby named fixtures (not crate-wide cargo), all `ok`:

- `session_backend_cannot_query_or_cancel_foreign_child`
- `loop_tracking_covers_pending_active_and_nested_reparenting`
- `cancel_parent_session_spares_nested_workflow_children`
- `teardown_rejects_spawn_from_cancelled_parent`

## Files changed

- `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/coordinator_tests.rs`
  - Added the named contract test.
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/coordinator.rs`
  - Remember immediate spawner in `spawned_by_session`.
  - Query/cancel visibility uses free `belongs_to_session` (root parent or immediate spawner).
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/coordinator/spawn.rs`
  - On nested reparent, insert `child id → L2 session` before overwriting `parent_session_id` to the root.
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/coordinator/query.rs`
  - Query and inspect match the immediate spawner as well as the root parent.

Did not touch ACP file-lock modules, `implementations/opencode/edit`, or `xai-grok-shell` session `acp_session` auto-wake / cancel-barrier files.

## Root cause

When L2 spawned L3, production bound `ChannelBackend::for_session` to the L2 child session (the L2 subagent id). The coordinator then reparented that nested spawn to the L1 root session so limits and ParentSession stop still work, and it set `surface_completion = false` so L2 is supposed to wait. After that overwrite, `request.parent_session_id` was the root, not L2.

Query, inspect, and cancel-by-id only asked "does this request's parent session equal the caller?" L2's wait used L2's session id. That check failed. The backend answered `None`, which the wait tool surfaces as `not_found`.

The parent thread queries unbound or as the root session, so it still saw the live L3 (example `01a00873-ebae-7491-8d84-40d73273cba4` still running after the L2 had already exited `not_found`). The Subagents list uses the same root-visible map.

The fix keeps a side map of `child id → session that issued the spawn` at reparent time. Visibility is now: unbound, or the stored root parent, or that immediate spawner. A foreign session still cannot see the child.

## Leftovers / blockers

- A background `TaskTool` spawn can still return an id from the tool layer before the coordinator has processed `Spawn`. That is a short fire-and-forget race, not the persistent miss (hundreds of seconds, tens of tool calls) this job fixed.
- `spawned_by_session` entries are dropped when the completed-cache evicts that child, same as the completed snapshot. Live and recently completed children stay findable.
- No blockers. The wait bug did not live in the stay-off in-flight files.
