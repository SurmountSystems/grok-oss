# Implement report: spend Business included SuperGrok period limits before personal

**Board:** `impl:te-business-included-before-personal`  
**Parent:** `feat:token-economy-all-plans-ipc`  
**Prior report:** `.agents/reports/impl-te-sibling-included-before-extras.md`  
**Date:** 2026-08-14  
**Isolated compile:** rustc 1.97.1, `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-sibling-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`

SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok."

Slice D (limits snapshot hub) is **not** done.

---

## Operator contract

When both stored SuperGrok logins still have included SuperGrok period limits remaining, spend **Business / Team included first**, then personal.

This is not sooner-reset across mixed personal+Team. Among two Team logins (or two personal), sooner reset then `identity_id` still applies.

Full spend order:

1. Included SuperGrok period limits on stored Business / Team SuperGrok logins that still have remaining.
2. Included SuperGrok period limits on stored personal SuperGrok logins that still have remaining.
3. SuperGrok dollar credits (never expire).
4. Console team prepaid / console API credits.

If Team included is exhausted and personal still has remaining, stay on personal included. If only personal is stored, behavior is unchanged. No grok.com account-switcher OAuth.

---

## Red (observed, then product)

Command:

```bash
cargo test -p xai-grok-shell --lib -- \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  -- --test-threads=1
```

**1. `pick_prefers_business_included_before_personal_when_both_have_remaining`**

- Personal remaining 80, reset sooner. Business remaining 20, reset later.
- Fail: `left: Use { identity_id: "personal-1", role: Personal }`  
  `right: Use { identity_id: "business-1", role: Business }`
- Message: Business included SuperGrok period limits beat personal included even when personal resets sooner and has more remaining.

**2. `order_credentials_business_included_before_personal_when_both_have_room`**

- Fail: `primary: Some("tok-personal-included")`, expected `Some("tok-business-included")`.
- Failover was Business. Console was omitted (that part was already correct).

Same-role sooner-reset asserts in the pick test did not fail (that ranking was already correct).

---

## Product

Smallest change: one compare used by both pick and order.

- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`
  - `cmp_included_headroom_rank`: Business class before Personal, then sooner reset, then `identity_id`.
  - `pick_supergrok_identity_for_auto` and `sort_live_supergrok_by_reset` both use that compare.
  - Role still comes from existing `SupergrokAccountRole` / `role_from_session_fields` (Team principal + `team_id`).

Align and per-turn reconstruct already call these helpers, so they pick Team when both have remaining.

---

## Existing tests updated (named operator contract, not weakened)

These tests picked personal because of sooner reset or lex `identity_id` while both had remaining. They now encode: Business included beats personal included.

| Test | Old expect | New expect |
|------|------------|------------|
| `both_have_headroom_personal_resets_sooner` | personal | Business |
| `equal_reset_tiebreak_by_identity_id_not_business_first` | lex personal (`aaa-per`) | Business (`zzz-biz`); same-role lex kept for two Team ids |
| `unknown_reset_sorts_after_known` | personal known reset | Business even with unknown reset; same-role unknown-after-known kept |
| `ranked_free_period_primary_personal_when_equal_headroom_not_sticky_business` | lex personal JWT | Business JWT; sticky personal must align |
| `auto_order_not_business_first_when_personal_resets_sooner` | personal primary (was not even `#[test]`) | Business primary; now a real test |
| `align_to_ranked_free_period_primary_switches_sticky_team_base_to_personal` | sticky Team → personal | sticky personal → Business |
| `auth_manager_new_auto_use_aligns_sticky_team_base_to_ranked_free_period_primary` | new() wires personal | new() wires Business from sticky personal |

`business_exhausted_personal_used` is unchanged: Team full + personal remaining stays on personal included.

---

## Green

```bash
cargo test -p xai-grok-shell --lib -- \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  -- --test-threads=1
# 2 passed (same tests that were red)

cargo test -p xai-grok-shell --lib -- supergrok_identity_rank::tests -- --test-threads=1
# 42 passed
```

Keep-green (sibling hop, single-identity extras, combined remaining, afterburner sibling skip, align after billing, per-turn reconstruct):

```bash
cargo test -p xai-grok-shell --lib -- \
  order_credentials_personal_full_with_extras_hops_to_business_included_before_extras \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  apply_billing_marks_personal_full_when_business_sibling_has_included \
  sampling_config_hops_to_sibling_included_before_extras \
  sampling_config_auto_use_extras_keep_session_console_failover \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool \
  align_to_ranked_free_period_primary_switches_sticky_team_base_to_personal \
  auth_manager_new_auto_use_aligns_sticky_team_base_to_ranked_free_period_primary \
  -- --test-threads=1
# 11 passed
```

```bash
cargo fmt -p xai-grok-shell           # FMT_EXIT:0
cargo clippy -p xai-grok-shell --lib -- -D warnings   # CLIPPY_EXIT:0
```

---

## Pins

- Plan section 2 (`.agents/plans/token-economy-all-plans-ipc.md`): Business / Team included first, then personal included, then extras, then console.
- `doc/dev/upstream-regression-filters.md` class 5: both new `fn` names.
- `FORK.md` land cheat sheet dual-auth block: same two names.

---

## Honest leftovers

1. **Slice D is not done.** No flock-backed limits snapshot hub.
2. **Second SuperGrok login is operator-gated.** Rank only sees what is already in `auth.json`.
3. **Live TUI stays old until rebuild / re-exec.**
4. Align test **function names** still say `...sticky_team_base_to_personal` even though the contract is now sticky personal to Business. The bodies match the operator order. Renaming the `fn`s is leftover polish, not a rank bug.

No next `/implement` prompt. Slice D belongs to the next implementer.
