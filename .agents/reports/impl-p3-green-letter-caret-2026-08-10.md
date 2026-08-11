# P3: mid-buffer letter under caret — no neon green ink on empty half

**Board:** `impl:p3-green-letter-caret`
**Date:** 2026-08-10
**Tree:** `/home/hunter/Projects/surmount/grok-build`

## Problem

Mid-buffer letter under the software caret painted **neon green ink** on the
empty blink half (`fg=accent_user` on the grapheme). A `T` under the caret
read as a solid green **T** (second green prompt glyph). Prior fix only blocked
solid `█` on mid-buffer **spaces**; tests still **required** green ink on letters.

## Contract (named)

| Phase / cell | Behaviour |
|--------------|-----------|
| Empty half, mid-buffer **letter** | Normal text ink (`text_primary`), canvas bg — **not** `accent_user` on the letter |
| Solid half, mid-buffer letter | Reverse plate: Human green bg, readable canvas ink (not neon letter) |
| End-of-buffer blank | Solid `█` / hollow space (unchanged) |
| Mid-buffer space | Keep ` `, reverse plate on solid; never solid `█` (unchanged) |
| Ctrl-Home/End/Pg* | Unchanged |

## Red (observed)

Rewrote/extended tests first, then ran before paint change:

```text
CARGO_TARGET_DIR=/tmp/grok-pager-tdd cargo test -p xai-grok-pager --lib -- \
  paint_composer_box_cursor_grapheme_phases_keep_letter \
  left_through_letters_empty_phase_not_neon \
  paint_composer_box_cursor_uses_human_green
# FAILED: 0 passed; 3 failed
```

Fail reasons (product still painted `fg=Rgb(0,255,0)` on empty half):

- `paint_composer_box_cursor_grapheme_phases_keep_letter` — empty half must NOT be accent green
- `left_through_letters_empty_phase_not_neon_green_letter` — empty half on `'T'` was neon green
- `paint_composer_box_cursor_uses_human_green_not_agent_magenta` — empty grapheme must not use Human green ink

## Green (minimal product fix)

`paint_composer_box_cursor_phase` empty grapheme branch:

```text
// was:  Style::default().fg(accent).bg(bg)
// now:  Style::default().fg(theme.text_primary).bg(bg)
```

Solid reverse plate unchanged (`fg=canvas`, `bg=accent`). Blank insertion path
unchanged.

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs` | Empty-half grapheme → `text_primary`; doc comments |
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/tests.rs` | New empty-half contract; Left-through-letters full-draw; residue test forces solid phase |
| `crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md` | Caret contract accurate for forks |
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | Composer nav note: no neon letter ink |
| `FORK.md` | Composer caret bullet + report link |

## Green proof

```text
CARGO_TARGET_DIR=/tmp/grok-pager-tdd cargo test -p xai-grok-pager --lib -- \
  paint_composer_box_cursor_grapheme_phases_keep_letter \
  left_through_letters_empty_phase_not_neon \
  paint_composer_box_cursor_uses_human_green \
  mid_buffer_space_caret \
  left_arrow_with_chrome_prefix \
  caret_move_clears \
  paint_composer_box_cursor_blank \
  left_arrow_does_not_insert_prompt_prefix
# ok (9 passed)
```

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager` | ok |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | ok |

## Dogfood

1. Rebuild and relaunch the pager binary you dogfood.
2. Type letters (e.g. `TEST`), arrow Left through them: caret reverse-plates on
   solid blink half; empty half leaves the letter normal white/text colour —
   **not** a solid neon green T.
3. Mid-draft spaces still reverse-plate without solid `█`.
4. At buffer end: solid green `█` still blinks on/off.
5. Ctrl+Home / End / PageUp / PageDown still jump whole-buffer ends.
