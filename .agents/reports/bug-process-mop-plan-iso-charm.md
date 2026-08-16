# Process mop — isolated plan.md five-CTA tests

**Date:** 2026-08-14
**Tag:** `[process-mop]`
**Package:** `xai-grok-pager`
**Primary:** `.agents/reports/bug-plan-iso-missing-charm.md`
**Implementer:** `01a0029e-6350-7f11-a819-a1a704e8a74f`

Backup only. Re-ran fmt, clippy, and the named isolated-plan filters. SuperGrok is paid. This report never says "free SuperGrok."

## Environment

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
```

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-plan-charm-mop-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

Did not use host `/tmp`. Isolated mop target is separate from the implementer's `/home/hunter/.cache/grok-oss-plan-charm-target`.

## fmt

```bash
cargo fmt -p xai-grok-pager
cargo fmt -p xai-grok-pager -- --check
```

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager` | **0** |
| `cargo fmt -p xai-grok-pager -- --check` | **0** |

No formatting changes.

## clippy

```bash
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

| Attempt | Result |
|---------|--------|
| 1 (cold mop target) | Finished `dev` in 4m 27s. **CLIPPY_EXIT:0** |

No clippy warnings. No product edits.

## Named tests

```bash
cargo test -p xai-grok-pager --lib -- \
  file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder \
  plan_approval_draw_uses_one_five_cta_vocabulary \
  plan_approval_footer_paints_five_cta_vocabulary \
  exit_plan_without_inline_content_uses_file_backed_source \
  new_present_turn_row_is_review_park_not_approve
```

| Attempt | Result |
|---------|--------|
| 1 (300s foreground) | Wrapper killed at 300s while compiling the test graph (`xai-grok-pager` and deps still building) |
| 2 (background retry, same target) | Finished `test` in 7m 11s. **TEST_EXIT:0** |

```
running 5 tests
test app::acp_handler::tests::plan_mode::exit_plan_without_inline_content_uses_file_backed_source ... ok
test views::file_search::line_viewer::tests::plan_approval_footer_paints_five_cta_vocabulary ... ok
test app::agent_view::render::voice_recording_overlay_tests::file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder ... ok
test app::agent_view::render::plan_turn_row_revising_copy_tests::new_present_turn_row_is_review_park_not_approve ... ok
test app::agent_view::render::voice_recording_overlay_tests::plan_approval_draw_uses_one_five_cta_vocabulary ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 8883 filtered out; finished in 0.07s
```

## Edits

None. fmt, clippy, and the five named tests were already green. Did not rewrite plan chrome, spend-order, or limits-hub. Did not weaken tests. Did not `git add`, commit, or push.

## Leftovers (not this mop)

- Live TUI still needs a successful `/rebuild` and a full quit/reopen to drop the 1.0.3 request-changes bar. Source and unit tests already paint five-CTA.
- Pty e2e for isolated `plan.md` resume was not run.
- Composer 40% `accent_plan` border blend during plan mode was left alone (not the all-yellow 1.0.3 box).
