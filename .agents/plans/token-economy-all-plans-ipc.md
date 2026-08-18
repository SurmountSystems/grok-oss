# Spend included SuperGrok period limits on every stored plan before dollar credits

**Status:** proposed plan only. Not accepted until the operator approves via the plan panel or an explicit freeform approve.

**Scope:** make the live token economy check every stored SuperGrok login and console key, add remaining included SuperGrok period limits across distinct plans, hop to the next plan that still has included SuperGrok period limits before never-expiring SuperGrok dollar credits, and let only one `grok-oss` process call the limits APIs. SuperGrok is paid. Meters stay distinct: included SuperGrok period limits, SuperGrok dollar credits, console team prepaid / console API credits.

This plan is grounded in code and named tests. Docs and residual can be stale.

---

## 1. Problem

Hunter has personal SuperGrok Heavy and a Business / Team SuperGrok Heavy plan (grok.com account switcher: Hunter Beast personal vs Surmount Team). The live token economy never switched to the Business SuperGrok Heavy plan when included SuperGrok period limits on the current plan were full. He wants:

1. Check limits across every stored key and plan.
2. Add remaining included SuperGrok period limits together. That sum is the real remaining included quota.
3. Always spend included SuperGrok period limits (across those keys and plans) before SuperGrok dollar credits that never expire.
4. When one plan's included SuperGrok period limits are full, switch to the next plan that still has included SuperGrok period limits, not immediately to never-expiring credits.
5. Only one `grok-oss` process may call the rate-limit / limits APIs. Other live TUIs get the snapshot over IPC. Otherwise the limits calls themselves get rate-limited.

---

## 2. Proposed spend order (plain English)

1. **Included SuperGrok period limits on stored Business / Team SuperGrok logins that still have remaining.** When personal and Team both still have included remaining, spend Team included first. This is not sooner-reset across mixed personal+Team. Among two Team logins (or two personal), keep sooner reset then `identity_id`.
2. **Included SuperGrok period limits on stored personal SuperGrok logins that still have remaining.** If Team included is exhausted and personal still has included remaining, stay on personal included. Do not jump to SuperGrok dollar credits yet. If only personal is stored, behavior is unchanged.
3. **The real remaining included quota is the sum of remaining included SuperGrok period limits across distinct pools.** If two rows are the same unified pool (wire `is_unified_billing_user`, or the same included used percent plus the same reset), count that pool once. Do not invent included SuperGrok period used percent on the client.
4. **Only after every distinct included pool is full,** spend SuperGrok dollar credits (never-expiring Extra Usage Credits / `prepaidBalance`) on a live SuperGrok session.
5. **Only after SuperGrok dollar credits are gone or unknown,** use console team prepaid / console API credits.

While any distinct included SuperGrok period pool still has room, stay on SuperGrok session. Do not make the console API key primary. Do not paint SuperGrok extras as the live driver.

This is the same three-meter order already named in `AGENTS.md` and `FORK.md`, with two mechanical clauses: **next plan's included SuperGrok period limits beat this plan's SuperGrok dollar credits**, and **Business / Team included SuperGrok period limits beat personal included while both still have remaining.**

---

## 3. What the tree already does

### 3.1 Spend order and hop (one plan, then extras, then console)

Pure ranking lives in `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`.

- `pick_supergrok_identity_for_auto` prefers any identity with `included_remaining > 0`, sooner `reset_at` first, then `identity_id`. Tests: `one_exhausted_other_with_headroom`, `personal_exhausted_business_used`, `both_exhausted_signals_need_console_or_dollars`.
- `order_credentials_for_preferred_auto` omits console keys while any SuperGrok candidate still has included remaining. After every included pool is exhausted, SuperGrok dollar credits stay primary with console as failover. Tests: `auto_hop_after_one_supergrok_exhaust_uses_other_before_console`, after-burner tests around `auto_afterburner_*`.
- `included_remaining_from_usage_pct` maps used percent ≥ 100 to remaining 0, else at least 1.

