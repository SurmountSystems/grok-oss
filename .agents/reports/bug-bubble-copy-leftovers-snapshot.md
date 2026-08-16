# Snapshot: bubble-copy leftovers reports

IMPLEMENTER REPORT NOT READY

`bug-bubble-copy-leftovers-impl.md` is missing.

## Path inventory

| # | Path | Status |
|---|------|--------|
| 1 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-bubble-copy-leftovers-map.md` | exists |
| 2 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-bubble-copy-leftovers-impl.md` | missing |
| 3 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-bubble-copy-leftovers-impl-ready.md` | missing |
| 4 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-bubble-copy-leftovers-map-ready.md` | missing |
| 5 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message.md` | exists |
| 6 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-impl.md` | exists |
| 7 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-explore.md` | exists (216 lines; first 200 below) |
| 8 | `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-mop.md` | exists |

---

## 1. EXISTS — `bug-bubble-copy-leftovers-map.md`

```
# Leftover map: human-message bubble copy click

Source: the four reports only (`bug-copy-human-message.md`, `bug-copy-human-message-impl.md`, `bug-copy-human-message-explore.md`, `bug-copy-human-message-mop.md`). This map does not re-read the product tree.

Crate: `xai-grok-pager` only. Cargo cache: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`. Mop also set `TMPDIR=/home/hunter/.cache/grok-oss-tmp` and used `--offline` on clippy and tests.

## What already landed

The always-on bubble copy glyph is `⧉` from `copy_icon()` (U+29C9; legacy ConHost paints `c`). Settings key `bubble_copy_buttons` is on by default (Policy A: one `⧉` per bubble). The click path is now wired. It is not keyboard `y`, not `/copy`, and not drag selection.

Product behavior that is in the tree:

- `append_bubble_copy_button` marks `BlockLine.copy_button_col` and, when the line is `Selectable::All`, shrinks selectable spans so drag-copy does not include `⧉`.
- Scrollback content paint and sticky user headers publish `Vec<(Rect, entry_idx)>` as `bubble_copy_hits`.
- `AgentView` stores those rects in `hit_bubble_copy`.
- Left mouse down on a published rect (before `hit_sb_copy` and before scrollback drag) selects that entry and returns `Action::CopyBlockContent`. Dispatch still uses `UserPromptBlock::copy_text()`.
- Hover restyles the glyph (`text_primary` + bold) and includes those rects in the OSC 22 pointer (same as link hover). Human rail color stays `accent_user`.

The named human click contract is green. Paint tests were not rewritten to finish green.

## Files and symbols already mentioned

| Path | Symbols / role |
|------|----------------|
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` | `append_bubble_copy_button` (mark column, shrink `Selectable::All`) |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` | `UserPromptBlock::output`, `UserPromptBlock::copy_text` |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/agent.rs` | `AgentMessageBlock::output` (same paint helper) |
| `crates/codegen/xai-grok-pager/src/scrollback/types.rs` | `copy_button_col`, hit rect helper; `BlockLine`, `Selectable` |
| `crates/codegen/xai-grok-pager/src/scrollback/selection.rs` | `RenderOutput.bubble_copy_hits` |
| `crates/codegen/xai-grok-pager/src/scrollback/render.rs` | publish `bubble_copy_hits` |
| `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs` | content hits; `render_sticky_header` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `hit_bubble_copy`, `hovered_bubble_copy` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | init |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | assign hits, hover restyle, OSC 22 pointer |
| `crates/codegen/xai-grok-pager/src/app/mouse.rs` | click, hover, contract test |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` | `render_selection_buttons` (still hides selection-box `⧉` when bubble copy is on) |
| `crates/codegen/xai-grok-pager/src/app/dispatch/transcript.rs` | `dispatch_copy_block_content` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/notices.rs` | `copy_to_clipboard` |

Mop lint-only (no product behavior): `agent_view/render.rs` (clear-finished test expect), `scrollback/selection.rs` (identity math), `benches/edit_highlight.rs`, `tests/doctor_early_dispatch.rs`, `src/diagnostics/fix_tests.rs`, `tests/settings_e2e.rs`.

Explore also named, but this slice did not change them: `settings/defs.rs` (`bubble_copy_buttons`), `xai-grok-pager-render` `appearance/config.rs` (`ScrollbackDisplayConfig.bubble_copy_buttons`), `app/dispatch/settings/setters.rs`, `appearance/cache.rs`. Crate version is still `1.0.3` in `crates/codegen/xai-grok-pager-bin/Cargo.toml`. No user-guide edit. No settings-registry change.

