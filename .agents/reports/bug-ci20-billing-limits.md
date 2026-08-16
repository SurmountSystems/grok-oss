# Report: `impl:ci20-billing-limits` (four billing / limits unit tests)

**Board:** `impl:ci20-billing-limits` under `bug:ci-20-unit-fails`
**Date:** 2026-08-14
**Packages:** `xai-grok-pager`, `xai-grok-shell`
**Status:** named tests green; spend-order keep-green still green

Isolated compile: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-billing-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`. rustc 1.97.1.

Did **not** undo Business-before-personal rank, hop sibling included before SuperGrok dollar credits, or combined remaining for multi-pool chrome. Did **not** touch `supergrok_identity_rank.rs`.

## Red (per named test)

Reproduced with isolated `cargo test` (not only `just ci`). First cold cargo was killed at the 120s host wrapper; retry with a longer timeout reproduced the four fails below.

### `views::credit_bar::tests::test_boundary_at_80_percent`

Observed:

```
assertion `left == right` failed
  left: Some(Rgb(224, 175, 104))   # theme.warning
 right: Some(Rgb(158, 206, 106))   # theme.accent_success
```

`credit_bar_line(&bal(79.9), …)` must paint success. Combined included remaining floors remaining, then rebuilds used percent. One pool at 79.9 became 80. Color then used that reconstructed percent, so just-below-80 painted warning.

`bal(80.0)` warning was already correct. The fail is the 79.9 color, not the 80.0 color.

### `views::limits_honesty::tests::branch_2b_stack_base_flat_and_c6_when_evidence`

Observed:

```
must keep C4 honesty (no invent debit): Note: included SuperGrok period limits can stay
flat … product does not invent included-period debit and does not treat team settlement
as SuperGrok dollar extras.
```

The test required the substring `does not invent free-period debit`. Product copy already says `does not invent included-period debit` (SuperGrok is paid). C4 honesty (do not invent debit) was still in the note. This was a vocabulary miss, not a weaker honesty bar.

### `auth::allowance_exhaust_from_billing::tests::apply_billing_100_pct_marks_session_when_dual_auth_ready`

Observed:

```
assertion `left == right` failed
  left: None
 right: Cleared
```

`apply_billing_usage_to_session_exhaust(100.0)` still returned `Marked`. The next apply at 12% must return `Cleared` (that is how the test proves the mark existed). It returned `None`.

Cause: `apply_billing_usage_to_session_exhaust_inner` always called `any_sibling_has_included_remaining`, which calls `load_supergrok_session_candidates`. Candidate load enriches and **clears** exhaust memos when live used percent is below 100. That ran **before** `sync_allowance_exhaust_from_usage`. Sync then saw no memo and returned `None`, so `Cleared` never fired.

### `auth::allowance_exhaust_from_billing::tests::period_reset_clears_memo_and_ranks_supergrok_primary_without_console`

Same apply-path swallow. After `Marked` at 100%, apply at 7% (period reset) must return `Cleared` and rank SuperGrok primary without console. Load-candidates cleared the memo first, so apply returned `None` and the rank asserts never ran.

This was deterministic in isolated `cargo test`, not only a nextest cache-pollution flake.

## Product change

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Color threshold uses live `balance.usage_pct` when `combined.distinct_pool_count <= 1`. Multi-pool chrome still uses combined reconstructed used percent. 79.9 stays success; 80.0 stays warning. |
| `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` | C4 assert accepts `does not invent included-period debit` or `does not invent included SuperGrok period debit`. Named contract: SuperGrok is paid; this is a vocabulary rename, not a weaker honesty bar. Product note text unchanged. |
| `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` | Sibling remaining is loaded only when `usage_pct >= INCLUDED_ALLOWANCE_EXHAUST_PCT` (100). After-burner skip only matters at 100% anyway. Period-reset apply no longer hits `load_supergrok_session_candidates` before sync, so `Cleared` is visible. |

**Test edit (named contract, equal-or-stronger):** only the C4 substring in `branch_2b_stack_base_flat_and_c6_when_evidence`. It still requires "does not invent" plus included-period debit language. It does **not** drop C4. The three other named tests were not rewritten.

**Not changed:** `supergrok_identity_rank.rs`. Combined remaining still floors remaining for multi-pool chrome. After-burner still marks when a sibling has included remaining.

Rejected path: coloring always from combined reconstructed percent (would keep 79.9 red). Rejected path: skipping sibling load always (would break after-burner-with-sibling at 100%). Rejected path: reverting Business-before-personal so period-reset rank matches an older personal-sooner-reset story (that rank assert did not fire; the fail was `Cleared`).

## Green re-run

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-billing-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager -p xai-grok-shell          # FMT_EXIT=0
cargo clippy -p xai-grok-pager --lib -- -D warnings    # CLIPPY_PAGER_EXIT=0
cargo clippy -p xai-grok-shell --lib -- -D warnings    # CLIPPY_SHELL_EXIT=0
cargo test -p xai-grok-pager --lib -- \
  test_boundary_at_80_percent \
  branch_2b_stack_base_flat_and_c6_when_evidence
# 2 passed; PAGER_TEST_EXIT=0
cargo test -p xai-grok-shell --lib -- \
  apply_billing_100_pct_marks_session_when_dual_auth_ready \
  period_reset_clears_memo_and_ranks_supergrok_primary_without_console \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining
# 4 passed; SHELL_TEST_EXIT=0
```

Keep-green both passed: Business included still ranks before personal when both have remaining; after-burner still does not skip the mark when a sibling has included remaining.

## Leftovers

- Combined remaining still floors remaining then rebuilds used percent when two or more distinct included pools are in play. Color at 79.9 vs 80.0 is live only for a single pool.
- `load_supergrok_session_candidates` still clears exhaust memos on the remember-only path when live used percent is below 100. `load_candidates_period_reset_billing_clears_stale_memo_without_apply` still covers that. Apply at below-100 no longer calls that load first.
- Test comments on `period_reset_clears_memo_and_ranks_supergrok_primary_without_console` still say "free SuperGrok period" in a few places. Product copy does not. SuperGrok is paid. Left those comments (out of named-fail scope).
- Did not re-run full `just ci` / nextest. Other crate filters stayed with their implementers.
- Stayed off settings_e2e, prompt_widget, dashboard peek, router initializer, session_loaded, turn_status, agent config env_keys, models prefetch.
