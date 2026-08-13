# Join: Slice 4 — SuperGrok $ extras before console (after-burner)

**Date:** 2026-08-02
**Implementer:** L2
**Plan:** limits-first ideal Slice 4 / C5
**Also:** `/tmp/grok-$(id -u)/grok-impl-summary-limits-s4.md`

## Outcome

When `auto_use_included_limits` is on: **limits before credits always**. Included headroom still omits console (Design A). After included is full, **positive SuperGrok $ extras** keep the SuperGrok session primary with console only as failover. When extras are 0 or unknown, console is primary as before. `preferred_method=api_key` still pins console first.

## RED

Contracts introduced / tightened before product green:

| Test | Expected fail reason before product |
|------|-------------------------------------|
| `auto_order_keeps_supergrok_when_included_full_but_extras_remain` | ExhaustedAll always put console first; ignored `prepaid_balance_cents` |
| `auto_after_included_and_extras_gone_console_primary` | Guard for 0/None still console (no invent after-burner) |
| `auto_with_included_headroom_still_omits_console` | Design A regression |
| `resolve_auto_after_included_exhausted_keeps_session_while_extras_positive` | resolve followed old order |
| `resolve_enforced_auto_use_included_limits_prefers_console_when_supergrok_included_exhausted` | **Updated** contract: console only when extras also gone/unknown |

Command:

```bash
cargo test -p xai-grok-shell --lib -- \
  auto_order_keeps_supergrok \
  auto_after_included_and_extras \
  auto_with_included_headroom \
  resolve_auto_after_included_exhausted \
  resolve_enforced_auto_use_included_limits
```

## GREEN

```bash
cargo test -p xai-grok-shell --lib -- \
  auto_order_keeps_supergrok \
  auto_after_included_and_extras \
  auto_with_included_headroom \
  auto_order_omits_console \
  auto_both_included_exhausted \
  resolve_auto_after_included_exhausted \
  resolve_enforced_auto_use_included_limits \
  resolve_auto_both_supergrok_exhausted
# 8 passed

cargo test -p xai-grok-shell --lib -- allowance_exhaust_from_billing
# 17 passed
```

`cargo fmt -p xai-grok-shell` applied.

---

## Isolation round (re-review)

`#[serial_test::serial]` on `remember_dollar_extras_stores_prepaid_cents_for_limits_fill` and three siblings that thrash `clear_included_billing_cache` without serial. Product unchanged. `allowance_exhaust_from_billing` filter: 21 passed.

## Product behavior

1. **Candidates** carry `prepaid_balance_cents` (from process billing remember via enrich/load).
2. **Order** while any included headroom: SuperGrok-only chain (console omitted).
3. **Order** after included exhaust + extras > 0: SuperGrok primary, console failover (after-burner).
4. **Order** after included exhaust + extras 0/None: console primary; SuperGrok tokens omitted.
5. **Memo:** `apply_billing_usage_to_session_exhaust*` with auto_use + dual-auth + known positive extras does not mark (and clears a prior mark) so sampler prefer_live does not skip SuperGrok before extras burn.
6. Docs/status copy updated so they no longer claim “always console after included full.”

## Files

- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`
- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs` (export `has_positive_supergrok_dollar_extras`)
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs`
- `crates/codegen/xai-grok-shell/src/agent/config.rs`
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md`

## Not done (by design)

- Default `auto_use_included_limits=true` for new installs (Slice 5)
- M6 usage series, bare resolve sites audit

---

## Review fix round (2026-08-02)

All 5 open review issues **fixed** (none wontfix). See `/tmp/grok-1000/grok-review-limits-s4.md`.

| Issue | Fix |
|-------|-----|
| 1 memo gate | Pure gate + hermetic apply tests (no-mark / clear / mark when extras gone); home config.toml auto_use |
| 2 hard-expired | `hard_expired` field; after-burner filter; load wire-up; pure order tests |
| 3 api_key pin | Resolve test: extras + auto_use cannot override ApiKey pin |
| 4 bare C5 | Scrubbed comments to plain after-burner language |
| 5 flag name | Docs + assert both flags true on after-burner |

### RED / GREEN (review round)

**RED:** memo tests expected Marked under broken home-config parse / missing gate; hard-expired primary; missing pin resolve.

**GREEN:**
```bash
cargo test -p xai-grok-shell --lib -- \
  afterburner_skips_allowance_mark \
  apply_billing_100_pct_with_positive_extras \
  apply_billing_100_pct_auto_use_marks \
  auto_afterburner_skips_hard_expired \
  auto_afterburner_prefers_live_extras \
  resolve_api_key_pin_stays_console \
  auto_order_keeps_supergrok \
  resolve_auto_after_included \
  apply_billing_100_pct_marks
# 10 passed

# related suite: 65 passed
```

`cargo fmt -p xai-grok-shell` applied.