## Exact leftover quotes: omit the glyph when the first line is too wide

Explore (how the icon is painted):

> If `used + 1 + icon.width()` would exceed `ctx.content_width()`, it **drops the icon** (no wrap, no right-align).

Explore (suggested fix, still leftover polish):

> If the first line is too wide, the icon is omitted today. That is a separate polish. Do not block the click wire on right-align.

Implementer leftovers:

> If the first line is too wide, the icon is still omitted (pre-existing polish). That is not this contract.

Final synthesizer leftovers:

> Wide first lines still drop the icon. That is separate polish, not this contract.

Next implementer: this is the remaining product polish. The click wire must not wait on right-align. Today a first line that cannot fit space plus icon is painted without `⧉`, so there is no hit.

## Other leftovers (honesty, not this contract)

- The unit test asserts the action and `copy_text` payload. It does not talk to a host clipboard daemon.
- Assistant bubble `⧉` shares the same paint helper and the same hit path. There is no dedicated assistant click test. This slice's named test is the human message only. Explore also noted there is no agent-message paint test for the same icon.
- Live TUI may still be an old `1.0.3` binary. Rebuild and fully quit and reopen.
- Keyboard `y` on a selected human line already copied and was not the bug.
- Residual (explore) mentions `pointer_cursor` next to `bubble_copy_`. Explore said no `pointer_cursor` test exists in `*.rs` in that tree. Hover/OSC 22 for bubble copy later landed in product; a dedicated `pointer_cursor` test name was not added in these reports.
- Stale coordinator files `bug-copy-human-message-action.md` and `bug-copy-human-message-brief.md` still say the implementer report is missing. Ignore those.

## Existing test names and cargo commands

**Named click contract (red then green):**

- Module path: `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`
- File: `crates/codegen/xai-grok-pager/src/app/mouse.rs`
- Fixture text: `COPY-HUMAN-MSG-CONTRACT`
- Red fail: `clicking the human-message copy control must copy via CopyBlockContent, got Changed`

Implementer command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt -- --nocapture
```

**Paint tests (user bubble, not rewritten):**

- `bubble_copy_buttons_on_paints_copy_icon` in `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs`
- `bubble_copy_buttons_off_omits_copy_icon` in the same file

Related filter after green: `clicking_human_bubble_copy` / `bubble_copy_` (3 passed). Also `block_line_exhaustive_literal_keeps_legacy_shape` passed.

Mop (offline, same crate):

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test --offline -p xai-grok-pager --lib -- clicking_human_bubble_copy bubble_copy_
```

That was 3 passed, later 6 with mopped-site tests:

`clicking_human_bubble_copy bubble_copy_ action_button_without_close_reserves_close_slot clear_finished_hit_does_not_intersect full_preview_safely_renders_backtick`

Mop also ran:

```
cargo test --offline -p xai-grok-pager --test settings_e2e -- render_with_filter_active_and_small_viewport_clamps_scroll
```

**Settings / payload tests named by explore (not the click contract):**

- `bubble_copy_buttons_space_dispatches_typed_setter` in `crates/codegen/xai-grok-pager/tests/settings_e2e.rs`
- `bubble_copy_buttons_mouse_click_two_stage_toggles` in the same file (settings row, not bubble)
- `bubble_copy_buttons_default_on` in `crates/codegen/xai-grok-pager-render/src/appearance/config.rs`

**Fmt / clippy used:**

- `cargo fmt -p xai-grok-pager` (exit 0)
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` (exit 0)
- `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings` (first 101, then 0 after mop)

## Catalog

File: `doc/dev/upstream-regression-filters.md`

Current enrolled filter (explore + synthesizer): only the paint test `bubble_copy_buttons_on_paints_copy_icon`, described as "Bubble copy chrome reads the flag."

The click contract `clicking_human_bubble_copy_copies_the_prompt` is not enrolled. Next implementer should add that filter if catalog work is in scope.

## Suggested next product slice

1. Wide first line: stop dropping `⧉` when `used + 1 + icon.width()` exceeds `ctx.content_width()` (explore: no wrap, no right-align today). Separate polish.
2. Optional dedicated assistant-bubble click test on the same helper and mouse branch.
3. Enroll the click test in `doc/dev/upstream-regression-filters.md`.
4. Do not treat host clipboard daemon coverage as required unless the operator asks.

Stop.
```

