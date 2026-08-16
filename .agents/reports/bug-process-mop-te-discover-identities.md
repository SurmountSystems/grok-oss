# Process mop: honest discovered SuperGrok identities (Slice A)

**Board:** `impl:process-mop-te-discover-identities`  
**Date:** 2026-08-14  
**Isolated compile:** rustc 1.97.1, `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-discover-mop-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`

SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok."

Clippy used `--lib` on both crates (not `--all-targets`), as specified.

No product files were edited. Fmt, clippy, and the named filters were already clean. There was no compile, lint, or test fallout to mop.

The first pager `--lib` test compile was killed by the host timeout while still linking. A second run finished incrementally. The first shell `--lib` test compile was cancelled mid-build. A second run finished incrementally. Neither retry changed source.

---

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 |
| fmt check | `cargo fmt -p xai-grok-pager -p xai-grok-shell -- --check` | 0 |
| clippy pager | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy shell | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | 0 |
| named Slice A + pager keep-green | `cargo test -p xai-grok-pager --lib -- limits_json_lists_two_supergrok_principals_when_both_slots_exist limits_json_honest_single_supergrok_session_cannot_see_team_plan format_dual_principals dual_principals_stack_in_report` | 0 (after incremental retry) |
| shell keep-green | `cargo test -p xai-grok-shell --lib -- format_human_single_supergrok_session_says_cannot_see_team_plan dual_supergrok_principals_listed_with_fingerprints_only pick_prefers_business_included_before_personal_when_both_have_remaining sampling_config_hops_to_sibling_included_before_extras limits_snapshot_second_process_reads_file afterburner_does_not_skip_mark_when_sibling_has_included_remaining` | 0 (after incremental retry) |

---

## Named tests (still green)

Pager (`xai-grok-pager --lib`): **6 passed; 0 failed**

- `limits_cmd::tests::limits_json_lists_two_supergrok_principals_when_both_slots_exist`
- `limits_cmd::tests::limits_json_honest_single_supergrok_session_cannot_see_team_plan`
- `limits_cmd::tests::dual_principals_stack_in_report`
- `views::limits_snapshot::tests::format_dual_principals_shows_both_pools_and_live_role`
- `views::limits_snapshot::tests::format_dual_principals_keep_distinct_included_pct`
- `views::limits_snapshot::tests::format_dual_principals_honest_absence_for_unknown_pool`

Shell (`xai-grok-shell --lib`): **6 passed; 0 failed**

- `auth::dual_auth_status::tests::format_human_single_supergrok_session_says_cannot_see_team_plan`
- `auth::dual_auth_status::tests::dual_supergrok_principals_listed_with_fingerprints_only`
- `auth::supergrok_identity_rank::tests::pick_prefers_business_included_before_personal_when_both_have_remaining`
- `agent::config::tests::sampling_config_hops_to_sibling_included_before_extras`
- `auth::limits_snapshot_hub::tests::limits_snapshot_second_process_reads_file_and_does_not_http`
- `auth::allowance_exhaust_from_billing::tests::afterburner_does_not_skip_mark_when_sibling_has_included_remaining`

Rank, hop, flock snapshot hub, and spend order were not changed by this mop.

---

## Leftovers

None from fmt, clippy, or the named test filters.
