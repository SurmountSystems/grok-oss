# Final report: human-message bubble copy click

Synthesized from explore, implementer, mop, and mop-status reports. Stale
action and brief files still say the implementer report is missing. Those
snapshots are wrong. `bug-copy-human-message-impl.md` and
`bug-copy-human-message-mop.md` are on disk. Cargo is not running.

## What the button is in product terms

The control is the always-on bubble copy glyph on a human message. It is
`⧉` (`copy_icon()`, U+29C9). Legacy ConHost paints `c` instead. The glyph
is on the first line of user and assistant bubbles when **Bubble copy
buttons** is on. That setting is on by default. Settings key:
`bubble_copy_buttons`. When the flag is on, the selection box hides its
own copy icon (Policy A: one `⧉` per bubble).

On a typical human line there is no fullscreen `↗`, so the only visible
copy affordance is this glyph to the right of the prompt text, next to the
green human rail (`accent_user`). Clicking or otherwise activating that
control must copy that human message to the clipboard.

This is not keyboard `y` after the block is already selected. It is not
`/copy` (assistant only). It is not drag text selection. Human chrome
stays green. The caret is not flipped to magenta.

## Source broken vs old live binary (evidence)

Source was broken. This is not only an old live binary.

Explore and the implementer both show the icon was paint-only.
`append_bubble_copy_button` appended the glyph and did not record a hit.
`render_selection_buttons` hid the only previously wired `⧉`
(`hit_sb_copy`) whenever bubble copy was on. `handle_mouse` only copied
from `hit_sb_copy`, which is empty in that mode. A left click on the
painted human icon selected the block or started a drag. It returned
`InputOutcome::Changed`. It did not emit `Action::CopyBlockContent` and
did not write the clipboard.

`UserPromptBlock::copy_text()` already returned the real prompt. Keyboard
`y` on a selected human message could already copy. The click path did
not reach that dispatcher.

The product fix keeps Policy A and wires the painted icon: mark
`copy_button_col` at paint time, publish `bubble_copy_hits` from content
paint and sticky user headers, store them on `AgentView` as
`hit_bubble_copy`, and on left mouse down select that entry and return
`Action::CopyBlockContent`. Hover restyles the glyph and includes those
rects in the OSC 22 pointer. Dispatch still uses
`UserPromptBlock::copy_text()`.

Crate version is still `1.0.3`. If the operator is looking at a live TUI
built before this wire, the button will still fail until they rebuild and
fully quit and reopen the process.

## Red: test name, command, fail reason, before product edit

Test: `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`

The implementer report says the test was added first. Product edits that
publish hits and handle the click came after this observed fail. The
test draws an `AgentView` with `bubble_copy_buttons` on, pushes
`UserPromptBlock` text `COPY-HUMAN-MSG-CONTRACT`, finds `copy_icon()` in
the scrollback pane, sends left mouse down on that cell, and asserts
`Action::CopyBlockContent`, that the human entry is selected, that
`copy_text` equals the original prompt, and that the payload does not
contain `⧉`.

Command (implementer):

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt -- --nocapture
```

A later mop pass used `--offline` and `TMPDIR=/home/hunter/.cache/grok-oss-tmp`
on the same crate and filter.

Fail reason (icon still painted; click path not wired):

```
