# Report: `impl:ci20-settings` (five settings tests)

**Board:** `impl:ci20-settings` under `bug:ci-20-unit-fails`  
**Date:** 2026-08-14  
**Package:** `xai-grok-pager`  
**Status:** named tests green

Isolated compile: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-settings-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

Named tests were **not** rewritten to match a fatter leftover catalog. Product search, catalog copy, and Appearance row order were changed so the existing uniqueness and next-row contracts hold.

Did **not** write `cfg.subagents`. Config persist still has no `subagents` field.

## Red (per named test)

### `views::settings_modal::tests::advance_prev_recovers_when_selection_is_hidden`

Observed:

```
cargo test -p xai-grok-pager --lib -- advance_prev_recovers_when_selection_is_hidden
assertion `left == right` failed
  left: 19
 right: 7
```

`set_query("ascii")` is supposed to leave exactly one visible setting (`simple_mode`, keyword `ascii`). Up from a hidden `compact_mode` must land on that row (index 7). After leftover `/settings` restore, `scrub_ascii_punct` also matched: keyword `ascii`, label/description containing those letters, and interior key substring `ascii` inside `scrub_ascii_punct`. `advance_prev` walked to the last visible hit (index 19).

### `settings_e2e::filter_and_semantics_narrow_strictly`

Same catalog/search pollution. `SettingsRegistry::search("ascii")` must return exactly `simple_mode`. Restored `scrub_ascii_punct` made `len() == 1` false. First isolated e2e compile was killed at the host 120s wrapper before this test ran; red is the same match set as the observed `advance_prev` fail.

### `settings_e2e::filter_with_multiple_matches_navigates_between_settings`

Types `ascii minimal`, then backspaces to `ascii`. Both queries must leave only `simple_mode`. Extra `ascii` hit from `scrub_ascii_punct` broke `assert_eq!(setting_keys, vec!["simple_mode"])` and the after-pop uniqueness assert.

### `settings_e2e::repeat_j_navigation_is_processed`

Repeat `j` from the initial `compact_mode` row must land on `screen_mode`. Leftover restore declared `hide_header` between those two Appearance rows, so one `j` selected `hide_header`.

### `settings_e2e::token_economy_ints_stepper_commit_dispatches_typed_setters`

Stepper buffer must seed from product defaults (`min_implement_effort = 1`, `max = 3`, `desired = 2`, `lock = 0`). Token Economy live cache seeds from disk on first read. Host `~/.grok/config.toml` has `[token_economy]` min=2, max=4, desired=3. `make_state` did not pin defaults, so `editing_buffer()` was the disk value (this machine previously failed the same test with `left: Some("2")` / `right: Some("1")` for min).

## Product change

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/settings/defs.rs` | Appearance order is now `compact_mode` → `screen_mode` → `hide_header`. `scrub_ascii_punct` label/description/keywords no longer contain the letters a-s-c-i-i. Persist key stays `scrub_ascii_punct`. |
| `crates/codegen/xai-grok-pager/src/settings/registry.rs` | Search haystack is label + description + keywords only. Key match is whole key or a leading `{word}.` / `{word}_` prefix. Interior key segments are not substring matches. |
| `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs` | `rows_contain_categories_and_settings_through_pr_14` expected Appearance order updated to match the catalog move. Named `advance_prev` still queries `"ascii"` and still expects `simple_mode`. |
| `crates/codegen/xai-grok-pager/tests/settings_e2e.rs` | `make_state` calls `reset_token_economy_live_to_defaults()`. Named filter/j/stepper asserts unchanged. |

Rejected path: aligning the five named tests to origin/main (two `ascii` hits, next row `hide_header`, query `"simple"`). That would loosen uniqueness.

## Green re-run

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-settings-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager                         # FMT_EXIT:0
cargo clippy -p xai-grok-pager --lib -- -D warnings # CLIPPY_EXIT:0
cargo test -p xai-grok-pager --lib -- advance_prev_recovers_when_selection_is_hidden
# 1 passed; LIB_EXIT:0
cargo test -p xai-grok-pager --test settings_e2e -- \
  filter_and_semantics_narrow_strictly \
  filter_with_multiple_matches_navigates_between_settings \
  repeat_j_navigation_is_processed \
  token_economy_ints_stepper_commit_dispatches_typed_setters
# 4 passed; E2E_EXIT:0
```

## Leftovers

- Searching `/settings` for `ascii` still finds `simple_mode` only. It does **not** find the punctuation-scrub row. Operators can search `scrub` or `punctuation`. Persist key is still `scrub_ascii_punct`.
- Prefix key match still finds `scrub_ascii_punct` on query `scrub` (`scrub_` prefix). That is intended.
- Other `settings_e2e` tests (318 filtered) were not re-run.
- Stayed off credit_bar, limits_honesty, allowance_exhaust, prompt_widget, dashboard peek, router initializer, session_loaded, turn_status, agent config, models.
- No `cfg.subagents` write.
