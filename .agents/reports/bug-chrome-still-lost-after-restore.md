# Chrome still looks wrong after restore (honest leftover)

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:chrome-still-lost-after-restore`  
**Date:** 2026-08-14  
**Agent:** L2 implementer (no L3)

SuperGrok is paid. Meter copy in this report is **included SuperGrok period limits**. Never "free SuperGrok."

Screenshot used (session asset; `/home/workdir/attachments/image.jpg` was missing):

`/home/hunter/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/assets/image-e47e9a03-9f0a-49d3-aeca-1b1b1564c072.jpg`

This is the live grok-build TUI on Fri Aug 14, not the earlier Colibri disaster. The transcript in that same frame already says live windows stay on the old 1.0.3 binary until a successful `/rebuild` and a full quit/reopen. This slice did **not** run `/rebuild`.

## Honest split

A lot of DOGE role paint is **already correct in source** and is **already visible in the screenshot**: titled composer, Human-green box caret, magenta model label, cyan included SuperGrok period limits meter, white assistant body, cyan inline code, yellow timestamps, yellow "Thought for 1.9s" / tool diamonds, green `>` prefix.

What still looked bad in the live frame, and was a **real source hole**, is the **all-yellow titled composer frame**. FORK / `Theme::doge()` say the box is `prompt_border_active` (white). Prior restore reports left the titled top rule remapped to `theme.gray` (yellow) so the title would contrast. That made the box read as context chrome, not a frame. This slice flipped that: **white frame, yellow title**.

The screenshot also paints **yellow sides and bottom** on the composer, and an all-yellow footer with ASCII ` | `. Current source already paints sides/bottom as `prompt_border_active` (white) and footer keys as `text_secondary` (white, bold) with box-drawing `  │  `. Those two screenshot details are the live binary, not remaining source remaps. I did not invent a second product change for them.

Human left rails are **not missing in source**. The screenshot has no human transcript lines (the draft is inside the composer), so green `┃` would not appear there.

## Screenshot vs source vs FORK

| Surface | Screenshot (live TUI) | Source now | Action |
|---------|----------------------|------------|--------|
| Compact limits meter | Cyan `92K / 500K \| included SuperGrok period limits · ...% \| 301/314` | `credit_bar` + status push already paint that copy | No churn. Already matches FORK. |
| Composer title | Yellow `Grok OSS` on the top rule | Title uses `theme.gray` (yellow) when caption would vanish into the white rule | Intended. Title stays context chrome. |
| Composer frame | Entire box yellow (top, sides, bottom) | Focused frame is `prompt_border_active` (white on DOGE). **Was:** titled top `─` remapped to `theme.gray`. **Now:** top/sides/bottom stay white. | **Fixed source** for the titled top rule. Sides/bottom were already white in source; live yellow sides are the old binary. |
| Composer frame as Human green | Not green (good) | Must not be `accent_user` | Guarded in the new test. Did not invent a green box. |
| Composer prefix `>` | Green | `style.accent_color` / `accent_user` | Already correct. |
| Box caret | Green block at end of draft | `paint_composer_box_cursor` uses `accent_user`; mid-draft empty blink is `text_primary` | Already correct. Tests still green. |
| Model · flag | Magenta `Grok 4.6 (xhigh)` · yellow `always-approve` | `accent_model` + `theme.gray` | Already correct. |
| Composer left `┃` | None (boxed composer) | `show_accent_line: false` in agent view | By design. Rails live on scrollback entries, not inside the box. |
| Footer hints | All yellow, ASCII ` \| ` | `ShortcutsBar`: keys `text_secondary`+BOLD (white), labels `theme.gray` (yellow), sep `  │  ` | No source edit. Live all-yellow / ASCII is the old binary. |
| Assistant body | White | `md_text` / `text_primary` | Already correct. |
| Inline code | Cyan (`hide_header`, `/settings`, paths) | `md_code` | Already correct. Role mix, not a random flip. |
| Right timestamps | Yellow | `theme.gray` in scrollback overlay | FORK context/time. No churn. |
| `Thought for 1.9s` / tool `Read 4 files` | Yellow diamond + yellow label | Collapsed thinking uses `theme.muted()` = `theme.gray`. Tool collapsed header same family. | FORK timer/context. Not agent magenta. |
| Human left `┃` | Not in frame (no human lines) | `UserPromptBlock::accent()` + `EntryRenderer` paint `accent_user` | Source already paints. New paint-through test passed without product edit. |
| Agent left `┃` while running | Not in frame (idle) | Magenta only while `ctx.is_running`; finished is `None` | Already matches FORK. |
| Last assistant paragraph yellow left `│` | Visible in the JPEG | Finished agent `accent()` is `None`. `selection_border` is **white** on DOGE. | Ruled out finished-agent rail and source selection color. I do **not** know why the live line is yellow. Not churning. |
| Whole human line neon green | Not in this shot | Must not | Not this hole. |
| Caret flipped magenta | No | Must not | Already guarded. |
| "free SuperGrok" | Not painted | Compact copy is included SuperGrok period limits | Already correct. |

## What I changed (one source hole)

Titled DOGE composer used to remap the **top rule** to `theme.gray` when chrome-caption fg equaled `prompt_border` (white-on-white after DOGE solid-step). That is why a titled box looked like a yellow context frame.

**Now:** the rule stays `border_color` (`prompt_border_active`). Only the session **name** steps to `theme.gray` (or `gray_dim` if gray collided) so the title still contrasts. Sides and bottom were already `border_color`.

Did **not** paint the box Human green. FORK Human green is caret, rails, success, OSC 12. The composer frame is white.

## TDD

### 1. Titled composer frame (product fix)

**Named contract:** On DOGE, a focused titled composer (`Grok OSS` + model + `always-approve`) paints top/side/bottom frame `prompt_border_active` (white), session title `theme.gray` (yellow), prefix `accent_user` (green). Not context yellow on the rule. Not Human green. Not agent magenta.

**Red (test first, product still remapping the top rule):**

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-chrome-leftover-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib --offline -- \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow
```

