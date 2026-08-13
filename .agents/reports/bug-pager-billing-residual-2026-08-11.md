# Pager billing residual (ShowUsage chain) — green

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Filter:** `cargo test -p xai-grok-pager --lib 'app::dispatch::tests::billing'`

## Result

| Before | After |
|--------|-------|
| 73 pass / **6 fail** | **79 pass / 0 fail** |

## Root cause

Not a half-merge in product billing dispatch. Product is intentional dual-path:

| Screen mode | `/usage` / ShowUsage |
|-------------|----------------------|
| Full TUI (default fixture `Inline`) | Opens usage modal; multi-effect fetch (context + session info + session usage + silent billing) |
| **Minimal** | Scrollback: single `FetchSessionUsage`, then complete → session block + non-silent billing / redirect |

Status tests already document this (`show_usage_opens_modal_on_usage_limit_tab_with_fetches`, `show_usage_with_redirect_url_fetches_session_only` with `ScreenMode::Minimal`, comment "Scrollback flow is minimal-only.").

The six red billing tests encoded the **minimal scrollback contracts** but used the default **Inline** fixture, so ShowUsage opened the modal and `SessionUsageComplete` (nonce 0, no open modal) dropped without pushing scrollback.

## Fix

File: `crates/codegen/xai-grok-pager/src/app/dispatch/tests/billing.rs`

Set `app.screen_mode = ScreenMode::Minimal` on the six scrollback-chain tests (expects unchanged):

1. `show_usage_schedules_session_fetch_only`
2. `show_usage_without_session_still_surfaces_credits`
3. `session_usage_complete_pushes_block_and_chains_billing`
4. `session_usage_complete_no_billing_when_surface_hidden`
5. `session_usage_complete_redirect_after_session_block`
6. `session_usage_failed_pushes_error_and_chains_billing`

No product code change. Credit chrome / limits language paths already green in the rest of the billing module.

## Verify

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests::billing' -- --test-threads=8
# → 79 passed; 0 failed
cargo fmt -p xai-grok-pager
```

## Contracts still held (minimal path)

- ShowUsage always schedules session-usage fetch only (even when `usage_visible = false`)
- Complete/fail push session block; chain non-silent billing when surface visible
- Surface hidden: session block lands, no billing effect
- Redirect URL deferred until after session block
- No session: "unavailable" system line + non-silent billing