Sampler hop after a memoized out-of-allowance mark lives in `crates/codegen/xai-grok-sampler/src/exhausted_identity.rs` (`sync_allowance_exhaust_from_usage`) and `prefer_live_primary.rs` (`prefer_live_identity_after_credit_exhaust`). Catalog land tests in `crates/codegen/xai-grok-shell/src/agent/config_tests.rs`:

- `sampling_config_auto_use_fills_console_hop_after_included_full`
- `sampling_config_auto_use_omits_console`
- `resolve_model_to_sampling_config_auto_use`
- `sampling_config_auto_use_extras_keep_session_console_failover`

Those catalog tests use **one** SuperGrok candidate. They do not lock "next plan's included before this plan's extras."

### 3.2 Identities and keys (what the product can see)

The product can store two SuperGrok OIDC principals in `$GROK_HOME/auth.json`. There is **no** grok.com account-switcher OAuth flow in this tree. A second SuperGrok plan is visible only after a second `grok login` that upserts a multi-slot.

| Kind | Where it lives | What it is |
|------|----------------|------------|
| SuperGrok OAuth session (active base) | `auth.json` base OIDC scope | Last login. AuthManager refresh and SessionToken bearer. |
| SuperGrok personal multi-slot | `{base}::personal` | Sibling personal principal. |
| SuperGrok Business / Team multi-slot | `{base}::team::{team_id}` | Sibling Business principal when `principal_type` is Team and `team_id` is set. |
| Console API key | keyring / `XAI_API_KEY` / `xai::api_key` | Different meter class. Not included SuperGrok period limits. |
| Management team prepaid | keyring URL `https://management-api.x.ai` plus `[endpoints] management_team_id` | Console team prepaid / postpaid. Not a second SuperGrok login. |

Code:

- `upsert_supergrok_session` / `multi_slot_scope_for_auth` in `crates/codegen/xai-grok-shell/src/auth/model.rs`. Tests: `upsert_personal_then_business_keeps_both_multi_slots`.
- Live login: `team_login_then_personal_keeps_both_principals` in `crates/codegen/xai-grok-shell/src/auth/manager_tests.rs`.
- Doctor / `grok login --list-api-keys`: `DualAuthStatus.supergrok_principals` in `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs`.
- Ranking load: `load_supergrok_session_candidates` in `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`. Dedupes by `identity_id`. Default remaining is 1 when the JWT is not hard-expired and not memoized exhausted.

If the operator has only ever logged into grok-oss once, the Business SuperGrok Heavy plan is **not** in `auth.json`. grok.com's account switcher is a different product. Do not invent a workspace-switcher OAuth flow in this plan.

### 3.3 Limits fetch today (every process, process-local cache)

Active path: pager `Effect::FetchBilling` sends ACP `x.ai/billing`. Shell `handle_get_billing` in `crates/codegen/xai-grok-shell/src/extensions/billing.rs` calls `GET {proxy}/billing?format=credits` with the **active** SuperGrok JWT. Comment in that file: every prompt / `/usage` / poll path hits this endpoint.

Sibling path: `poll_and_remember_non_active_supergrok_included_billing` then polls every other stored SuperGrok JWT on the same credits URL. Failures are best-effort. After three consecutive auth-class fails, automatic sibling re-poll skips that identity (`SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD`).

CLI: `grok limits` / `grok limits --json` in `crates/codegen/xai-grok-pager/src/limits_cmd.rs` does the same credits fetch plus Management prepaid/postpaid/series.

Caches (not a shared snapshot):

- Process mutex maps in `allowance_exhaust_from_billing.rs` (`INCLUDED_BILLING_BY_IDENTITY`, poll outcomes). Cleared with the process.
- Management meters: 60s process TTL in `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`.
- Durable included poll **history** (flat-detector samples only) under `$GROK_HOME/included_poll_history/{identity}.json` in `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs`. Not a live snapshot other TUIs read for chrome.
- Local spend ledger: `$GROK_HOME/grok_oss.db` plus session `usage.jsonl`. Settlement book, not live included remaining.

