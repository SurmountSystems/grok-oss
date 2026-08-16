# Slice D map: `grok-oss limits` collect and ForceRefresh

Read-only inventory for making explicit ForceRefresh become leader-or-wait. No snapshot hub exists yet. Today every credits poll is its own HTTP, plus a 429 cooldown flock.

## 1. Exact paths

| Piece | Path |
| --- | --- |
| CLI collect + ForceRefresh policy | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/limits_cmd.rs` |
| Policy enum | `ManagementMeterCachePolicy::ForceRefresh` (line 79). Helpers: `management_meter_cache_policy_for_explicit_limits_collect` (86), `_open` (92), `_background_billing_poll` (100), `should_clear_management_meter_caches` (109). |
| Collect entry | `collect_limits_report` (693) → `collect_limits_report_at` (698). CLI runner: `run` (1085) → collect or `run_multipoll` (1479). |
| SuperGrok credits HTTP | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/extensions/billing.rs` — `fetch_credits_config_with_session` (252), URL `{proxy}/billing?format=credits` (262). TUI/ACP: `handle` `x.ai/billing` → `handle_get_billing` (484). Siblings: `poll_and_remember_non_active_supergrok_included_billing` (314). |
| Management meters | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/xai_management.rs` — `clear_console_team_billing_meter_caches` (720); `fetch_console_team_prepaid_balance*` / postpaid / usage series. Process TTL: `CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS` (same file; comments say 60s). |
| 429 coordination only | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/shared_http_rate_limit.rs` (`wait_before_http`, `billing_provider_key`). Store: `/home/hunter/Projects/surmount/grok-build/crates/codegen/grok-rate-limit/src/store.rs`. Kill switch: `GROK_DISABLE_SHARED_RATE_LIMIT=1`. |
| TUI `/limits` | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/slash/commands/limits.rs` → `Action::ShowLimits`. Dispatch: `app/dispatch/status.rs` `dispatch_show_limits` (459) always queues silent `Effect::FetchBilling`. Effect: `app/effects/mod.rs` (~4171) ACP `x.ai/billing`. |
| Planned hub (not in tree) | Plan: `.agents/plans/token-economy-all-plans-ipc.md` §5. Suggested new file: `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs`. No `limits_snapshot_hub` module exists. |
| Binary | `xai-grok-pager-bin` default-run `grok-oss`: `crates/codegen/xai-grok-pager-bin/src/main.rs`. Clap: `xai-grok-pager/src/app/cli.rs` `Command`. |

**Wiring gap (verified):** `LimitsArgs` / `limits_cmd::run` are not on `Command` and have no match arm in `pager-bin`. Grep finds no caller of `limits_cmd::run` outside `limits_cmd.rs`. `just check-limits-first-live` and `just limits-multipoll` still invoke `${bin} limits --json` / `limits multipoll`. Today clap treats `limits` as the optional positional `PROMPT` and would start the TUI, not collect. Slice D must add `Command::Limits(LimitsArgs)` and `xai_grok_pager::limits_cmd::run(args).await` before collect can run as a CLI.

## 2. How collect fetches billing and credits

`collect_limits_report_at`:

1. Disk config: preferred method + `cfg.endpoints.proxy_url()`.
2. Dual-auth + `load_supergrok_billing_poll_targets(grok_home)` (every stored SuperGrok session, one target per identity).
3. **For each target, always** `fetch_credits_config_with_session(proxy_base, access_token, user_id)`.
   - `GET {proxy}/billing?format=credits` with Bearer session, `x-userid`, client version.
   - First `wait_before_http` on a token-fingerprint provider key (shared 429 cooldown only).
   - On success: map config → `CreditBalance`; `remember_supergrok_included_billing` / dollar extras / Build %; `record_included_poll_history_from_config`; mark poll ok.
   - Does **not** write hop exhaust memos (comment: CLI is a read-only report).
   - Does **not** OIDC-refresh before poll (TUI sibling path does via `ensure_fresh_access_token_for_supergrok_billing_poll`).
4. Management (CLI collect only in this crate): if management key present, `should_clear_management_meter_caches(ForceRefresh, true)` then `clear_console_team_billing_meter_caches()`, then `fetch_console_team_prepaid_balance_default`, `fetch_console_team_postpaid_preview_default`, `fetch_console_team_usage_series_default`. Those fetches themselves honor process TTL unless just cleared.
5. Build `LimitsSnapshot` / `LimitsCliReport` (JSON schemaVersion 1). `run` prints human or `--json`.

TUI `handle_get_billing`: one credits GET for the **active** session, remember + exhaust apply + sibling poll loop (each sibling another credits GET). No Management prepaid/postpaid/series on this path. Pager `fetch_console_team_*` live calls exist only in `limits_cmd` collect.

## 3. Coordination today: always HTTP (except 429 wait)

- **No** flock snapshot leader. **No** `limits_snapshot.json`. Auth `single_flight.rs` is login only.
- SuperGrok credits: every `fetch_credits_config_with_session` sends HTTP after `wait_before_http`. Warm process remember maps do not skip the GET.
- CLI collect loops all principals. TUI billing does active + siblings. N TUIs plus a CLI = N + 1 independent GET storms to `…/billing?format=credits`.
- Shared flock is **cooldown after 429**, not “one fetch.” `GROK_DISABLE_SHARED_RATE_LIMIT=1` turns that into a no-op (each process still HTTP).
- Management: in-process 60s TTL. Only collect (this crate) clears it. TUI `/limits` policy is ForceRefresh in unit tests, but `dispatch_show_limits` never calls `should_clear_management_meter_caches`. TUI does not live-fetch Management meters.

## 4. How to wire the hub (Slice D)

Put the flock snapshot in **shell** (`limits_snapshot_hub.rs` as the plan says). Call it from:

1. `fetch_credits_config_with_session` **or** a new wrapper used by both `handle_get_billing` and collect (do not leave a second uncoordinated GET).
2. `handle_get_billing` sibling loop (followers must not HTTP siblings if the snapshot already has them).
3. `collect_limits_report_at` SuperGrok loop **and** Management fetches.

Contract for explicit collect / ForceRefresh:

- Try exclusive flock on `$GROK_HOME/limits_snapshot.lock` (same style as `grok-rate-limit` / `included_poll_history`).
- **This process is leader:** ForceRefresh stays true. Clear Management process caches only on the leader. Fetch SuperGrok credits (all identities) and Management meters **once**. Write `limits_snapshot.json` (identity ids, used %, reset, extras cents, poll outcome class; never JWTs or keys). Fill `remember_*` from the live fetch.
- **This process is follower:** wait (shared lock or poll until snapshot mtime/generation advances). Do **not** GET credits. Apply snapshot into the same `remember_*` maps. Do **not** ForceRefresh-clear and re-fetch Management.
- Stale snapshot (older than the 60s Management-class window) + exclusive flock available → become leader and fetch once.
- Dead leader: flock releases on exit; next waiter becomes leader.
- `GROK_DISABLE_SHARED_RATE_LIMIT=1` remains the isolation kill switch: skip hub, each process HTTP (hermetic tests).
- After clap wiring, `grok-oss limits` is one more client of the same hub. ForceRefresh only if **this** CLI holds the exclusive flock.

TUI `/limits` open should use the same leader-or-wait. If you keep `should_clear_management_meter_caches`, call it only after winning the flock, then queue `FetchBilling` (or a hub read) so background HonorProcessTtl polls stay TTL-only.

## 5. Tests for the limits command

| Where | What |
| --- | --- |
| `xai-grok-pager` lib `limits_cmd.rs` `#[cfg(test)]` from line 1635 | Policy: `management_meter_cache_policy_collect_force_background_honor_ttl`, `should_clear_management_meter_caches_force_with_key_only`, queue/live-fetch gates. Hermetic JSON/human snapshot tests (no network). `check_limits_first_*`, multipoll extractors. Ignored live: `live_check_limits_first_from_env_json`. |
| Suggested filter | `cargo test -p xai-grok-pager --lib limits_cmd` |
| Broader honesty | `cargo test -p xai-grok-pager --lib -- limits_honesty flat_poll format_surfaces format_dual_principal format_flat_poll usage_summary limits_cmd:: limits_snapshot::` |
| Just | `just check-limits-first-path` (hermetic policy + checker). `just check-limits-first-live` / `just limits-multipoll` need a working `grok-oss limits` subcommand. |
| TUI slash | `slash/commands/limits.rs` (ShowLimits vs `--json`). Dispatch tests under `app/dispatch/tests/status.rs`. |
| Credits HTTP / 429 | `xai-grok-shell` `extensions/billing.rs` unit tests; `shared_http_rate_limit.rs`; `xai_management.rs` cache-bust + HTTP-count tests (~2284, 2754, 3011). |
| Hub (not written) | Plan names: `limits_snapshot_second_process_reads_file_and_does_not_http`, `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`, `limits_snapshot_never_writes_access_tokens`, `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http`. |

There is no integration test that runs the clap `limits` subcommand, because that variant is missing.

## 6. Pager vs shell ownership

**`xai-grok-pager` (UI + CLI surface):** `limits_cmd` report types, ForceRefresh *policy* functions, collect orchestration, `run` / multipoll, clap `LimitsArgs`, TUI `/limits` and `/limits --json`, `views/limits_snapshot.rs` / `limits_honesty.rs` / `limits_modal.rs` / `credit_bar.rs`. Binary dispatch lives in `xai-grok-pager-bin`.

**`xai-grok-shell` (auth + HTTP + caches):** credits GET, ACP `x.ai/billing`, remember maps, poll targets, included poll history, Management API + process TTL, shared 429 store. **Own the hub here.** Pager collect should call hub APIs, not grow a second flock.

**`grok-rate-limit`:** reuse flock JSON style; do not overload 429 files as the billing snapshot.

**Do not** put hub I/O in pager-only code. **Do** add the missing `Command::Limits` arm so collect is actually the `grok-oss limits` process.
