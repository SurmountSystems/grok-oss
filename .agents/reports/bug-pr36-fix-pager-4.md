# Pager four-test fix (PR #36 / `2174fd75` names)

Surgical product edits in `xai-grok-pager` only. Tests were the spec. No git. No shell / sampler / pager-bin / pty-harness.

## Red (CI contract)

Local first run blocked on the workspace cargo lock (other live fixers). Fail text from the earlier extracted CI log (`/tmp/pr36-ci-a036327e/fail-detail.txt`, same four names as `2174fd75`):

| Test | Panic |
|------|--------|
| `doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view` | `tests.rs:246` `assert_eq!(report.issue_count(), 1)` |
| `entry_renderer::tests::background_block_gutter_uses_block_background_fill` | gutter `bg` is not `theme.bg_light` |
| `dashboard::render::tests::render_row_needs_input_yellow_blink_no_badge_pending_prefix` | dim blink phase still full yellow |
| `prompt_widget::tests::title_renders_on_top_border_with_corners_intact` | `assert_ne!(border.fg, title_cell.fg)` |

Default theme on CI is DOGE (`ThemeKind::Doge`). Doctor snapshot composition also called a live mic probe.

## Product edits

1. **Doctor facts stay a function of the snapshot.**  
   `collect_report_with` only runs `diagnostics::view`. Live `apply_voice_probe` moved to `collect_report()` (real `grok doctor` CLI). On GHA there is no recorder, so the old path added `voice.no-input-device` as an Issue and `issue_count()` became 2.

2. **Background-block gutter keeps the full-area fill.**  
   UserPrompt per-line band uses `Theme::current()` (DOGE black). The test renderer is `Theme::groknight()`. Line-bg used to paint the timestamp gutter and overwrite groknight `bg_light`. For blocks that already have a background, the line band now stops before `ts_reserved`.

3. **Needs-input dim blink uses opacity 0.4.**  
   DOGE `blend_color` is a solid step: `opacity >= 0.5` keeps the original color. Dim at 0.5 stayed full `warning` (yellow). 0.4 steps to `bg_base` on DOGE and still fades on continuous themes.

4. **Titled top-border rule stays distinct from the caption.**  
   Chrome-caption at 0.6 opacity on DOGE stays `text_secondary` (white), same as `prompt_border_active`. When a title will paint and those two match, the rule uses `theme.gray` (yellow on DOGE). Title style is unchanged (still the 0.6 blend the test asserts).

Files:

- `crates/codegen/xai-grok-pager/src/doctor_cmd/mod.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs`
- `crates/codegen/xai-grok-pager/src/views/dashboard/render.rs`
- `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs`

## Green

```
cargo fmt -p xai-grok-pager
cargo nextest run -p xai-grok-pager --locked --test-threads=2 --build-jobs 2 \
  -E 'test(fake_standalone_facts_compose_through_shared_view) or test(background_block_gutter_uses_block_background_fill) or test(render_row_needs_input_yellow_blink_no_badge_pending_prefix) or test(title_renders_on_top_border_with_corners_intact)'
```

Summary: **4 passed**, 9322 skipped.

`cargo clippy -p xai-grok-pager --lib --locked -- -D warnings` finished clean (exit 0).

`cargo clippy -p xai-grok-pager --all-targets --locked -- -D warnings` is still red on **untouched** files (not this slice):

- `benches/edit_highlight.rs` `needless_range_loop`
- `src/app/session_startup.rs` `bool_assert_comparison`
- `src/diagnostics/fix_tests.rs` `disallowed_methods` (`Path::canonicalize`)

## Result

**4 / 4 green.**
