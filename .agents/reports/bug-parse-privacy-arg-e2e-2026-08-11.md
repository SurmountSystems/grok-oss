# bug: parse_privacy_arg e2e compile — 2026-08-11

## Root cause

`settings_e2e.rs` still imported `xai_grok_pager::slash::commands::privacy::parse_privacy_arg` and exercised prompt-side opt-in/opt-out aliases.

Product `/privacy` no longer has that helper. `PrivacyCommand` always returns `Action::OpenSettingsFocus { key: "coding_data_sharing" }` and **ignores** trailing args (muscle-memory `/privacy opt-in` lands on the page, does not mutate). Unit pins live in `src/slash/commands/privacy.rs`:

- `privacy_opens_settings_row_in_every_screen_mode`
- `arguments_are_ignored_not_honored`

Re-exporting or restoring `parse_privacy_arg` would revive a dead API that product intentionally dropped. Integration tests also cannot construct `CommandExecCtx` (`screen_mode` / `pager_state` are `pub(crate)`), so a parallel e2e runner of the unit contract is not free.

## Files changed

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/tests/settings_e2e.rs` | Removed stale `pr9_privacy_slash_command_parses_aliases` (import of missing `parse_privacy_arg`). Left a short comment pointing at the unit contracts. |

No product source changes.

## Commands + exit codes

All cargo with `nice -n 19 ionice -c3`.

| Command | Exit |
|---------|------|
| `nice -n 19 ionice -c3 cargo test -p xai-grok-pager --test settings_e2e --no-run` | **0** (binary builds; prior E0432 gone) |
| `nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib slash::commands::privacy` | **0** (2 passed) |
| `nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager` | **0** |

## Done / not done

**Done**

- Compile error fixed by dropping the obsolete e2e test, not by reintroducing the parser.
- Confirmed unit privacy contract still green.
- fmt on package.

**Not done**

- Did not restore `parse_privacy_arg` (wrong product direction).
- Did not re-run the full `settings_e2e` suite (compile-only was the ask).
