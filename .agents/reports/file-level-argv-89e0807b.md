# File-level rustfmt / clippy-driver argv (operator 2026-08-15)

Date: 2026-08-15
Crate: `xai-grok-tools`
Module: `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs`

Named contract: file-level verify. The edited `.rs` path is in argv. Not `cargo fmt -p`. Not `cargo clippy -p <crate> --lib`.

## Inventory (before this change)

Current helper still built crate-wide cargo argv (see `.agents/reports/green-edit-verify-helper-89e0807b.md`):

- **Format:** `cargo fmt -- --edition 2024 --config-path rustfmt.toml <abs.rs>…`
  - The real rustfmt child already used `rustfmt --edition 2024 --config-path <nearest rustfmt.toml> <abs.rs>…` because cargo-fmt rejects a second `--edition`.
- **Lint:** `cargo clippy -p <crate> --lib --locked -- -D warnings`
  - Added `--bin <name>` for `src/bin/<name>.rs`.
  - Added `--tests` for `tests/` or `tests.rs`.
  - The source path was **not** in argv. A test (`clippy_argv_does_not_pass_source_path_after_double_dash`) asserted that on purpose.
- **Tests:** `tests/<stem>.rs` → `cargo test -p <package> --test <stem>`. Any other path, including a `src/` module, skipped with `no local tests`.

That crate-wide clippy shape is the product the operator killed (`cargo clippy -p xai-grok-shell --lib`, 29-minute compile). It was not restored.

## Files changed

- `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs` only (argv builders, per-file clippy flush, argv tests).

Did not edit `bash/mod.rs`, `dangerous_cargo.rs`, `search_replace`, `apply_patch`, `execute_tool_calls`, `tool_calls.rs`, `AGENTS.md`, `FORK.md`, `RESIDUAL.md`, user-guide, or implement `SKILL.md`.

## Exact argv the helper now builds for a `.rs` path

Example path: `/abs/crates/codegen/xai-grok-shell/src/session/foo.rs` (package `xai-grok-shell`).

**Format (`rustfmt_argv`):**

```
rustfmt --edition 2024 --config-path rustfmt.toml /abs/crates/codegen/xai-grok-shell/src/session/foo.rs
```

Not `cargo fmt`. Not `cargo fmt -p`.

**Lint (`clippy_argv`):** clippy-driver (same argv rustc uses; file is INPUT).

```
clippy-driver --edition 2024 --crate-name xai_grok_shell --crate-type lib --emit metadata -D warnings /abs/crates/codegen/xai-grok-shell/src/session/foo.rs
```

- The file path is in argv.
- `-p` is not in argv.
- `--lib` is not in argv (that is cargo target selection). `--crate-type lib` is rustc.
- This is **not** `cargo clippy -p xai-grok-shell --lib`.

Other shapes:

- `src/bin/tool.rs` → `clippy-driver --edition 2024 --crate-name tool --crate-type bin --emit metadata -D warnings <abs.rs>`
- `tests/owns_this.rs` → `clippy-driver --edition 2024 --crate-name <pkg_underscored> --test --emit metadata -D warnings <abs.rs>`

Flush now starts **one clippy-driver per edited file** (clippy-driver takes one INPUT). rustfmt still batches listed files in one argv.

**Tests (`test_plan_for_file`):**

- `src/util/rust_edit_verify.rs` → `cargo test -p <package> --lib util::rust_edit_verify`
- `tests/owns_this.rs` → `cargo test -p <package> --test owns_this`
- `src/bin/tool.rs` → `cargo test -p <package> --bin tool`
- crate-root `src/lib.rs` / `src/main.rs` → skip (`no local tests`); no cheap filter, so no whole-lib `cargo test --lib`.

Non-`.rs` still classifies as `NotRust` and skips Rust cargo.

## RED

Observed against the old crate-wide builders (tests updated first; product argv not yet changed).

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **101**

| Test | Fail reason |
|------|-------------|
| `clippy_argv_lints_the_edited_file_not_crate_lib` | argv was `["cargo", "clippy", "-p", "fixture", "--lib", "--locked", "--", "-D", "warnings"]`; expected `clippy-driver` and the file path |
| `clippy_argv_is_file_level_not_package_lib` | argv was `["cargo", "clippy", "-p", "xai-grok-shell", "--lib", "--locked", "--", "-D", "warnings"]` |
| `clippy_argv_includes_bin_path_not_package_lib` | same cargo clippy `-p --lib --bin` shape; file missing |
| `clippy_argv_includes_integration_test_path_not_package_lib` | same cargo clippy `-p --lib --tests` shape; file missing |
| `rustfmt_argv_edition_2024_config_and_absolute_files` | argv started `cargo fmt --` not `rustfmt --edition 2024` |
| `test_plan_src_module_uses_lib_filter_from_path` | got `Skip { reason: "no local tests" }` instead of `--lib util::rust_edit_verify` |
| `several_rust_writes_run_file_level_clippy_per_file` | one crate-wide `cargo clippy -p fixture --lib` (count 1), not 3 file-level clippy-driver runs |

18 passed; 7 failed.

## GREEN

Same command after the argv builders (and per-file clippy flush) were changed:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **0**

`test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 3215 filtered out`

Same tests. Expectations were not loosened. Format of this file used `rustfmt --edition 2024 --config-path rustfmt.toml <abs.rs>` only.

Did not run `cargo clippy -p xai-grok-shell`, `cargo test -p xai-grok-shell --lib`, `cargo fmt -p`, `cargo clippy --all-targets`, workspace cargo, or `just check`.

## What was not implemented

- Wiring in `search_replace` / `apply_patch` / `execute_tool_calls` / `tool_calls.rs` (later job)
- Bash refuse for crate-wide cargo
- Process-law doc edits
- Restoring crate-wide `cargo clippy -p <crate> --lib`

Nothing was staged or committed.
