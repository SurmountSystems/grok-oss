# bug: pager jump/timeline mode_support FullscreenOnly

**Date:** 2026-08-11
**Crate:** `xai-grok-pager`
**Status:** green

## Contract

`/jump` and `/timeline` must refuse in minimal mode via `ModeSupport::FullscreenOnly`, not only the legacy `available_in_minimal` flag. Central dispatch and completion gate on `mode_support()`, so a bare `available_in_minimal = false` left default `ModeSupport::Both` and skipped the pinned refusal.

Pinned refusals (`slash::mode_support::tests::mode_specific_builtin_refusals_are_pinned`):

- `/jump isn't available in minimal mode (minimal scrolls with your terminal's native scrollback). Run /fullscreen to switch this session.`
- `/timeline isn't available in minimal mode (the timeline rail needs the interactive scrollback pane). Run /fullscreen to switch this session.`

## Red (observed)

```text
cargo test -p xai-grok-pager --lib mode_support -- --test-threads=8
# slash::mode_support::tests::mode_specific_builtin_refusals_are_pinned FAILED
# actual list missing jump + timeline (mode_support defaulted to Both)
# 5 passed; 1 failed
```

## Product fix

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/slash/commands/jump.rs` | Replace `available_in_minimal` with `mode_support() -> FullscreenOnly(SwitchMode { why: "minimal scrolls with your terminal's native scrollback" })` |
| `crates/codegen/xai-grok-pager/src/slash/commands/timeline.rs` | Same pattern; why: `"the timeline rail needs the interactive scrollback pane"` |

Local unit tests renamed to `fullscreen_only_in_minimal` and assert `mode_support().supports(...)`. Matches peers (`find`, `theme`, `dashboard`).

## Green

```text
cargo test -p xai-grok-pager --lib mode_support -- --test-threads=8
# 6 passed; 0 failed

cargo test -p xai-grok-pager --lib fullscreen_only_in_minimal -- --test-threads=8
# 2 passed (jump + timeline)

cargo fmt -p xai-grok-pager
# clean
```

Clippy `-p xai-grok-pager --lib -D warnings` blocked on pre-existing `xai-grok-tools` dead_code / disallowed `Command::spawn` (not this slice).
