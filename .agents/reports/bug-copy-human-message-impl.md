# Implementer report: human-message copy control

Workspace: `/home/hunter/Projects/surmount/grok-build`. L3 implementer. Followed `.agents/reports/bug-copy-human-message-explore.md`.

Named contract: clicking or activating the always-on copy control on a human message copies that human message to the clipboard.

## What the button is

The control is the **always-on bubble copy button**. It is the `⧉` glyph (`copy_icon()`, U+29C9, or `c` on legacy ConHost) painted on the first line of a user (and assistant) message bubble when `[scrollback.display] bubble_copy_buttons` is on. That setting is on by default.

Settings label: **Bubble copy buttons**. Registry key `bubble_copy_buttons`. Policy A: one `⧉` per bubble. The selection-box copy icon is hidden on those blocks. A typical human line has no fullscreen view button, so the inline bubble `⧉` to the right of the prompt text (next to the green user rail) is the only copy affordance the operator sees.

This is not the selection-box `⧉` (`hit_sb_copy`), not keyboard `y` after the block is already selected, not `/copy` (assistant only), and not drag text selection.

Human chrome stays green (`accent_user`). The caret is not flipped to magenta.

## Source broken vs old live binary

**Source was broken.** This is not only an old live binary.

Explore and the red test both show the icon was paint-only. `append_bubble_copy_button` appended the glyph and did not record a hit. `render_selection_buttons` hid the only clickable `⧉` whenever bubble copy was on. `handle_mouse` only copied from `hit_sb_copy`, which is empty in that mode. A left click on the painted cell returned `InputOutcome::Changed` (select or start drag) and did not emit `Action::CopyBlockContent`.

`UserPromptBlock::copy_text()` already returns the real prompt. Keyboard `y` on a selected human message could already copy. The click path did not reach that dispatcher.

Crate version is still `1.0.3`. The live TUI can still be a pre-fix binary. After this source is built, the operator needs a **rebuild and a full quit/reopen**. A running 1.0.3 process will keep showing a dead `⧉`.

## Red (before any product edit that makes the test pass)

- **Test:** `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`
- **Command:** `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt -- --nocapture`
- **Fail reason:** `clicking the human-message copy control must copy via CopyBlockContent, got Changed`
- The buffer **did** contain the copy icon. The click was the miss.
- The test was added first. Product edits that publish hits and handle the click came after this observed fail.

The test draws an `AgentView` with `bubble_copy_buttons` on, pushes `UserPromptBlock` text `COPY-HUMAN-MSG-CONTRACT`, finds `copy_icon()` in the scrollback pane, sends left mouse down on that cell, and asserts:

1. `InputOutcome::Action(Action::CopyBlockContent)`
2. that human entry is selected
3. `entry.block.copy_text(...)` equals the original prompt
4. the payload does not contain `⧉`

## Product fix

Keep Policy A. Wire the icon that was already painted.

1. `append_bubble_copy_button` now sets `BlockLine.copy_button_col` and, when the line is `Selectable::All`, shrinks selectable spans so drag-copy does not include `⧉`.
2. Scrollback render (content paint and sticky user headers) publishes `Vec<(Rect, entry_idx)>` as `bubble_copy_hits`.
3. `AgentView` stores those rects in `hit_bubble_copy`.
4. Left mouse down on a hit, before `hit_sb_copy` and before scrollback drag, selects that entry and returns `Action::CopyBlockContent`. Existing `dispatch_copy_block_content` plus `UserPromptBlock::copy_text()` write the clipboard.
5. Hover on those rects restyles the glyph (`text_primary` + bold) and includes them in OSC 22 pointer (same as link hover). Human rail color is unchanged.

## Green (same filter after the product edit)

Command:

`CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt -- --nocapture`

Result: **1 passed**, 0 failed. Later same-filter plus paint tests: `clicking_human_bubble_copy_copies_the_prompt`, `bubble_copy_buttons_on_paints_copy_icon`, `bubble_copy_buttons_off_omits_copy_icon` all passed (3 passed, 8886 filtered). `block_line_exhaustive_literal_keeps_legacy_shape` passed.

Paint tests were not rewritten to finish green.

## Files changed

- `crates/codegen/xai-grok-pager/src/app/mouse.rs` (red test, click branch, hover)
- `crates/codegen/xai-grok-pager/src/scrollback/types.rs` (`copy_button_col`, hit rect helper)
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` (mark column, shrink `Selectable::All`)
- `crates/codegen/xai-grok-pager/src/scrollback/render.rs` (publish `bubble_copy_hits`)
- `crates/codegen/xai-grok-pager/src/scrollback/selection.rs` (`RenderOutput.bubble_copy_hits`)
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs` (content + sticky-header hits)
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` (`hit_bubble_copy`, `hovered_bubble_copy`)
- `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` (init)
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` (assign hits, hover restyle, OSC 22 pointer)

No user-guide edit. No settings-registry change. Assistant bubbles use the same paint helper, so they get the same click path, but this slice's named test is the human message only.

## Leftovers / honesty

- The unit test asserts the action and `copy_text` payload. It does not talk to a host clipboard daemon.
- If the first line is too wide, the icon is still omitted (pre-existing polish). That is not this contract.
- There is still no dedicated assistant-bubble click test. Same helper, same mouse branch.
- `clippy --all-targets -- -D warnings` is still red on **pre-existing** lints this work did not introduce: `needless_range_loop` in `benches/edit_highlight.rs`, `expect_fun_call` in `agent_view/render.rs` (clear-finished test), `Path::canonicalize` in diagnostics/doctor tests, `identity_op` in `scrollback/selection.rs`. Those were not mopped.
- Live process remains crate `1.0.3` until rebuild and a full quit/reopen.

## fmt / clippy / test exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-pager` | **0** |
| clippy lib | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **0** |
| clippy all-targets | `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **101** (pre-existing benches/tests only; see leftovers) |
| contract test (green) | `cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt` | **0** (1 passed) |
| related lib tests | filter `clicking_human_bubble_copy` / `bubble_copy_` | **0** (3 passed) |
| `BlockLine` shape | `block_line_exhaustive_literal_keeps_legacy_shape` | **0** (1 passed) |

All cargo commands used `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`.

Stop. No further product work in this slice.
