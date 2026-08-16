# Wave 1 RED: rust_edit_verify helper

Date: 2026-08-15
Crate: `xai-grok-tools`
Module: `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs`

## Files changed

- `crates/codegen/xai-grok-tools/src/util/mod.rs` (one line: `pub mod rust_edit_verify;`)
- `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs` (module docs + `#[cfg(test)]` contract tests only)

No other crates. Did not extend `plan_mode_allows_plan_file_edit` / `plan_mode_edit_gate_tests`. Those tests pin the plan-mode *edit gate* (may the model write `plan.md`). Wave 1 skip is the helper classify contract (`classify_session_plan_file_is_skipped`, `classify_session_plan_path_skips_even_when_suffix_is_rs`).

## Named tests added

All under `xai_grok_tools::util::rust_edit_verify::tests`:

Classify (`.rs` vs non-`.rs` vs `third_party` vs session plan vs kill switch):

- `classify_rust_source_runs_verify`
- `classify_markdown_is_not_rust`
- `classify_toml_is_not_rust`
- `classify_third_party_rust_is_skipped`
- `classify_session_plan_file_is_skipped`
- `classify_session_plan_path_skips_even_when_suffix_is_rs`
- `classify_kill_switch_skips_verify`

Crate from nearest `Cargo.toml` `[package]` name (tiny temp dirs only):

- `package_name_from_nearest_member_manifest`
- `package_name_skips_workspace_only_manifest`
- `package_name_skips_workspace_package_table`
- `package_name_prefers_nearest_package_over_parent`
- `package_name_none_without_package_manifest`

Argv (pure construction, no workspace cargo):

- `rustfmt_argv_edition_2024_config_and_absolute_files`
- `clippy_argv_lib_locked_deny_warnings_without_all_targets`
- `clippy_argv_adds_bin_when_binary_edited`
- `clippy_argv_adds_tests_when_integration_test_edited`
- `clippy_argv_does_not_pass_source_path_after_double_dash`
- `test_plan_integration_file_runs_package_test_filter`
- `test_plan_lib_without_cfg_test_skips_and_says_so`

## Intended API the tests import (not implemented)

- `EditVerifyDecision` / `EditVerifySkipReason` / `FileTestPlan`
- `classify_edit_path(path, session_plan_file)`
- `package_name_from_path(path)`
- `rustfmt_argv(&[PathBuf])` → `cargo fmt -- --edition 2024 --config-path rustfmt.toml <abs.rs>…`
- `clippy_argv(package, &[PathBuf])` → `cargo clippy -p <crate> --lib --locked -- -D warnings` plus `--bin` / `--tests` when the path needs them. Never `--all-targets` or `--workspace`. Nothing after `--` is a source path.
- `test_plan_for_file(package, file)` → `cargo test -p fixture --test owns_this` for `tests/owns_this.rs`; skip with a "no local tests" reason when the lib file has no `#[cfg(test)]`.

## RED

Command (env as required):

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **101**

Named contract: the helper API is missing. Tests failed to compile (`error[E0432]`), which is the Wave 1 observed red for "module/API missing."

Fail reason (compiler):

```
error[E0432]: unresolved imports `super::EditVerifyDecision`, `super::EditVerifySkipReason`,
`super::FileTestPlan`, `super::classify_edit_path`, `super::clippy_argv`,
`super::package_name_from_path`, `super::rustfmt_argv`, `super::test_plan_for_file`
 --> crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs:11:9
```

`could not compile xai-grok-tools (lib test) due to 1 previous error`

No test body ran. That is expected: no stubs, no product functions.

## What was not implemented

- No `classify_edit_path`, crate walk, rustfmt/clippy/test argv builders, re-entrancy guard, or env reader
- No format-on-write in `search_replace` / `apply_patch`
- No batch clippy/test flush in `execute_tool_calls`
- No bash refuse for crate-wide cargo
- No process-law doc edits
- No workspace clippy, `cargo fmt --all`, `cargo fmt -p` without a file list, `just test-fmt`, `just check`, or the operator's live nextest

Green wave owns the types and functions those tests import.
