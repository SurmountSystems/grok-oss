# Implement report: Dual SuperGrok billing honesty (Option B)

**Date:** 2026-08-07
**Plan:** Dual SuperGrok billing honesty (comprehensive)
**Workspace:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** Option B full vertical (poll outcome + CLI/JSON + fill label + active chrome + doctor + rank + docs). Optional multi-slot refresh / N-fail demote parked as residual.

## What shipped

### 1. Process-local poll outcome model (no secrets)

Per SuperGrok `identity_id`:

| Kind | Meaning |
|------|---------|
| `Ok` | Credits poll succeeded |
| `AuthFailed` | Auth-class fail (no auth context, expired, 401, …) |
| `OtherFailed` | Network / other |
| `Never` | Unrecorded this process |

APIs in `allowance_exhaust_from_billing.rs` (re-exported from `auth`):

- `remember_supergrok_billing_poll_ok` / `remember_supergrok_billing_poll_failed`
- `classify_supergrok_billing_poll_error`
- `demote_included_billing_on_auth_fail` (clears free-period `usage_pct` + reset; keeps SuperGrok $ extras)
- `supergrok_billing_poll_outcome` / snapshot helpers
- `format_supergrok_billing_fail_note(role, fingerprint, err)` with re-login CTA

Writers:

- CLI `collect_limits_report` (Ok + Err)
- Sibling `poll_and_remember_non_active_supergrok_included_billing`
- Active `remember_active_supergrok_included_billing` (Ok)
- Active FetchBilling error paths (AuthFailed / OtherFailed)

`clear_included_billing_cache` also clears poll outcomes.

### 2. CLI fail-loud notes (role + fingerprint + re-login)

Failed collect no longer uses only a 12-char id:

```text
SuperGrok (personal) billing failed (fingerprint abcdef012345): …. Re-login that SuperGrok account with: grok login
```

### 3. JSON additive fields (`PrincipalCliMeter`)

- `pollSucceeded: bool`
- `includedSource`: `live_poll` | `process_cache` | `shared_pool_fill` (optional)
- `pollErrorClass`: `auth` | `network` | `other` (optional)

### 4. Labeled unified fill (math kept)

`fill_unified_included_on_empty_slots` still copies free SuperGrok period % (and dollar extras fill unchanged). Filled slots get:

- `included_source = SharedPoolFill`
- `poll_succeeded = false`

Human `/limits` / notes:

- Shared pool line (existing)
- Fail note when `poll_error_class` present (role + re-login)
- Fill note: free-period % / Extra Usage Credits from shared SuperGrok pool, not a successful poll of that login

### 5. Active free-period chrome honesty

`compact_meter_text_for_live_identity_with_active_poll` + `credit_bar_line_for_session`:

- If **active** SuperGrok last poll is AuthFailed → honest `...%`
- Never paint sibling-only free-period success as active healthy

### 6. TUI `/limits` sibling fail surface

`format_limits_detail` includes dual poll honesty notes (not debug-only). Snapshot build reads process poll outcomes for `poll_succeeded` / error class.

### 7. Doctor dual poll health

`DualAuthStatus::format_human` adds:

```text
SuperGrok billing poll health (this process):
  personal · fingerprint … · last poll auth failed (re-login: grok login)
  business · fingerprint … · last poll OK
```

### 8. Rank hygiene

`order_live_supergrok_for_auto`:

- Prefer free-period headroom that is **not** last-poll AuthFailed
- Auth-failed-only headroom → `exhausted_all_included` (do not primary a known-dead JWT)
- Auth fail demotes free-period process cache (no fresh headroom from stale %)

### 9. Docs / residual / filters

- User-guide `02-authentication.md`: dual login poll fail, shared-pool fill label, meters table, which role to re-login
- `RESIDUAL.md` Half A: Option B shipped; soft residual multi-slot refresh + N-fail demote
- `doc/dev/upstream-regression-filters.md` §2c dual poll honesty filters

## Red → green evidence

### Shell

| Test | Command | Red intent | Result |
|------|---------|------------|--------|
| `auth_failed_poll_demotes_included_usage_pct_not_fresh_headroom` | `cargo test -p xai-grok-shell --lib -- auth_failed_poll` | Auth fail clears free-period % | **pass** |
| `billing_fail_note_names_role_fingerprint_and_relogin` | `… billing_fail_note` | Role + fingerprint + `grok login` | **pass** |
| `remember_poll_ok_sets_outcome_ok` | `… remember_poll_ok` | Ok outcome recorded | **pass** |
| `order_live_prefers_poll_ok_supergrok_over_auth_failed` | `… order_live_prefers_poll_ok` | Rank prefers poll-OK | **pass** |
| `format_human_dual_poll_health_names_auth_failed_role` | `… format_human_dual_poll` | Doctor dual poll health | **pass** |

### Pager

| Test | Command | Red intent | Result |
|------|---------|------------|--------|
| `dual_fill_provenance_not_live_poll_and_names_role` | `cargo test -p xai-grok-pager --lib -- dual_fill_provenance` | Fill ≠ live_poll; JSON + human role | **pass** |
| `compact_status_active_auth_failed_not_sibling_free_period_pct` | `… compact_status_active_auth_failed` | Active auth fail → `...%` | **pass** |
| Existing unified fill / dual format / limits_honesty | `… format_unified_fills format_dual limits_honesty` | Keep green | **pass** |

TDD order: poll outcome + note helpers and rank/doctor contracts landed with product code in one pass; new contract tests were written against the intended API and observed green after product wiring. Pre-existing dual fill tests remained green (fill math kept, provenance labeled).

## Keep-green regression (ran)

```text
cargo test -p xai-grok-shell --lib -- upsert_personal_then_business team_login_then_personal_keeps dual_supergrok load_supergrok_candidates two_principals_billing enrich_candidates principal_limits_label non_active_poll_targets remember_both_principals included_usage poll_non_active_remembers auth_failed_poll billing_fail_note remember_poll_ok order_live_prefers_poll_ok format_human_dual_poll
# 20 passed

cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal format_dual_unified fill_unified limits_honesty dual_fill_provenance compact_status_active_auth_failed format_unified_fills format_dual
# 39 passed
```

## Post-impl verify

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-shell -p xai-grok-pager` | 0 |
| clippy lib | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | 0 |
| clippy lib | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy all-targets | Pre-existing unrelated failures in other test modules (not introduced by this work) | n/a |

## Files touched

### Shell

- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`
- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs`
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`

### Pager

- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`

### Docs

- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `doc/dev/upstream-regression-filters.md`
- `RESIDUAL.md`

## Residual (not in this pass)

1. **Multi-slot OIDC refresh before sibling poll** (only if existing refresher targets multi-slot without large AuthManager rewrite).
2. **N consecutive auth fails → demote from poll list** with operator-visible stale login (no auto-delete of `auth.json` secrets).
3. Optional once-per-session toast for sibling fail (limits body + doctor already surface).

## Non-goals respected

- No C4 SuperGrok period debit invent
- No Token Economy product work
- No console-dead recovery / Design A rewrite (wired into)
- No auto-delete of auth secrets
- No console scrape
- No git add / commit / push
