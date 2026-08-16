VERIFY_OK

Checked against:
- `.agents/reports/bug-copy-human-message.md` (current synthesized final)
- `.agents/reports/bug-copy-human-message-SYNTH-DONE.md`
- `.agents/reports/bug-copy-human-message-impl.md`
- `.agents/reports/bug-copy-human-message-explore.md`
- `.agents/reports/bug-copy-human-message-mop.md`
- `.agents/reports/bug-copy-human-message-mop-status.md`

Did not implement product. Did not run git. Did not spawn. Did not re-run cargo.

## Required-section checklist

| Required section | Present? | Notes |
|------------------|----------|--------|
| What the button is in product terms | YES | Always-on bubble `⧉` (`copy_icon()`, U+29C9; ConHost `c`). Settings **Bubble copy buttons** / `bubble_copy_buttons`, default on, Policy A. Not `y`, not `/copy`, not drag. Human chrome stays green. |
| Source broken vs old live binary | YES | Source was paint-only. `append_bubble_copy_button` had no hit; `hit_sb_copy` hidden when bubble copy is on; click returned `Changed`. Crate still `1.0.3`; live process needs rebuild and full quit/reopen. |
| Red | YES | Test `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`. Fail: `got Changed`. Exit 101. Cited from implementer; synthesizer did not re-run red. |
| Green | YES | Same filter after hit-test + `CopyBlockContent`. Exit 0. Paint tests not rewritten. |
| Files changed | YES | Nine `xai-grok-pager` product files plus lint-only mop sites. No user-guide. No settings-registry change. |
| Leftovers | YES | No host clipboard; wide-line drop; no assistant click test; catalog still paint-only; live `1.0.3`; stale action/brief. |
| fmt/clippy/test exit codes | YES | Implementer table plus mop table. Lib clippy 0. Implementer `--all-targets` 101. Mop `--all-targets` 0 after lint mop. |
| Whether mop ran | YES | Yes. `bug-copy-human-message-mop.md`. Mop-status conclusion `mop_done`. No live cargo at that snapshot. |

## Honesty check (not a fail)

- Final report is not empty. SYNTH-DONE matches it.
- Stale `bug-copy-human-message-action.md` / `brief.md` still say impl is missing. Final report already tells the reader to ignore those.
- Mop-status captured an older `bug-copy-human-message.md` that said a later pass disabled the click branch to observe red. Current final follows `impl.md` (test added first, then product). Same fail line either way. Current Red section names its source (implementer) and says this synthesizer did not re-run.
- Mop table in the final report collapses two `--all-targets` 101s (leftover lints, then `settings_e2e` `unnecessary_min_or_max`) into one first-101 then final-0. The paragraph under the mop table still names the extra lint. Mop.md has the full sequence.
- No required honesty is missing enough to rewrite the final report.

## Full final report

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
