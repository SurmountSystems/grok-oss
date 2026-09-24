# Plan: L2 window shows when each row happened

This is the plan. It was not shown for approval before the code change. That was a miss.

## What you showed

Screenshot Thu Sep 24, 2026, 11:12 AM. L2 window titled "Wrap SelfApplyFs on 4.7", project `~/Projects/ai/iso`.

The header says `11:12:36 AM`. The rows do not. A thought says `Thought for 15.4s`. A review row says `26m9s` and `1m42s`. There is no clock time on the thought, the tool run, or the specialist row.

## Requirement

Each thought, each tool run, and each specialist row in the L2 window shows the local time that row happened. A duration may stay. The main transcript does not need that clock.

## What is already in the source, without this plan being approved

Paint reads `ScrollbackEntry.created_at`. It does not call `Local::now()`. The format is `%-I:%M %p`, so 2026-09-24 11:12:36 paints `11:12 AM`. Seconds are not shown. If `created_at` is missing, no clock is painted. `AgentView::draw` turns this on only when the view is the L2 window. The nested L2 overlay paints the clock even when `is_subagent_view` is false.

Files:

- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/render.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/render_tests.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`

Test `scrollback::wrappers::entry_renderer::tests::l2_window_thought_row_shows_local_clock_not_only_duration`:

- Red: the row was `Thought for 15.4s` and the clock was absent. Exit 101.
- Green: same test, exit 0, 1 passed.

## Not done

The grok-oss you are looking at does not have this. The last install is `1.0.3 (8c46e6b95136)`, from before this paint change. Tool rows and specialist rows were not a separate red test. Only the thought row was. Seconds, as in `11:12:36`, are not in the format.
