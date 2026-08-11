# Composer undeletable yellow dot (plan comment mode)

**Board:** `bug:composer-undeletable-dot`
**Date:** 2026-08-09
**Tree:** `/home/hunter/Projects/surmount/grok-build`

## Operator report

Plan panel open, plan comment mode (footer `Enter:save comment | Esc:cancel`, status `commenting L13`). Bottom composer has a yellow filled circle on the left inside the yellow composer border. Operator cannot delete it.

## Root cause (plain English)

That circle was **not** typed text and **not** the caret.

It was **composer prefix chrome** for plan line-comment mode: the product painted a yellow filled circle (`●`, U+25CF; legacy `•`) instead of the normal prompt arrow (`❯`), in the same place bash uses `!`, feedback uses `~`, and remember uses `#`.

Because the prefix is drawn next to the textarea and is **not** in the edit buffer, Backspace / Delete never remove it. Status already said `commenting L#`, the outline was already plan yellow, and the placeholder already said `Type your comment...`, so the filled circle mainly looked like a stuck character.

| Piece | Role |
|-------|------|
| Yellow box outline | Plan / comment surface tint (`border_color_override` + `accent_plan`) |
| Left glyph | **Was** mode prefix `●` (chrome). **Now** normal prompt arrow `❯` (still chrome) |
| Green/blinking block | Separate software caret on the insertion cell (Human green), not this yellow circle |
| `●` on plan body lines | Still used for **saved** line comments in the plan viewer; unrelated to the composer prefix |

### Files (before / after)

| File | What |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Built `PromptStyle` for the agent composer. Comment / casual-comment used to set `prefix_override` to yellow `● `. **Fixed:** comment mode no longer overrides the prefix; uses normal `❯` with plan-yellow accent. Regression test added. |
| `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs` | Paints `prefix_override` or default `prompt_arrow()` left of the textarea (unchanged). |
| `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` | Plan-body comment bullets still use `filled_dot()` (intentional; not the composer bug). |

## Intentional vs bug

- **Was intentional UI** (mode marker), **bad UX** (reads as undeletable text).
- **Not a stuck glyph, zero-width junk, or overlay bug.**
- **Fix:** keep mode clear via yellow outline + `commenting L#` + placeholder; stop using `●` as left chrome so the left side matches normal undeletable prompt chrome.

## Product change

In `AgentView` composer style build:

- **Removed** the comment/casual-comment branch that set `prefix_override` to yellow `● `.
- **Kept** plan-yellow border/accent and `"Type your comment..."` placeholder.
- Comment mode now uses the same left arrow as ordinary plan-tinted compose.

## Tests

```text
cargo test -p xai-grok-pager --lib -- plan_commenting_composer_prefix_is_prompt_arrow_not_filled_dot
# ok

cargo test -p xai-grok-pager --lib -- plan_commenting_composer_prefix prompt_outline_plan soft_park_draw doge_plan_mode_prompt
# 10 passed
```

New contract: `plan_commenting_composer_prefix_is_prompt_arrow_not_filled_dot`
(`prompt_outline_plan_view_tests` in `render.rs`)

### Post-impl verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager` | ok |
| Targeted tests above | ok |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | Pre-existing failures outside this change (e.g. dead code in `queue.rs`, canonicalize in tests). No new lint pointed at the comment-prefix edit. |

## Operator action

Rebuild and relaunch the pager binary you dogfood (same path you used for the screenshot). No config change. After relaunch, plan comment mode should show a yellow **prompt arrow** on the left, not a filled circle. The arrow is still chrome and still cannot be deleted; that is expected for the prompt prefix.
