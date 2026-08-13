# Bug report: Limits chrome shows free SuperGrok period when on credits

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Status:** Diagnosed + fixed (Design A)

## Symptom

When live spend is **credits** (console team prepaid/spend, or SuperGrok dollar top-up extras), the TUI compact status meter still painted **free SuperGrok period** chrome (`N%`, cold `...%`) as if free SuperGrok period allowance drove the turn.

Dogfood examples:

- Console team 403 while status still showed SuperGrok-style `...%`
- SuperGrok free period full + after-burner on SuperGrok `$` extras still showed free-period `100%` in the status bar

Meters must stay distinct: free SuperGrok period `%` ≠ SuperGrok `$` extras ≠ console team prepaid.

## Design A (product contract)

| Live spend path | Compact status meter |
|-----------------|----------------------|
| Console live | Console team prepaid `$` (or honest gap). **Never** free SuperGrok period `%` / `...%` |
| SuperGrok live, free period has room | Free-period used `%` |
| SuperGrok live, free period full, SuperGrok `$` extras remain | SuperGrok extras `$` (not bare free-period `100%`) |
| SuperGrok live, free period full, no extras | `100%` (included empty; no second meter) |

## Root cause

### 1. SuperGrok-on-extras status bar always painted free-period `%` (clear product bug)

`credit_bar_line_for_session` always rendered `usage_pct` (free SuperGrok period included %), even when `usage_pct >= 100` and `prepaid_balance_cents > 0` (SuperGrok dollar extras).

Footer warning already switched to **SuperGrok extras left: $N** at 100% + positive prepaid (with auto-topup gates). Status bar did not share that honesty.

### 2. Console-live sticky pin ran only on the prompt footer path (status bar lag / dogfood `...%`)

In `agent_view/render.rs`:

- **Footer** probed `supergrok_out_of_allowance_with_console_ready` and pinned `sampling_identity = ConsoleKey` before footer paint.
- **Status bar** painted earlier in the same frame **without** that probe. Tracked identity could stay `SuperGrokSession` → cold SuperGrok `...%` or free-period `N%` while console was the live spend pool (prefer_live / sticky exhaust without hop toast).

`is_api_key_auth` and hop toasts already set ConsoleKey; dual-auth free-period-full → console was the gap.

### 3. What already shipped (partial)

| Ship | What it fixed | Gap left |
|------|---------------|----------|
| `impl-console-dead-supergrok-recovery` (`compact_meter_text_for_live_identity`) | Pure helper + unit test: console live ≠ bare SuperGrok `...%` | Helper not used for SuperGrok extras; status bar still duplicated console formatting and did not pin sticky console before paint |
| Status bar console branch (prior) | When `sampling_identity` already ConsoleKey → `console · $N` | Identity lag + SuperGrok-on-extras path |
| SuperGrok-live team usage (`impl-supergrok-live-team-usage`) | Footer + `/limits` + `/usage` team prepaid when SuperGrok live | Did not change compact free-period vs extras status meter |
| Footer `usage_warning_*` | Console live never SuperGrok %; SuperGrok full + extras → extras warning | Status bar independent |

### 4. Other surfaces (checked)

| Surface | Honesty |
|---------|---------|
| Footer warning strip | Already Design A for console vs SuperGrok extras (subject to `billing_surface_visible` / team hide) |
| Soft `/usage` | Lists included % **and** SuperGrok extras as separate lines; does not claim free period is the sole live driver. Acceptable multi-meter summary. |
| `/limits` snapshot | Distinct blocks for included, extras, team prepaid. Acceptable. |
| Compact status (always-on) | **Was** the main lie for operators who only glance at the bar |

## Fix (minimal)

### Product

1. **`compact_meter_text_for_live_identity`** — SuperGrok branch: free period full + positive SuperGrok extras → `SuperGrok extras · $N`; free period with room → `N%`; full without extras → `100%`. Console branch unchanged (extras arg ignored).
2. **`credit_bar_line_for_session`** — builds text via that helper; no free-period pacing chip on the extras path; color by extras low-balance vs included % thresholds.
3. **Status bar** — pin sticky console (`supergrok_out_of_allowance_with_console_ready`) **before** meter paint (same probe as footer); console branch uses `compact_meter_text_for_live_identity`.

### Files

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Compact helper Design A + line paint + unit tests |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Status bar sticky pin + console helper + integration tests |

## RED → GREEN

| Contract | Test | Result |
|----------|------|--------|
| SuperGrok full + extras → extras $ not free-period % | `compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct` | GREEN |
| SuperGrok free period room → % even if extras on account | `compact_status_supergrok_free_period_room_shows_pct_not_extras` | GREEN |
| SuperGrok full, no extras → 100% | `compact_status_supergrok_full_without_extras_shows_100_pct` | GREEN |
| Console live still not SuperGrok chrome | `compact_status_console_live_does_not_imply_supergrok_drives_turn` | GREEN |
| Status bar paints extras $ | `status_bar_supergrok_on_extras_paints_dollars_not_free_period_pct` | GREEN |
| Console live ignores cached SuperGrok %/extras | `status_bar_console_live_ignores_cached_supergrok_free_period_pct` | GREEN |

### Commands

```text
cargo test -p xai-grok-pager --lib -- compact_status_ status_bar_supergrok_on_extras status_bar_console_live_ignores status_bar_credits_meter status_bar_console_meter
cargo test -p xai-grok-pager --lib -- views::credit_bar
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

All green. Clippy lib clean on `xai-grok-pager`.

## Residual / open questions

None blocking for Design A. Optional later:

- Footer SuperGrok extras warning still gated by auto-topup rules (status bar always shows extras `$` when full + positive prepaid). Intentional: compact always names the active meter; footer is a low-balance *warning*.
- Sticky-console pin still depends on process/disk exhaust memo + console ready. If those are cold wrong, identity can still lag until hop toast / billing sync. Existing `sampling_identity_after_allowance_sync` paths remain.
- `/usage` soft dump still leads with included `%` then extras block; not changed (summary is multi-meter, not live-driver chrome).

## Git

No `git add` / commit (agent rule).
