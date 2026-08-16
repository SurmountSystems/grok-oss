# Restore status-bar compact included SuperGrok period limits meter

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 implementer  
**Date:** 2026-08-13  
**Diagnosis:** `.agents/reports/fork-loss-postmortem-2026-08-13.md` §3. Not re-litigated.

## Named contract

1. Top status bar always paints a compact chip from existing `credit_bar` helpers.
2. `status.push("credits", …)` so `hit_credits.rect` is a real rect.
3. Click on that chip dispatches `Action::ShowLimits` (same as `/limits`).
4. SuperGrok is paid. Compact label is **included SuperGrok period limits · N%**. Do not paint "free SuperGrok period". Do not invent a new meter. Do not replace `/limits` or footer `usage_warning_for_session`.

## TDD (red → green)

**Red (tests written first, product path still dead):**

```
cargo test -p xai-grok-pager --lib status_credits_meter -- --nocapture
```

- `status_bar_pushes_credits_compact_included_supergrok_period_limits`  
  panicked: `status bar must push "credits" so hit_credits.rect is a real rect`
- `hit_credits_click_dispatches_show_limits`  
  panicked: `clicking the credits chip must open /limits, got Unchanged`

**Green (same filters after paint + click + compact label):**

```
cargo test -p xai-grok-pager --lib status_credits_meter
# 2 passed

cargo test -p xai-grok-pager --lib -- views::credit_bar::
# 91 passed

cargo test -p xai-grok-pager --lib -- app::agent_view::render::
# 16 passed
```

Post-impl verify:

```
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib --bins -- -D warnings
# exit 0
```

## Product edits (smallest)

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | After context, `status.push("credits", credit_status_line_for_live_session(…))`. Draw + click tests. |
| `crates/codegen/xai-grok-pager/src/app/mouse.rs` | `hit_credits` left-click → `Action::ShowLimits`. |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Compact painted string is `included SuperGrok period limits · N%`. New `credit_status_line_for_live_session` wires existing helpers (SuperGrok line / loading / console compact). `/limits` `as_human()` and footer `usage_warning_for_session` unchanged. |

Build sessions always get a chip: warm percent, extras `$`, console `$` / honest gap, or cold `included SuperGrok period limits · ...%`. Gateway chat still returns `None`.

## Leftovers

- **Live TUI is old until a successful rebuild** and a full quit/reopen. Source restore does not appear in already-running `grok-oss` windows.
- Plan-approval overlay mouse path (`agent_view/input.rs`) already updates `hit_credits` hover. It does **not** dispatch `ShowLimits`. Main `handle_mouse` does. Click while the plan panel owns keys is leftover.
- `/limits` Active line still says "free SuperGrok period" via `ActiveSpendDriver::as_human()`. That is the language residual (`feat:supergrok-period-limits-language`), not this paint restore.
- Catalog cheat sheet should list `status_bar_pushes_credits_compact_included_supergrok_period_limits` and `hit_credits_click_dispatches_show_limits`. Not edited here.

## Paths

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/mouse.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/fork-loss-postmortem-2026-08-13.md`