`grok-rate-limit` (`crates/codegen/grok-rate-limit/`) is a **cooldown flock** under `$GROK_HOME/rate_limits/`. Every process still issues the HTTP call after `not_before`. It does not elect a fetcher and it does not publish a snapshot.

### 3.4 Existing "one process, others listen" patterns (none is a limits hub)

| Pattern | Path | What it does | Reuse? |
|---------|------|--------------|--------|
| Shared rate-limit flock | `crates/codegen/grok-rate-limit/src/store.rs` | Cooldown JSON + flock. Every process still calls the API. | Steal the flock/JSON style. Do not turn it into a second daemon. |
| Active session PID list | `crates/codegen/xai-grok-active-sessions/src/lib.rs` | `$GROK_HOME/active_sessions.json` plus lock. Crash recovery. | Use live PIDs to know who may be a follower. |
| Rebuild SIGUSR1 fleet | `crates/codegen/xai-grok-update/src/rebuild.rs` | Writes `rebuild_relaunch_request.json` and SIGUSR1 other TUIs to re-exec. | Do **not** reuse SIGUSR1 for limits. That signal means rebuild relaunch. |
| TUI leader unix socket | pager `leader_cluster`, shell `leader/` | Multi-client **session** attach for one working directory. | Do not overload this as a host-wide limits hub. |
| Workspace daemon flock | `xai-grok-workspace-daemon` | Single-instance workspace watcher. | Unrelated. Do not add a second daemon. |

There is **no** limits leader and **no** shared live limits snapshot today. Do not invent a second daemon. Add a flock-backed snapshot file under `$GROK_HOME`, same spirit as `rate_limits/` and `included_poll_history/`.

### 3.5 Status chrome (one identity, not a sum)

`compact_meter_text_for_live_identity_with_active_poll` in `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` paints one reading:

- SuperGrok live + included used percent below 100: `included SuperGrok period limits · N%`
- SuperGrok live + included ≥ 100 + positive extras: `SuperGrok extras · $N`
- Cold or active poll auth-failed: `included SuperGrok period limits · ...%`

`active_spend_driver` uses the same single-identity percent. `/limits` can show two SuperGrok rows (`LimitsSnapshot::extra_principals`) and may mark `shared_unified_supergrok_pool` when `is_unified_billing_user` is true or both rows share the same included percent and reset (`crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`). Tests: `format_dual_principals_keep_distinct_included_pct`, `format_dual_unified_same_included_explains_shared_pool_not_console_business`.

Compact status land test: `status_bar_pushes_credits_compact_included_supergrok_period_limits` (one 24% reading).

There is **no** combined remaining included sum in chrome or ranking.

### 3.6 Why Business SuperGrok Heavy never became the spend path

Ranking already hops to a sibling with included remaining **when that sibling is a loaded candidate with remaining > 0**. The live miss is wiring, not the pick helper.

Grounded gaps in code:

1. **After-burner ignores sibling included.** `afterburner_skips_allowance_mark` in `allowance_exhaust_from_billing.rs` returns true when the **active** identity is at ≥ 100% used and that identity has SuperGrok dollar credits, with no sibling check. The apply path then **clears** the out-of-allowance memo so `prefer_live` will not hop. Tests: `afterburner_skips_allowance_mark_pure_policy` and the apply tests that require no mark when extras remain. There is no test "personal 100% + extras + business remaining → do not skip."
2. **Align is not run after billing.** `align_to_ranked_free_period_primary` runs in `AuthManager::new` and in `prepare_sampling_config_for_model` (session setup / model switch). `handle_get_billing` remembers sibling billing but does not align. `prepare_sampler_for_turn` / `reconstruct_full_config` in `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` reuse chat-state credentials and `AuthManager::current_wire_valid()`. They do not re-rank. SessionToken turns therefore stay on the sticky base after personal included fills.
3. **Chrome follows the active poll only.** When personal included hits 100% and extras remain, compact status paints SuperGrok extras even if a sibling Business row still has included remaining.
4. **Second identity may not be stored.** If only one SuperGrok session is in `auth.json`, ranking has nothing to hop to. The product cannot see grok.com's Hunter Beast vs Surmount Team switcher.

