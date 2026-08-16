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
| Clipboard toast | `.../app/agent_view/notices.rs` | `copy_to_clipboard` |
| Flag | `.../xai-grok-pager-render/src/appearance/config.rs` | `ScrollbackDisplayConfig.bubble_copy_buttons` |
| Settings | `.../settings/defs.rs` | key `bubble_copy_buttons` |
| Pattern to copy | `.../app/agent_view/media.rs` | `AffordanceKind::CopySource` + `inline_media_hits` |

Working comparison: Mermaid `[Copy source]` publishes rects at paint and copies on click. Bubble `⧉` never publishes rects.

## Bottom line

The human-line copy control is the always-on bubble `⧉`. Source **paints** it on user (and assistant) bubbles and **intentionally disables** the only clickable `⧉`. Clicking the painted icon selects the human block (or starts a drag). It does **not** write the clipboard. `UserPromptBlock::copy_text` is already correct. Smallest fix is hit-test + `CopyBlockContent` on that cell, with a red test that the click emits that action for a user prompt.
