# Queue hold while any background subagent is live (Slice 3)

**Date:** 2026-07-24  
**Workspace:** grok-build  
**Status:** implemented (working tree; no agent commit)

## Goal

Hold the parent’s pending-prompt queue while **any live background subagent**
is running — not only when the parent is blocked on a wait tool — so typed
follow-ups do not start a conflicting main turn while children work.

## Predicate (verified)

| Item | Location / value |
|------|------------------|
| Hold gate | `AgentView::holds_queue_for_background()` |
| File | `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs` |
| Condition | `self.watchers().subagents > 0` |
| What counts | Standalone unfinished subagents (`is_running()` and no `workflow_run_id`) — same set as the still-running status cue |
| What does **not** count | Monitors, plain background commands, scheduled loops, workflow-owned children |

Drain path: `maybe_drain_queue` → `maybe_drain_queue_with(..., bypass=false)`
blocks with log reason `background_subagents_live`. Send-now uses
`force_drain_queue_past_background` / `Action::ForceDrainQueue` to bypass.

On last child finish (ACP subagent terminal update), if the parent is idle and
has local pending prompts, the handler tries `maybe_drain_queue` so the queue
starts without another keystroke.

## UX

| Surface | Behavior |
|---------|----------|
| Status (idle + live children + held rows) | `… still running · N queued — Interject to force` (or `· N queued` if top not sendable) |
| Mid-turn sendable wait | `· N queued — Enter to interject` (soft; never cancel) |
| Interject while idle + hold | Force drain / enqueue-front + force drain; toast *Interject — starting despite background subagents*; queue row `[Interject]` shown |
| Bare Enter idle + hold | Enqueue + hold (local drip-feed); no conflicting turn |
| Monitors only | No hold; drain proceeds |

## Tests

In `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs`:

- `background_subagent_holds_queue_while_parent_idle`
- `background_subagent_hold_lifts_when_children_finish`
- `monitors_alone_do_not_hold_queue`

In `crates/codegen/xai-grok-pager/src/views/turn_status.rs`:

- `idle_with_subagents_and_held_queue_shows_force_hint`

## Docs

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | Hold while blocked **or** any live background subagent; monitors exception; auto-drain; send-now force while idle |
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | § *Queue hold while subagents run* |
| Host mirrors | `~/.grok/docs/user-guide/03-keyboard-shortcuts.md`, `…/16-subagents.md` (copied to match) |

## Files touched (product code)

- `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs` — predicate + `held_queue_count`
- `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` — drain hold + force path + tests
- `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` — `ForceDrainQueue`
- `crates/codegen/xai-grok-pager/src/app/actions.rs` — `ForceDrainQueue`
- `crates/codegen/xai-grok-pager/src/app/agent_view/prompt.rs` — send-now while idle+hold
- `crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs` — drain after last child finishes
- `crates/codegen/xai-grok-pager/src/views/turn_status.rs` — idle held-queue suffix + render test
- user-guide paths above + this join note

## Out of scope / intentional

- Monitors do **not** hold (optional; left out on purpose — indefinite runs).
- Workflow-owned children roll into the workflow watcher count, not the
  subagent hold count (matches still-running cue).
- No git commit (human-only).
