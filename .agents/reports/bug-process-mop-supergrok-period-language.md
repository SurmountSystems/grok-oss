# Process mop: included SuperGrok period limits language

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:process-mop-supergrok-period-language`  
**Implementer report:** `.agents/reports/feat-supergrok-period-limits-language.md`

SuperGrok is a **paid** product. This mop did not change chrome. Operator-facing copy must stay **included SuperGrok period limits** (or short **SuperGrok period limits** / **included SuperGrok period · N%**). Never "free SuperGrok."

## Isolated compile

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-limits-language-mop-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

`rustc` 1.97.1 (`/usr/bin/rustc`). `--offline` after first fetch.

Pager clippy/tests used the mop target above. Shell clippy/tests used a second dir (`/home/hunter/.cache/grok-oss-limits-language-mop-target-shell`) so the two `--lib` jobs did not share one rustc lock. Same isolated `/tmp` (`TMPDIR`).

Cold clippy and first `cargo test --lib` compiles were killed at the 300s wrapper. Incremental retry finished.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 |
| `cargo clippy -p xai-grok-pager --offline --lib -- -D warnings` | 0 (cold killed at 300s; incremental 0) |
| `cargo clippy -p xai-grok-shell --offline --lib -- -D warnings` | 0 (cold killed at 300s; incremental 0) |
| pager `--lib` Active driver / Work C compact / honesty notes / limits JSON (8) | 0 (8 passed) |
| pager `--lib` compact + loading + status credits (3) | 0 (3 passed) |
| pager `--lib` `format_supergrok_session_with_weekly_and_extras` + C6 + license honesty (3) | 0 (3 passed) |
| shell `--lib` doctor + block message + `/spend` report + pacing (7) | 0 (7 passed) |

`--lib` clippy was green. This mop did **not** run `--all-targets`. No mid-flight API collision was observed on `--lib`.

Libtest treats the cargo filter as a substring, not `a|b` regex. Grouped `|` filters matched 0 tests. The same 21 names were re-run one filter each on the compiled `--lib` binaries (same contracts as the implementer groups). Spot-check `cargo test -p xai-grok-pager --offline --lib compact_status_names_included_supergrok_period_limits_not_bare_intent` also exited 0.

### Named filters (21)

Pager `--lib` (8):

- `active_driver_free_period_headroom_even_with_extras_and_team_prepaid`
- `active_driver_afterburner_extras_when_free_period_full`
- `work_c_free_period_headroom_intent_compact_and_quiet_footer`
- `active_driver_intent_not_settlement_note_when_team_meters_under_supergrok`
- `json_report_shape_and_no_secrets`
- `limits_json_active_driver_free_period_with_extras_on_account`
- `default_credits_note_when_reading_present`
- `base_note_when_supergrok_live_with_included_reading`

Pager `--lib` (3):

- `compact_status_names_included_supergrok_period_limits_not_bare_intent`
- `credit_bar_loading_line_is_honest_placeholder`
- `status_bar_pushes_credits_compact_included_supergrok_period_limits`

Pager `--lib` (3):

- `format_supergrok_session_with_weekly_and_extras`
- `c6_team_usage_note_when_oauth_postpaid_dominates`
- `license_honesty_names_team_usage_and_zeros_expected`

Shell `--lib` (7):

- `format_human_auto_use_names_extras_before_console_after_included_full`
- `block_message_names_meters_and_opt_in_block`
- `report_names_meters_distinctly`
- `limits_section_points_to_spend`
- `ahead_of_linear_burn`
- `behind_linear_burn`
- `never_dollarizes`

## Edits

None. Fmt was already clean. `--lib` clippy and the 21 named tests passed. No language-pass fallout to mop.

Wire keys were not renamed (`activeDriver=supergrok_free_period`, `allow_spend_when_free_period_debit_unproven`, `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN`).

No `git add` / commit / push. No `/rebuild`.

## Leftover collisions

None on `--lib`. Other writers are mid-flight in the tree; this mop stayed on language `--lib` and did not open those files.
