# Join: Slice 1 — poll history + flat honesty (S1 only)

**Date:** 2026-08-02
**Mode:** implementer TDD (verify-first; already shipped)
**Scope:** Slice 1 only (no M3, no ExhaustedAll reorder, no Design A change)

## Operator pin

**Limits before credits.** Always. Design A (console ApiKey omitted while
SuperGrok included has headroom) unchanged. No hop to console to "fix" Usage $.

## What shipped

| Piece | Path / API |
|-------|------------|
| Pure sample + detector | `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs` |
| Process ring per `identity_id` | `record_included_poll_sample` / `record_included_poll_now` |
| Flat detector | `included_debit_unproven(samples, min_polls, min_window)` |
| Defaults | `DEFAULT_MIN_POLLS = 2`, `DEFAULT_MIN_WINDOW = 30s`, ring cap 32 |
| S1 wire-up | `record_included_poll_history_from_config` in `extensions/billing.rs`; called on active `x.ai/billing`, sibling poll, and `limits_cmd` collect |
| Limits surface | `attach_flat_poll_from_history` → `LimitsSnapshot.flat_poll_unproven_debit` in `limits_cmd` + TUI `build_limits_snapshot` |
| Optional log | `billing: poll_delta` when included % / Build % / extras cents step vs previous sample |

## Named contracts (green)

| Test | Crate |
|------|--------|
| `poll_history_marks_flat_when_included_and_extras_unchanged` | `xai-grok-shell` |
| `poll_history_clears_flat_when_included_pct_steps` | `xai-grok-shell` |
| `poll_history_clears_flat_when_build_product_usage_steps` | `xai-grok-shell` |
| `poll_history_clears_flat_when_extras_cents_drop` | `xai-grok-shell` |
| `limits_snapshot_sets_flat_poll_from_history_not_only_tests` | `xai-grok-pager` |
| `flat_poll_note_when_evidence_flag_set` (kept) | `xai-grok-pager` |
| billing identity / Build log tests (kept) | `xai-grok-shell` `extensions::billing` |

## Commands + evidence

### RED (named contract intent; product already in tree this run)

Named pure detector contracts encode fixture behavior:

- flat included % + flat extras (+ optional flat Build %) over `min_polls` and
  `min_window` → `included_debit_unproven` true
- any step in included %, Build product %, or extras cents → false
- wire: `limits_snapshot_sets_flat_poll_from_history_not_only_tests` requires
  process history + `attach_flat_poll_from_history` (not only
  `with_flat_poll_unproven_debit(true)`)

This verify pass did **not** strip product to re-observe red (already green in
tree; residual Slice 1 gaps: **none**).

### GREEN (re-ran 2026-08-02 verify pass)

```bash
cargo test -p xai-grok-shell --lib included_poll_history
# 8 passed (4 named + helpers: min_polls, min_window, process ring, empty id)

cargo test -p xai-grok-pager --lib flat_poll
# 4 passed including limits_snapshot_sets_flat_poll_from_history_not_only_tests
# and flat_poll_note_when_evidence_flag_set

cargo test -p xai-grok-shell --lib extensions::billing::
# 16 passed (identity_id / grok_build_usage_percent log contracts)
```

No `*.rs` edits this turn; no fmt required.

## Explicit out of scope (unchanged)

- M3 postpaid preview client
- ExhaustedAll / extras-before-console reorder (Slice 4)
- M6 usage series
- Design A strip-console logic
- Grok Business licenses

## Residual pin

`RESIDUAL.md` dual-auth section: Slice 1 poll history / flat honesty shipped
2026-08-02; Limits before credits; join link this file.

## Host summary

`/tmp/grok-1000/grok-impl-summary-limits-s1.md`