---

## 2. MISSING — `bug-bubble-copy-leftovers-impl.md`

IMPLEMENTER REPORT NOT READY

---

## 3. MISSING — `bug-bubble-copy-leftovers-impl-ready.md`

---

## 4. MISSING — `bug-bubble-copy-leftovers-map-ready.md`

---

## 5. EXISTS — `bug-copy-human-message.md`

```
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
clicking the human-message copy control must copy via CopyBlockContent, got Changed
test app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt ... FAILED
```

The click returned `Changed` (select or start drag). It did not copy.

This synthesizer did not re-run the red step. The fail line above is from
the implementer report.

## Green: same filter after

Clicking a published bubble-copy rect now selects that entry and returns
`Action::CopyBlockContent`. Dispatch still uses
`UserPromptBlock::copy_text()`.

Same command. Implementer result: 1 passed, 0 failed.

```
test app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt ... ok
```

Related paint tests still pass:
`bubble_copy_buttons_on_paints_copy_icon` and
`bubble_copy_buttons_off_omits_copy_icon`. Combined filter
`clicking_human_bubble_copy` / `bubble_copy_` was 3 passed. Those paint
tests were not rewritten to finish green.
`block_line_exhaustive_literal_keeps_legacy_shape` also passed.

## Files changed

Crate: `xai-grok-pager` only. No user-guide edit. No settings-registry
change.

- `crates/codegen/xai-grok-pager/src/scrollback/types.rs`
  (`copy_button_col`, hit rect helper)
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs`
  (mark column, shrink `Selectable::All`)
- `crates/codegen/xai-grok-pager/src/scrollback/selection.rs`
  (`RenderOutput.bubble_copy_hits`)
- `crates/codegen/xai-grok-pager/src/scrollback/render.rs`
  (publish `bubble_copy_hits`)
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs`
  (content and sticky-header hits)
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
  (`hit_bubble_copy`, `hovered_bubble_copy`)
- `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` (init)
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
  (assign hits, hover restyle, OSC 22 pointer)
- `crates/codegen/xai-grok-pager/src/app/mouse.rs`
  (click, hover, contract test)

Process mop later changed lint-only sites (no product behavior):
`agent_view/render.rs` (clear-finished test expect),
`scrollback/selection.rs` (identity math), `benches/edit_highlight.rs`,
`tests/doctor_early_dispatch.rs`, `src/diagnostics/fix_tests.rs`,
`tests/settings_e2e.rs`.

## Leftovers / honesty

- The unit test asserts the action and `copy_text` payload. It does not
  talk to a host clipboard daemon.
- Wide first lines still drop the icon. That is separate polish, not this
  contract.
- Assistant bubble `⧉` shares the same paint helper and the same hit
  path. There is no dedicated assistant click test. This slice's named
  test is the human message only.
- The upstream regression catalog still lists only the paint test, not
  the click contract.
- Live TUI may still be an old `1.0.3` binary. Rebuild and fully quit
  and reopen.
- Keyboard `y` on a selected human line already copied and was not the
  bug.
- Stale coordinator files (`bug-copy-human-message-action.md`,
  `bug-copy-human-message-brief.md`) still say the implementer report is
  missing. Ignore those. The implementer and mop reports exist.

## fmt / clippy / test exit codes

Implementer (before mop lint fixes):

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-pager` | 0 |
| clippy lib | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy all-targets | `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | 101 |
| contract (red) | `cargo test -p xai-grok-pager --lib clicking_human_bubble_copy_copies_the_prompt` | 101 |
| contract (green) | same filter | 0 |
| related | `clicking_human_bubble_copy` / `bubble_copy_` | 0 (3 passed) |
| BlockLine shape | `block_line_exhaustive_literal_keeps_legacy_shape` | 0 |

Implementer `--all-targets` clippy was 101 on leftover lints outside the
copy click: bench `needless_range_loop`, clear-finished
`expect(&format!(...))`, diagnostics `Path::canonicalize`,
clear-finished `0 + 40 - 1`. Product `--lib` clippy was already clean.

All cargo commands used
`CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`.

## Whether mop ran

Yes. Process mop ran and finished. Report:
`.agents/reports/bug-copy-human-message-mop.md`. Mop-status conclusion
is `mop_done`. No live cargo, rustc, or clippy at that snapshot. This
synthesizer did not start a second cargo.