---

## 4. Open questions (operator)

Do not invent answers. Park these until the operator replies.

- Does grok-oss already store two SuperGrok logins (personal plus Surmount Team), or is only one SuperGrok session in `auth.json` today? Check with `grok login --list-api-keys` / doctor SuperGrok rows. If only one session is stored, the next plan is a second `grok login` as the Team principal. This plan will not invent a grok.com workspace-switcher OAuth flow.
- When the credits API sets `is_unified_billing_user` or both JWTs return the same included used percent and the same reset, should we treat that as **one** included pool (count once, hopping cannot add included quota) or still hop because Hunter Beast SuperGrok Heavy and Surmount Team SuperGrok Heavy are meant to be distinct Heavy plans? Distinct Heavy plans that report different included percent stay separate and are summed. The unified-pool case is the only fork.

---

## 5. Implement slices (smallest vertical first)

Red/green TDD on every behavior slice. Observe the named test fail, then the smallest product edit. `cargo fmt -p <crate>` then clippy `-D warnings` then the same test filter before handoff.

### Slice A. Discover stored identities (honesty first)

Make "what we can see" a first-class, tested surface.

- `limits --json` and `/limits` already list dual SuperGrok principals when both slots exist. Add a combined **discovered identities** block that names each SuperGrok role plus fingerprint (no secrets) and each console key fingerprint, and says honestly when only one SuperGrok session is stored.
- Doctor dual-auth line already lists principals. Keep that. Add one sentence when only one SuperGrok session is present: included SuperGrok period limits can only be checked for that login until a second `grok login`.

Named tests (red first):

- `limits_json_lists_two_supergrok_principals_when_both_slots_exist` (extend `limits_cmd.rs` fixtures; may already be partly covered; tighten if missing).
- `limits_json_honest_single_supergrok_session_cannot_see_team_plan` (one slot → no invented Business row).

No new login UX in this slice.

### Slice B. Sum remaining included SuperGrok period limits across distinct pools

Pure helper next to ranking (suggested name: `combined_included_remaining` in `supergrok_identity_rank.rs`).

Rules:

- Remaining for one identity stays `included_remaining_from_usage_pct` (percent remaining units, 0 at ≥ 100% used). Do not invent a percent when usage is unknown; unknown identities do not add to the sum.
- Distinct pools: sum remaining units.
- Unified pool (`is_unified_billing_user == true`, or same floored used percent and same reset): count once (max remaining, not 2×).
- Combined used percent for chrome: `100 - floor(sum_remaining / (100 * distinct_pool_count) * 100)`, clamped. Compact label stays `included SuperGrok period limits · N%` while combined remaining > 0. Optional `/limits` line: remaining included SuperGrok period limits across N plans.

Wire `active_spend_driver` and compact meter to the **combined** remaining, not only the active JWT's percent. While combined remaining > 0, driver stays included SuperGrok period limits even if the active JWT is at 100%.

Named tests (red first):

- `combined_included_remaining_sums_distinct_personal_and_business_pools`
- `combined_included_remaining_does_not_double_count_unified_pool`
- `compact_meter_stays_included_while_sibling_pool_has_remaining`
- `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`

Files: `supergrok_identity_rank.rs`, `credit_bar.rs`, `limits_snapshot.rs`, `limits_cmd.rs`.

### Slice C. Hop to the next plan's included SuperGrok period limits before SuperGrok dollar credits

This is the spend-order contract Hunter asked for.

