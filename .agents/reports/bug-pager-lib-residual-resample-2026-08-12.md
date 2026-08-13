# pager lib residual resample (2026-08-12)

Live re-sample of `xai-grok-pager --lib` after layout / acp / singletons + key_owner + plan CTA mops. Residual was small (3 fails); all fixed same wave.

## Counts

| Wave | Passed | Failed | Ignored | Wall |
|------|--------|--------|---------|------|
| **Before** (full lib, `--test-threads=8`) | **8810** | **3** | 11 | 13.54s |
| **After** (full lib, `--test-threads=8`) | **8813** | **0** | 11 | 12.33s |

Before log: `/tmp/pager-lib-resample2.txt`
After log: `/tmp/pager-lib-resample2-after.txt`

## Remaining fails before fix (full list)

1. `views::permission_view::tests::execute_header_display_matches_overlay_body`
2. `views::settings_modal::tests::picker_highlights_current_choice`
3. `views::tasks_pane::tests::bg_task_styled_prefix_uses_secondary_color`

### Cluster by module

| Module | Count | Fail names |
|--------|-------|------------|
| `views::permission_view` | 1 | `execute_header_display_matches_overlay_body` |
| `views::settings_modal` | 1 | `picker_highlights_current_choice` |
| `views::tasks_pane` | 1 | `bg_task_styled_prefix_uses_secondary_color` |

All three are **theme-cache race flakes under parallel lib runs**, not product logic regressions:

- Each passes alone with `--test-threads=1`.
- Sister tests already pin `crate::theme::cache::pin_theme()` for multi-sample style asserts (e.g. `wrap_rows_keep_the_unwrapped_line_styles`).
- Process-global `Theme::current()` / `terminal_native_locked()` can flip mid-test when another test calls `set` / lock without serializing.

## Root cause (tests-as-spec)

| Test | Symptom under race | Product intent |
|------|--------------------|----------------|
| `execute_header_display_matches_overlay_body` | Left/right of `assert_eq!(render_bash…, build_permission…)` diverge on bash highlight colors (one side Reset, other RGB palette) | Header and overlay share one renderer; styles must match when theme is stable |
| `picker_highlights_current_choice` | Focused row bg `DarkGray` vs `theme.bg_visual == Reset` | Focused row uses `settings_list_row_bg` (terminal-native elevate → DarkGray when `bg_visual` is Reset; themed → `bg_visual`) |
| `bg_task_styled_prefix_uses_secondary_color` | Span fg white (DOGE secondary at build) vs `Theme::current().text_secondary == Reset` at assert | Prefix uses `theme.text_secondary` at `from_bg_task` time; assert must sample the same theme |

No product code path was wrong. Fix is hermetic pin (same contract sister tests already encode).

## What was fixed

Added `let _pin` / `let _theme = crate::theme::cache::pin_theme();` at the start of each of the three tests:

1. `crates/codegen/xai-grok-pager/src/views/permission_view.rs` — `execute_header_display_matches_overlay_body`
2. `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs` — `picker_highlights_current_choice`
3. `crates/codegen/xai-grok-pager/src/views/tasks_pane.rs` — `bg_task_styled_prefix_uses_secondary_color`

No assert rewrites, no product behavior change. Comments note why the pin is required (parallel theme cache).

## Greened filters re-check

| Filter | Result | Exit |
|--------|--------|------|
| Full `--lib` after fix | 8813 passed, 0 failed | 0 |
| `key_owner` | 30 passed | 0 |
| `approve_plan_flush` | 118 passed (includes soft_park CTA suite) | 0 |
| `soft_park` | 48 passed | 0 |
| layout / acp / queue share-wake | covered by full lib green (no named residual reds) | 0 |

## Commands + exit codes

```bash
# 1. Resample before
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8
# → FAILED. 8810 passed; 3 failed; 11 ignored  (exit 101)
# log: /tmp/pager-lib-resample2.txt

# 2. Isolation (each of the 3)
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib execute_header_display_matches_overlay_body -- --test-threads=1
# → ok (exit 0)
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib picker_highlights_current_choice -- --test-threads=1
# → ok (exit 0)
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib bg_task_styled_prefix_uses_secondary_color -- --test-threads=1
# → ok (exit 0)

# 3. After pin_theme fixes
nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8
# → ok. 8813 passed; 0 failed; 11 ignored  (exit 0)
# log: /tmp/pager-lib-resample2-after.txt

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib key_owner -- --test-threads=8
# → 30 passed (exit 0)
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib approve_plan_flush -- --test-threads=8
# → 118 passed (exit 0)
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib soft_park -- --test-threads=8
# → 48 passed (exit 0)
```

## Remaining residual after this wave

**None** for `xai-grok-pager --lib` (0 failed).

Optional hygiene (not required for green): pin similar unpinned theme-sensitive siblings (e.g. `monitor_task_styled_with_monitor_tag`) if they ever flake under higher concurrency.

## Git

No commit / add / push (operator-owned).
