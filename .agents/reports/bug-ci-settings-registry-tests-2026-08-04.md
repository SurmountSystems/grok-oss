# Report: fix 6 failing settings registry / modal tests

**Date:** 2026-08-04
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Package:** `xai-grok-pager`

## Red (observed)

| Test | Assert |
|------|--------|
| `rows_contain_categories_and_settings_through_pr_14` | Modal key list missing 9 new keys |
| `section_headers_have_blank_line_above_except_first` | Blank above `Session` missing: viewport height 60 too small after registry growth (blank gap skipped when one row from list bottom) |
| `every_registered_setting_is_exercised` | 9 keys not in `ALL_SETTINGS_EXERCISED` |
| `registry_kind_membership_through_pr_14` | Bool/Int membership lists missing Token Economy + resume |
| `defaults_round_trip_through_registry` | No expected default for `token_economy.cap_…` |
| `settings_value_payload_matches_kind` | Space on Token Economy bools → `SetTokenEconomyBool` unmatched |

## Inventory (product already correct)

Registered in `settings/defs.rs`, wired through registry / actions / dispatch / modal state:

**Agent / Token Economy**

- `token_economy.cap_implement_effort_when_economic` (Bool, default true)
- `token_economy.max_implement_effort` (Int 1–5, default 3)
- `token_economy.min_implement_effort` (Int 1–5, default 1)
- `token_economy.desired_implement_effort` (Int 1–5, default 2)
- `token_economy.lock_implement_effort` (Int 0–5, default 0 = unlocked)
- `token_economy.show_period_pacing` (Bool, default true)
- `token_economy.local_spend_ledger` (Bool, default true)
- `token_economy.reconcile_management_usage` (Bool, default true)

**Session**

- `resume_canceled_turn_on_restart` (Bool, default true)

Product registration/metadata was complete. Failures were **test fixtures and exercise coverage** lagging the GUI work, plus modal blank-line test viewport size.

## Green (what changed)

### `src/views/settings_modal/tests.rs`

- Extended expected key order in `rows_contain_categories_and_settings_through_pr_14` after `economic_mode` and before Session recap knobs.
- Raised section-header blank-line viewport height **60 → 120** so all category headers + gaps fit (product still inserts blanks; old height dropped the Session gap).

### `tests/settings_e2e.rs`

- Added the 9 keys to `ALL_SETTINGS_EXERCISED`.
- Bool/Int kind membership lists extended (no weakening; stronger membership).
- Defaults round-trip expectations for Token Economy + resume.
- `assert_set_bool_action` arms for `SetResumeCanceledTurnOnRestart` and `SetTokenEconomyBool { field, … }`.
- `settings_value_payload_matches_kind` accepts those Action variants.
- Keyboard + mouse exercise tests (looped) for the new bools and ints; meta category/bounds pin.

### Not changed

- Product `defs.rs` / registry / dispatch (already wired).
- No git add/commit.
- No full `just check`.

## Commands (green)

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- rows_contain_categories_and_settings_through_pr_14 section_headers_have_blank_line
# 2 passed

cargo test -p xai-grok-pager --test settings_e2e
# 310 passed (includes the 4 prior e2e fails + new exercise tests)
```

## Note on defaults round-trip

Token Economy `current_value_for` reads `token_economy_from_disk()`. Expectations match `TokenEconomyConfig::default()`. A host `config.toml` with a non-default `[token_economy]` table can fail `defaults_round_trip_through_registry` without product bug. This machine had no such table when verified.
