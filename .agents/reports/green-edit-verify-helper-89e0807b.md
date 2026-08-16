# Wave 2 GREEN: rust_edit_verify helper

Date: 2026-08-15
Crate: `xai-grok-tools`
Module: `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs`

## Files changed

- `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs` (product types and functions above the Wave 1 tests)

Did not edit `util/mod.rs` (the `pub mod rust_edit_verify;` line was already there). Did not touch `bash/mod.rs`, search_replace, apply_patch, docs, or other crates.

## Design

Smallest API the Wave 1 tests import. Classify and build argv only. No cargo child is spawned from this module.

- `classify_edit_path`: kill switch `GROK_SKIP_EDIT_VERIFY=1` first, then exact session plan path, then a `third_party` path component, then non-`.rs`. Otherwise `Verify`.
- `package_name_from_path`: walk parents, read each `Cargo.toml`, take the first exact `[package]` table `name = "..."`. Skip workspace-only manifests and `[workspace.package]`. No `cargo metadata`.
- `rustfmt_argv`: `cargo fmt -- --edition 2024 --config-path rustfmt.toml` plus the listed paths as strings.
- `clippy_argv`: `cargo clippy -p <crate> --lib --locked -- -D warnings`. Add `--bin <name>` for `src/bin/<name>.rs` (and `src/bin/<name>/main.rs`). Add `--tests` when a path sits under a `tests/` directory or is named `tests.rs`. Never `--all-targets`, `--workspace`, or a source path after `--`.
- `test_plan_for_file`: `tests/<stem>.rs` → `cargo test -p <package> --test <stem>`. Any other path (including a lib file with no local tests) → `Skip { reason: "no local tests" }`.

## RED (cite existing; not re-proved)

See `.agents/reports/red-edit-verify-helper-89e0807b.md`.

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **101**. Fail reason: `error[E0432]` unresolved imports of the helper API. No test body ran.

## GREEN

Same command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **0**

```
running 19 tests
test util::rust_edit_verify::tests::classify_markdown_is_not_rust ... ok
test util::rust_edit_verify::tests::classify_kill_switch_skips_verify ... ok
test util::rust_edit_verify::tests::classify_rust_source_runs_verify ... ok
test util::rust_edit_verify::tests::classify_session_plan_file_is_skipped ... ok
test util::rust_edit_verify::tests::classify_session_plan_path_skips_even_when_suffix_is_rs ... ok
test util::rust_edit_verify::tests::classify_third_party_rust_is_skipped ... ok
test util::rust_edit_verify::tests::classify_toml_is_not_rust ... ok
test util::rust_edit_verify::tests::clippy_argv_adds_tests_when_integration_test_edited ... ok
test util::rust_edit_verify::tests::clippy_argv_adds_bin_when_binary_edited ... ok
test util::rust_edit_verify::tests::clippy_argv_lib_locked_deny_warnings_without_all_targets ... ok
test util::rust_edit_verify::tests::clippy_argv_does_not_pass_source_path_after_double_dash ... ok
test util::rust_edit_verify::tests::rustfmt_argv_edition_2024_config_and_absolute_files ... ok
test util::rust_edit_verify::tests::test_plan_lib_without_cfg_test_skips_and_says_so ... ok
test util::rust_edit_verify::tests::test_plan_integration_file_runs_package_test_filter ... ok
test util::rust_edit_verify::tests::package_name_prefers_nearest_package_over_parent ... ok
test util::rust_edit_verify::tests::package_name_from_nearest_member_manifest ... ok
test util::rust_edit_verify::tests::package_name_none_without_package_manifest ... ok
test util::rust_edit_verify::tests::package_name_skips_workspace_only_manifest ... ok
test util::rust_edit_verify::tests::package_name_skips_workspace_package_table ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 3209 filtered out
```

Test expectations were not rewritten.

Post-green format of the edited file used `rustfmt --edition 2024 --config-path rustfmt.toml <abs.rs>` (file list, edition 2024, workspace `rustfmt.toml`). `cargo fmt -- --edition 2024 --config-path rustfmt.toml <abs.rs>` failed with `Option 'edition' given more than once` because cargo-fmt already injects the package edition. Did not run `cargo fmt -p` without a file list. Did not run `cargo clippy --all-targets`.

## Named tests (all green)

Classify: `classify_rust_source_runs_verify`, `classify_markdown_is_not_rust`, `classify_toml_is_not_rust`, `classify_third_party_rust_is_skipped`, `classify_session_plan_file_is_skipped`, `classify_session_plan_path_skips_even_when_suffix_is_rs`, `classify_kill_switch_skips_verify`.

Package: `package_name_from_nearest_member_manifest`, `package_name_skips_workspace_only_manifest`, `package_name_skips_workspace_package_table`, `package_name_prefers_nearest_package_over_parent`, `package_name_none_without_package_manifest`.

Argv: `rustfmt_argv_edition_2024_config_and_absolute_files`, `clippy_argv_lib_locked_deny_warnings_without_all_targets`, `clippy_argv_adds_bin_when_binary_edited`, `clippy_argv_adds_tests_when_integration_test_edited`, `clippy_argv_does_not_pass_source_path_after_double_dash`, `test_plan_integration_file_runs_package_test_filter`, `test_plan_lib_without_cfg_test_skips_and_says_so`.

## What was not implemented

- Format-on-write in `search_replace` / `apply_patch` (later wave)
- Batch clippy / test flush in `execute_tool_calls` (later wave)
- Re-entrancy guard around rustfmt rewrite
- Running rustfmt, clippy, or tests from this helper
- Lib-file `#[cfg(test)]` module filter (Wave 1 only requires skip when there are no local tests)
- Bash refuse for crate-wide cargo (owned by the other live agent)
- Process-law doc edits
- Workspace clippy, `cargo fmt --all`, `just test-fmt`, `just check`, or the operator's live nextest

Nothing was staged or committed.
