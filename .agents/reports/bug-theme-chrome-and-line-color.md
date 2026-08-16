# Theme chrome + inconsistent line coloring (2026-08-13)

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 implementer  
**Contract:** restore FORK DOGE chrome that 1.0.3 restack dropped; do not invent a new theme or flip the caret to magenta.

## What was actually missing vs FORK

Docs said the rails shipped. Code on first read did not.

| FORK / AGENTS chrome | First-read code | After this slice |
|----------------------|-----------------|------------------|
| Human left `┃` is static `accent_user` (green on DOGE; Cyan when Reset) | `UserPromptBlock::accent()` returned `None` | Restored. All prompt kinds (plain / bash / skill / cron / interjection / mid-text tokens). |
| Agent left `┃` is static `accent_running` (magenta on DOGE) **only while `ctx.is_running`** | `AgentMessageBlock::accent()` returned `None` | Restored. Finished turns stay rail-less. |
| Info-line model label uses `accent_model` (magenta on DOGE) | `render_info_line` painted `chrome_caption_style` (DOGE focused → white / washed caption) | Focused model uses `accent_model`; unfocused blends that same token. |
| Catalog tests `user_prompt_block_accent_*` / `user_prompt_prefix_matches_*` | **Gone** from `*.rs` (2026-08-11 catalog reported 4 PASS; recap test was the only survivor) | Restored those three names + agent rail tests. |
| Composer caret Human green / mid-draft `text_primary` empty blink | Present (not this slice) | Untouched. |
| Titled composer border (`╭─╮` + session title) | Present | Untouched. Title stays caption blend. |
| Status-bar credit / limits compact meter | Present in `credit_bar.rs` | Untouched. |
| Recap idle `accent_tool` | Present (`recap_accent_and_bullet_use_neutral_tool_color_when_idle`) | Untouched. |

Did **not** invent new decorations, a new theme, or a magenta caret.

## What the inconsistent colors were

Screenshot (~10:51 Thu Aug 13) vs code:

1. **Same assistant paragraph: some identifiers cyan, some bold white.**  
   Not a missing highlighter on prose. Markdown roles: `` `inline code` `` → `md_code` (cyan + bold); `**strong**` / plain → `md_text` (white). That mix is role, not a random flip. `doge_markdown_same_role_spans_share_one_token` pins the map. Did not flatten markdown.

2. **Code-fence identifiers mixing cyan and white on one line.**  
   Real bug: `doge.tmTheme` had two `Object Property` rules (`variable.other.property*` = `#00FFFF`, `variable.other.object.property` = `#FFFFFF`). Unified the second rule to `#00FFFF` so the same role is one token.

3. **Human path line teal.**  
   `theme.path` / `accent_system` are cyan on DOGE. Intended system/path family. Not a rail bug.

4. **Yellow “Grok OSS” titled border.**  
   Existing DOGE contrast: title uses `chrome_caption_style`; when that fg matches the border token, the rule steps to `theme.gray` (yellow). Left as-is.

5. **Status “Grok 4.6 (xhigh) · always-approve” yellow/magenta.**  
   Flags use `theme.gray` (yellow). Model is supposed to be magenta (`accent_model`). After restack the model sat on caption white, so the row looked washed. Restored magenta model; flags stay yellow.

## TDD (red → green)

**Red evidence (before product restore):**

- First read: both `accent()` impls were `None`. Catalog names were absent.
- Tests written first (catalog names + `info_line_model_name_uses_accent_model_not_gray` + Object Property same-token + markdown same-role).
- First `cargo test -p xai-grok-pager --lib` did **not** reach those asserts: concurrent `/rebuild` work failed compile (`post_restore_relaunch_action` / `rebuild_relaunch` on `app/mod.rs`). That crate compiled later without edits from this slice.
- `doge_tmtheme_object_property_rules_share_one_foreground` would fail on `#FFFFFF` vs `#00FFFF` (file had both before the one-line unify).
- `doge_markdown_same_role_spans_share_one_token` first run failed because the test compared unquantized `Theme::doge()` to `style()` / `Theme::current()` (NO_COLOR → Reset / `None`). Product map was already consistent. Test now compares `style()` to **live** `Theme::current()` tokens; palette uniqueness still asserted on `Theme::doge()`.

**Green (same filters, after restore):**

```
cargo test -p xai-grok-pager --lib --offline -- \
  user_prompt_block_accent user_prompt_prefix_matches \
  agent_message_block_accent recap_accent \
  info_line_model_name_uses_accent_model
# 8 passed

cargo test -p xai-grok-pager --lib --offline -- \
  render_block_agent_message_accent_color
# 1 passed

cargo test -p xai-grok-pager-render --lib --offline -- \
  doge_tmtheme_object_property_rules_share_one_foreground \
  doge_markdown_same_role_spans_share_one_token \
  doge_accent_user_is_pure_green doge_accent_model_is_pure_magenta \
  default_theme_is_doge
# 5 passed
```

## Product edits (smallest)

| File | Change |
|------|--------|
| `xai-grok-pager` `scrollback/blocks/user.rs` | Restore static Human rail + catalog tests. |
| `xai-grok-pager` `scrollback/blocks/agent.rs` | Restore running-only Agent rail + tests. |
| `xai-grok-pager` `views/prompt_widget/mod.rs` | Model label uses `accent_model`. Title caption unchanged. |
| `xai-grok-pager` `views/prompt_widget/tests.rs` | Restore `info_line_model_name_uses_accent_model_not_gray`. |
| `xai-grok-pager-render` `assets/doge.tmTheme` | Both Object Property rules `#00FFFF`. |
| `xai-grok-pager-render` `syntax.rs` / `theme/md_style.rs` | Same-role tests. |

Did **not** edit `rebuild.rs`, justfile, or install.

## Verify

```
cargo fmt -p xai-grok-pager -p xai-grok-pager-render
cargo clippy -p xai-grok-pager-render --lib --bins --offline -- -D warnings   # ok
cargo clippy -p xai-grok-pager --lib --bins --offline -- -D warnings           # ok
```

## Leftover honesty

- **Live TUI will not show this until rebuild + reopen.** This session is still the old binary. Another implementer owns `/rebuild` failure recovery; this slice does not install.
- **pager-minimal** still sets `hide_accent` for non-thinking rows (flush-left). Fullscreen / default DOGE session is the FORK rail contract. Minimal dim thinking rail was already restored (2026-08-12 report).
- `RenderBlock::accent_color()` for finished `AgentMessage` stays `None` (finish-flash lookup, not the live rail). Live paint uses `block.accent(&ctx)`.
- Composer caret / OSC 12 / mid-draft letter ink were not in the broken paint path; not re-tested here.
- Credit / limits footer treatment was not dropped; no new footer invented.
