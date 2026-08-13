# bug: xai-grok-pager-minimal API drift after onto

**Date:** 2026-08-11
**Package:** `xai-grok-pager-minimal` (+ one restored method on `xai-grok-pager` `ScrollbackState`)

## Problem

Onto / pager API drift broke compile of `xai-grok-pager-minimal`:

1. `EntryRenderer::with_dim_accent` missing
2. `PromptStyle` missing field `bg_override` in struct literals
3. `ScrollbackState::insert_block_before` missing

## API mapping (old → new / fix)

| Call site | Old | Current product API / fix | Semantics |
|-----------|-----|---------------------------|-----------|
| `commit.rs` `minimal_renderer` | `.with_dim_accent(true)` | **Removed** (no replacement builder). Keep `.with_hide_accent(hide_accent)`. Collapsed / non-thinking reclaim the accent column; remaining thinking rail uses appearance `scrollback.display.dim_accent` blend on collapsed accents. | Old API painted accent with `Modifier::DIM` for `Color::Reset` terminal-native palette. That flag was dropped from `EntryRenderer` during onto. Blind rename to `with_hide_accent` would **change** semantics (hide vs dim). |
| `live.rs` / `overlay.rs` / `plan.rs` `PromptStyle { ... }` | no `bg_override` field | `bg_override: None` | Matches `PromptStyle::default()` / `PromptStyle::inline` — optional solid chrome fill override; minimal surfaces do not set it. |
| `plan.rs` + `commit_tests.rs` | `ScrollbackState::insert_block_before(anchor, block)` | **Restored** on `ScrollbackState` (same public name) | Mid-list insert before a still-running `exit_plan_mode` tool so the plan body sits above the tool and the minimal commit frontier can print it while approval is parked. `entries` is private; consumer-side cannot reimplement. |

## Edits

### Consumer (`xai-grok-pager-minimal`)

- `src/commit.rs` — drop `.with_dim_accent(true)`; comment notes mapping.
- `src/live.rs` — add `bg_override: None` to live `PromptStyle`.
- `src/overlay.rs` — add `bg_override: None` to `inline_input_style`.
- `src/plan.rs` — add `bg_override: None` to `input_style`. Call sites for `insert_block_before` unchanged.

### Pager (`xai-grok-pager`) — required for private state

- `src/scrollback/state/mod.rs` — restore `ScrollbackState::insert_block_before`:
  - Fallback to `push_block` if anchor missing.
  - `debug_assert` anchor not already committed (native scrollback is append-only).
  - `shift_insert` at anchor index; bump selection when at/after insert.
  - Clamp `commit_scan_cursor` to `min(index)` so the new uncommitted entry is never below the scan cursor.
  - Same Edit / Thinking display-mode defaults as `push`.
  - No `arm_structural_scroll_anchor` (that helper was removed in onto); layout invalidated the usual way.

## Verify

| Command | Exit |
|---------|------|
| `cargo check -p xai-grok-pager-minimal --all-targets` | **0** |
| `cargo clippy -p xai-grok-pager-minimal --all-targets -- -D warnings` | **0** |
| `cargo fmt -p xai-grok-pager-minimal` (+ pager `state/mod.rs`) | **0** |

Targeted lib tests for plan anchoring may be re-run with:

```bash
cargo test -p xai-grok-pager-minimal --lib -- plan_body_anchored anchored_plan_body revised_plan_anchors
```

## Notes

- Prefer consumer adapt when possible; `insert_block_before` had to live on `ScrollbackState` because `entries: IndexMap` is private and mid-list insert is load-bearing for minimal plan commit.
- Do **not** map `with_dim_accent` → `with_hide_accent` for the always-true dim call; hide already keys off thinking/collapsed.
