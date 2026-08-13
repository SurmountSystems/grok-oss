# Implement: panel plan-approval footer five Surmount CTAs (2026-08-11)

## Goal

Make three soft-park panel/fullscreen approval-footer tests green without breaking strip fallback.

## Red (observed)

```
cargo test -p xai-grok-pager --lib soft_park_draw_paints_panel_approval_footer_chrome
```

- `soft_park_draw_paints_panel_approval_footer_chrome` **FAILED**
  - assert: panel footer must expose all five approval CTA hit targets after soft-park draw
  - missing: `approve_notes_button_area`, `questions_button_area` (old approve / request-changes / comment / quit path)

## Root cause

`crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` modal footer (step 8) still painted the old four-action layout on `feedback_active`:

- `a approve` / `s request changes` / `c comment` / `q quit plan`
- never set `approve_notes_button_area` or `questions_button_area`
- always set `comment_button_area` (tests require `None` while approval parked)

Soft-park strip already uses `paint_soft_park_cta_buttons` (a / A notes / ? clarify / s revise / q quit) and is the fallback when the panel has no approval hits.

## Product fix (minimal)

In `line_viewer.rs` footer paint:

**Approval mode (`feedback_active`):** call `paint_soft_park_cta_buttons` on the footer row and map hit rects:

| SoftParkCtaAreas | PlanViewerExtras |
|------------------|------------------|
| approve | approve_button_area |
| notes | approve_notes_button_area |
| clarify | questions_button_area |
| revise | send_button_area |
| quit | abandon_button_area |

Clear `comment_button_area` and `copy_button_area` while approval is parked.

**Casual mode:** keep comment / send / copy footer; clear notes / questions / send / abandon hit areas.

## Green (same filters + strip safety)

```
cargo test -p xai-grok-pager --lib soft_park
# 48 passed; 0 failed
```

Named contracts (all ok):

- `soft_park_draw_paints_panel_approval_footer_chrome`
- `soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared`
- `soft_park_fullscreen_draw_paints_approval_ctas`
- `soft_park_draw_falls_back_to_strip_ctas_when_panel_cannot_paint` (kept)
- `soft_park_draw_strip_ctas_when_panel_dismissed` (kept)
- exit_plan_mode soft-park tests under the same `soft_park` filter (kept)

## fmt

`rustfmt --edition 2024` on the touched file (package `cargo fmt -p` blocked by unrelated missing pty e2e mod). No further style drift on `line_viewer.rs`.

## Files touched

- `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs`

## Not done

- No commit / push
- No recon stash drops
