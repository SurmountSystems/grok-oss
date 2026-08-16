# Green report: pager width / selection / render cluster

Always-on bubble copy is still paint plus click. Wrap columns, table detect, and selectable line identity no longer change when the glyph is on.

## Red (before this product change)

Same env as the diagnosis report (`.agents/reports/bug-pager-selection-render-red.md`).

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- \
  table_copy_uses_width_snapshot_when_anchor_block_scrolled_out \
  message_block_content_width_subtracts_timestamp_reservation \
  overlay_pretty_link_url_with_cjk_text \
  test_selection_model_top_clipped_markdown_entry
```

Result: **4 failed, 0 passed**, exit 101.

| Test | Fail |
|------|------|
| `table_copy_uses_width_snapshot_when_anchor_block_scrolled_out` | `markdown table renders a detectable grid` (`TableGeometry::detect` `None` on every probe) |
| `test_selection_model_top_clipped_markdown_entry` | `screen_y` 1 vs 0 |
| `overlay_pretty_link_url_with_cjk_text` | combined overlay width 67 vs URL display width 68 |
| `message_block_content_width_subtracts_timestamp_reservation` | `effective_output` line count 6 vs selectable model 5 |

Root: `append_bubble_copy_button` either appended space plus `⧉` onto line 0 (inflated `Line::width`) or inserted a non-selectable chrome `BlockLine`.

## Product change

`append_bubble_copy_button` only records `copy_button_col` on the first line. It does not append spans and does not insert a `BlockLine`.

- Slack in the wrap: hit column is `used + 1` (one cell after the last wrap glyph).
- Slack gone: hit column is `ctx.content_width()`, the first timestamp-gutter or right-pad cell. The last wrap cell is not overwritten.

`BlockLine::paint_bubble_copy_button` paints `⧉` at that column after content and the timestamp overlay. Call sites:

- `EntryRenderer::render` (`wrappers/entry_renderer.rs`)
- sticky-header path (`scrollback_pane.rs`)

Hit-testing still uses `bubble_copy_button_rect` / `copy_button_col`. The four cluster tests were not rewritten and did not turn `bubble_copy_buttons` off.

Files:

- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/types.rs` (`paint_bubble_copy_button`)
- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` (helper tests only; call site unchanged)

## Green

Same env. Cluster of four:

```
cargo test -p xai-grok-pager --lib -- \
  table_copy_uses_width_snapshot_when_anchor_block_scrolled_out \
  message_block_content_width_subtracts_timestamp_reservation \
  overlay_pretty_link_url_with_cjk_text \
  test_selection_model_top_clipped_markdown_entry
```

**4 passed**, 0 failed.

Keep-green paint plus click:

```
cargo test -p xai-grok-pager --lib -- \
  append_bubble_copy_button_paints_when_first_line_fills_content_width \
  bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width \
  clicking_wide_human_bubble_copy_still_paints_and_copies \
  clicking_assistant_bubble_copy_copies_the_message \
  clicking_human_bubble_copy_copies_the_prompt \
  bubble_copy_buttons_on_paints_copy_icon
```

**6 passed**, 0 failed.

## fmt / clippy

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --all-targets -- -D warnings
```

`cargo fmt -p xai-grok-pager`: exit 0.
`cargo clippy -p xai-grok-pager --all-targets -- -D warnings`: exit 0.

## New fork seam

**No.** This is a fix inside the existing always-on bubble copy seam (paint plus click). It does not add a new product class.
