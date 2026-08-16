# Process mop: Clear finished

Mop only. No product work. Did not run `cargo fmt -p xai-grok-pager`. Did not touch `plan.rs`, `event_loop.rs`, `docs/user-guide/**`, or pause chips.

## Env

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-clear-finished-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
```

## Clippy

```
cargo --offline clippy -p xai-grok-pager --lib -- -D warnings
```

- First two foreground runs: killed at the 300s shell cap while still compiling `xai-grok-pager` (cold target).
- Retry (background, incremental after those kills): **exit 0**. Finished `dev` in 4m 28s. No warnings under `-D warnings`.

## Named tests (16)

```
cargo --offline test -p xai-grok-pager --lib -- \
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

**exit 0.** `16 passed; 0 failed; 0 ignored; 8844 filtered out.` Compile finished `test` in 12m 24s. `render.rs` compiled; no half-written other-writer fail.

Matched tests:

- `scrollback::selection::tests::action_button_x_stable_with_or_without_close`
- `scrollback::selection::tests::action_button_without_close_reserves_close_slot`
- `scrollback::selection::tests::clear_finished_action_hover_is_stronger_than_idle`
- `scrollback::selection::tests::clear_finished_disabled_reserves_slot_and_paints_dim`
- `scrollback::selection::tests::action_button_sits_left_of_close_with_gap`
- `scrollback::selection::tests::clear_finished_action_idle_is_quiet_not_neon_green_or_magenta`
- `views::agent::tests::unfocused_unhovered_without_action_yields_no_chrome`
- `views::agent::tests::clear_finished_action_x_stable_with_close_reserved`
- `views::agent::tests::clear_finished_chrome_paints_when_label_supplied`
- `app::agent_view::panes::clear_completed_todos_key_tests::clear_finished_click_dispatches_when_hit_rect_set`
- `app::agent_view::panes::clear_completed_todos_key_tests::clear_completed_todos_x_key_only_when_todo_pane_focused`
- `app::agent_view::render::clear_finished_paint_tests::clear_finished_click_does_not_open_subagent`
- `app::agent_view::render::clear_finished_paint_tests::open_todo_with_finished_paints_clear_even_when_unfocused`
- `app::agent_view::render::clear_finished_paint_tests::clear_finished_hit_does_not_intersect_tasks_subagent_open_or_kill`
- `app::agent_view::render::clear_finished_paint_tests::clear_finished_only_when_open_with_finished_rows`
- `app::agent_view::render::clear_finished_paint_tests::click_tasks_model_timer_chrome_opens_subagent`

## Edits

None. Clippy and tests were already clean. No mop fallout.

## Leftover

A live TUI built from the previous binary will not show `[−]` until that binary is rebuilt and restarted. Tests see the new paint.
