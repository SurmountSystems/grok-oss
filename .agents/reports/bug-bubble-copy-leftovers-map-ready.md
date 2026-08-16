MAP READY
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
