# Plan CTA residual — approve_plan_flush green (2026-08-12)

## Goal

Green the remaining **7** `approve_plan_flush_tests` plan decision surface fails
(casual `c-comment` footer only; missing five Approve/Notes/Clarify/Revise/Quit
hits; missing top-bar `⧉` copy hit). Cluster B from
`.agents/reports/bug-pager-residual-live-2026-08-11.md`.

## Root cause

`crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` had regressed
to the monorepo/casual paint path:

| Missing | Effect |
|---------|--------|
| Side-panel geometry (`side_panel_rect`, `else if viewer.side_panel`) | Plan dock painted as dimmed 75% popup |
| Five-CTA approval footer (a / A / ? / s / q) | Footer painted casual `c comment` + legacy 3-button row |
| Top-bar `⧉` copy next to `↗`/`✗` | `LineViewerState::copy_button_area` never set on paint |
| `build_shortcut_button_key_only` + compact label fallthrough | Narrow side panels dropped all CTA hits |

AgentView plan park / `sync_plan_viewer_approval_chrome` already armed
`feedback_active`; paint was the broken surface.

## Product fix

Restored Surmount plan decision surface in `line_viewer.rs` from the known-good
implementation (historical Surmount commit `a1515fe1`):

1. **`side_panel_rect`** + side-panel branch in `render_line_viewer` (right dock ~45%, no dim).
2. **Top bar:** `[⧉][↗][✗]` cluster; plan-review omits close; sets `viewer.copy_button_area`.
3. **Approval footer:** five clickable CTAs with full → compact → key-only packing;
   `comment_button_area = None` while `feedback_active`.
4. **Casual footer:** `c comment` [badge] + optional `s send` only.
5. Unit tests restored: `side_panel_rect_*`, `line_viewer_top_bar_sets_copy_button_hit_area`,
   `plan_approval_narrow_side_panel_footer_sets_cta_hit_areas`.

No changes to ACP wire outcomes, `approve_plan` / revise flush logic, or soft-park
strip paint (`paint_soft_park_cta_buttons` was already five-CTA).

## Red → green

### Red (pre-fix, live resample)

From residual live report: 7 fails under `approve_plan_flush_tests` with:

- `must not paint casual c-comment as the only footer while decision is pending`
- `panel footer must expose all five approval CTA hit targets after soft-park draw`
- `painted plan top bar must set ⧉ hit target`

### Green (post-fix)

```text
cargo test -p xai-grok-pager --lib 'approve_plan_flush_tests' -- --test-threads=8
# test result: ok. 118 passed; 0 failed; … finished in 0.74s

cargo test -p xai-grok-pager --lib 'file_search::line_viewer::tests::' -- --test-threads=8
# test result: ok. 14 passed; 0 failed; …
```

Named residual seven (all green inside the 118):

1. `idle_plan_decision_draw_paints_approve_and_revise_ctas`
2. `idle_plan_view_only_panel_draw_self_heals_to_approval_ctas`
3. `plan_preview_copy_button_click_copies_whole_plan_body`
4. `soft_park_draw_paints_panel_approval_footer_chrome`
5. `soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared`
6. `soft_park_fullscreen_draw_paints_approval_ctas`
7. `view_plan_while_plan_mode_awaiting_decision_parks_ctas_not_view_only`

### Verify

- `cargo fmt -p xai-grok-pager` — done
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` — blocked on pre-existing
  dependency fails in `xai-grok-tools` (dead_code + disallowed `Command::spawn`),
  not on `line_viewer.rs`

## Files touched

- `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs`

## Contract reminder (FORK / user-guide)

- Soft-park / panel present ≠ Approve
- Five CTAs: Approve · Notes · Clarify · Revise · Quit (mouse primary; empty-prompt keys)
- Empty freeform Enter never approves
- Casual `/view-plan` keeps `c comment`; idle/plan-mode decision park must not
