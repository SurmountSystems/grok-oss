# Red diagnosis: pager width / selection / render cluster

Diagnosis only. No product edit in this turn.

These four tests are one cluster. They are not four unrelated bugs. They share one product root with two branches.

## Commands and env

```
cd /home/hunter/Projects/surmount/grok-build
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- \
  table_copy_uses_width_snapshot_when_anchor_block_scrolled_out \
  message_block_content_width_subtracts_timestamp_reservation \
  overlay_pretty_link_url_with_cjk_text \
  test_selection_model_top_clipped_markdown_entry
```

Result: **4 failed, 0 passed**, 8889 filtered out, exit 101.

Contrast (same env) that stayed green:

- `overlay_pretty_link_url_wraps_across_rows`
- `overlay_pretty_link_url_wraps_multi_row_paragraph`
- `overlay_pretty_link_url_no_wrap_single_row`
- `overlay_pretty_two_wrapping_links_distinct_ids`
- `overlay_pretty_link_url_in_blockquote_wraps_correctly`
- `overlay_pretty_link_url_in_list_wraps_correctly`
- `table_rows_fill_content_width_with_emoji`
- `detects_from_content_and_border_lines` (clean string fixture, no bubble copy)
- `reconstruct_drag_copy_uses_width_snapshot_when_anchor_block_scrolled_out` (linear, no table detect)
- `xai-grok-markdown` `cjk_link_uses_display_width_for_column_range`
- `xai-grok-markdown` `pretty_inline_link_column_range_excludes_brackets`

Timestamp reservation itself is not the failing step. The timestamp test's first assert (`block.content_width == pane_content_width - 10`) did not fire. Markdown CJK column ranges are not the failing step.

## Per-test red

### 1. `app::agent_view::selection::tests::table_copy_uses_width_snapshot_when_anchor_block_scrolled_out`

- File: `crates/codegen/xai-grok-pager/src/app/agent_view/selection.rs:2204`
- Panic: `markdown table renders a detectable grid`
- What failed: `TableGeometry::detect` returned `None` for every probe line `0..6` on `with_entry_output_text_source(0, 0, Some(40), ...)`.
- The later copy asserts never ran.

### 2. `scrollback::render::tests::test_selection_model_top_clipped_markdown_entry`

- File: `crates/codegen/xai-grok-pager/src/scrollback/render_tests.rs:1333`
- `assert_eq!(range.lines[0].screen_y, 0)`
- pretty-assertions: left `1`, right `0`
- Actual first selectable model line is at screen row 1. Expected 0 after `scroll_offset = 1`.

### 3. `scrollback::render::tests::overlay_pretty_link_url_with_cjk_text`

- File: `crates/codegen/xai-grok-pager/src/scrollback/render_tests.rs:3314`
- `assert_eq!(combined_width as usize, UnicodeWidthStr::width(url))` with message `combined fragment widths must equal URL display width`
- pretty-assertions: left `67`, right `68`
- The URL is 68 ASCII cells. Overlay fragments for that URL cover 67. `group.len() >= 2` and consecutive-row checks passed.

### 4. `scrollback::render::tests::message_block_content_width_subtracts_timestamp_reservation`

- File: `crates/codegen/xai-grok-pager/src/scrollback/render_tests.rs:1415`
- `assert_eq!(entry_lines_narrow, model_lines)` with message `model lines must match a re-derivation at the per-block content_width`
- pretty-assertions: left `6`, right `5`
- `effective_output(...).output().lines.len()` is 6. Selectable model lines are 5.
- The width-reservation assert passed. The `assert_ne!(entry_lines_wide, model_lines)` assert never ran.

## Named contracts (plain English)

1. **Table snapshot copy.** If you start a table-cell drag, then the anchor block scrolls fully out of `visible_blocks`, copy still works from the drag-start width snapshot. Detection must still see the box-drawing grid in the entry's full output at that width. Without the snapshot, copy fails.
2. **Top-clipped markdown selection.** After scrolling one row off a wrapping assistant message, the first selectable line of that entry is at the top of the viewport (`screen_y == 0`). Clip is content, not a hole of non-selectable chrome.
3. **CJK pretty-link overlay.** For `[日本語のリンク テキスト](long-url)`, OSC 8 fragments on the wrapped URL must cover every display cell of the URL. Combined fragment widths equal the URL display width. Markdown already does this in `xai-grok-markdown` (those tests passed).
4. **Timestamp reservation width.** Assistant / user / btw messages wrap at `pane content width - 10` when timestamps are on. `VisibleBlockGeometry.content_width` is that reduced width. Re-deriving `effective_output` at that width must produce the same line count the selection model used, so `block_line_idx` on copy stays aligned. Passing the wider pane width must wrap differently.

## One root, two branches

**One product root:** always-on bubble copy mutates assistant `BlockOutput` after markdown wrap.

Evidence:

- `AppearanceConfig` / `ScrollbackDisplayConfig` default `bubble_copy_buttons: true`.
- `AgentMessageBlock::output` always calls `append_bubble_copy_button` (`scrollback/blocks/agent.rs`).
- All four failing tests build `RenderBlock::agent_message` / `make_markdown_entry`.
- `append_bubble_copy_button` (`scrollback/blocks/mod.rs`) has two branches:
  1. **Append** when `used + 1 + copy_icon() <= content_width`: push `" "` + `⧉` onto line 0, set `copy_button_col`, shrink `Selectable` so the glyph is not copied.
  2. **Insert** when the first line is already full: `lines.insert(1, icon_line)` with `selection_range: None`, `Selectable::None`, `joiner: Some("")`.
