# Soft interject / wait-park catalog contracts (green)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior residual:** `.agents/reports/impl-upstream-catalog-filters-2026-08-11.md` § B + § D

---

## Goal

Green the six residual soft-interject / wait-park catalog reds with red→green TDD. Prefer surgical restores of Surmount product seams over rewriting tests.

## Red (observed)

### Shell (3)

| Test | Fail |
|------|------|
| `interject_contract_queued_prompt_buffers_without_cancel` | order `["running","p1","held"]` vs `["running","held"]` (row re-inserted as send-now) |
| `interject_contract_idle_keeps_row_queued_no_cancel` | order `["q2","q1"]` vs `["q1","q2"]` (idle promote reorder) |
| `interject_contract_queued_prompt_images_ride_pending_interjections` | `soft interject must never request cancel` (returned `true`) |

**Root cause:** onto hybrid left `handle_interject_queued_prompt` on **send-now promote + cancel** path. Surmount soft-interject law (from `5026d71c`) is buffer into `pending_interjections` mid-turn, never cancel, idle/bash stay queued with LWW edit only.

### Pager (3)

| Test | Fail |
|------|------|
| `interject_contract_queue_shared_never_arms_cancel_while_running` | `is_self_originated_prompt("srv-row-1")` false |
| `wait_on_already_completed_task_pushes_no_parked_marker` | `!renders_parked()` failed (imminent wait still looked parked) |
| `task_backgrounded_after_zero_work_wait_all_restores_park` | `count_parked` 0 vs 1 (no re-eval after `task_backgrounded`) |

**Root causes:** soft dispatch no longer noted the server row as self-originated; `renders_parked` ignored imminent-wait skip; `handle_task_backgrounded` never called `maybe_push_parked_marker` (unlike `SubagentSpawned`).

---

## Product fixes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/prompt_queue.rs` | Restore **soft** `handle_interject_queued_prompt`: buffer plain mid-turn rows into `pending_interjections` + broadcast interjection; never cancel (`false`); idle/bash keep place + LWW `new_text`; turn_running from `current_prompt_id` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` | Soft `QueueInterjectShared`: `note_self_originated_prompt(&id)` without arming send-now cancel |
| `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs` | `renders_parked` also requires `!parked_wait_resolves_imminently()` |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/background.rs` | After bg task insert, `agent.maybe_push_parked_marker()` (mirrors subagent spawn re-eval) |

---

## Green (same filters)

```
cargo test -p xai-grok-shell --lib -- \
  interject_contract_queued_prompt_buffers_without_cancel \
  interject_contract_idle_keeps_row_queued_no_cancel \
  interject_contract_queued_prompt_images_ride_pending_interjections
# 3 passed

cargo test -p xai-grok-pager --lib -- \
  interject_contract_queue_shared_never_arms_cancel_while_running \
  wait_on_already_completed_task_pushes_no_parked_marker \
  task_backgrounded_after_zero_work_wait_all_restores_park
# 3 passed
```

### Broader neighbors (all green)

- Shell `interject` filter: **32 passed** (contracts + bash/idle/stale/drain/goal send-now)
- Pager `interjection::` module: **25 passed** (imminent, wait-all, subagent spawn, markers)

### Verify notes

- Touched sources formatted via `rustfmt --edition 2024` on the four product files (`cargo fmt -p …` blocked by pre-existing missing `pty_e2e/reparked_wait_repushes_buried_marker.rs` include).
- `cargo clippy -p xai-grok-shell -p xai-grok-pager --lib -D warnings` blocked on **pre-existing** `xai-grok-tools` dead-code / disallowed-spawn issues (dep), not on these edits. Shell/pager lib **test** targets compile and run green.
- Stashes `recon-temp-work-b-wip-2026-08-10` / `recon-resume-local-dirt-2026-08-10` **not** dropped.
- No commit / push.

---

## Residual

None for § B / § D six contracts. Catalog items still open elsewhere (from prior mop report): plan five-CTA panel footer, usage.jsonl identity writes, `settings_e2e` timeout, full `just check`.

---

## 10-line summary

1. Observed all six catalog reds on onto tip.
2. Shell soft-interject was send-now promote+cancel; restored Surmount soft buffer path.
3. Mid-turn plain queue rows leave the queue into `pending_interjections` with LWW edit + images.
4. Idle/bash soft interject keep queue order and never cancel.
5. Return value of `handle_interject_queued_prompt` is always `false`.
6. Soft `QueueInterjectShared` notes self-originated without arming cancel.
7. `renders_parked` excludes imminent (already-finished) waits.
8. `task_backgrounded` re-evals park like `SubagentSpawned`.
9. Target 6 + broader interject/park neighbors green.
10. Report path: this file; no commit/push; stashes kept.
