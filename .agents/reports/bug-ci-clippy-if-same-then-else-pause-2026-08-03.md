# CI clippy: if_same_then_else in global_work_pause

**Date:** 2026-08-03
**Scope:** `crates/codegen/xai-grok-pager/src/app/global_work_pause.rs`
**Status:** fixed, clippy + tests green

## Error

Clippy `if_same_then_else` on `PausedSessionSnapshot::mark_resume_consumed`: both arms of

`if pending_queue_len > 0 { Waiting } else { Waiting }`

were identical.

## Intent

`WorkLifecycle` after a one-shot mid-turn resume is consumed:

| Condition | State | Why |
|-----------|--------|-----|
| `pending_queue_len > 0` | `Waiting` | Remaining drip-feed queue is still incomplete |
| `pending_queue_len == 0` | `Finished` | Interrupted turn was re-queued once; unit is terminal for this pause cycle |

Empty-queue → `Finished` matches design notes (finished work is never re-spawned) and `capture` idle → `Finished`. The prior identical `Waiting`/`Waiting` arms were a product bug, not a pure lint nuisance.

## Change

- `mark_resume_consumed`: empty queue → `WorkLifecycle::Finished`; non-empty → `Waiting`.
- Tests: assert Waiting when queue had items (`mid_turn_resume_continues_once`); new `mark_resume_consumed_finished_when_queue_empty`.

## Verification

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib --locked -- -D warnings   # ok
cargo test -p xai-grok-pager --lib -- global_work_pause global_pause
# 19 passed (including mark_resume_consumed_finished_when_queue_empty)
```

No git commit/add.
