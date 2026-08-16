# Process mop: Human-green box caret (full prompt widget)

Process mop only. No product edits. No new features.

SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

Primary implementer report: `.agents/reports/bug-composer-box-caret-unused.md`.

`/tmp` was 100% full (tmpfs). First clippy used the default temp dir and died in `xai-grok-tools-api` protoc (`No space left on device`). Later commands set `TMPDIR=/home/hunter/.cache/grok-build-tmp`. Workspace `target/` was used. No other `CARGO_TARGET_DIR` trees were wiped.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager` | 0 |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` (default `/tmp`) | 101 |
| same clippy with `TMPDIR=/home/hunter/.cache/grok-build-tmp` | 101 |
| named `cargo test -p xai-grok-pager --lib --` filters below (same `TMPDIR`) | 101 |

Named test filters (same list as the implementer):

- `paint_composer_box_cursor_grapheme_phases_keep_letter`
- `paint_composer_box_cursor_uses_human_green`
- `left_through_letters_empty_phase_not_neon`
- `paint_composer_box_cursor_blank`
- `mid_buffer_space_caret`
- `caret_move_clears`
- `focused_composer_paints_human_green_box_caret`
- `left_arrow_with_chrome_prefix`
- `left_arrow_does_not_insert_prompt_prefix`
- `ctrl_home_end_page_move`
- `paint_composer_box_cursor_phase_only_styles`

Those functions are present in `views/prompt_widget/tests.rs`. The crate did not compile, so they did not run.

## Clippy / rustc

First clippy: ENOSPC while compiling `xai-grok-tools-api` (`/tmp/.tmp…/debug-redact.pbbin`).

Retry with home-cache `TMPDIR` compiled as far as `xai-grok-pager` lib, then rustc failed (3 errors on `--lib`, 4 on `--lib` tests). Clippy never reached unused-item lint.

Not the implementer's unused constants in `settings/defs.rs` (`CANCEL_SUBAGENTS_ON_TURN_CANCEL_CHOICES`, `AUTO_COMPACT_THRESHOLD_*`). Rustc failed first on other live writers:

1. `ActionId::ToggleGlobalPause` missing (`app/agent_view/render.rs`, and on the test compile also `views/agent.rs`). Pause chips writer is live. `render.rs` and `actions.rs` are do-not-touch.
2. `PagerLocalSnapshot` initializers missing `auto_compact_threshold_percent`, `auto_compact_threshold_tokens`, `features_session_recap`, and two other fields (`app/dispatch/dashboard.rs`, `app/dispatch/prompt.rs`). Remaining-settings restorer is live. Settings files are do-not-touch.

## Fallout

None mopped. Those compile errors are other in-flight writers, not caret-slice fmt/clippy/test fallout.

No files changed by this mop.
