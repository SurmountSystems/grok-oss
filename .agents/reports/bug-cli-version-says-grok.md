# CLI identity: `grok-oss --version` and resume hints

**Date:** 2026-08-14
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:cli-version-says-grok`

SuperGrok is paid. This report does not discuss SuperGrok meters. This slice did not change billing copy.

## Named contracts

- The product CLI name is **grok-oss**. Do not tell operators to run upstream **grok** for this fork.
- `grok-oss --version` (and any `--version` path that does not need a TTY) prints a line whose first token is **grok-oss**, then the real compiled version and git SHA. Example shape: `grok-oss 1.0.3 (f1abb5fd33b6)`. Not `grok 1.0.3 (f1abb5fd33b6)`. Do not invent a version. Do not rename the binary.
- Quit, relaunch, and ambiguous-title copy that says `Resume this session with:` (or `Resume by session id instead:`) uses `grok-oss --resume`, not `grok --resume`.
- User-guide `--version` / `--resume` examples use `grok-oss`. `01-getting-started` already had `grok-oss --version` and `grok-oss --resume`; leftover `grok --yolo` is now `grok-oss --yolo`.
- Welcome / window titles stay Grok OSS / `grok-oss` (prior restore). This slice is CLI identity only.

## TDD red (before product edit)

Command:

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-branding-cli-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --offline --lib -- \
  print_exit_resume_hint_writes_expected_lines \
  print_exit_resume_hint_includes_minimal_flag \
  print_exit_resume_hint_includes_session_summary \
  print_exit_resume_hint_truncates_summary_to_width \
  failed_relaunch_hint_includes_screen_mode_env \
  two_manual_renames_stay_ambiguous \
  cli_command_name_is_grok_oss \
  cli_help_output_header \
  user_guide_resume_and_version_examples_use_grok_oss \
  -- --test-threads=1
```

**Observed red.** 0 passed, 9 failed. Fail reasons (product still said `grok`):

| Test | Fail |
|------|------|
| `print_exit_resume_hint_writes_expected_lines` | left `grok --resume sess-abc`, right `grok-oss --resume sess-abc` |
| `print_exit_resume_hint_includes_minimal_flag` | left `grok --minimal --resume`, right `grok-oss --minimal --resume` |
| `print_exit_resume_hint_includes_session_summary` | same `grok --resume` product token |
| `print_exit_resume_hint_truncates_summary_to_width` | missing `grok-oss --resume` |
| `failed_relaunch_hint_includes_screen_mode_env` | left `... grok --fullscreen --resume ...` |
| `two_manual_renames_stay_ambiguous` | `Resume by session id instead: grok --resume <session-id>` |
| `cli_command_name_is_grok_oss` | clap name `grok`, expected `grok-oss` |
| `cli_help_output_header` | `Grok Build TUI` / `Usage: grok [...]` |
| `user_guide_resume_and_version_examples_use_grok_oss` | `01-getting-started.md` still had `grok --yolo` |

The old `version_without_tty` helper was green on the operator's exact bug: it only required stdout to contain the substring `grok` plus a digit, so `grok 1.0.3 (f1abb5fd33b6)` passed. That helper was tightened so the first token must be `grok-oss` and a line starting with bare `grok` fails.

## Files and what changed

- `crates/codegen/xai-grok-pager/src/client_identity.rs`: `product_version_line` and `resume_session_command` (always `PRODUCT_CLI_NAME`). Tests for the operator's `1.0.3 (f1abb5fd33b6)` line and `grok-oss --resume 01`.
- `crates/codegen/xai-grok-pager-bin/src/main.rs`: `print_cli_version` uses `product_version_line` instead of `grok {}`.
- `crates/codegen/xai-grok-pager-bin/tests/version_without_tty.rs`: first-token contract.
- `crates/codegen/xai-grok-pager/src/app/mod.rs`: exit resume hint uses `resume_session_command`. Clap help tests expect `Grok OSS TUI` / `Usage: grok-oss`.
- `crates/codegen/xai-grok-pager/src/app/cli.rs`: clap `name` is `PRODUCT_CLI_NAME`; about is `Grok OSS TUI`; `parse_cli` defaults to `grok-oss` (still keeps argv0 `agent`).
- `crates/codegen/xai-grok-pager/src/app/screen_mode_relaunch.rs`: relaunch paste uses `grok-oss`.
- `crates/codegen/xai-grok-pager/src/app/session_title_resolve.rs`: ambiguous-title hint uses `grok-oss --resume`.
- `crates/codegen/xai-grok-pager/src/docs.rs`: user-guide regression that resume / `--version` / `--yolo` / `--continue` examples are `grok-oss`.
- User-guide: `01-getting-started` (`grok-oss --yolo`); `18-sandbox` resume commands; `05-configuration` `--version` / update / plain launch; `04-slash-commands` plain launch; `02-authentication` "Run `grok-oss`".
- `FORK.md` branding bullet: `--version` product token and `grok-oss --resume` are standing law, with the named tests.

## Green re-run

Same lib filter plus the new identity tests:

```bash
cargo test -p xai-grok-pager --offline --lib -- \
  print_exit_resume_hint_writes_expected_lines \
  print_exit_resume_hint_includes_minimal_flag \
  print_exit_resume_hint_includes_session_summary \
  print_exit_resume_hint_truncates_summary_to_width \
  failed_relaunch_hint_includes_screen_mode_env \
  two_manual_renames_stay_ambiguous \
  cli_command_name_is_grok_oss \
  cli_help_output_header \
  user_guide_resume_and_version_examples_use_grok_oss \
  product_cli_name_is_grok_oss \
  product_version_line_uses_grok_oss_not_bare_grok \
  resume_session_command_uses_grok_oss \
  -- --test-threads=1
```

**Green.** 12 passed, 0 failed.

```bash
cargo test -p xai-grok-pager-bin --offline --test version_without_tty -- --test-threads=1
```

**Green.** 3 passed (`stdin=/dev/null`, closed pipe, rebuild capture).

Also green: `version_flags_parse_as_early_intent_without_exiting`, `version_subcommand_is_version_only`, `update_semver_flag_is_not_version_only`, `exec_failure_hint_uses_screen_mode_resume_hint`, `restore_blocked_hint_mentions_cleanup_and_resume`.

```bash
cargo fmt -p xai-grok-pager -p xai-grok-pager-bin
cargo clippy -p xai-grok-pager --offline --lib -- -D warnings
cargo clippy -p xai-grok-pager-bin --offline --bins -- -D warnings
```

**fmt and clippy exit 0.** Isolated `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-branding-cli-target`.

## Leftover honesty

- The **installed** `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss` is still the previous artifact until the operator runs `/rebuild` or `just install`. Source and the test binary now print `grok-oss`. This slice did not `/rebuild`.
- Other operator-facing leftovers still say `grok` as a command in places this slice did not own: `17-sessions` `grok sessions` / `grok du` / `grok worktree`; `mcp_cmd` usage `grok mcp add`; some `grok login` lines in `02-authentication` and `17-sessions`. Internal comments and upstream changelogs still mention `grok --resume`.
- JSON `--version` payload is still `{ currentVersion, channel }` with no product name field. Plain-text `--version` is the operator path that was wrong.
- Welcome / titles were not retouched.
