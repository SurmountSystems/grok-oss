# Process mop: branding + chrome leftover

**Date:** 2026-08-14  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:process-mop-branding-chrome`  
**Role:** process mop only. No new product work.

Isolated compile:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-branding-chrome-mop-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
```

`/tmp` was not used. rustc 1.97.1. `--offline` after the first fetch (all cargo commands used `--offline` except `fmt`).

## Commands and exits

| Step | Command | Exit |
|------|---------|------|
| 1. fmt | `cargo fmt -p xai-grok-pager -p xai-grok-pager-bin` | **0** |
| 2. clippy lib | `cargo clippy -p xai-grok-pager --offline --lib -- -D warnings` | **0** (Finished in 2m 40s) |
| 3. clippy bins | `cargo clippy -p xai-grok-pager-bin --offline --bins -- -D warnings` | **0** (Finished in 2m 49s) |
| 4a. branding lib | `cargo test -p xai-grok-pager --offline --lib --` plus the 12 names below | **0** — 12 passed, 0 failed |
| 4b. version crate | `cargo test -p xai-grok-pager-bin --offline --test version_without_tty -- --test-threads=1` | **0** — 3 passed, 0 failed |
| 5. chrome lib | `cargo test -p xai-grok-pager --lib --offline --` plus the chrome filters below | **0** — 12 passed, 0 failed |

`--all-targets` clippy was not expanded. `--lib` and `--bins` were both green.

Did **not** edit any files. fmt did not rewrite anything. No compile, lint, or test fallout to mop.

## Branding named tests (12 lib + version crate)

Lib filter names (space-separated libtest substrings, `--test-threads=1`):

- `print_exit_resume_hint_writes_expected_lines`
- `print_exit_resume_hint_includes_minimal_flag`
- `print_exit_resume_hint_includes_session_summary`
- `print_exit_resume_hint_truncates_summary_to_width`
- `failed_relaunch_hint_includes_screen_mode_env`
- `two_manual_renames_stay_ambiguous`
- `cli_command_name_is_grok_oss`
- `cli_help_output_header`
- `user_guide_resume_and_version_examples_use_grok_oss`
- `product_cli_name_is_grok_oss`
- `product_version_line_uses_grok_oss_not_bare_grok`
- `resume_session_command_uses_grok_oss`

Each matched exactly one test. 12 passed.

`version_without_tty`: `version_flag_exits_zero_when_rebuild_captures_stdio`, `version_flag_exits_zero_when_stdin_is_dev_null`, `version_flag_exits_zero_when_stdin_pipe_is_closed`. 3 passed.

## Chrome named tests

The chrome implementer report lists **9** filter strings that match **12** tests (substring match, not a `|` regex). Ran those 9:

- `titled_doge_composer_frame_is_prompt_border_not_context_yellow`
- `title_renders_on_top_border_with_corners_intact`
- `no_title_keeps_plain_top_border`
- `user_prompt_entry_renderer_paints_green_rail`
- `info_line_model_name_uses_accent_model_not_gray`
- `user_prompt_block_accent` (2 tests: static rail + DOGE green rail)
- `agent_message_block_accent` (3 tests: running, DOGE running, finished)
- `paint_composer_box_cursor_uses_human_green`
- `focused_composer_paints_human_green_box_caret`

12 passed. No filter matched 0 tests.

## Leftovers (from the two implementer reports; not mopped)

These are product leftovers those slices already named. This mop did not change them.

- Installed `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss` is still the previous artifact until the operator `/rebuild`s or `just install`s. This mop did not `/rebuild`.
- Live TUI still shows the old binary (yellow titled composer sides, all-yellow footer) until a successful rebuild and a full quit/reopen.
- User-guide leftovers still say `grok sessions` / `grok login` / `grok mcp add` in places those slices did not own. Tracked separately. This mop did not rewrite them.
- JSON `--version` payload is still `{ currentVersion, channel }` with no product name field.
- Yellow `│` on the last assistant paragraph in the live screenshot: chrome report ruled out finished-agent rail and source `selection_border`. Cause still unknown. Not churned here.
- SuperGrok is paid. Neither slice nor this mop painted "free SuperGrok."

## Did not do

- No product source edits
- No `git add` / commit / push
- No `/rebuild`
- No L3 spawn
- No user-guide `grok sessions` / `grok login` rewrite
