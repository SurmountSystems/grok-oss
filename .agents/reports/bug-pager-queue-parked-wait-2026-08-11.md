# Report: queue parked_wait cluster green

**Date:** 2026-08-11
**Crate:** `xai-grok-pager`
**Scope:** Residual Cluster A (queue parked look vs held occupancy) + nearby park re-eval reds.

## Red observed

```
cargo test -p xai-grok-pager --lib parked_wait -- --test-threads=8
# FAIL: parked_wait_holds_queue_and_explains_itself  (!renders_parked with held row)
# FAIL: parked_wait_clears_progress_bar_notification (progress not active before marker)
```

Also red (same contract / nearby):

- `local_delete_of_last_held_row_flips_parked_look_on` (held → delete last → marker + parked)
- `wait_on_already_completed_task_pushes_no_parked_marker` (imminent wait still looked parked)
- `task_backgrounded_after_zero_work_wait_all_restores_park` (no re-eval after bg register)

## Root cause

Monorepo half-merge simplified Surmount’s slot-based parked look:

```rust
// broken (monorepo-simplified)
fn renders_parked(&self) -> bool {
    self.is_parked_on_sendable_wait() && !self.is_waiting_on_subagent()
}
```

Surmount live-test contract (and prior Surmount tips, e.g. `f17e84d8` / soft-interject wave):

1. **Parked look requires the parked-marker slot** for the current prompt (`Rendered` or `Forgone`).
2. Held queue withholds the marker → no parked look, progress stays busy.
3. Local/server queue delete of the last held row must re-call `maybe_push_parked_marker` (no ACP rebroadcast for local).
4. `task_backgrounded` must re-eval park like `SubagentSpawned` (wait-all zero-work skip → work registers later).

Tests are product spec; product restored to match.

## Product fixes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs` | Restore slot-based `renders_parked` (slot prompt id matches current + sendable wait + not subagent). Docs match. |
| same | After local **and** optimistic server delete: `maybe_push_parked_marker()` so last-held-row delete flips parked chrome immediately. |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/background.rs` | After root `task_backgrounded` insert: re-borrow agent and `maybe_push_parked_marker()`. |

## Green

```
cargo test -p xai-grok-pager --lib -- \
  'app::dispatch::queue::tests::parked_wait' \
  'app::dispatch::queue::tests::local_delete_of_last_held_row' \
  'app::acp_handler::tests::interjection' \
  -- --test-threads=8
# 28 passed (incl. all parked_wait + interjection module)

cargo test -p xai-grok-pager --lib 'app::dispatch::queue::tests' -- --test-threads=8
# 68 passed
```

`cargo fmt -p xai-grok-pager` run after edits.

## Contract (restored)

`renders_parked` ≡ parked-marker slot consumed for this prompt **and** still in a sendable non-subagent wait.

- Held rows → marker withheld → running chrome / OSC progress can stay active.
- Empty-queue park after `maybe_push` → marker + stopped look + progress off.
- Forgone slot (interject continued park) → parked chrome without marker line.
- Imminent (already-finished) wait → no slot → not parked.
- Last held-row delete / task_backgrounded / subagent spawn → re-eval marker.

## Residual

None for this cluster. Other residual mop clusters (mode_support, structural layout) unchanged.
