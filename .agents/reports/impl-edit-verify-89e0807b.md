# Synthesis: file-level edit verify (89e0807b)

Date: 2026-08-15

Operator contract: after `search_replace` / `apply_patch`, a `.rs` write means rustfmt that file and clippy that file (file-level argv). Not `cargo clippy -p <crate> --lib`. Not crate-wide fmt. The command-running tool refuses crate-wide cargo and does not start cargo.

Sources (read only; this file does not re-run cargo):

- `.agents/reports/file-level-argv-89e0807b.md`
- `.agents/reports/confirm-bash-refuse-89e0807b.md`
- `.agents/reports/wire-edit-verify-89e0807b.md`

## Outcome

1. **Helper argv is file-level.** `rustfmt` and `clippy-driver` take the edited `.rs` path. Product clippy is not `cargo clippy -p <crate> --lib`.
2. **Write tools already call that helper.** `search_replace` and `apply_patch` rustfmt on write and queue the path. Batch flush starts one `clippy-driver` per edited file.
3. **The command-running tool already refuses crate-wide cargo** and does not spawn the shell for those argv shapes. Honest `cargo test -p <crate> --lib <filter>` is still allowed.

No second argv rewriter. No second refuse agent. One wire agent confirmed the hook; no product edit on that turn.

## Argv the product now builds

Example path: `/abs/crates/codegen/xai-grok-shell/src/session/foo.rs`

**Format:**

```
rustfmt --edition 2024 --config-path rustfmt.toml /abs/crates/codegen/xai-grok-shell/src/session/foo.rs
```

**Lint:**

```
clippy-driver --edition 2024 --crate-name xai_grok_shell --crate-type lib --emit metadata -D warnings /abs/crates/codegen/xai-grok-shell/src/session/foo.rs
```

Proof:

- The edited path is in argv.
- First lint word is `clippy-driver`.
- No `cargo`, no `-p`, no cargo `--lib`.
- This is not `cargo clippy -p xai-grok-shell --lib`.

## RED then GREEN

### 1. File-level argv builders (`rust_edit_verify`)

Observed red against the old crate-wide builders (tests updated first; product argv not yet changed).

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

RED exit **101**. 18 passed; 7 failed.

| Test | Fail reason |
|------|-------------|
| `clippy_argv_lints_the_edited_file_not_crate_lib` | argv was `["cargo", "clippy", "-p", "fixture", "--lib", "--locked", "--", "-D", "warnings"]`; expected `clippy-driver` and the file path |
| `clippy_argv_is_file_level_not_package_lib` | argv was `["cargo", "clippy", "-p", "xai-grok-shell", "--lib", "--locked", "--", "-D", "warnings"]` |
| `clippy_argv_includes_bin_path_not_package_lib` | cargo clippy `-p --lib --bin`; file missing |
| `clippy_argv_includes_integration_test_path_not_package_lib` | cargo clippy `-p --lib --tests`; file missing |
| `rustfmt_argv_edition_2024_config_and_absolute_files` | argv started `cargo fmt --` not `rustfmt --edition 2024` |
| `test_plan_src_module_uses_lib_filter_from_path` | skip `no local tests` instead of `--lib util::rust_edit_verify` |
| `several_rust_writes_run_file_level_clippy_per_file` | one crate-wide `cargo clippy -p fixture --lib`, not 3 file-level `clippy-driver` runs |

Same command after the argv builders and per-file flush:

GREEN exit **0**. `25 passed; 0 failed; 0 ignored; 0 measured; 3215 filtered out`. Same tests. Expectations were not loosened.

### 2. Command-running tool refuses crate-wide cargo

Already green. No product edit on the confirm turn.

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib -- dangerous_cargo_ -- --nocapture
```

Exit **0**. 8 passed; 0 failed; 3231 filtered out.

Refuse (cargo is not started):

1. `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell` (`cargo fmt --all`)
2. `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell` (`cargo fmt -p xai-grok-pager`)
3. `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell` (`cargo clippy --all-targets`)
4. `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell` (`cargo clippy -p xai-grok-pager --all-targets -- -D warnings`)
5. `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell` (`cargo clippy --workspace`)
6. `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell` (`cargo test --workspace`)
7. `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell` (`cargo nextest run`)

Allowed:

8. `dangerous_cargo_test_package_lib_filter_is_not_refused` (`cargo test -p xai-grok-tools --lib implement_memory_snapshot_intercept`)

### 3. Write-tool wire (`search_replace` / `apply_patch`)

Earlier wave (format-on-write) had observed red, then green. Named test `search_replace_formats_rust_file_after_write`: file stayed `fn  foo( ){1+2}` until the hook ran rustfmt. After the hook: `fn foo() {\n    1 + 2\n}\n` on disk and in `FileWritten.content`.

This confirm turn: write tools already call `after_structured_rust_write` / `after_structured_rust_writes`. Flush already runs `clippy_argv` once per file. No product edit. No new red.

GREEN commands (each exit **0**):

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_formats_rust_file_after_write -- --nocapture
```

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_clippy_findings_do_not_rollback_write -- --nocapture
```

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_skips_verify_on_session_plan_file -- --nocapture
```

Named wire tests:

- `search_replace_formats_rust_file_after_write`
- `search_replace_clippy_findings_do_not_rollback_write`
- `search_replace_skips_verify_on_session_plan_file`
- `several_rust_writes_run_file_level_clippy_per_file`
- `clippy_findings_appear_in_report_and_write_is_not_rolled_back`
- `flush_runs_package_test_for_integration_file_and_skips_lib_without_tests`
- `plan_file_write_skips_rustfmt_even_when_suffix_is_rs`
- `rustfmt_rewrite_does_not_reenter_the_hook`

`apply_patch` uses the same `after_structured_rust_writes` helper. Existing apply_patch tool tests do not start crate-wide cargo clippy.

## What was not run

No `cargo clippy -p xai-grok-shell --lib`. No `cargo test -p xai-grok-shell --lib` without a filter. No `cargo fmt --all`. No `cargo clippy --all-targets` / `--workspace`. No workspace cargo. No `just check`.

## Stop

Synthesis complete. No review. No mop.