| Step | Exit |
|------|------|
| `cargo fmt -p xai-grok-pager` | 0 |
| `cargo clippy --offline -p xai-grok-pager --lib -- -D warnings` | 0 |
| `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings` (first) | 101 |
| `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings` (after lint mop) | 0 |
| `cargo test --offline -p xai-grok-pager --lib -- clicking_human_bubble_copy bubble_copy_` | 0 (3 passed; later 6 with mopped-site tests) |
| `cargo test --offline -p xai-grok-pager --test settings_e2e -- render_with_filter_active_and_small_viewport_clamps_scroll` | 0 |

First `--all-targets` was 101 on the leftover lints the implementer
listed, plus `unnecessary_min_or_max` in `settings_e2e`. Mop fixed those
lint-only sites. Crate `--all-targets` clippy is now 0. No product
behavior change from mop.
```

---

## 6. EXISTS — `bug-copy-human-message-impl.md`

```
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
```

---

## 7. EXISTS — `bug-copy-human-message-explore.md` (first 200 of 216 lines)

```
# Explore: copy control on a human message does not copy

Read-only. No product edits. Evidence is from this tree, not from the live TUI process.

## Product name

The control is the **always-on bubble copy button**: the `⧉` glyph (`copy_icon()`, U+29C9, or `c` on legacy ConHost) painted on the first line of **user** and **assistant** message bubbles when `[scrollback.display] bubble_copy_buttons` is on (default on).

Settings label: **Bubble copy buttons** (Appearance). Registry key `bubble_copy_buttons`. Description: "Show a copy button on user and agent message bubbles. When on, the selection box omits its copy icon."

That is **Policy A**: one `⧉` per bubble. The selection-box `⧉` is hidden on those blocks. The selection-box **view** button (`↗`) can still appear on blocks that support fullscreen. User prompts usually do **not** support fullscreen, so a typical human line only has the inline bubble `⧉`.

This is **not**:

- The selection-box `⧉` (`hit_sb_copy`) that appears only when `bubble_copy_buttons` is **off** and `selection_buttons` is on.
- Keyboard `y` / `Action::CopyBlockContent` after the block is already selected.
- `/copy` / `Action::CopyAssistantMessage` (assistant only).
- Drag text selection, which copies the selected columns.
- Plan-viewer or Mermaid `[Copy source]` affordances.
- The composer / prompt-draft copy chrome claimed in FORK. In this tree, `copy_icon()` is **not** painted on the prompt widget.

Human chrome is the green user rail (`accent_user`) plus the elevated prompt band. The bubble `⧉` is appended **after** the first line of prompt text (space + dim icon), so it sits to the **right of the human text**, next to the green rail/band. That matches the screenshot description.

## Source vs possibly-old live binary

**Source is broken for click-to-copy on the painted bubble `⧉`.** This is not a guess from prose. The icon is paint-only. There is no hit rect, no hover, and no mouse branch that copies that cell.

Both **user** and **assistant** bubbles use the same helper. There is **no** human-only click path that works in source, and **no** assistant-only click path that would make only agent `⧉` work. If the live TUI copies an assistant bubble by clicking `⧉`, that is not explained by this tree.

Live TUI age cannot be proved from this tree. Crate version is still `1.0.3` (`crates/codegen/xai-grok-pager-bin/Cargo.toml`). Same class as other "maybe the running binary is old" reports: do not claim the operator's process is stale. Claim only: **if they are on this source, click on the human `⧉` cannot copy.**

If they were on a build from **before** always-on bubble `⧉`, they would see the **selection-box** `⧉` instead (when `selection_buttons` is on). That older path **does** write the clipboard in this source (`hit_sb_copy` → `Action::CopyBlockContent` → `UserPromptBlock::copy_text()`). The report is about the inline bubble control, which this source paints and does not wire.

## How the icon is painted