1. Change `afterburner_skips_allowance_mark` (or a new wrapper used by `apply_billing_usage_to_session_exhaust_inner`) so extras skip **only when every distinct included pool is exhausted**. If a sibling still has included remaining, mark the full identity out of allowance so `prefer_live` and rank can hop. Do not skip.
2. After a successful billing refresh (active plus sibling remember), call `align_to_ranked_free_period_primary` when `auto_use_included_limits` is on.
3. `prepare_sampler_for_turn` / `reconstruct_full_config` must use the ranked primary JWT, not a sticky Team/personal base, when auto-use is on and two SuperGrok candidates exist. Smallest path: call `align_to_ranked_free_period_primary` at the start of `prepare_sampler_for_turn` (same as `prepare_sampling_config_for_model` already does). Rebuild `failover_api_keys` from `order_credentials_for_preferred_auto` so the hop list is sibling SuperGrok first, console only after all included pools are empty.
4. Keep `sampling_config_auto_use_extras_keep_session_console_failover` green for the **single-identity** case (included full, extras remain, no sibling). Do not rewrite that contract.

Named tests (red first):

- `order_credentials_personal_full_with_extras_hops_to_business_included_before_extras` in `supergrok_identity_rank.rs`
- `afterburner_does_not_skip_mark_when_sibling_has_included_remaining` in `allowance_exhaust_from_billing.rs`
- `sampling_config_hops_to_sibling_included_before_extras` in `config_tests.rs` (catalog name; add to the land filter list)
- `align_after_billing_switches_sticky_personal_full_to_business_included` in `manager_tests.rs` or a billing unit test
- `prepare_sampler_for_turn_aligns_to_ranked_included_primary` (hermetic; do not require a live network)

### Slice D. One limits fetcher, others read a snapshot

Do not add a daemon. Do not reuse rebuild SIGUSR1.

Add a flock-backed snapshot under `$GROK_HOME` (suggested path: `limits_snapshot.json` plus `limits_snapshot.lock`). Same flock style as `grok-rate-limit` and `included_poll_history`.

- **Leader:** the process that holds the exclusive flock. It is the only process that may call SuperGrok `GET …/billing?format=credits` (active and siblings) and Management prepaid/postpaid/series.
- **Followers:** take a shared lock or wait, read the snapshot, apply it into the same process maps `remember_supergrok_included_billing` already fills. Chrome and ranking then work as today.
- **Stale snapshot:** if the file is older than the existing background TTL (60s Management class; match credits to that window unless tests say otherwise) and this process can take the exclusive flock, it becomes leader and fetches.
- **Dead leader:** flock releases on process exit. Next waiter becomes leader.
- **Live PID list:** `active_sessions.json` can be used only as a hint (who is listening). Flock is the authority.
- **CLI `grok limits`:** explicit collect stays ForceRefresh, but it must either become leader and fetch once, or wait on the leader and then ForceRefresh only if this CLI holds the flock. Never let N TUIs plus a CLI stampede the credits URL.
- **Secrets:** snapshot stores identity ids, used percent, reset, extras cents, poll outcome class. Never JWTs or API keys.

Suggested module: `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs` (or under `token_economy/` if that stays shell-only). Call it from `handle_get_billing`, `fetch_credits_config_with_session`, and `limits_cmd` collect.

Named tests (red first):

- `limits_snapshot_second_process_reads_file_and_does_not_http` (two temp homes or one home, mock HTTP counter)
- `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`
- `limits_snapshot_never_writes_access_tokens`
- `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http` (mock)

Honor `GROK_DISABLE_SHARED_RATE_LIMIT` as a kill-switch for coordination (each process fetches) so tests that need isolation stay hermetic.

---

## 6. Named files to change

