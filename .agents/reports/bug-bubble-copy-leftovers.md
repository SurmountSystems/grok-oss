# Bubble-copy leftovers: wide first line, assistant click, catalog

Workspace: `/home/hunter/Projects/surmount/grok-build`. L2 closeout from L3 implementer and process-mop reports. Not a re-walk of the pager.

Sources:
- `.agents/reports/bug-copy-human-message.md` (and impl / explore / mop)
- `.agents/reports/bug-bubble-copy-leftovers-map.md`
- `.agents/reports/bug-bubble-copy-leftovers-impl.md`
- `.agents/reports/bug-bubble-copy-leftovers-mop.md`

## Wide-line

**What landed:** wrap, not right-align.

When `used + 1 + icon.width()` would exceed `ctx.content_width()`, `append_bubble_copy_button` used to drop `⧉`. That omit is gone. The helper now inserts a following line that carries the same dim `⧉`, `copy_button_col = 1`, and the first line's background band. Hit publishing already walks every line, so that wrap row still publishes a click rect.

Right-align was not used. Overlaying the last column of a full first line would hide the last prompt character or get clipped. Wrap keeps every prompt character and still paints a clickable glyph.

The helper now takes `&mut Vec<BlockLine>` so it can insert that wrap line. Call sites already passed a `Vec`.

**Red (before the wrap product edit):**

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib -- clicking_wide_human_bubble_copy_still_paints_and_copies bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width append_bubble_copy_button_paints_when_first_line_fills_content_width clicking_assistant_bubble_copy_copies_the_message -- --nocapture
```

Exit **101**. Three wide-line tests failed:

| Test | Fail reason |
|------|-------------|
| `scrollback::blocks::tests::append_bubble_copy_button_paints_when_first_line_fills_content_width` | a full-width first line must still paint the copy icon and mark a hit column |
| `scrollback::blocks::user::tests::bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | output had no `copy_icon()` |
| `app::mouse::tests::clicking_wide_human_bubble_copy_still_paints_and_copies` | a full-width first line must still paint the bubble copy icon when bubble_copy_buttons is on |

A first draft of the fixtures used hyphenated words, so word wrap made the first line short and the icon still fit. That was a false green. The fixtures were tightened to one unbreakable word plus a precondition that `used + 1 + icon.width() > content_width()`, and those stronger tests were run before the product edit.

**Green (same filters after wrap):** exit **0**. Named wide-line tests were not rewritten to finish green. The click test finds `copy_icon()` in the scrollback pane, left-clicks that cell, and asserts `Action::CopyBlockContent`, that the human entry is selected, that `copy_text` equals the original unbreakable prompt, and that the payload does not contain `⧉`.

## Assistant click test

**Name:** `app::mouse::tests::clicking_assistant_bubble_copy_copies_the_message`

Same `draw_agent_frame` / `find_copy_icon` helpers and the same `hit_bubble_copy` mouse branch as the human click test. Clicks the always-on bubble `⧉` on an assistant message, asserts `Action::CopyBlockContent`, that the selected entry is the assistant block, that `copy_text` contains `COPY-ASSISTANT-MSG-CONTRACT`, and that the payload does not contain `⧉`.

**Already green on first run.** The human click wire already covers assistant bubbles (same helper, same hit list). No product edit was needed for this path. This is not a fake red. The human test was not weakened.

## Catalog

File: `doc/dev/upstream-regression-filters.md`

Land class 2 table gained these filter rows (plain-English land sentences so a restack cannot drop click-to-copy and still pass a chrome-only paint test):

| Filter | Land sentence |
|--------|----------------|
| `xai-grok-pager` `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | A full-width first line still paints the always-on copy glyph |
| `xai-grok-pager` `append_bubble_copy_button_paints_when_first_line_fills_content_width` | The paint helper still marks a hit column when the first line fills the width |
| `xai-grok-pager` `clicking_human_bubble_copy_copies_the_prompt` | Clicking the always-on human bubble copy glyph copies that prompt. Paint-only chrome is a failed land |
| `xai-grok-pager` `clicking_assistant_bubble_copy_copies_the_message` | Clicking the always-on assistant bubble copy glyph copies that message |
| `xai-grok-pager` `clicking_wide_human_bubble_copy_still_paints_and_copies` | A full-width first line still paints a clickable copy glyph |

Class 2 / cheat-sheet cargo filters also gained:

```
append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy clicking_wide_human_bubble_copy
```

The always-on bubble copy section now says a restack that keeps the paint-only test and drops click-to-copy is a failed land.

## Files changed

- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` (wrap the glyph onto the following line; helper unit test)
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` (wide-line paint test)
- `crates/codegen/xai-grok-pager/src/app/mouse.rs` (wide-line click test; assistant click test)
- `doc/dev/upstream-regression-filters.md` (enroll click and wide-line contracts)

No user-guide edit. No settings-registry change. Human chrome stays green. Hover / OSC 22 was already landed and was not touched.

## Leftovers that are truly operator-gated only

- Live TUI may still be an old `1.0.3` binary. Rebuild and fully quit and reopen. Agents did not rebuild or restart the running process.
- Unit tests assert the action and `copy_text` payload. They do not talk to a host clipboard daemon.

No other leftover from this slice is parked.

## fmt / clippy / test exit codes

All cargo used `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target` and `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

**Implementer (first):**

| Step | Exit |
|------|------|
| wide-line red (three wide tests plus assistant) | **101** (3 failed; assistant already ok) |
| wide-line green (same plus existing human click and paint tests) | **0** (7 passed) |
| `cargo fmt -p xai-grok-pager` | **0** |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **0** |
| targeted tests after fmt (green filters plus `block_line_exhaustive_literal_keeps_legacy_shape`) | **0** (8 passed) |

**Process mop (re-ran; no fallout):**

| Step | Exit |
|------|------|
| `cargo fmt -p xai-grok-pager` | **0** (did not dirty the tree) |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **0** |
| `cargo test -p xai-grok-pager --lib --` eight named filters | **0** (8 passed, 0 failed) |

## Whether mop ran

Yes. Separate process-mop L3 re-ran fmt, clippy `-D warnings`, and the eight named filters. Nothing failed. The mop did not edit any files.

Stop.
