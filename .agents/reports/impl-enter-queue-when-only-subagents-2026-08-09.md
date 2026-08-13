# Report: Enter:send when only background subagents run

**Date:** 2026-08-09
**Board:** `bug:enter-queue-when-only-subagents`
**Tree:** `/home/hunter/Projects/surmount/grok-build`
**No git commit** (human-only).

## Operator fact

Status showed subagents still running / queued. Footer composer cue was
`Enter:queue` even though the **primary** turn was not mid-tool/thinking.
Operator: background subagents alone must not force queue-only main input.
Main chat should send normally when the primary is idle; queue/interject is
for when the **primary** turn is busy.

## Prior contract (wrong for this intent)

`AgentView::holds_queue_for_background()` was `watchers().subagents > 0` and
drove:

- drip-feed drain hold (`maybe_drain_queue`)
- `enter_prompt_mode` → Queue
- `held_queue_count` / status `· Enter queues` / `· N queued — Interject to force`
- idle Interject force-drain

That treated “any L2 subagent live” like primary busy.

## New contract

| State | Enter with text | Interject | Status |
|-------|-----------------|-----------|--------|
| Primary **idle** + live background subagents | **Send** (normal main turn; children parallel) | Toast: press Enter to send | `N subagent(s) still running` + pause/stop; **no** Enter queues / force-drain |
| Primary **busy** (thinking/tools/stream/wait) | **Queue** | Soft mid-turn interject (unchanged) | Mid-turn `N queued` / Enter to interject as before |
| Parked empty sendable wait | Cancel-and-send when empty (unchanged) | — | Unchanged |
| Monitors only | Send (unchanged) | — | Still-running only |

Pause/stop while children are live: **kept** via
`has_live_background_subagents()` (same count as still-running cue).
Cancel-resume keep while children live: **kept** on that predicate.

## Product changes

| Area | Change |
|------|--------|
| `agent_view/queue.rs` | `has_live_background_subagents()`; removed queue-hold method; `held_queue_count` ignores children-only idle |
| `dispatch/queue.rs` | Removed drain block on background subagents |
| `dispatch/prompt.rs` | `queued_while_busy` = primary turn running only |
| `views/agent.rs` | `enter_prompt_mode` ignores bg hold; Enter:send when idle + children; Interject mid-turn only; pause still uses live children |
| `views/turn_status.rs` | No idle `Enter queues` / force suffix |
| `agent_view/prompt.rs` | Idle Interject always toast (no force-drain) |
| `agent_view/queue.rs` interject | Idle row Interject toast only; removed dead force-drain helper |
| `render.rs` / `mouse.rs` | Queue `[Interject]` only when primary turn running |
| `dispatch/turn.rs` / ACP finish | Cancel-resume / last-child clear use `has_live_background_subagents` |
| User-guide `03-keyboard-shortcuts`, `16-subagents`; `FORK.md` Work A line | Match new contract |

## Tests (named contracts)

- `enter_prompt_mode_matrix_matches_dispatch_predicates` — idle + live bg → **Send**
- `prompt_idle_with_live_subagents_submit_hint_is_send`
- `prompt_running_submit_hint_is_queue_and_interject` (mid-turn unchanged)
- `background_subagents_do_not_hold_queue_while_parent_idle`
- `idle_with_subagents_empty_queue_does_not_show_enter_queues_cue`
- `idle_with_subagents_does_not_claim_enter_queues_or_force`
- `idle_with_subagents_paints_pause_and_stop_hits` (pause/stop kept)
- `force_interject_idle_with_live_subagents_is_noop_toast`
- `mouse_interject_not_painted_when_idle_with_live_subagents`
- Cancel-resume with live children still green

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
cargo test -p xai-grok-pager --lib -- \
  enter_prompt_mode prompt_idle prompt_running background_subagent \
  idle_with_subagent force_interject mouse_interject monitors_alone \
  force_drain held_queue cancel_resume zombie successful_turn_without_live \
  interject_contract work_control_chrome
# 40 passed
```

## Not done

- Host `~/.grok/docs/user-guide/` mirror not dual-written (product guide updated).
- Research note `doc/dev/research/queue-hold-background-subagents-2026-07-24.md` is historical; product supersedes it.
- No git commit / stage.
