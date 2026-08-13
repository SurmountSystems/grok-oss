# Fix: three xai-grok-pager settings tests (Token Economy live cache)

**Date:** 2026-08-04
**Packages:** `xai-grok-shell`, `xai-grok-pager`
**Status:** green

## RED evidence

Reproduced with operator `$GROK_HOME` (`~/.grok/config.toml` has
`[token_economy] min_implement_effort = 2`):

```
cargo test -p xai-grok-pager --lib every_setting_has_action_for_reset_arm -- --nocapture
→ action_for_reset(token_economy.min_implement_effort, ...) round-trip drift:
  left: Some(Int(2))  right: Some(Int(1))

cargo test -p xai-grok-pager --test settings_e2e defaults_round_trip_through_registry -- --nocapture
→ current_value_for(`token_economy.min_implement_effort`) drifted from expected
  left: Int(2)  right: Int(1)

cargo test -p xai-grok-pager --test settings_e2e token_economy_ints_stepper_commit_dispatches_typed_setters -- --nocapture
→ buffer for `token_economy.min_implement_effort` must seed from default 1
  left: Some("2")  right: Some("1")
```

With empty `GROK_HOME`, the same three tests passed (vacuous for reset arm:
setters only emitted `PersistSetting` and never changed live reads).

## Root cause

1. **Product default is correct:** `TokenEconomyConfig::default().min_implement_effort == 1`,
   registry meta default `1`, e2e hard-coded expectation `1`.
2. **`current_value_for` for Token Economy keys** called
   `token_economy_from_disk()`, which always re-read effective config from disk
   (operator home). No process live cache (unlike appearance settings).
3. **Settings setters** (`set_token_economy_bool` / `_int`) only emitted
   `Effect::PersistSetting` and did not update any live state. After dispatch,
   `current_value_for` still returned disk values. Reset round-trip was vacuous
   when disk matched defaults, and **failed when disk differed** (e.g. min=2).
4. **Rollback** explicitly no-oped Token Economy keys ("no in-memory rollback
   mirror"), so failed-persist recovery could not restore live values either.
5. **Tests** pinned appearance caches for hermeticity but not Token Economy.

Not a missing registry key or mis-mapped `action_for_reset` arm; wiring for
reset/setters existed. Drift was host-config pollution + missing live mirror.

## Product fix (minimal)

Process-wide **live Token Economy cache** (same idea as appearance caches):

| API | Role |
|-----|------|
| `token_economy_from_disk()` | Returns live copy; seeds from disk on first call |
| `set_token_economy_live` / `_bool` / `_int` | Optimistic updates |
| `reset_token_economy_live_to_defaults` | Hermetic test pin |
| `clear_token_economy_live` | Force next read to re-seed from disk |

- Pager setters update live then emit `PersistSetting`.
- `apply_setting_rollback` restores live field values.
- Successful disk write in `update_token_economy_key` syncs live to validated config.
- Tests pin defaults: `make_state`, `defaults_round_trip_through_registry`,
  `every_setting_has_action_for_reset_arm`, `set_token_economy_bool_emits_persist_setting`.
- Unit test: `live_cache_overrides_disk_seed`.

## Files changed

- `crates/codegen/xai-grok-shell/src/token_economy/config.rs` — live cache + APIs + unit test
- `crates/codegen/xai-grok-shell/src/token_economy/mod.rs` — re-exports
- `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs` — sync live after write
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/setters.rs` — optimistic live update
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/ui.rs` — rollback arms
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/settings.rs` — pin defaults in tests
- `crates/codegen/xai-grok-pager/tests/settings_e2e.rs` — pin in `make_state` + defaults round-trip

## GREEN evidence

```
cargo test -p xai-grok-pager --lib every_setting_has_action_for_reset_arm -- --nocapture
→ ok (with real ~/.grok min_implement_effort=2)

cargo test -p xai-grok-pager --test settings_e2e defaults_round_trip_through_registry -- --nocapture
→ ok

cargo test -p xai-grok-pager --test settings_e2e token_economy_ints_stepper_commit_dispatches_typed_setters -- --nocapture
→ ok

cargo test -p xai-grok-pager --test settings_e2e token_economy -- --nocapture
→ 5 passed

cargo test -p xai-grok-pager --lib every_persisting_setting_has_rollback_arm -- --nocapture
→ ok

cargo test -p xai-grok-pager --lib set_token_economy -- --nocapture
→ ok

cargo test -p xai-grok-shell --lib token_economy::config -- --nocapture
→ 11 passed (incl. live_cache_overrides_disk_seed)
```

## Post-impl verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-shell -p xai-grok-pager` | done |
| `cargo clippy -p xai-grok-shell --lib -- -D warnings` | clean for our surface |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | clean for our surface |
| Three named failing tests + related TE/settings filters | green |

Note: `cargo clippy -p … --all-targets -D warnings` still reports **pre-existing**
issues in unrelated files (`session_startup.rs`, `fix_tests.rs`,
`shared_http_rate_limit.rs`, etc.). None in the files this fix touched.

## Git

No `git add` / `git commit` (agent policy).
