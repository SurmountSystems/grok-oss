# Implement report: Grok Business license zeros vs team Grok Build burn

**Date:** 2026-08-07
**Plan:** session `plan.md` (hybrid D = A honesty + B prominence)
**Inventory:** `.agents/reports/plan-grok-business-usage-zeros-2026-08-07.md`

## Outcome

Shipped P0 honesty + P1 prominence. License chart zeros stay a non-goal (no invent, no scrape). Product points dogfood at team Usage / Grok Build class $ and Management postpaid/series. P2 background series fetch **parked**.

## RED → GREEN (named contracts)

| Contract | RED evidence | GREEN |
|----------|--------------|-------|
| **1. Doctor dogfood block** names licenses not proof AND team Usage / Grok Build as settlement | New `doctor_dogfood_block_names_wrong_page_and_right_proof` would fail without `dogfood_burn_proof_doctor_block` + doctor append | `views::limits_honesty::tests::doctor_dogfood_block_names_wrong_page_and_right_proof` PASS |
| **2. Limits honesty** names team Usage + zeros expected (not only "not SuperGrok") | New `license_honesty_names_team_usage_and_zeros_expected`; snapshot `format_limits_honesty_distinguishes_license_page_from_product_meters` extended | PASS |
| **3. SuperGrok-live footer** Grok Build class $ when fixture cents known; not prepaid; Design A compact `%` stays | New `footer_supergrok_live_surfaces_team_grok_build_class_when_known` + standalone class chip | PASS |
| **4. `/limits` Console** Grok Build class prominent (before Balance when known) | New `format_console_surfaces_grok_build_class_prominently` | PASS |
| **Keep green:** Design A compact free-period; SuperGrok-live team prepaid; dual auth; limits_cmd postpaid/series | Existing filters re-run | PASS |

Note: `limits_json_postpaid_gap_when_no_management_key` briefly went red after the license note started containing "team Usage dollars" (false positive for C6). Assert tightened to C6-specific phrase `can still move team Usage dollars`. That is a test-intent precision fix, not weaker coverage.

## Product changes

### P0 Honesty

| Area | Change |
|------|--------|
| `limits_honesty.rs` | Sharper `NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER`: not dogfood proof; zeros expected; real burn = team Usage / Grok Build + SuperGrok + prepaid/postpaid/series |
| Same | `dogfood_burn_proof_doctor_block()` pure helper |
| `doctor_cmd/mod.rs` | Human doctor appends dogfood proof block after dual-auth status |
| User-guide `02-authentication.md` | New subsection **If Grok Business license Usage is all zeros** |
| User-guide `04-slash-commands.md` | `/limits` honesty + footer Grok Build chip wording |

### P1 Prominence

| Area | Change |
|------|--------|
| `limits_snapshot.rs` `format_console` | When postpaid OAuth class > 0, **Team postpaid OAuth / Grok Build class** line near top of Console (before prepaid Balance) |
| `credit_bar.rs` | `team_grok_build_class_footer_chip`; `usage_warning_for_session_with_identity_principal_gap_and_postpaid`; merge attaches class as separate chip |
| `agent_view/render.rs` | Footer reads `cached_console_team_postpaid_default().oauth_class_cents` and passes into gap_and_postpaid |

Meters stay distinct: free SuperGrok period % (Design A compact) ≠ SuperGrok extras ≠ team prepaid ≠ team Grok Build class period $.

### Residual / FORK

- `RESIDUAL.md` § Half B: shipped 2026-08-07 bullet + P2 park note
- `FORK.md` billing meters bullet: zeros expected; proof map; Grok Build footer/console prominence

## Non-goals honored

- No license chart invent / scrape
- Dual SuperGrok poll honesty not re-opened
- No C4 SuperGrok period debit invent
- Design A compact free-period `%` not replaced by team `$`
- No mashed single "credits" string
- No `git add` / commit / push

## Parked P2

Background usage series on FetchBilling cadence (cost/latency careful). Postpaid OAuth class on footer + Console is enough for Image 2-class proof without series on every poll. Optional later if dogfood still cannot see series without explicit `limits` collect.

## Files touched

- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `crates/codegen/xai-grok-pager/src/doctor_cmd/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` (C6 false-positive assert + OAuth label)
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md`
- `RESIDUAL.md`, `FORK.md`

## Verify (exit codes)

| Step | Command | Result |
|------|---------|--------|
| fmt | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 |
| clippy | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| tests | `cargo test -p xai-grok-pager --lib views::limits_honesty::` | 19 ok |
| tests | `cargo test -p xai-grok-pager --lib views::credit_bar::` | 78 ok |
| tests | `cargo test -p xai-grok-pager --lib views::limits_snapshot::` | 38 ok |
| tests | `cargo test -p xai-grok-pager --lib limits_cmd::` | 32 ok (1 ignored live) |
| tests | `compact_status_supergrok_free_period` filter | ok |
| tests | `cargo test -p xai-grok-shell --lib dual_auth_status::` | (run in same verify wave) |

## Operator dogfood (after rebuild)

1. `grok-oss limits` / TUI `/limits`: Console shows **Team postpaid OAuth / Grok Build class** early when Management works; team prepaid Balance separate; free SuperGrok period on SuperGrok rows.
2. `grok-oss doctor`: dual-auth block + **Dogfood burn proof** wrong-page / right-page lines.
3. SuperGrok-live footer: team prepaid when known **plus** `team Grok Build class: $N` when postpaid cache warm. Compact bar still free-period `%` when free period has room.
4. Browser licenses Usage may still be zeros. Expected. Team Usage (~Grok Build $) remains the browser settlement chart.

## Not done

- P2 series background fetch
- License Management API (blocked until public docs)
- git stage/commit (human-only)
