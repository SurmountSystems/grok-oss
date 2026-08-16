# Clear finished `[−]` todo-pane paint restored

Host omitted `spawn_subagent`. This L2 implementer walked the diagnosis, restored origin/main contracts, and landed the paint. No L3 specialists.

## Named contract

1. When the todo pane is open and at least one completed or cancelled row exists, paint clickable `[−]` (`clear_finished_button`, U+2212 minus) in the todo header next to close. Focused or unfocused.
2. Click archives finished rows via the existing Clear finished / `/clear-completed-todos` path (`Action::ClearCompletedTodos` → `ClearedReason::UserClearCompleted`). No second wipe.
3. Hidden board or no finished rows: no button.
4. Hover is quiet stronger (`gray` idle, `text_primary` hover). Not `accent_user` green. Not `accent_running` magenta.
5. Must not overlap tasks chrome hits. Tasks open/kill win z-order over Clear finished.
6. Compact layout still reserves one row above the todo body so Clear cannot paint into tasks model/timer / `[↗]` / `[x]`.
7. Optional focused `X` still archives when the todo pane is focused.

Slash `/clear-completed-todos` was already wired. Glyphs already existed. Paint and hit were unused.

## Red (before product paint)

Test added first. `hit_todo_clear_done` existed as an empty field so the test could compile. Draw did not register a hit.

Command:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-clear-finished-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test --offline -p xai-grok-pager --lib -- open_todo_with_finished_paints_clear_even_when_unfocused
```

Fail reason:

```
app::agent_view::render::clear_finished_paint_tests::open_todo_with_finished_paints_clear_even_when_unfocused
open + finished must register clear-finished hit when unfocused
```

That was the named contract fail. Product paint had not been added yet.

## Green (same tests)

Command:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-clear-finished-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test --offline -p xai-grok-pager --lib -- \
  open_todo_with_finished_paints_clear_even_when_unfocused \
  clear_finished_only_when_open_with_finished_rows \
  clear_finished_hit_does_not_intersect \
  click_tasks_model_timer_chrome_opens_subagent \
  clear_finished_click_does_not_open_subagent \
  clear_finished_click_dispatches_when_hit_rect_set \
  clear_completed_todos_x_key_only_when_todo_pane_focused \
  clear_finished_chrome_paints_when_label_supplied \
  clear_finished_action_x_stable_with_close_reserved \
  unfocused_unhovered_without_action_yields_no_chrome \
  action_button_sits_left_of_close_with_gap \
  action_button_without_close_reserves_close_slot \
  action_button_x_stable_with_or_without_close \
  clear_finished_action_idle_is_quiet_not_neon_green_or_magenta \
  clear_finished_action_hover_is_stronger_than_idle \
  clear_finished_disabled_reserves_slot_and_paints_dim
```

Result: 16 passed, 0 failed.

Clippy: `cargo clippy --offline -p xai-grok-pager --lib -- -D warnings` exit 0.

Did not run `cargo fmt -p xai-grok-pager` (other writers own `plan.rs` / `event_loop.rs`). Did not edit user-guide.

## What changed

Restored origin/main contracts rather than inventing weaker ones.

- `SelectionBox` paints an optional action label left of close, with a reserved close slot so focus does not jump the control. Idle is theme gray. Hover is `text_primary`. Disabled is `gray_dim`.
- `render_todo_chrome_with_close_label` accepts that action label. Unfocused open boards use action-only paint (no focus rails).
- Todo layout always reserves one chrome row above the body (`todo_chrome_gap = pane_gap.max(1)`), including compact.
- Draw wires `[−]` when `todo.counts().completed + cancelled > 0` and the pane height is live. Registers `hit_todo_clear_done`.
- Mouse: tasks open/kill first, then Clear finished click dispatches `Action::ClearCompletedTodos`. Hover updates the quiet-stronger paint.
- Focused `X` on the todo pane dispatches the same action. Tasks and catalog do not.

## Files touched

- `crates/codegen/xai-grok-pager/src/scrollback/selection.rs`
- `crates/codegen/xai-grok-pager/src/views/agent.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/panes.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs`
- `crates/codegen/xai-grok-pager/src/app/mouse.rs`

Not touched: `plan.rs`, `event_loop.rs`, `docs/user-guide/**`, `settings/defs.rs`, turn-status pause/resume chips, spend/ledger, welcome, prompt caret, F9, branding.

## Leftover

A live TUI built from the previous binary will not show `[−]` until you rebuild and restart that binary. Tests see the new paint. User-guide `03` / `04` / `17` is another writer's job.
