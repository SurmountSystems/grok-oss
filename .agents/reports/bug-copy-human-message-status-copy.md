# Status: copy control on a human message

Gathered Sat Aug 15 01:40:32 PM MDT 2026 (rechecked 01:40:42 PM MDT 2026).

## 1. date

Sat Aug 15 01:40:32 PM MDT 2026

## 2. ls -la .agents/reports/

Directory: `/home/hunter/Projects/surmount/grok-build/.agents/reports/`

```
total 2752
drwxr-xr-x 1 hunter hunter 27460 Aug 15 13:36 .
drwxr-xr-x 1 hunter hunter    34 Aug 12 18:03 ..
```

Copy-human files in that listing:

```
-rw-r--r-- 1 hunter hunter 14547 Aug 15 13:10 bug-copy-human-message-explore.md
-rw-r--r-- 1 hunter hunter 14547 Aug 15 13:10 bug-copy-human-message-explore-READY.md
-rw-r--r-- 1 hunter hunter 45740 Aug 15 13:31 bug-copy-human-message-impl-TIMEOUT.md
-rw-r--r-- 1 hunter hunter 31095 Aug 15 13:38 bug-copy-human-message-status.md
```

`bug-copy-human-message-impl.md` is not in the directory. The rest of the folder is other reports (hundreds of files). This file overwrites the 13:38 status snapshot.

## 3. test -f explore and impl report paths

| Path | test -f |
|------|---------|
| `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-explore.md` | EXISTS |
| `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-impl.md` | MISSING |

Related files (not the required impl path):

- `bug-copy-human-message-explore-READY.md` exists (same first lines as explore)
- `bug-copy-human-message-impl-TIMEOUT.md` exists (waiter note: waited 20 minutes, target never appeared)

### First 80 lines of explore (exists)

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
```

### Impl report

`bug-copy-human-message-impl.md` does not exist. First 80 lines not included.

First 80 of the timeout sidecar `bug-copy-human-message-impl-TIMEOUT.md` (not the required impl path):

```
# Timeout: bug-copy-human-message-impl.md never qualified

Waited 20 minutes (poll every 10s from 2026-08-15T13:08:56-06:00 to 2026-08-15T13:28:47-06:00).
Target never appeared: /home/hunter/Projects/surmount/grok-build/.agents/reports/bug-copy-human-message-impl.md
```

That sidecar then dumps an `ls -la` of `.agents/reports/` from 13:31.

## 4. ps -eo pid,etime,cmd | rg cargo|rustc|grok-build-target

No `cargo`, `rustc`, or `grok-build-target` worker process. The only matches were this gather's own `bash` and `rg`.

## 5. ls -lt /home/hunter/.cache/grok-build-target

```
total 4
drwxr-xr-x 1 hunter hunter 176 Aug 15 13:15 debug
-rw-r--r-- 1 hunter hunter 177 Aug 15 13:15 CACHEDIR.TAG
```

Cache exists. Last listing time on those entries is Aug 15 13:15. No live compile process against it now.

## 6. Verdict

Explore report is on disk. Required impl report path is missing. Timeout sidecar says the impl path never appeared after a 20 minute wait. No cargo/rustc compile is running.

CONCLUSION
died_no_report