| File | Why |
|------|-----|
| `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | Combined remaining helper; hop-before-extras order test and any order tweak. |
| `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` | After-burner sibling gate; apply path; candidate load already here. |
| `crates/codegen/xai-grok-shell/src/auth/manager.rs` | Align after billing; keep `align_to_ranked_free_period_primary`. |
| `crates/codegen/xai-grok-shell/src/extensions/billing.rs` | After remember, align; route fetches through the snapshot hub. |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | Per-turn align + ranked hop list. |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` / `config_tests.rs` | Sampling hop catalog test for sibling-before-extras. |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Compact meter and `active_spend_driver` use combined remaining. |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | Combined remaining line; keep per-plan rows; do not double-count unified pool. |
| `crates/codegen/xai-grok-pager/src/limits_cmd.rs` | JSON combined remaining; explicit collect uses the hub. |
| New: `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs` | Flock snapshot. No new daemon. |
| `doc/dev/upstream-regression-filters.md` and `FORK.md` land checklist | New land class (below). |
| User-guide `02-authentication` / `04-slash-commands` | After product is green: second `grok login`, combined remaining, one-process fetch. |

Do not bulk-edit. Do not put implement-run hex in product files.

---

## 7. Named tests to add (red first)

Minimum new contracts:

1. `combined_included_remaining_sums_distinct_personal_and_business_pools`
2. `combined_included_remaining_does_not_double_count_unified_pool`
3. `order_credentials_personal_full_with_extras_hops_to_business_included_before_extras`
4. `afterburner_does_not_skip_mark_when_sibling_has_included_remaining`
5. `sampling_config_hops_to_sibling_included_before_extras`
6. `compact_meter_stays_included_while_sibling_pool_has_remaining`
7. `limits_snapshot_second_process_reads_file_and_does_not_http`

Keep existing catalog greens: `sampling_config_auto_use`, `status_bar_pushes_credits_compact_included_supergrok_period_limits`, `team_login_then_personal_keeps_both_principals`, dual `/limits` distinct vs unified tests.

---

## 8. What would make a restack drop this again

This is a new land-class candidate, same bar as "dual-auth hop after included SuperGrok period limits are full" in `FORK.md` (rank helpers are not hop).

Required catalog names after this work (add to `doc/dev/upstream-regression-filters.md` and the FORK cheat sheet):

- `sampling_config_hops_to_sibling_included_before_extras` (hop list, not only `pick_supergrok_identity_for_auto`)
- `combined_included_remaining_sums_distinct_personal_and_business_pools`
- `compact_meter_stays_included_while_sibling_pool_has_remaining`
- `limits_snapshot_second_process_reads_file_and_does_not_http`

A restack that keeps rank helpers and the old one-identity extras-after-full tests, but drops sibling-before-extras hop or the snapshot hub, is a failed land.

---

## 9. Honest leftovers and operator-gated items

- **Second SuperGrok login** is operator-gated if `auth.json` only has one session. Product can store two. Product cannot log into grok.com's account switcher by itself.
- **C4 included SuperGrok period debit** stays a server ticket. Do not invent included debit on the client.
- **Absolute Heavy token units** are not on the preferred wire (`credit_usage_percent`). The sum is remaining percent-units per distinct pool, not a made-up token count. If `monthly_limit` / `used` cents exist on a row, prefer those units for that row; do not mix scales in one sum.
- **Console keys** have no included SuperGrok period limits. They stay last in the spend order. Checking them means listing fingerprints and Management prepaid, not adding them into the included sum.
- **Live TUI binary** may still be the old 1.0.3 install until rebuild/install. Source-green is not dogfood.
- **Settlement honesty** stays: `activeDriver` is spend-order intent, not proof of which wallet xAI debited.
- **Do not reuse SIGUSR1** for snapshot wakeups.
- **Do not add a limits daemon.**

---

## 10. Suggested implement order

Slice A (discover / honesty) can ship alone and tells Hunter whether Business SuperGrok Heavy is even stored.

Slice B (sum + chrome) and Slice C (hop before extras) are the product fix. Do C before or with B: chrome that still paints extras while a sibling has included remaining would lie about the new spend order.

Slice D (single fetcher) is required by the operator ask. Implement after B/C helpers exist so the snapshot schema can carry combined remaining plus per-identity fields. If limits 429s are already burning dogfood, D can start in parallel on new files (`limits_snapshot_hub.rs`) without racing the rank/chrome edits.

Stop after the named tests are green and the land-catalog names exist. Do not invent a grok.com switcher.