`append_bubble_copy_button` in  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs`

- Returns immediately if `ctx.appearance.scrollback.display.bubble_copy_buttons` is false.
- Takes the **first** `BlockLine`.
- If `used + 1 + icon.width()` would exceed `ctx.content_width()`, it **drops the icon** (no wrap, no right-align).
- Else pushes `Span::raw(" ")` and `Span::styled(icon, Theme::current().dim())`.
- Does **not** record a hit kind, span marker, or column.
- Does **not** shrink `selectable` to exclude the new spans. Lines that were `Selectable::Spans(1..content_end)` (normal prefixed user lines) already exclude the icon. Compact / `Selectable::All` lines would include `⧉` in drag-copy text.

Call sites:

- `UserPromptBlock::output` in `scrollback/blocks/user.rs` (after `wrap_prompt_lines`).
- `AgentMessageBlock::output` in `scrollback/blocks/agent.rs` (after markdown output).

`UserPromptBlock::copy_text()` returns `self.text` (the real prompt). That payload is correct. `RenderBlock::supports_copy()` includes `UserPrompt`. `RenderBlock::copy_text` forwards user blocks to that method.

## Hit-test / click path

### What actually runs on left-click

`AgentView::handle_mouse` in  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/mouse.rs`

Order that matters:

1. Various chrome (todos, voice, cwd, …).
2. **`self.hit_sb_copy.contains(...)` → `InputOutcome::Action(Action::CopyBlockContent)`.** This is the only scrollback `⧉` click that copies.
3. Scrollback pane: store `pending_scrollback_click`, start text/block drag. **No scan for `copy_icon()`.**
4. On mouse up without a drag: text multi-click, inline media / Mermaid, then `handle_scrollback_click` which **selects** the entry (or folds). **No copy.**

Hover: `hit_sb_copy.update_hover` only. OSC 22 pointer cursor in `agent_view/render.rs` is **link hover only** (`hovered_link_idx`). FORK says copy chrome requests a pointer cursor. That is **not** implemented for the bubble icon, and `hit_sb_copy` is empty when bubble copy is on.

### Why the working copy button is gone on human bubbles

`AgentView::render_selection_buttons` in  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs`

```text
has_copy = entry.block.supports_copy() && !header_selected && !bubble_copy
```

When `bubble_copy_buttons` is on (default), `has_copy` is false. `hit_sb_copy` is cleared. Policy A is implemented as "hide the only clickable `⧉`", not "move the hit onto the bubble icon."

User prompts: `has_normal_fullscreen_viewer()` is false, so a typical human selection box has **no** `⧉` and **no** `↗`. The only visible copy glyph is the paint-only bubble icon.

### Clipboard write (when something actually copies)

`dispatch_copy_block_content` in  
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/dispatch/transcript.rs`

- Requires `scrollback.selected()`.
- Bails if `entry_content_hidden_by_group(idx)` (group "N more" header, or height 0).
- User: `entry.block.copy_text(entry.raw)` → `UserPromptBlock::copy_text()` → `self.text`.
- Then `agent.copy_to_clipboard(&text)` in `agent_view/notices.rs` (`copy_text_or_file` + toast).

Router: `Action::CopyBlockContent` → that dispatcher (`app/dispatch/router.rs`).

So **keyboard `y` on a selected human message should copy** the prompt text, unless the selected entry is a hidden group header. The click bug is that the painted `⧉` never fires this action and never selects-then-copies.

### Settings gate

| Flag | Default | Effect |
|------|---------|--------|
| `bubble_copy_buttons` | **true** | Paint bubble `⧉` on user + agent. Hide selection-box `⧉`. |
| `selection_buttons` | **true** (code; user-guide sample still shows false) | Paint selection-box `⧉`/`↗` when the block is selected **and** `has_copy` / `has_view`. |

Cache: `appearance/cache.rs` `load_bubble_copy_buttons` / `set_bubble_copy_buttons`.  
Persist: `persist_bubble_copy_buttons` in `xai-grok-pager-render` appearance config.  
Setter: `set_bubble_copy_buttons_inner` in `app/dispatch/settings/setters.rs`.  
Defs: `settings/defs.rs` key `bubble_copy_buttons`.  
CI20: `settings_e2e.rs` enrolls the settings row (toggle only, not transcript click).

Turning **off** bubble copy and leaving selection buttons on restores the **select-first** selection-box `⧉`, which **is** wired. That is a workaround, not the intended always-on control.

## Human vs assistant (same source)

| Step | User | Assistant |
|------|------|-----------|
| Paint `⧉` | `UserPromptBlock::output` | `AgentMessageBlock::output` |
| Helper | same `append_bubble_copy_button` | same |
| `supports_copy` | yes | yes |
| `copy_text` payload | `self.text` | markdown raw/pretty |
| Click hit | none | none |
| Selection-box `⧉` when bubble copy on | hidden | hidden |
| Selection-box `↗` | usually no | yes if fullscreen |

