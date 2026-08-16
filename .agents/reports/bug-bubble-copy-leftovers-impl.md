# Implementer report: leftover bubble-copy contracts

Workspace: `/home/hunter/Projects/surmount/grok-build`. L3 implementer.

Named leftovers: a full-width first line must still paint a clickable always-on
bubble copy glyph; assistant bubbles need a dedicated click test; the upstream
regression catalog must enroll click (not paint-only).

## Wide-line: wrap, not right-align

When the first line already fills `ctx.content_width()`,
`append_bubble_copy_button` used to return without painting `⧉`. That omit is
no longer product behavior.

The fix is **wrap**. If space plus `copy_icon()` does not fit on the first
line, the helper inserts a following line that carries the same dim `⧉`,
`copy_button_col = 1`, and the first line's background band. Hit publishing
already walks every line, so that new row still publishes a click rect.

Right-align (overlay on the last column of the first line) was not used. That
would hide the last character of the prompt, or get clipped by
`set_line_safe`. Wrap keeps every prompt character and still paints a
clickable glyph.

Signature change: `append_bubble_copy_button` now takes `&mut Vec<BlockLine>`
so it can insert that wrap line. Call sites already passed a `Vec`.

## Wide-line TDD

The first draft of the wide tests used `WIDE-FIRST-LINE-COPY-CONTRACT-` plus
`W`s. Word wrap broke at the hyphens, so the first line was short and the
icon still fit. That was a false green. The tests were tightened to one
unbreakable word (`WIDEFIRSTLINECOPYCONTRACT` plus `W`s) and a helper
precondition that `used + 1 + icon.width() > content_width()`. Those
strengthened tests were run **before** the product edit.

**Red (before the wrap product edit):**

| Test | Fail reason |
|------|-------------|
| `scrollback::blocks::tests::append_bubble_copy_button_paints_when_first_line_fills_content_width` | `a full-width first line must still paint the copy icon and mark a hit column` |
| `scrollback::blocks::user::tests::bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | output had no `copy_icon()` |
| `app::mouse::tests::clicking_wide_human_bubble_copy_still_paints_and_copies` | `a full-width first line must still paint the bubble copy icon when bubble_copy_buttons is on` |

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- clicking_wide_human_bubble_copy_still_paints_and_copies bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width append_bubble_copy_button_paints_when_first_line_fills_content_width clicking_assistant_bubble_copy_copies_the_message -- --nocapture
```

Exit **101**. Three wide-line tests failed. The assistant click test passed on
that same run (see below).

**Green (same filters after wrap):**

Same command plus the existing human click and flag-on/flag-off paint tests:
**7 passed**, then **8** with `block_line_exhaustive_literal_keeps_legacy_shape`.
The named wide-line tests were not rewritten to finish green.

```
test scrollback::blocks::tests::append_bubble_copy_button_paints_when_first_line_fills_content_width ... ok
test scrollback::blocks::user::tests::bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width ... ok
test app::mouse::tests::clicking_wide_human_bubble_copy_still_paints_and_copies ... ok
```

The click test finds `copy_icon()` in the scrollback pane, left-clicks that
cell, and asserts `Action::CopyBlockContent`, that the human entry is
selected, that `copy_text` equals the original unbreakable prompt, and that
the payload does not contain `⧉`.

## Assistant click test

**Test:** `app::mouse::tests::clicking_assistant_bubble_copy_copies_the_message`

Same `draw_agent_frame` / `find_copy_icon` helpers and the same
`hit_bubble_copy` mouse branch as the human click test. Pushes
`RenderBlock::agent_message("COPY-ASSISTANT-MSG-CONTRACT")`, clicks the
painted `⧉`, and asserts `Action::CopyBlockContent`, that the selected entry
is the assistant block, that `copy_text` contains that message, and that the
payload does not contain `⧉`.

**Already green on first run.** The human click wire already covers assistant
bubbles (same helper, same hit list). No product edit was needed for this
path. This is not a fake red. The human test was not weakened.

## Catalog: exact filter lines added

File: `doc/dev/upstream-regression-filters.md`

Land class 2 table (after the paint-only row):

```
| `xai-grok-pager` `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | A full-width first line still paints the always-on copy glyph |
| `xai-grok-pager` `append_bubble_copy_button_paints_when_first_line_fills_content_width` | The paint helper still marks a hit column when the first line fills the width |
| `xai-grok-pager` `clicking_human_bubble_copy_copies_the_prompt` | Clicking the always-on human bubble copy glyph copies that prompt. Paint-only chrome is a failed land |
| `xai-grok-pager` `clicking_assistant_bubble_copy_copies_the_message` | Clicking the always-on assistant bubble copy glyph copies that message |
| `xai-grok-pager` `clicking_wide_human_bubble_copy_still_paints_and_copies` | A full-width first line still paints a clickable copy glyph |
```

Class 2 and operator cheat-sheet cargo filters gained:

```
  append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
```

The "Always-on bubble copy / one-click copy" section now says a restack that
keeps the paint-only test and drops click-to-copy is a failed land, and runs:

```
cargo test -p xai-grok-pager --lib -- bubble_copy_ append_bubble_copy_button_paints \
  clicking_human_bubble_copy clicking_assistant_bubble_copy clicking_wide_human_bubble_copy
```

The combined DOGE / bubble / clear-done filter line gained the same click
names.

## Files changed

- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` (wrap the
  glyph onto the following line; helper unit test)
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` (wide-line
  paint test)
- `crates/codegen/xai-grok-pager/src/app/mouse.rs` (wide-line click test;
  assistant click test)
- `doc/dev/upstream-regression-filters.md` (enroll click and wide-line
  contracts)

No user-guide edit. No settings-registry change. Human chrome stays green.

## Leftovers (operator-gated only)

- Live TUI may still be an old `1.0.3` binary. Rebuild and fully quit and
  reopen. This implementer did not rebuild or restart the running process.
- Unit tests assert the action and `copy_text` payload. They do not talk to a
  host clipboard daemon.

## fmt / clippy / test exit codes

All cargo commands used
`CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target` and
`TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

| Step | Command | Exit |
|------|---------|------|
| wide-line red | `cargo test -p xai-grok-pager --lib --` the three wide tests plus assistant | **101** (3 failed, assistant already ok) |
| wide-line green | same plus existing human click and paint tests | **0** (7 passed) |
| fmt | `cargo fmt -p xai-grok-pager` | **0** |
| clippy | `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **0** |
| targeted tests after fmt | same green filters plus `block_line_exhaustive_literal_keeps_legacy_shape` | **0** (8 passed) |

Stop. No git add, commit, or push. No parent L2 report.