- `MarkdownContent::ensure_wrapped` calls `set_max_table_width(Some(width))`. Table rows fill the content width (`table_rows_fill_content_width_with_emoji` is green). At snapshot width 40 the first table line is full, so the **insert** branch runs.
- The timestamp test's 6 vs 5 is exactly one extra output line that the selection model skips (`selection_range` is `None`, so `push_line` never runs).
- The top-clip test uses viewport width 20, chrome 4, timestamp reserve 10, so per-block width is 6. The first wrap line fills that width. Insert puts a non-selectable icon line at index 1. Scroll 1 skips the first text line. The icon occupies screen row 0 and is not in the model. The next text line is the first model line at `screen_y == 1`. That is the red `1` vs `0`.
- Table detect reads lines through `with_entry_output_text_source`, which returns `None` when `selection_range != Some(range_id)`. The inserted icon line is a hole. `TableGeometry::detect` treats `None` as a hard boundary (`text_at(line)?`). Walking the grid dies. Every probe `0..6` fails. The unit test `detects_from_content_and_border_lines` stays green because it feeds clean strings with no icon line.
- CJK overlay is the **append** branch. Label display width is 23. Viewport 40 → content width 26. `23 + 1 + 1 <= 26`, so the helper appends space + icon on line 0. `map_hyperlinks_to_overlay` uses `line.content.width()`, which now includes those two extra cells. Pretty wrap is `label (url)`: first line is the CJK label, joiner is the space, URL starts on the next line. Inflating line 0 by 2 leaves the first URL cell (column 25 of 0..68) in the gap between the fake line-0 end and the shifted line-1 start. Combined width 67. I did not dump live overlay fragments. The 67 vs 68 number matches that arithmetic. Sibling ASCII overlay tests stay green because their first lines fill the wrap width (insert, or a shift that still conserves total URL cells). Markdown's own CJK column test is green, so this is not a byte-vs-cell parser bug.

`effective_output` and the render cache share one `RefCell` on the same `ScrollbackEntry`. A stale cache at a different wrap width cannot explain 6 vs 5. After render, a re-derive at `block.content_width` hits the same cache. The extra line is in `output().lines` and missing from the selectable model.

FORK names the competing contract: always-on bubble copy is **paint plus click**. Paint-only is a failed land. A full-width first line must still paint a hit. Do not turn `bubble_copy_buttons` off in these four tests. Do not delete wrap-to-next-line paint.

## Smallest intended product fix

Do not rewrite the four tests. Do not hide the glyph. Do not put bubble-copy chrome into content geometry.

Keep paint plus click. Stop letting the glyph change wrap columns, table detect, or selectable line identity.

Preferred smallest shape:

1. **`append_bubble_copy_button` records a hit column. It does not change content geometry.** Do not append spans that `Line::width()` / `map_hyperlinks_to_overlay` will count as pre-wrap columns. Do not insert a `BlockLine` into `output().lines`. Set `copy_button_col` (on line 0, including a column in the right pad or the 10-column timestamp gutter when the first content line is full).
2. **`EntryRenderer` paints `⧉` at that column** into pad / timestamp gutter when there is no slack in the content wrap. That gutter is already reserved and is not part of the wrapped markdown. This is not "overlay on the last content character" (that would hide a glyph). Click stays live through `copy_button_col` / `bubble_copy_button_rect` / existing mouse hit tests.
3. **If any chrome line must remain in `BlockOutput`**, then every consumer must skip it:
   - `map_hyperlinks_to_overlay`: ignore `copy_button_col` lines; measure selectable width only.
   - `with_entry_output_text_source` / `TableGeometry::detect`: skip a bubble-copy-only hole instead of ending the grid.
   - Selection mapping already skips `selection_range == None`. The timestamp test still compares **all** `output().lines` to **selectable** model lines, so an extra output line keeps that test red. That is why the extra line must not live in `output().lines`.

Do not satisfy the timestamp proxy by making the icon line selectable. That would copy chrome and still leave table detect and CJK mapping broken.

Keep these green while fixing: `append_bubble_copy_button_paints_when_first_line_fills_content_width`, `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width`, `clicking_wide_human_bubble_copy_still_paints_and_copies`, `clicking_assistant_bubble_copy_copies_the_message`.

## Files and functions for the implementer

| Path | What to touch |
|------|----------------|
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` | `append_bubble_copy_button` |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/agent.rs` | `AgentMessageBlock::output` (call site) |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` | `UserPromptBlock::output` (same helper; human full-width click tests) |
| `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs` | Paint `⧉` in pad / timestamp gutter when content slack is gone |
| `crates/codegen/xai-grok-pager/src/scrollback/types.rs` | `BlockLine::copy_button_col`, `bubble_copy_button_rect` if the overflow column lives past content width |
| `crates/codegen/xai-grok-pager/src/scrollback/render.rs` | `map_hyperlinks_to_overlay` only if content width is still inflated |
| `crates/codegen/xai-grok-pager/src/app/agent_view/selection.rs` | `with_entry_output_text_source` only if detect still sees a hole |
| `crates/codegen/xai-grok-pager/src/scrollback/table_geometry.rs` | `TableGeometry::detect` only if a remaining chrome line sits inside the grid |

Timestamp reservation in `timestamp_reserved_for_block` / `EntryRenderer::timestamp_reserved` does not need a behavior change for this cluster.

## Stop

Red is observed. Product fix is not done in this turn.
