# Join: Branch 2b soft honesty polish

**Date:** 2026-08-02
**Implementer:** L2
**Also:** `/tmp/grok-1000/grok-impl-summary-limits-2b-honesty.md`
**Review:** `/tmp/grok-1000/grok-review-limits-residual-edges.md` (Issues 1,4,5,6 docs,7)

**Evidence (not re-proved):**
`.agents/joins/slice2-dogfood-g4-2026-08-02.md`,
`.agents/joins/impl-slice1-poll-history-2026-08-02.md`,
`.agents/joins/impl-slice3-m3-postpaid-2026-08-02.md`,
`.agents/joins/impl-slice4-extras-before-console-2026-08-02.md`

## Outcome

Soft honesty for residual branch **2b** + review fix for flat-note overclaim
and sibling Build %. No resolve/rank policy change.

## Product behavior

| Surface | Behavior |
|---------|----------|
| `/limits` human | Base poll note; **conditional** flat note (only meters observed flat); C6 when OAuth postpaid dominates; **Grok Build product usage: N% used** when principal has wire % |
| `/usage` (non-silent billing) | Same honesty stack via process flat **evidence** + postpaid cache; Build % shared phrase |
| Doctor dual-auth | Auto-use: included weekly first; when included full, SuperGrok $ extras before console (Slice 4; **regression pin**, already green before this wave) |
| Flat note | Names SuperGrok included % always; Build / SuperGrok $ extras **only if observed** on the flat window |
| C6 | Team Usage $ can move without proving included weekly moved; console not live |

## RED (honest)

### Original wave
- New Build % + C6 copy: RED before product, then green.
- Doctor extras-before-console: **already green** (not product RED this wave).

### Review fix wave
| Test / contract | Expected fail before fix |
|-----------------|--------------------------|
| `flat_poll_note_without_build_on_wire_does_not_claim_build_flat` | Static note always named Build |
| `flat_evidence_included_only_does_not_mark_build_or_extras_observed` | No evidence flags |
| `remember_build_usage_stores_product_pct_for_limits_fill` | No field / remember |
| `format_dual_principal_surfaces_sibling_grok_build_usage` | Sibling hard-coded None |
| Shared Build phrase asserts | Usage used `N%` without `used` |

## GREEN

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell

cargo test -p xai-grok-shell --lib -- \
  flat_evidence remember_build included_poll_history remember_dollar
# 12 passed

cargo test -p xai-grok-pager --lib -- \
  limits_honesty flat_poll format_surfaces format_dual_principal \
  format_flat_poll usage_summary limits_cmd:: limits_snapshot::
# 71 passed
```

## Files

- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
- `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs`
- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` (doctor regression test only)

## Not done (by design)

- Design A / after-burner resolve (other agent / bare-resolve track)
- Flip default `auto_use_included_limits`
- Claim C4 SuperGrok debit proven
- M6 usage series charts

## Residual note for parent

Soft OAuth Usage honesty polish **shipped** including review Issues 1,4,5,7.
C4 / G4 remains branch **2b** (no invent debit).
