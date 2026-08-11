# Work A: Composer Enter send vs queue cue (2026-08-09)

## Goal

Footer and status must match dispatch: plain Enter is not always "send" when
background subagents hold the queue while the parent looks idle.

## Named contract (TDD)

**Contract:** With parent idle and live background subagent hold, composer text
present, the shortcuts bar advertises `Enter: queue` (not `send`). With hold and
an empty queue, the still-running status line includes `Enter queues` before any
item is queued. Monitors alone must not claim Enter queues.

### Red → green

1. Added matrix unit tests and dogfood cases first (would fail on old
   `is_turn_running`-only footer logic and status that only suffix when
   `held_queue > 0`).
2. Product change: pure `enter_prompt_mode` + wire footer/status.
3. Same filters green.

| Test | Role |
|------|------|
| `enter_prompt_mode_matrix_matches_dispatch_predicates` | Pure function matrix: send / queue / interject / blocked |
| `prompt_idle_with_background_hold_submit_hint_is_queue` | Footer idle + hold + text → queue (+ Interject force chord) |
| `prompt_idle_submit_hint_is_send` | Regression: clean idle still send |
| `prompt_running_submit_hint_is_queue_and_interject` | Regression: mid-turn queue + soft-interject chord |
| `prompt_empty_mid_turn_queue_advertises_interject_including_multiline` | Empty Enter soft-interject |
| `idle_with_subagents_empty_queue_shows_enter_queues_cue` | Status pre-queue hold cue |
| `idle_with_subagents_and_held_queue_shows_force_hint` | Status after queue: force, not empty cue |
| `idle_with_monitors_only_does_not_show_enter_queues_cue` | Monitors do not hold |

## Product changes

### Pure helper (`views/agent.rs`)

- `EnterPromptMode { Send, Queue, Interject, Blocked }`
- `enter_prompt_mode(can_send, turn_running_for_footer, holds_queue_for_background, has_queued_follow_up)`
- `footer_label()` → `"send"` / `"queue"` / `"interject"` / none

Predicates match dispatch: sendable text + (turn running **or** bg hold) → queue;
clean idle → send; empty + mid-turn + queued → interject; else blocked.
Callers pass `is_turn_running && !renders_parked()` so parked empty wait still
labels Enter as send (cancel-and-send).

### Footer (`build_hints` + `normal_pane_hints`)

- New arg `holds_queue_for_background` from `AgentView::holds_queue_for_background()`.
- Prompt Enter label always from `enter_prompt_mode`.
- Queue pane Interject hint when turn running **or** hold.
- Prompt Interject chord also when hold + (composer text or queued follow-up)
  so force-drain is advertised.

### Status (`turn_status.rs`)

Idle + `watchers.subagents > 0` + empty held queue → suffix ` · Enter queues`.
When `held_queue > 0`, keep ` · N queued — Interject to force` (no duplicate
empty cue). Monitors-only still-running rows get no Enter queues suffix.

### User-guide

- `03-keyboard-shortcuts.md`: focus table no longer claims Enter always sends;
  "During an active turn" documents footer truth and empty-queue status cue.
- `16-subagents.md` § Queue hold: footer `Enter: queue`, empty-queue status
  `Enter queues`, then force suffix after items are held.

## Commands

```text
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # exit 0
cargo test -p xai-grok-pager --lib -- views::agent::tests   # 59 ok
cargo test -p xai-grok-pager --lib -- views::turn_status::tests   # 53 ok
```

(`cargo clippy -p xai-grok-pager --all-targets` hits pre-existing failures in
unrelated tests/benches; lib clippy is clean.)

## Files touched

- `crates/codegen/xai-grok-pager/src/views/agent.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `crates/codegen/xai-grok-pager/src/views/turn_status.rs`
- `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md`

## Out of scope (Work B/C/E)

Composer mode chip, persistent mid-turn status beyond hold cue, other plan
verticals.

## Acceptance check

With parent idle and one live background subagent, empty draft, before any
Enter: status shows `… still running · Enter queues`; after typing, footer
shows `Enter: queue`. Interject remains the force path once text or held rows
exist.