There is **no** source path that copies a human bubble on click of `⧉`. Assistant `⧉` is equally unwired. Assistant can still be copied via `/copy` or `y` when selected.

## Existing tests (names + files)

**Paint only (user bubble):**

- `bubble_copy_buttons_on_paints_copy_icon`  
  `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs`  
  Asserts the icon **string** is in `UserPromptBlock::output` when the flag is on. No mouse, no clipboard.
- `bubble_copy_buttons_off_omits_copy_icon`  
  Same file. Flag off omits the glyph.

**Catalog / CI20:**

- `doc/dev/upstream-regression-filters.md` enrolls `bubble_copy_buttons_on_paints_copy_icon` as "Bubble copy chrome reads the flag."
- Residual mentions `pointer_cursor` next to `bubble_copy_`. **No `pointer_cursor` test exists** in `*.rs` in this tree.

**Settings (not transcript copy):**

- `bubble_copy_buttons_space_dispatches_typed_setter`  
  `crates/codegen/xai-grok-pager/tests/settings_e2e.rs`
- `bubble_copy_buttons_mouse_click_two_stage_toggles`  
  Same file (settings row, not bubble).
- `bubble_copy_buttons_default_on`  
  `crates/codegen/xai-grok-pager-render/src/appearance/config.rs`
- Registry default drift checks in `settings/registry.rs` and `app/dispatch/tests/router.rs`.

**Copy payload (not click):**

- `UserPromptBlock::copy_text` is the user payload. No dedicated "click human `⧉` writes clipboard" test.
- `RenderBlock::copy_text` / `copy_visible_text_in_state` tests in `scrollback/block.rs` cover wrap/join, not bubble click.
- `dispatch_copy_block_content` has **no** test that a user entry is selected and copied from a mouse hit.
- Mermaid `AffordanceKind::CopySource` tests in `agent_view/paste.rs` are a **working** copy-hit pattern (unrelated surface).

**Missing:**

- Agent-message paint test for the same icon.
- Any test that a click on the bubble `⧉` (user or agent) yields `Action::CopyBlockContent` or `copy_to_clipboard` of that message.
- Hover / OSC 22 pointer on bubble copy chrome.

## Suggested smallest fix

Keep Policy A (do not bring back selection-box `⧉` on user/agent while bubble copy is on). Wire the icon that is already painted.

1. **Mark the icon at paint time** in `append_bubble_copy_button`: e.g. a `BlockLine` field for the icon span range / column, and exclude those spans from `selectable` even when `Selectable::All`.
2. **Publish hit rects at render time** (content paint **and** sticky user headers, which re-run `UserPromptBlock::output` and will include the same icon). Store `Vec<(Rect, entry_idx)>` on `AgentView` (one `HitArea` is not enough: many bubbles are visible).
3. **Mouse down, before scrollback drag:** if a bubble-copy rect contains the cell, `scrollback.set_selected(Some(entry_idx))` and return `InputOutcome::Action(Action::CopyBlockContent)`. That reuses `dispatch_copy_block_content` and `UserPromptBlock::copy_text()`.
4. **Hover** the same rects (brighten like `render_char_buttons`; OSC 22 pointer if you honor the FORK sentence).
5. If the first line is too wide, the icon is omitted today. That is a separate polish. Do not block the click wire on right-align.

Do **not** use `copy_visible_text_in_state` as the click payload: it is rendered text and can pick up the `⧉` on `Selectable::All` lines. `copy_text` is the right user payload.

## Suggested red test contract

Named contract: **Clicking the always-on bubble `⧉` on a human message copies that prompt's text through the existing block-copy action.**

Smallest red test (unit, no host clipboard):

1. Build an `AgentView` (or a thinner helper) with one `UserPromptBlock` whose text is a unique string, `bubble_copy_buttons = true`, `selection_buttons` either on or off.
2. Draw into a buffer wide enough that `append_bubble_copy_button` actually paints (short first line).
3. Find the screen cell that contains `copy_icon()` on that user row (or use the new hit list once it exists; red first can locate the glyph in the buffer).
4. Send left mouse **down** on that cell through `handle_mouse`.
5. **Assert** `InputOutcome::Action(Action::CopyBlockContent)` (or that the dispatcher ran with that entry selected).
6. **Assert** `scrollback.selected()` is that user entry.
7. **Assert** `entry.block.copy_text(...)` equals the original prompt (no `⧉` in the payload).