Failed: top rule `Rgb(255, 255, 0)` vs expected white `Rgb(255, 255, 255)`.

**Product:** `views/prompt_widget/mod.rs` titled top divider. Keep `div_style` on `border_color`. Contrast the title only.

**Green (same filter, plus neighbors):**

```
cargo test -p xai-grok-pager --lib --offline -- \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  title_renders_on_top_border_with_corners_intact \
  no_title_keeps_plain_top_border \
  user_prompt_entry_renderer_paints_green_rail \
  info_line_model_name_uses_accent_model_not_gray \
  user_prompt_block_accent \
  agent_message_block_accent \
  paint_composer_box_cursor_uses_human_green \
  focused_composer_paints_human_green_box_caret
# 12 passed
```

`title_renders_on_top_border_with_corners_intact` is the GrokNight caption-blend contract. After the DOGE pin, it raced `Theme::current()` and expected DOGE yellow while paint used Reset. **Test isolation only:** that test now `pin_theme()` (GrokNight). Asserts were not weakened.

### 2. Human rail paint-through (no product edit)

**Named contract:** `EntryRenderer` actually paints left `┃` in `accent_user` for a user prompt, not only `accent()` returning green.

**Red:** wrote `user_prompt_entry_renderer_paints_green_rail` first. It **passed** on first run. Source already paints the rail. No product churn.

## Verify

```
cargo fmt -p xai-grok-pager
# exit 0

cargo clippy -p xai-grok-pager --lib --offline -- -D warnings
# exit 0  (Finished in ~19s on the isolated target)

# named tests above: 12 passed
```

Isolated dirs: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-chrome-leftover-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

## Files

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs` | Titled composer: white frame, yellow title only |
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/tests.rs` | New DOGE frame test; GrokNight title test pinned |
| `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs` | Paint-through rail test only |

## Leftovers (only what I actually checked)

- **Live TUI.** This window will still look like the screenshot until someone rebuilds and fully quits/reopens. Not done here.
- **Footer keys.** Source already splits key vs label color. Screenshot all-yellow + ASCII ` | ` is the old binary. No product edit.
- **Human rails.** Source already green via `EntryRenderer`. Not visible in this screenshot because there are no human transcript lines.
- **Agent rails while running.** Source already magenta only while the turn is active. Screenshot is idle.
- **Thinking / tool yellow.** Matches FORK context/time (`theme.muted()` / `theme.gray`). Not a hole.
- **Composer `show_accent_line`.** Left false on purpose (boxed composer).
- **Yellow `│` on the last assistant paragraph.** Ruled out finished-agent rail (`None`) and source `selection_border` (white). I do not know the live cause. Did not invent a remapping.
- **Clarify human line, plan PTY e2e, host `~/.grok/docs` extract.** Not re-checked this slice. The screenshot leftover list names them; I did not open those paths again.

Did not touch CLI `--version` / `grok --resume` branding. Did not run crate-wide `fmt --all`.
