# Join: F1b attribution soft residual

**Date:** 2026-08-02
**Implementer:** L2
**Also:** `/tmp/grok-1000/grok-impl-summary-f1b-soft.md`
**Prior evidence (not re-proved end-to-end):**
`.agents/joins/impl-slice3-m3-postpaid-2026-08-02.md`,
`.agents/joins/impl-branch-2b-honesty-2026-08-02.md`,
`.agents/joins/console-api-usage-547-evidence-2026-08-02.md`,
`.agents/joins/console-burn-one-turn-investigation-2026-08-02.md`

## Outcome

**F1b soft product honesty is complete.** No code change this wave. Close-out
with green unit evidence. Soft only; no rank policy change; auto_use default
untouched; no invented C4 debit.

## What F1b was

Browser console team API Usage showed **$547.87** while SuperGrok stayed ~65% /
flat extras and `console.isLive=false`. Most of that class of spend is **team
postpaid OAuth / Grok Build**, not SuperGrok **included weekly** debit and not
"secret console-key primary." Cached M3: OAuth ~**$202** vs API ~**$6**.

## Gap check (this wave)

### 1. Doctor / limits / usage: OAuth Usage $ ≠ included weekly

| Surface | Verdict | Evidence |
|---------|---------|----------|
| Doctor dual-auth | Clear on included-first + extras-before-console (auto-use). No claim OAuth Usage is included debit. | `format_human_auto_use_names_extras_before_console_after_included_full` green |
| `/limits` / `grok limits` | Postpaid OAuth vs API class + C6 when OAuth dominates + base/flat honesty | `c6_team_usage_note_*`, `limits_json_surfaces_postpaid_*`, `branch_2b_stack_*` green |
| `/usage` | SuperGrok live emits C6 with "without proving … included weekly" | `usage_summary_supergrok_live_surfaces_c6_team_usage_honesty` green |

**C6 copy (shipped):**

> Note: SuperGrok session can still move team Usage dollars (OAuth / Grok Build
> class on the team invoice) without proving SuperGrok included weekly moved,
> even when the console API key is not live.

### 2. `isLive=false` must not read as "no team $"

| Check | Verdict |
|-------|---------|
| SuperGrok live still surfaces prepaid Balance + Team postpaid when Management data known | **Yes** — `limits_json_surfaces_postpaid_oauth_vs_api_and_c6_honesty` (session live, prepaid $340, OAuth $201.76, API $5.80, C6) |
| Key on file + SuperGrok handling requests | `Requests: SuperGrok` — not missing, not "not live" jargon (`console_key_on_file_requests_supergrok_is_not_missing`) |
| Honest gaps only when Management path missing | Distinct `no management key` / team id / loading / unavailable — `"no $ meter yet"` retired |

### 3. Soft only

No resolve/rank change. No `auto_use` default flip. No SuperGrok debit invent.

## GREEN (2026-08-02)

```bash
cargo test -p xai-grok-pager --lib -- \
  limits_honesty c6_team_usage usage_summary_supergrok_live \
  limits_json_surfaces_postpaid limits_json_postpaid \
  console_key_on_file_requests_supergrok format_console_live_skips \
  format_console_section human_output_names_console
# 22 passed

cargo test -p xai-grok-shell --lib -- \
  format_human_auto_use classify_postpaid fetch_postpaid_preview_hermetic
# 4 passed
```

## Product paths (already shipped; not edited)

- `xai-grok-pager`: `views/limits_honesty.rs`, `views/limits_snapshot.rs`,
  `views/credit_bar.rs`, `limits_cmd.rs`, `app/dispatch/billing.rs`
- `xai-grok-shell`: `auth/xai_management.rs` (M3), `auth/dual_auth_status.rs` (doctor)

## Soft leftover (not F1b product)

| Item | Note |
|------|------|
| Browser $547 vs M3 ~$208 class totals | Window / dashboard composite; optional live re-fetch. Does not re-open product honesty. |
| C4 included debit proof | Separate residual (do not invent). |
| Operator default `auto_use=true` | Gated; not this residual. |
| User-guide/FORK prose may lag postpaid field names | Soft docs; contract lives in code + tests. |

## Residual recommendation

Demote **rank 5 F1b attribution soft residual** from open ranking (product honesty
shipped + green contracts). Keep meters distinct language in residual open body
if useful; lasting ship note may later pin on FORK under billing meters (M3 + C6
already true in tree).
