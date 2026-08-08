# Fix: `render_paints_tracked_remaining_bar_with_bounds`

**Date:** 2026-08-07
**Package:** `xai-grok-pager`
**Filter:** `views::limits_modal::tests::render_paints_tracked_remaining_bar_with_bounds`

## Fail output (red)

```
cargo test -p xai-grok-pager --lib \
  views::limits_modal::tests::render_paints_tracked_remaining_bar_with_bounds \
  -- --nocapture

thread 'views::limits_modal::tests::render_paints_tracked_remaining_bar_with_bounds'
  panicked at crates/codegen/xai-grok-pager/src/views/limits_modal.rs:556:9:
limits modal must paint a tracked remaining bar with [ ] bounds
test ... FAILED
```

Buffer dump (debug) showed the modal filled with always-on honesty notes; the last content row was the wrapped allowance line (`Included weekly allowance: 25% used ·`) and no `[`/`█`/`░` bar row.

## Root cause

Two product issues stacked:

1. **`format_limits_detail` put long honesty notes before meters.**
   `NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER` is always on and wraps to ~11 rows at typical modal width (~42 cols). On an 80×30 area the SuperGrok included line landed as the **last content row**.

2. **Remaining bar was painted only as an extra row after that line in the viewport.**
   Paint required `y < content.y + content.height` *after* the allowance line. When the allowance line was the last content row, the bar was skipped. The bar was also **not** in the display stream, so scroll max did not reserve a row for it.

Contract of the test (tracked bar with `[` `]` and `░`/`█`) was still correct. Product regressed under honesty-note bulk + layout.

## Fix

### 1. Meters before long honesty (`limits_snapshot.rs`)

- Keep dual-poll honesty near the top (short, trust caveats for meters about to be read).
- Move `honesty_notes_for_snapshot` to **after** SuperGrok principal + Console sections, before double-entry spend.
- Named product intent: open `/limits` / `grok limits` → meters first, longer caveats second. Long license / poll-reading notes no longer bury the remaining bar under the fold on typical heights.

### 2. Bar in display stream (`limits_modal.rs`)

- After wrap, inject a private sentinel (`REMAINING_BAR_SENTINEL`) after the first line matching `Included` + `allowance:` + `% used`.
- Scroll max and viewport layout reserve that row.
- Render paints `progress_bar_tracked_spans` for the sentinel instead of after-the-fact paint that can fall off the last content row.

Files touched:

- `crates/codegen/xai-grok-pager/src/views/limits_modal.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`

No test expectation rewrite; same contract, product corrected.

## Green

```
cargo test -p xai-grok-pager --lib \
  views::limits_modal::tests::render_paints_tracked_remaining_bar_with_bounds \
  -- --nocapture
# ok

cargo test -p xai-grok-pager --lib views::limits_
# 68 passed

cargo test -p xai-grok-pager --lib limits_cmd::
# 35 passed, 1 ignored

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# clean
```

(`cargo clippy -p xai-grok-pager --all-targets` still hits pre-existing failures in unrelated test files: `session_startup.rs`, `diagnostics/fix_tests.rs`, `scrollback/selection.rs`, `test_util.rs`. Not introduced here.)

## Git

No `git add` / commit / stage (per job).
