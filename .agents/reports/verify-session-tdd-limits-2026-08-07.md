# Verify report: session TDD + limits-before-credits honesty (2026-08-07)

**Plan:** `.agents/plans/plan-verify-tdd-limits-2026-08-07.md`
**Inventory:** `.agents/reports/plan-verify-tdd-limits-inventory-2026-08-07.md`
**Tree tip:** `c87f66a` (`fixes`)
**Binary after install:** `grok-oss 0.2.111 (c87f66a61d94) [stable]`
**Binary SHA256:** `7f71087b3b4a808f774cbaf199f4a9d21b25a25a98bf2944b685dd6d03360da7`
**Product goal:** free SuperGrok period (limits) before SuperGrok $ extras and console credits.
**Honesty:** free period still ~6% after dogfood is Design A chrome + C4 server residual. No client invent of free-period debit.

---

## 1. Wave A filter batches

All exit codes **0**. No product fixes required (no reds).

| Batch | Command (summary) | Exit | Passed | Failed | Ignored |
|-------|-------------------|------|--------|--------|---------|
| Limits primary (pager) | `cargo test -p xai-grok-pager --lib -- check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b format_supergrok_session active_driver status_bar_free_period sticky_memo` | 0 | **57** | 0 | 1 (`live_check_limits_first_from_env_json`) |
| Rank / allowance (shell) | `cargo test -p xai-grok-shell --lib -- auto_order_omits_console auto_with_included_headroom auto_after_included allowance_exhaust_from_billing out_of_allowance_helper` | 0 | **35** | 0 | 0 |
| prefer_live (sampler) | `cargo test -p xai-grok-sampler --lib -- prefer_live exhausted` | 0 | **30** | 0 | 0 |
| Session ship (pager) | `cargo test -p xai-grok-pager --lib -- soft_park_empty_ctrl_c_abandons plan_panel_empty_ctrl_c_abandons plan_approval_ctrl_c_clears_draft quit_mid_turn_writes_canceled quit_idle_does_not_write slash::commands::rebuild dispatch::rebuild` | 0 | **8** | 0 | 0 |
| Session ship (shell) | `cargo test -p xai-grok-shell --lib -- leader_is_older_than parse_binary decide_relaunch canceled_turn_resume process_shutdown_class auth_failed_poll order_live_prefers_poll_ok sibling_poll_skips session_needs_oidc_refresh non_active_poll_targets` | 0 | **19** | 0 | 0 |
| rebuild (update) | `cargo test -p xai-grok-update --lib -- rebuild::` | 0 | **4** | 0 | 0 |
| also-guard (tools) | `cargo test -p xai-grok-tools --lib -- live_demote_guard todo_bound_task_id` | 0 | **5** | 0 | 0 |
| plan template (agent) | `cargo test -p xai-grok-agent --lib -- test_base_template_plan_present_includes_planning` | 0 | **1** | 0 | 0 |
| Dual-poll honesty (shell) | `cargo test -p xai-grok-shell --lib -- auth_failed_poll_demotes_included_usage_pct_not_fresh_headroom billing_fail_note_names_role_fingerprint_and_relogin remember_poll_ok_sets_outcome_ok order_live_prefers_poll_ok_supergrok_over_auth_failed format_human_dual_poll_health_names_auth_failed_role` | 0 | **5** | 0 | 0 |
| Design A smoking gun | `cargo test -p xai-grok-pager --lib -- compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` | 0 | **1** | 0 | 0 |

**Total named-filter passes observed this run:** 57+35+30+8+19+4+5+1+5+1 = **165** (some dual-poll / session names overlap the larger shell batch; those were green in both).

**Regressions:** none.

**TDD note (Wave D):** all green re-verify. Did **not** invent fake red for already-shipped code. Documented as re-verified green per plan Wave D.

---

## 2. Design A smoking gun

| Field | Value |
|-------|--------|
| Test | `views::credit_bar::tests::compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` |
| Result | **ok** (exit 0) |
| Asserts | Free period **6.0%** used + sticky exhaust memo claiming out + team prepaid **$340** + SuperGrok extras on account → compact meter text is exactly **`6%`**, identity stays SuperGrok session, no `console`, no `$` / `340`, no `extras` |

Source: `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` (~3139).

---

## 3. Wave B: install

```text
just install  → exit 0
grok-oss 0.2.111 (c87f66a61d94) [stable]
SHA256 7f71087b3b4a808f774cbaf199f4a9d21b25a25a98bf2944b685dd6d03360da7
path: ~/.cargo/bin/grok-oss
```

Installed identity matches tree tip SHA prefix `c87f66a`.

---

## 4. Wave C: live `limits --json` (post-install)

Command: `~/.cargo/bin/grok-oss limits --json` (exit 0). Captured evening 2026-08-07.

| Field | Live value | Expect / pass |
|-------|------------|---------------|
| `liveSampling` | `supergrok_session` | SuperGrok session |
| `livePrincipalRole` | `business` | OK |
| `activeDriver` | **`supergrok_free_period`** | free SuperGrok period while used &lt; 100% |
| Free period used (business) | **6.0%** (`live_poll`, OK) | honest server reading; do not invent higher |
| Free period used (personal) | **6.0%** (`live_poll`, OK) | shared pool |
| Free period remaining | 94% | headroom present |
| Next reset | August 10, 19:25 | weekly |
| SuperGrok $ extras | **$100.29** | side meter; not live driver |
| `console.isLive` | **false** | under free-period headroom |
| `console.keyAvailable` | true | key present, not live |
| Team prepaid | $340 | side meter |
| Team postpaid period | $1018.63 | |
| Team postpaid OAuth / Grok Build class | **$1012.83** | climbing vs free period flat |
| Team postpaid API class | $5.80 | |
| Team usage series OAuth class | ~$855.37 | Build-class settlement |

