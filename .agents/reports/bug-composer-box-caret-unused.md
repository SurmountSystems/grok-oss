# Full TUI prompt widget: Human-green box caret restored

**Date:** 2026-08-13  
**Tree:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:composer-box-caret`

SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

## Problem

After the 1.0.3 restack, `cursor_box_filled` / `cursor_box_hollow` lived only in
pager-render. The full prompt widget did not paint them. Catalog
`paint_composer_box_cursor_*` names were gone. Composer used the terminal
hardware caret (`cursor_pos = Some`) instead of the Human-green box.

Minimal overlay may keep a generic reverse-cell caret. This slice is the
**full** prompt widget only.

## Contract

| Cell / phase | Paint |
|--------------|--------|
| End-of-buffer blank, solid | `cursor_box_filled` (`█`), `fg=bg=accent_user` |
| End-of-buffer blank, empty | `cursor_box_hollow` (space), `fg=bg=canvas` |
| Mid-draft letter, solid | Keep letter; reverse plate (`fg=canvas`, `bg=accent_user`) |
| Mid-draft letter, empty | Keep letter; `fg=text_primary`, `bg=canvas` (not neon green ink) |
| Mid-draft space, solid | Keep ` `; reverse plate; never solid `█` |
| Colour | `accent_user` only. Never `accent_running` magenta. |
| Hardware cursor | Hidden while the box caret paints (`draw` returns `cursor_pos: None`) |

## Red (before product edit)

Restored named paint tests first. Ran:

```text
cargo test -p xai-grok-pager --lib -- \
  paint_composer_box_cursor_grapheme_phases_keep_letter \
  paint_composer_box_cursor_uses_human_green \
  left_through_letters_empty_phase_not_neon \
  paint_composer_box_cursor_blank \
  mid_buffer_space_caret \
  caret_move_clears \
  focused_composer_paints_human_green_box_caret
```

**Fail:** `error[E0425]: cannot find function paint_composer_box_cursor_phase in module super` (15 errors).

## Product fix

`crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs`:

1. Restored `paint_composer_box_cursor` (wall-clock phase) and
   `paint_composer_box_cursor_phase` (injectable). Uses existing
   `crate::glyphs::cursor_box_*`.
2. `draw` wipes the textarea rect (`cell.reset` + `text_primary` on canvas)
   before `TextArea` paint so a moved caret cannot leave a leftover `█` or
   green plate.
3. After ghosts, paint the box caret. Solid `█` only when the cursor is at
   buffer end. Return `cursor_pos: None` so the terminal caret is not shown
   on top.

`tests.rs`: restored the origin/main named paint tests.

Did not touch settings, welcome, `actions.rs`, status-row chips, or spend.

## Green

```text
cargo test -p xai-grok-pager --lib -- \
  paint_composer_box_cursor_grapheme_phases_keep_letter \
  paint_composer_box_cursor_uses_human_green \
  left_through_letters_empty_phase_not_neon \
  paint_composer_box_cursor_blank \
  mid_buffer_space_caret \
  caret_move_clears \
  focused_composer_paints_human_green_box_caret \
  left_arrow_with_chrome_prefix \
  left_arrow_does_not_insert_prompt_prefix \
  ctrl_home_end_page_move \
  paint_composer_box_cursor_phase_only_styles
# ok: 12 passed
```

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager` | ok |
| Named tests (same filters as red) | 12 passed |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | Fails on **pre-existing** unused constants in `settings/defs.rs` (`CANCEL_SUBAGENTS_ON_TURN_CANCEL_CHOICES`, `AUTO_COMPACT_THRESHOLD_*`). Out of scope. Prompt-widget rustc in that same pass had no errors. |

## Dogfood

Rebuild and relaunch the pager you actually run. Type letters, arrow Left:
solid half is a Human-green reverse plate; empty half leaves the letter
normal text colour. Mid-draft spaces do not become `█`. At buffer end the
green block blinks on/off. Agent chrome stays magenta.
