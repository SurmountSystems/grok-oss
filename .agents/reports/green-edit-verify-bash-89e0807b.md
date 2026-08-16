# GREEN: bash refuses crate-wide cargo argv

Wave 2 green. Dangerous crate-wide cargo is refused before `TerminalBackend`. Honest `cargo test -p <crate> --lib <filter>` still reaches the terminal mock.

## Files changed

- `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/dangerous_cargo.rs` (new sibling)
  - `try_parse_dangerous_cargo_refuse` classifies cargo argv and returns a one-line refuse message.
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/mod.rs`
  - `mod dangerous_cargo;`
  - In `BashTool::run`, after pkill / background-disabled checks and before skill-script intercepts: if refuse is `Some`, return `ToolError::invalid_arguments`. Cargo is not spawned.

`util/rust_edit_verify.rs` and `util/mod.rs` were not edited.

## Design

Mirror of `try_parse_memory_intercept`: parse the model command, decide before the shell, do not rewrite argv.

1. Tokenize (whitespace + simple quotes), skip `NAME=value` / `env` prefixes, find `cargo` (or `…/cargo`) at a statement start (`&&` / `||` / `;` / `|`).
2. Skip `+toolchain`. Find subcommand `fmt` / `clippy` / `test` / `nextest`.
3. Refuse:
   - `cargo fmt --all`
   - `cargo fmt -p` / `--package` with no file list after `--` (rustfmt flags such as `--edition` / `--config-path` are not files)
   - `cargo clippy --all-targets`
   - `cargo clippy --workspace`
   - `cargo test --workspace`
   - `cargo nextest run` with no `-p` / `--package` and no filter (`-E` / `--filterset` / `--filter` / positional)
4. Allow honest scoped cargo, including `cargo test -p <crate> --lib <filter>` and listed-file `cargo fmt -- --edition 2024 --config-path rustfmt.toml <abs.rs>`.
5. Message includes `Refused`, `cargo`, and `do not run crate-wide cargo` so the red contract asserts pass.

## RED cite

`.agents/reports/red-edit-verify-bash-89e0807b.md`

Command (same as green):

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib -- dangerous_cargo_ -- --nocapture
```

RED exit: **101**. Result: 1 passed, 7 failed. Fail: `dangerous cargo must not reach TerminalBackend` (`TrackingTerminal` `called == true`).

## GREEN

Same command after the product refuse.

Exit code: **0**

Result: **8 passed; 0 failed; 0 ignored; 0 measured; 3223 filtered out.** Finished in 0.01s (re-run after rustfmt of the edited files; still exit 0).

`cargo fmt -- --edition 2024 --config-path rustfmt.toml <abs.rs>` failed (`Option 'edition' given more than once` because cargo already passes edition). Formatted with:

```
rustfmt --edition 2024 --config-path rustfmt.toml \
  …/bash/dangerous_cargo.rs \
  …/bash/mod.rs
```

Did not run `cargo fmt -p`, `cargo clippy --all-targets`, workspace tests, or `just check`.

## Named tests

All under `implementations::grok_build::bash::tests` (plus parser unit tests in `dangerous_cargo::unit_tests`, not in the `dangerous_cargo_` filter):

Refuse (now green; `TrackingTerminal` not called; `ToolError::invalid_arguments`):

1. `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell` — `cargo fmt --all`
2. `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell` — `cargo fmt -p xai-grok-pager`
3. `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy --all-targets`
4. `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy -p xai-grok-pager --all-targets -- -D warnings`
5. `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell` — `cargo clippy --workspace`
6. `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell` — `cargo test --workspace`
7. `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell` — `cargo nextest run`

Allowed (still green):

8. `dangerous_cargo_test_package_lib_filter_is_not_refused` — `cargo test -p xai-grok-tools --lib implement_memory_snapshot_intercept`

## What was not implemented

- No rust_edit_verify format-on-write, batch clippy, or heuristic tests.
- No rewrite of refused argv into a guessed file list.
- No AGENTS / FORK / RESIDUAL / user-guide edits.
- No process-mop law delete.
- No `search_replace` / `apply_patch` / `execute_tool_calls` wiring.
- No git add / commit / push.
