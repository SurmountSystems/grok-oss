# Plan inventory: verify shipped work with red/green TDD (2026-08-07)

**Goal:** build+test everything shipped this session; free SuperGrok period / limits before credits remains primary; free period still ~6% after dogfood is expected client honesty.

**Sources:** `impl-limits-before-credits`, `impl-dual-supergrok-billing-honesty`, `impl-rebuild-slash`, `impl-ctrl-c-killall-resume-also-guard`, `bug-plan-approval-ctrl-c`, `still-6pct-chrome`, `how-to-fix-c4-free-period-debit`, plus related live/verify reports same day.

---

## 1. Named filters + packages (prove each shipped contract)

| Ship | Package | Filter / test names (from reports) |
|------|---------|-------------------------------------|
| **Limits-before-credits (sticky, driver, Design A)** | `xai-grok-pager` | `compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars`; `status_bar_free_period_headroom_not_console_prepaid_dollars`; `active_driver_free_period_headroom_even_with_extras_and_team_prepaid`; `limits_json_active_driver_free_period_with_extras_on_account`; `active_driver_afterburner_extras_when_free_period_full`; `limits_json_active_driver_extras_afterburner`; `status_identity_sticky_console_when_free_period_full_and_memo_out`; `c6_team_usage_note_when_oauth_postpaid_dominates`; `branch_2b_stack_base_flat_and_c6_when_evidence`; `compact_status_supergrok_on_extras_*` / `status_bar_supergrok_on_extras_*`. Bundle: `check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b format_supergrok_session active_driver status_bar_free_period sticky_memo` |
| **Rank / prefer free period** | `xai-grok-shell` | `auto_order_omits_console_while_any_supergrok_included_headroom`; `auto_with_included_headroom_still_omits_console`; `check_limits_first_*`; `auto_order_omits_console auto_order_keeps_supergrok auto_with_included_headroom auto_after_included format_human_auto_use allowance_exhaust_from_billing out_of_allowance_helper` |
| **prefer_live** | `xai-grok-sampler` | `prefer_live exhausted` |
| **Dual SuperGrok poll honesty** | `xai-grok-shell` | `auth_failed_poll_demotes_included_usage_pct_not_fresh_headroom`; `billing_fail_note_names_role_fingerprint_and_relogin`; `remember_poll_ok_sets_outcome_ok`; `order_live_prefers_poll_ok_supergrok_over_auth_failed`; `format_human_dual_poll_health_names_auth_failed_role` (+ dual upsert/billing bundle in impl report) |
| | `xai-grok-pager` | `dual_fill_provenance_not_live_poll_and_names_role`; `compact_status_active_auth_failed_not_sibling_free_period_pct`; `format_dual format_unified_fills limits_honesty` |
| **`/rebuild` + CLI** | `xai-grok-shell` | `leader_is_older_than`; `parse_binary`; `decide_relaunch` |
| | `xai-grok-update` | `rebuild::` |
| | `xai-grok-pager` | `slash::commands::rebuild`; `dispatch::rebuild` |
| **Ctrl+C plan approval** | `xai-grok-pager` | `soft_park_empty_ctrl_c_abandons_plan_approval`; `plan_panel_empty_ctrl_c_abandons_plan_approval`; `plan_approval_ctrl_c_clears_draft_then_second_abandons` |
| **Killall / SIGTERM resume** | `xai-grok-pager` | `quit_mid_turn_writes_canceled_turn_resume_marker`; `quit_idle_does_not_write_canceled_turn_resume_marker` |
| | `xai-grok-shell` | `canceled_turn_resume`; `process_shutdown_class_marker_is_auto_resume_eligible` |
| **Multi-track also-guard (first cut)** | `xai-grok-tools` | `live_demote_guard_rejects_bound_running_to_pending`; `live_demote_guard_allows_complete_and_cancel_while_bound`; `live_demote_guard_allows_unbound_demote`; `live_demote_guard_allows_bound_when_subagent_finished`; `todo_bound_task_id_reads_camel_case_task_id` |
| | `xai-grok-agent` | `test_base_template_plan_present_includes_planning` |
| **Related same-day (optional wave)** | pager/shell | license zeros / C6: `views::limits_honesty::`, `limits_cmd::`; usage series: `auth::xai_management::`; console-dead recovery filters in that impl report |

---

## 2. Gaps: weak or missing observed red→green

| Contract | Gap |
|----------|-----|
| **Limits-before-credits rank / prefer_live** | Explicitly **already green** audit (no new red). Sticky pin + activeDriver had new tests with red intent; paint path was the real bug. |
| **Dual SuperGrok billing honesty** | Report admits **one-pass** product + tests then green; not classic observed-fail-then-edit for every row. Keep-green only for pre-existing fill tests. |
| **`/rebuild` vertical** | Green table only; no report log of pre-edit red. Live multi-TUI dogfood operator-only. |
| **Killall resume** | Tests named with red→green *intent*; report does not quote a failed run before the product edit. |
| **Also-guard** | Green filters listed; auto-bind + sticky-on-new-message **not** shipped (soft residual). Explore report predated first cut. |
| **C4 free-period debit** | **No client unit filter can prove debit.** Dogfood multipoll + xAI ticket only. Invent debit banned. |
| **Ctrl+C plan** | **Strongest TDD:** bug report logs observed red then same tests green. |

---

## 3. Limits-before-credits unit filters (focus)

| Concern | Test / filter |
|---------|----------------|
| **Sticky pin:** free period headroom → compact **`6%`**, not **`console · $340`** | `compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` |
| **Paint path** same shape | `status_bar_free_period_headroom_not_console_prepaid_dollars` |
| **Sticky still console when free period full + memo** | `status_identity_sticky_console_when_free_period_full_and_memo_out` |
| **`activeDriver` free period** with extras + team prepaid | `active_driver_free_period_headroom_even_with_extras_and_team_prepaid`; `limits_json_active_driver_free_period_with_extras_on_account` |
| **After-burner** free full + extras | `active_driver_afterburner_extras_when_free_period_full`; `limits_json_active_driver_extras_afterburner` |
| **Compact 6% not console** (Design A) | sticky headroom tests above; after-burner extras only when free period ≥ 100% |

**Suggested verify command (from impl report):**

```bash
cargo test -p xai-grok-pager --lib -- \
  check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty \
  limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b \
  format_supergrok_session active_driver status_bar_free_period sticky_memo
cargo test -p xai-grok-shell --lib -- \
  auto_order_omits_console auto_with_included_headroom auto_after_included \
  allowance_exhaust_from_billing out_of_allowance_helper
cargo test -p xai-grok-sampler --lib -- prefer_live exhausted
```

---

## 4. One-line: is 6% client honesty vs C4 server?

**Still ~6% free SuperGrok period used after dogfood is expected client honesty (Design A free-period-first chrome + live poll); C4 is the separate server residual that free period / Build productUsage may not debit under SuperGrok session load.**

---

## Verify plan notes (for implementer)

1. Re-run filters in tables 1 and 3 on this tree (fmt + clippy packages touched if any edit).
2. Do not invent free-period debit tests that assert % must climb under load.
3. Operator dogfood: rebuild binary; `limits --json` → `activeDriver=supergrok_free_period`, compact `6%`, not false console dollars.
4. C4: escalate with evidence package; not a red/green product closeout.
