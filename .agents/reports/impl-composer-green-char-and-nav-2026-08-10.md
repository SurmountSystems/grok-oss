# Composer green char on Left + Ctrl-Home/End/PgUp/PgDn

**Board:** `bug:composer-green-char-on-arrow`, `feat:composer-ctrl-home-end-nav`
**Date:** 2026-08-10
**Tree:** `/home/hunter/Projects/surmount/grok-build`

## Operator dogfood

Multi-line green prompt composer. Three issues:

1. Green character when arrowing left through draft text.
2. Ctrl-Home / Ctrl-PgUp do not go to the beginning of the composer buffer.
3. Need reverse: Ctrl-End / Ctrl-PgDn to the end of the buffer.

## Root cause

### Green character on Left

Software Human-green box caret treated **every** space cell as an "insertion blank" and replaced it with solid `█` (full-cell green block) on the solid blink half.

When the caret sat on a **typed mid-buffer space** (or after wipe left a space cell mid-line), Left-arrow navigation painted a green block glyph into the line. That read as a stray green character (control glyph / stuck caret), not as reverse-plate styling on real draft text.

Prefix `❯` / `>` is chrome only and was never in the edit buffer; residue wipe for prior caret cells already existed. The mid-buffer solid-`█`-on-space path was the remaining dogfood gap.

**Fix:** only allow the solid block glyph when the cursor is at **buffer end** (`cursor == text.len()`). Mid-buffer spaces (and other graphemes) keep their symbol and use reverse-plate / accent-ink styling.

### Ctrl-Home / End / PgUp / PgDn

`TextArea::input` matched bare `Home` / `End` with `..` modifiers, so **Ctrl+Home** and **Ctrl+End** only did visual-row start/end (same as bare Home/End). Ctrl+PageUp / Ctrl+PageDown were unhandled in the textarea.

**Fix:** before the bare Home/End arms:

| Chord | Action |
|-------|--------|
| Ctrl+Home, Ctrl+PageUp | `set_cursor(0)` (whole buffer start) |
| Ctrl+End, Ctrl+PageDown | `set_cursor(text.len())` (whole buffer end) |
| bare Home / End | still visual-row local (soft wrap) |
| Ctrl+A / Ctrl+E | still logical line (unchanged) |

Bare PageUp/PageDown still scroll scrollback when prompt paging is on (registry exact-match on no modifiers).

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-ratatui-textarea/src/textarea.rs` | Ctrl+Home/End/PageUp/PageDown buffer-end navigation |
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs` | `allow_block_glyph` only at buffer end; pass through paint path |
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/tests.rs` | Red/green contracts for prefix purity, mid-space no `█`, chrome Left, Ctrl nav |

## Tests (red → green)

Named contracts:

| Test | Package | Contract |
|------|---------|----------|
| `ctrl_home_end_page_move_to_buffer_ends` | `xai-ratatui-textarea` | Ctrl chords → buffer ends; bare Home/End line-local |
| `ctrl_home_end_page_move_prompt_cursor_to_buffer_ends` | `xai-grok-pager` | Same via `PromptWidget::handle_key` |
| `left_arrow_does_not_insert_prompt_prefix_into_buffer` | `xai-grok-pager` | Left never inserts `❯` / `>` into draft |
| `mid_buffer_space_caret_does_not_paint_solid_block_glyph` | `xai-grok-pager` | Mid-buffer space keeps ` `, never solid `█` |
| `left_arrow_with_chrome_prefix_clears_caret_residue` | `xai-grok-pager` | Chrome+prefix Left leaves no solid `█` in textarea body |
| Existing `caret_move_clears_*` / `paint_composer_box_cursor_*` | `xai-grok-pager` | Still green after API change |

### Commands (proof)

```text
CARGO_TARGET_DIR=/tmp/grok-pager-tdd cargo test -p xai-ratatui-textarea --lib -- \
  ctrl_home_end_page_move_to_buffer_ends
# ok (1 passed)

CARGO_TARGET_DIR=/tmp/grok-pager-tdd cargo test -p xai-grok-pager --lib -- \
  left_arrow_does_not_insert_prompt_prefix mid_buffer_space_caret \
  left_arrow_with_chrome_prefix ctrl_home_end_page_move_prompt \
  caret_move_clears paint_composer_box_cursor
# ok (10 passed)
```

### Post-impl verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager -p xai-ratatui-textarea` | ok |
| `cargo clippy -p xai-ratatui-textarea --lib -- -D warnings` | ok |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | ok |
| Targeted tests above | ok |

## Dogfood steps

1. Rebuild and relaunch the pager binary you dogfood.
2. Type a multi-line draft with spaces. Arrow left through it: no green solid block stuck mid-line; caret reverse-plates letters/spaces only on the current cell; prior cells stay normal text.
3. With cursor mid-draft: Ctrl+Home and Ctrl+PageUp → start of entire buffer; Ctrl+End and Ctrl+PageDown → end of entire buffer.
4. Bare Home / End still move within the current visual row (wrapped line).
5. Prefix `❯` remains chrome on the left and never becomes editable buffer text.
