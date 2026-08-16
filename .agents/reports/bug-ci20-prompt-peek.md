# CI-20 prompt widget + dashboard peek

Board: `impl:ci20-prompt-peek` under `bug:ci-20-unit-fails`

Isolated target: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-prompt-target`, rustc 1.97.1.

First two `cargo test` runs were killed mid-dep compile (120s then 300s). Reds were diagnosed from `PromptWidget::draw` + `render_peek_panel` before a fail log landed. After the product edit, the named six ran green.

## Red reason (per test)

Software box caret is painted at the insertion cell, then `draw` returns `cursor_pos: None` so the hardware cursor does not stack.

| Test | Why it failed |
|------|----------------|
| `ghost_text_renders_at_cursor_when_at_end` | Ghost `" world"` starts at the insertion cell. Caret then treated the leading space as a blank and wrote `█` / reverse plate. First ghost cell was not `gray_dim` + italic. |
| `ghost_text_truncated_to_available_width` | Same overwrite. `" world…"` became `"█world…"` on the solid blink half. |
| `ghost_text_empty_string_not_rendered` | Empty ghost does not paint, but the box caret still wrote `█` at x=5 on the solid half. `trim()` does not strip `█`. |
| `ghost_text_suppressed_when_slash_active` | Slash correctly skipped the shell ghost, then the same end-of-text `█` landed in x=5..10. |
| `render_peek_shows_typed_reply_and_caret` | `"ship it"` painted. `draw().cursor_pos` is `None` after the software caret, so `PeekRenderResult.caret` was `None`. |
| `render_peek_reject_option_shows_inline_feedback` | `"do it differently"` painted. Same `cursor_pos: None` on the reject-feedback slot. |

Tests were not changed.

## Product change

`crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs`

- `PromptRenderResult` now has `caret_cell` (insertion cell while focused) separate from `cursor_pos` (hardware cursor; still `None` when the box caret paints).
- Box caret is skipped when a ghost suffix owns the insertion cell, and ghosts paint *after* the caret so dim italic wins.
- Chromeless, prefix-less, default-surface draws stay a raw textarea (no blinking `█` after the draft). Composer / dispatch / peek (`chrome`, prefix, or `PromptBg::Canvas`/`Panel`) still paint the Human-green box caret. Caret color is still `accent_user` (green), not magenta.

`crates/codegen/xai-grok-pager/src/views/dashboard/peek.rs`

- Focused reply and reject-feedback report `caret_cell` (fallback `cursor_pos`). Unfocused still returns no caret.

## Green re-run

```
cargo fmt -p xai-grok-pager                          # FMT_EXIT:0
cargo clippy -p xai-grok-pager --lib -- -D warnings  # CLIPPY_EXIT:0
cargo test -p xai-grok-pager --lib -- \
  ghost_text_empty_string_not_rendered \
  ghost_text_renders_at_cursor_when_at_end \
  ghost_text_suppressed_when_slash_active \
  ghost_text_truncated_to_available_width \
  render_peek_reject_option_shows_inline_feedback \
  render_peek_shows_typed_reply_and_caret
# 6 passed

# Nearby (not required): 39 ghost_text_ / render_peek_ / box-caret tests passed
```

## Leftovers

- Dashboard `render.rs` still parks the hardware cursor on `PeekRenderResult.caret`. Focused peek therefore reports a cell (tests / mouse) and may also Show the terminal cursor on top of the software box caret. Out of this slice (`dashboard/render.rs`).
- Other CI-20 clusters (settings e2e, credit bar, limits, router, session, turn status, agent config, models) untouched.