Human form agrees: **Active: free SuperGrok period**, included weekly **6% used · 94% remaining**, console requests labeled SuperGrok (not live console key).

### Wave C checks

| Check | Pass? |
|-------|-------|
| activeDriver free SuperGrok period while used &lt; 100% | **Yes** |
| console.isLive false under free-period headroom | **Yes** |
| includedUsedPct recorded honestly (~6%) | **Yes** (still 6.0%) |
| No client invent of higher free-period % | **Yes** |

---

## 5. Honesty split

| Layer | Status |
|-------|--------|
| **Limits-before-credits client (Design A)** | **OK.** Unit contracts green; live `activeDriver=supergrok_free_period`, `console.isLive=false`, compact path proven to prefer free-period **%** over console / extras dollars under headroom. |
| **C4 free-period absorption (server)** | **Open.** Free period still **6.0%** while team OAuth / Grok Build class sits at **~$1013** and continues to dominate settlement. Client must not invent debit. See C4 addendum. |

**Reading still-6% after dogfood:** expected Design A chrome honesty + C4 residual. Not a failed install. Not a failed limits-before-credits filter.

---

## 6. Product fixes this run

**None.** Wave A had zero failures. No red→green product edit. No fmt/clippy package pass required for product code (no edits).

---

## 7. Wave E residual

### 7.1 TDD hygiene gaps (inventory §2)

| Contract | This run |
|----------|----------|
| Limits / prefer_live / sticky | Re-verified green (no new red; assert-only not required) |
| Dual SuperGrok poll honesty | Re-verified green (5 named filters) |
| `/rebuild` vertical | Re-verified green (identity + rebuild:: + slash/dispatch) |
| Killall / canceled_turn_resume | Re-verified green |
| Also-guard demote | Re-verified green (5 tools tests) |
| Auto-bind + sticky-on-new-message | **Still soft residual** (not shipped; not invent-closed) |
| C4 free-period debit unit test | **Forbidden** — no client filter can prove server debit |

Did **not** add theater tests that rewrite weak asserts or invent fake observed-red for green product. Existing contracts already guard regression for Design A paint and demote.

### 7.2 C4 ticket addendum

Written: `.agents/reports/c4-ticket-addendum-2026-08-07.md`
Ready for operator paste to xAI. Not a product code fix.

### 7.3 `/rebuild` TUI glitch (`bug:rebuild-tui-glitch`)

Light diagnosis only (no TTY dogfood of live multi-TUI `/rebuild` this run).
Report: `.agents/reports/bug-rebuild-tui-glitch-diagnosis-2026-08-07.md`
**Board stays open** — no invented paint fix without observed red + root cause.

### 7.4 Complete verticals

Waves A–C complete (not parked). Report complete with live numbers and honesty split.

---

## 8. fmt / clippy

No product source edits → no fmt/clippy required this run.

---

## 9. Done criteria (plan)

| Criterion | Met? |
|-----------|------|
| All Wave A filters green (or fixed red→green) | **Yes** (all green; no fix needed) |
| Installed binary matches tree SHA/version | **Yes** `0.2.111 (c87f66a61d94)` |
| Live free-period-first + free period % recorded honestly | **Yes** (6.0%, activeDriver free period, console not live) |
| Report states client OK vs C4 open | **Yes** (§5) |
| No invented debit | **Yes** |

---

## Commands log (copy-paste)

```bash
# A — limits primary
cargo test -p xai-grok-pager --lib -- \
  check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty \
  limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b \
  format_supergrok_session active_driver status_bar_free_period sticky_memo
# → 57 passed; 0 failed; 1 ignored

# A — shell rank
cargo test -p xai-grok-shell --lib -- \
  auto_order_omits_console auto_with_included_headroom auto_after_included \
  allowance_exhaust_from_billing out_of_allowance_helper
# → 35 passed

# A — sampler
cargo test -p xai-grok-sampler --lib -- prefer_live exhausted
# → 30 passed

# A — session pager
cargo test -p xai-grok-pager --lib -- \
  soft_park_empty_ctrl_c_abandons plan_panel_empty_ctrl_c_abandons \
  plan_approval_ctrl_c_clears_draft \
  quit_mid_turn_writes_canceled quit_idle_does_not_write \
  slash::commands::rebuild dispatch::rebuild
# → 8 passed

# A — session shell
cargo test -p xai-grok-shell --lib -- \
  leader_is_older_than parse_binary decide_relaunch \
  canceled_turn_resume process_shutdown_class \
  auth_failed_poll order_live_prefers_poll_ok sibling_poll_skips \
  session_needs_oidc_refresh non_active_poll_targets
# → 19 passed

cargo test -p xai-grok-update --lib -- rebuild::   # 4
cargo test -p xai-grok-tools --lib -- live_demote_guard todo_bound_task_id  # 5
cargo test -p xai-grok-agent --lib -- test_base_template_plan_present_includes_planning  # 1

# Design A smoking gun
cargo test -p xai-grok-pager --lib -- \
  compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars
# → 1 passed

# Dual-poll honesty
cargo test -p xai-grok-shell --lib -- \
  auth_failed_poll_demotes_included_usage_pct_not_fresh_headroom \
  billing_fail_note_names_role_fingerprint_and_relogin \
  remember_poll_ok_sets_outcome_ok \
  order_live_prefers_poll_ok_supergrok_over_auth_failed \
  format_human_dual_poll_health_names_auth_failed_role
# → 5 passed

# B
just install
~/.cargo/bin/grok-oss --version

# C
~/.cargo/bin/grok-oss limits --json
```
