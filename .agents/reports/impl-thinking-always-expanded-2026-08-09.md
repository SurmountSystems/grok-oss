# Report: always expand thinking + hide Ctrl-e when on

**Date:** 2026-08-09
**Branch:** `fixes-2`
**Board:** `feat:thinking-always-expanded`

## Outcome

Shipped `[ui].always_expand_thinking` (default **off**).

When **on**:
- New streaming thinking opens fully expanded (not Truncated).
- Finished thinking stays Expanded (setting wins over sticky Ctrl+E mode).
- Turning the setting on expands existing thinking blocks immediately.
- Footer / chrome **omits** the Ctrl+E expand/collapse thinking hint (prompt + scrollback).
- Settings modal: **Always expand thinking** under Appearance (shell-owned).

When **off**: current collapse-on-finish + Ctrl+E footer affordance unchanged.

## Config

```toml
[ui]
always_expand_thinking = false   # default; true = always expanded + hide Ctrl+E hint
```

Also: env `GROK_ALWAYS_EXPAND_THINKING`, settings UI row, process cache, resolve at startup.

## Key touch points

| Area | Path |
|------|------|
| UiConfig field | `crates/codegen/xai-grok-shared/src/ui_config.rs` |
| Process cache | `crates/codegen/xai-grok-pager-render/src/appearance/cache.rs` |
| Resolve + env | `crates/codegen/xai-grok-shell/src/util/config/resolve/ui.rs` |
| Persist write | `…/settings_writes.rs` → `set_always_expand_thinking` |
| Settings meta | `xai-grok-pager/src/settings/defs.rs` |
| Live value / drift | `…/settings/registry.rs` |
| Action + setter | `Action::SetAlwaysExpandThinking`, `set_always_expand_thinking*` |
| Startup seed | `app/event_loop.rs` |
| Persist effect | `app/effects/helpers.rs` |
| Scrollback sticky + push/finish | `scrollback/state/mod.rs`, `selection.rs` (`apply_always_expand_thinking`) |
| Footer hide Ctrl+E | `views/agent.rs` `build_hints` |
| Docs | `docs/user-guide/05-configuration.md`, `03-keyboard-shortcuts.md` |

## TDD (observed green)

```
cargo test -p xai-grok-pager-render --lib always_expand
  set_then_load_round_trips_always_expand_thinking

cargo test -p xai-grok-shell --lib always_expand
  defaults_off / user_config / env_overrides

cargo test -p xai-grok-pager --lib always_expand_thinking
  always_expand_thinking_opens_streaming_and_keeps_finish_expanded
  apply_always_expand_thinking_opens_existing_collapsed
  always_expand_thinking_hides_ctrl_e_footer_hint
  set_always_expand_thinking_applies_persists_and_rolls_back
  set_always_expand_thinking_expands_existing_thinking

cargo test -p xai-grok-pager --lib ctrl_e_thinking_hint
  prompt + scrollback footer labels (still show when setting off)

cargo test -p xai-grok-pager --test settings_e2e always_expand
  always_expand_thinking_space_dispatches_typed_setter

cargo test -p xai-grok-pager --test settings_e2e show_thinking_blocks_renders
  Appearance order: show_thinking → always_expand → respect_manual_folds
```

All listed filters **passed**.

## Verify

- `cargo fmt` on touched packages.
- Clippy lib/`--all-targets` on the full pager/shell tree hits **pre-existing** warnings unrelated to this slice (queue dead_code, mouse private field mid-flight from parallel work, etc.). Feature-specific unit tests compiled and passed.

## Not done / out of scope

- No remote-settings tier for this flag (local config + env only).
- Ctrl+E chord still bound while the setting is on (only the **footer label** is hidden); sticky toggle can still change existing blocks until finish/new push re-applies expand.
- No git commit/stage/push.
