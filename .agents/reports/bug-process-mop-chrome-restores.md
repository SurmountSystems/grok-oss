# Process mop: plan five-CTA restore + included SuperGrok period limits meter

Process mop only. No product edits. No new features.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager -p xai-grok-shell -p xai-grok-pager-render -p xai-grok-update -p xai-grok-pager-bin` | 0 |
| `cargo clippy -p xai-grok-pager --lib --bins --offline -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-shell --lib --bins --offline -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-render --lib --bins --offline -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-update --lib --bins --offline -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-bin --lib --bins --offline -- -D warnings` | 101 (no library targets in this package) |
| `cargo clippy -p xai-grok-pager-bin --bins --offline -- -D warnings` | 0 |
| `cargo test -p xai-grok-pager --lib --offline --` (filters below) | 0 |
| `cargo test -p xai-grok-shell --lib --offline -- plan_approval_helper questions_plan_message` | 0 |

Clippy used `--lib --bins` as required. `xai-grok-pager-bin` has no lib; bins-only clippy is the matching check and is clean.

`--all-targets` on pager was not run. Prior mop notes said that path can fail on unrelated tests/bench (`canonicalize`, `saturating_sub`, `needless_range_loop`). Those files were not in this wave. No mop of that noise.

## Targeted tests

**xai-grok-pager** (98 passed, 0 failed, 8680 filtered):

- `plan_approval_footer_paints_five_cta_vocabulary`
- `plan_approval_draw_uses_one_five_cta_vocabulary`
- `status_bar_pushes_credits_compact_included_supergrok_period_limits`
- `hit_credits_click_dispatches_show_limits`
- `user_prompt_block_accent`
- `info_line_model_name_uses_accent_model`
- `views::credit_bar` (full module: compact included SuperGrok period limits meter, footer, warnings)

**xai-grok-shell** (5 passed, 0 failed, 6559 filtered):

- `plan_approval_helper` (ext method, revise feedback, outcome map, resume action)
- `questions_plan_message` (clarify is not revise and forbids rewrite)

## Fallout

None. Format, clippy, and targeted tests were already green on the plan five-CTA restore, the compact included SuperGrok period limits meter (`"credits"` paint + click ShowLimits), and the earlier theme rails / doge Object Property / rebuild SIGUSR1 `--version` packages.

No files changed by this mop.
