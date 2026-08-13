# Pager layout / ACP / singletons / slash residual — 2026-08-12

**Agent:** L2 implementer
**Source residual:** `.agents/reports/bug-pager-residual-live-2026-08-11.md` clusters D, C, F (partial), E (partial)
**Package:** `xai-grok-pager`
**No git commit.**

## Goal

Green remaining clusters:

| Cluster | Named reds |
|---------|------------|
| scrollback layout | growth / removal / wrapped park viewport pin (~3) |
| acp_handler | error marker, /loop SessionLoaded, /share kill-switch, wake marker (~4) |
| singletons | privacy banner, palette cursor, turn_end park (~3 of 5; interactions + settings bg out of scope or already green) |
| slash leftovers | dashboard minimal visibility, undo shell reserve (~2) |

Out of scope (other owners): `key_owner` render bar (~16), plan CTA deep rewrites (~7).

## Product roots and fixes

### Layout — structural anchor + stream compensate

Helpers `arm_structural_scroll_anchor` / `migrate_structural_anchor_past_removal` / `apply_structural_scroll_anchor` existed but were never wired.

| Site | Fix |
|------|-----|
| `remove_entry` | Arm before mutate; migrate after `shift_remove` |
| `remove_from` | Arm before bulk pop; prune dead anchor after |
| `prepare_layout` Case 1 | Take + apply structural after same-width rebuild (drop on width change) |
| Case 2 streaming | `compensate_scroll_for_upstream_height_deltas` when height delta is fully above manual viewport top |

**Files:** `scrollback/state/mod.rs`, `scrollback/state/layout.rs`

### ACP / share / wake / error marker

| Red | Root | Fix |
|-----|------|-----|
| `/share` kill-switch | `set_share_visible` used hard `hidden` | Menu-only: seed `menu_hidden` with `"share"`; `set_share_visible` toggles menu only, always clears hard hide |
| Failed wake marker | Silent `error` still pushed `TurnFailed` | `finish_wake_turn`: silent `error` \| `rate_limit` → `None` (same as cancelled silence) |
| Viewer error marker | Test expected raw `"boom"`; product uses `format_request_failure` | Keep product format (matches `viewer_finalize_stop_reason_to_marker_mapping`); align queue test to formatted contract |
| `/loop` SessionLoaded | Product adopt path OK; host `~/.grok/sessions/%2Ftmp/sess-loop-load/canceled_turn_resume.json` auto-resumed and blocked drain | Test clears cancel-resume marker at start (hermetic) |

**Files:** `slash/registry.rs`, `acp_handler/prompt_origin.rs`, `acp_handler/tests/queue_and_adoption.rs`, `acp_handler/tests/settings.rs`

### Singletons

| Red | Root | Fix |
|-----|------|-----|
| Privacy banner | Slot height 2 &lt; `MIN_HEIGHT` 3 so paint skipped | `app_view` allocates `privacy_banner::height(w).max(MIN_HEIGHT)`; agent `draw` also floors privacy slot |
| Turn park | `renders_parked` needs parked-marker slot; test only simulated wait | Call `maybe_push_parked_marker` after wait (matches ACP/draw production) |
| Palette cursor | False positive: `text_primary == Reset` under terminal-native makes every Buffer cell look like inverse cursor | Pin `GrokNight` in palette + picker cursor tests |

**Files:** `app/app_view.rs`, `agent_view/render.rs`, `turn_completion/tests.rs`, `app/modals.rs`, `views/picker.rs`

### Slash

| Red | Root | Fix |
|-----|------|-----|
| Dashboard in minimal | Default `visible()` always true | `DashboardCommand::visible` → `!screen_mode.is_minimal()` |
| Unreserved `undo` | Rewind alias missing from `SHELL_RESERVED` | Add `"undo"` |

**Files:** `slash/commands/dashboard.rs`, `slash/commands/mod.rs`

## Verification (max nice)

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8 \
  growing_entry_above_manual_viewport \
  removing_entry_above_manual_viewport \
  removal_above_wrapped_park \
  viewer_prompt_complete_error_pushes \
  loop_fire_mode_adopts \
  settings_update_sharing_enabled_true \
  failed_wake_turn_keeps \
  privacy_banner_owns_slot \
  command_palette_search_bar_cursor \
  turn_end_after_park \
  picker_highlights_current_choice \
  visible_everywhere_except_minimal \
  shell_collision_contract \
  set_share_visible_hides \
  get_for_dispatch_respects_hard \
  search_bar_cursor_visible_only_when_search_active \
  viewer_finalize_stop_reason_to_marker_mapping
```

**Result: 17 passed; 0 failed.**

Also: `cargo fmt -p xai-grok-pager`.

## Remaining outside this mop

- `agent_view::key_owner` (~16) + interactions bar (if still red) — other track
- `approve_plan_flush_tests` (~7) — other track
- Full `--lib` residual count not re-sampled this wave

## 5-line summary

1. Wired structural scroll anchors + upstream height compensate; layout 3 green.
2. Share menu_hidden kill-switch; silent wake markerless; viewer TurnFailed stays formatted.
3. Privacy slot height floor; park test calls `maybe_push`; palette tests pin non-Reset theme.
4. Dashboard `visible` hides minimal; `undo` reserved; loop SessionLoaded test hermetic vs cancel-resume disk.
5. Cluster filters 17/17 green; fmt done; no commit.