Optional sibling: same click on an `AgentMessageBlock` (same helper; proves the hole is not human-only).

Do not rewrite the existing paint tests to pass. They stay as flag-on/flag-off chrome. Add a **click** test. Observed red: today `handle_mouse` returns `Changed` and only selects / starts drag.

## Implementer map

| Job | File | Symbol |
|-----|------|--------|
| Paint icon | `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs` | `append_bubble_copy_button` |
| User output | `.../scrollback/blocks/user.rs` | `UserPromptBlock::output`, `copy_text` |
| Agent output | `.../scrollback/blocks/agent.rs` | `AgentMessageBlock::output` |
| Line model | `.../scrollback/types.rs` | `BlockLine`, `Selectable`, `derive_selection_text` |
| Sticky header re-paint | `.../scrollback/scrollback_pane.rs` | `render_sticky_header` |
| Hide selection `⧉` | `.../app/agent_view/viewer.rs` | `render_selection_buttons` |
| Click / hover today | `.../app/mouse.rs` | `handle_mouse` (`hit_sb_copy` branch ~line 446) |
| Hit storage | `.../app/agent_view/mod.rs` | `hit_sb_copy: HitArea` (add a list) |
| Clear on no box | `.../app/agent_view/render.rs` | `hit_sb_copy.clear()` |
| Dispatch / clipboard | `.../app/dispatch/transcript.rs` | `dispatch_copy_block_content` |
```

(truncated after line 200 of 216)

---

## 8. EXISTS — `bug-copy-human-message-mop.md`

```
# Process mop: human-message bubble copy click

Workspace: `/home/hunter/Projects/surmount/grok-build`.
`CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`.
`TMPDIR=/home/hunter/.cache/grok-oss-tmp`.
`--offline` on clippy and tests.

Product Rust was edited (`xai-grok-pager`). Mop ran. Not skipped.

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt (first) | `cargo fmt -p xai-grok-pager` | **0** |
| clippy lib | `cargo clippy --offline -p xai-grok-pager --lib -- -D warnings` | **0** |
| clippy all-targets (first) | `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings` | **101** |
| contract + paint (first) | `cargo test --offline -p xai-grok-pager --lib -- clicking_human_bubble_copy bubble_copy_` | **0** (3 passed) |
| fmt (after lint mop) | `cargo fmt -p xai-grok-pager` | **0** |
| clippy all-targets (second) | same `--all-targets -- -D warnings` | **101** (next hidden lint) |
| fmt (after settings e2e lint) | `cargo fmt -p xai-grok-pager` | **0** |
| clippy all-targets (final) | same `--all-targets -- -D warnings` | **0** |
| contract + mopped lib tests | `cargo test --offline -p xai-grok-pager --lib -- clicking_human_bubble_copy bubble_copy_ action_button_without_close_reserves_close_slot clear_finished_hit_does_not_intersect full_preview_safely_renders_backtick` | **0** (6 passed) |
| mopped settings e2e | `cargo test --offline -p xai-grok-pager --test settings_e2e -- render_with_filter_active_and_small_viewport_clamps_scroll` | **0** (1 passed) |

## First `--all-targets` fail (then mopped)

These were lint-only. They were already listed in the implementer leftovers as pre-existing. Mop fixed them so crate `--all-targets` is clean.

1. `src/app/agent_view/render.rs`: `expect(&format!(...))` → `unwrap_or_else(\|\| panic!(...))` (`clippy::expect_fun_call`, clear-finished test).
2. `src/scrollback/selection.rs`: `0 + 40 - 1` → `40 - 1` (`clippy::identity_op`, clear-finished layout test).
3. `benches/edit_highlight.rs`: range index loop → `enumerate().take(end).skip(start)` (`clippy::needless_range_loop`).
4. `tests/doctor_early_dispatch.rs`: `Path::canonicalize` → `dunce::canonicalize`.
5. `src/diagnostics/fix_tests.rs`: same `dunce::canonicalize`.
6. After those, `tests/settings_e2e.rs`: `visible.saturating_sub(1).max(0)` → `visible.saturating_sub(1)` (`clippy::unnecessary_min_or_max`).

No product behavior change. No new features.

## Final bar

- `cargo fmt -p xai-grok-pager`: **0**
- `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings`: **0**
- Named contract `clicking_human_bubble_copy_copies_the_prompt` plus paint tests: **0**

Stop. No git add, commit, or push.
```
