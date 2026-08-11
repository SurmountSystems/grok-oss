# Join: Slice E2 — Certainty harness for limits-before-credits spend order

**Date:** 2026-08-02
**Branch:** `fixes-2`
**Implementer:** L2
**Plan:** limits-first certainty plan §5 Slice E2
**Did not touch:** E1 default `auto_use`, E3 live extras-after-full, E4 server ticket, E5 edges; no rank / ExhaustedAll rewrite.

## Outcome

Runnable gates so we do not claim "limits-first path certain" without proof:

| Deliverable | Where |
|-------------|--------|
| One-command hermetic suite | `just check-limits-first-path` |
| Live C1/C3 after rebuild | `just check-limits-first-live` |
| Pure JSON path checker (unit-tested SoT) | `xai_grok_pager::limits_cmd::check_limits_first_path_json` |
| Dogfood window template | `.agents/joins/template-limits-dogfood-window.md` |

## Contract checked (C1 / C3)

When `auto_use_included_limits = true` and preferred is **not** `api_key`:

- If any SuperGrok principal has included weekly used **below 100%**, live sampling must be SuperGrok session and `console.isLive` must be false.
- Fail if `liveSampling=console_key` or `console.isLive=true` under that condition.
- Skip when auto_use is off or preferred pins `api_key`.
- No claim when `includedUsedPct` is absent; no step-2 extras-after-full assert (E3).

## How to run

```bash
# Hermetic (CI / local before "path certain"):
just check-limits-first-path

# Live after rebuild (operator home with auto_use on):
just check-limits-first-live
# GROK_OSS_BIN=./target/release/grok-oss just check-limits-first-live
```

Hermetic filters match plan §3 (shell spend-order + bare resolve + memo; pager flat_poll / prepaid lag / Management ForceRefresh policy) plus the new `check_limits_first*` tests.

Live recipe: `GROK_OSS_BIN` (default `./target/release/grok-oss` or PATH) runs `limits --json`, previews meters, then reuses the pure checker via ignored test `live_check_limits_first_from_env_json` (`LIMITS_FIRST_JSON`, `LIMITS_FIRST_AUTO_USE`, `LIMITS_FIRST_PREFERRED_API_KEY`).

## RED → GREEN (new behavior)

Named contracts on the pure checker (observed via unit tests; product is pure functions + harness only):

| Test | Contract |
|------|----------|
| `check_limits_first_ok_when_supergrok_live_and_included_below_100` | C1 pass |
| `check_limits_first_fails_when_console_live_and_included_below_100` | C1 fail |
| `check_limits_first_fails_on_console_key_wire_even_if_is_live_false` | wire alone fails |
| `check_limits_first_fails_on_console_is_live_even_if_wire_says_session` | isLive alone fails |
| `check_limits_first_skips_when_preferred_is_api_key` | pin skips |
| `check_limits_first_skips_when_auto_use_off` | classic path skips |
| `check_limits_first_ok_when_included_full_even_if_console_live` | not C1 after 100% |
| `check_limits_first_ok_when_included_pct_unknown` | no invent |
| `check_limits_first_uses_any_principal_below_100` | any principal |

```text
cargo test -p xai-grok-pager --lib -- check_limits_first
# 11 passed; 1 ignored (live entry)

just check-limits-first-path
# shell 16 passed; pager 22 passed (+1 ignored)
```

## Files touched

- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` — checker + tests
- `justfile` — `check-limits-first-path`, `check-limits-first-live`
- `.agents/joins/template-limits-dogfood-window.md` — dogfood fields
- this join

## Residual

- **E1** still open: default `auto_use_included_limits` for new installs (operator D1).
- **E3** live proof of extras after included full when period hits 100%.
- Live recipe needs a rebuilt binary + real credentials; not part of default `just test` / CI.
- Plan filter name `ForceRefresh` does not appear as a test name substring; recipe uses `management_meter_cache_policy` + `should_clear_management_meter` (same ForceRefresh policy contracts).
