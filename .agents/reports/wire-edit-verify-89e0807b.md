# Write-tool wire: file-level rustfmt + clippy-driver (already green)

Date: 2026-08-15
Crate: `xai-grok-tools` (write tools + helper) and `xai-grok-shell` (`execute_tool_calls` flush)
Helper argv (already green, not re-done): `.agents/reports/file-level-argv-89e0807b.md`

Named contract: after a successful `search_replace` or `apply_patch` write of a `.rs` file, rustfmt that file and clippy-driver that file (file-level argv). The edited path is in argv. Product clippy is not `cargo clippy -p <crate> --lib`.

## Decision

The write tools already call the file-level helper after a successful `.rs` write. The batch flush already starts one `clippy-driver` per edited file. Fixture tests already prove: edited path present; no `cargo`; no `-p`; no cargo `--lib`.

**No product edit.** The stale crate-wide `cargo clippy -p <crate> --lib` shape in the previous version of this report is dead. It was not restored.

## Wire (read, not rewritten)

`after_structured_rust_write` / `after_structured_rust_writes` rustfmt immediately and queue the path. `flush_batch_clippy_and_tests_for` runs `clippy_argv(pkg, [file])` once per file.

| Site | Call |
|------|------|
| `search_replace` create write | `after_structured_rust_write(path, &input.new_string)` |
| `search_replace` replace write | `after_structured_rust_write(path, &write_text)` |
| `apply_patch` after the write batch | `after_structured_rust_writes(&format_pairs)` |
| `execute_tool_calls` → `execute_tool_calls_batch` join | `take_pending_verify_paths()` then `flush_batch_clippy_and_tests_for(paths, Some(plan_path))` |

Flush body (`rust_edit_verify.rs`):

```
for file in &files {
    let clippy = clippy_argv(&pkg, std::slice::from_ref(file));
    let clippy_res = runner.run_cargo(&clippy, &cwd);
    ...
}
```

`clippy_argv` starts with `clippy-driver`, puts the absolute `.rs` path at the end, and never emits `cargo`, `-p`, or cargo `--lib`.

Other registered presets (`opencode` write/edit, `grok_build_hashline` edit) also call `after_structured_rust_write`. Not in this job's write list. Not changed.

## Files changed

none

## What I did not touch

- `rust_edit_verify.rs` argv builders (already file-level)
- `search_replace/mod.rs`, `apply_patch/tool.rs`, `tool_calls.rs`
- `bash/mod.rs` / crate-wide cargo refuse
- `AGENTS.md`, `FORK.md`, `RESIDUAL.md`, host `~/.grok/AGENTS.md`, `doc/dev/upstream-regression-filters.md`
- crate-wide cargo fmt / clippy / test
- git add / commit / push

## Argv proof

Example edited path (fixture lib, from `clippy_argv_lints_the_edited_file_not_crate_lib` and flush spy `several_rust_writes_run_file_level_clippy_per_file`):

**Format (`rustfmt_argv`):**

```
rustfmt --edition 2024 --config-path rustfmt.toml <abs.rs>
```

**Lint (`clippy_argv` / flush `run_cargo`):**

```
clippy-driver --edition 2024 --crate-name fixture --crate-type lib --emit metadata -D warnings <abs.rs>
```

Operator-named shell crate path (`clippy_argv_is_file_level_not_package_lib`):

```
clippy-driver --edition 2024 --crate-name xai_grok_shell --crate-type lib --emit metadata -D warnings /home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/foo.rs
```

Assertions in `assert_file_level_clippy_argv`:

- first argv word is `clippy-driver`
- no `cargo`
- no `-p`
- no cargo `--lib`
- no `--all-targets` / `--workspace` / `--locked`
- each edited path is present
- `--edition 2024` and `-D warnings`

This is **not** `cargo clippy -p xai-grok-shell --lib`.

## Exact tests and commands (already green)

No RED this wave. Product already matches the named contract. Same tests were not loosened.

### Helper argv + flush (file-level)

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib rust_edit_verify -- --nocapture
```

Exit code: **0**

`test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 3215 filtered out`

Named tests that pin argv / flush:

- `clippy_argv_lints_the_edited_file_not_crate_lib`
- `clippy_argv_includes_bin_path_not_package_lib`
- `clippy_argv_includes_integration_test_path_not_package_lib`
- `clippy_argv_is_file_level_not_package_lib` (xai-grok-shell path; asserts not `-p xai-grok-shell`)
- `rustfmt_argv_edition_2024_config_and_absolute_files`
- `several_rust_writes_run_file_level_clippy_per_file` (3 `clippy-driver` runs, last argv is file-level on `src/b.rs`)
- `clippy_findings_appear_in_report_and_write_is_not_rolled_back`
- `flush_runs_package_test_for_integration_file_and_skips_lib_without_tests`
- `plan_file_write_skips_rustfmt_even_when_suffix_is_rs`
- `rustfmt_rewrite_does_not_reenter_the_hook`

### search_replace write wire

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_formats_rust_file_after_write -- --nocapture
```

Exit code: **0**. `search_replace_formats_rust_file_after_write` passed. Unformatted `fn  foo( ){1+2}` becomes `fn foo() {\n    1 + 2\n}\n` on disk and in `FileWritten.content`.

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_clippy_findings_do_not_rollback_write -- --nocapture
```

Exit code: **0**. `search_replace_clippy_findings_do_not_rollback_write` passed. SearchReplace writes a temp crate, flush runs the file-level helper, unused-variable finding stays in the report, write stays on disk.

Command:

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib search_replace_skips_verify_on_session_plan_file -- --nocapture
```

Exit code: **0**. `search_replace_skips_verify_on_session_plan_file` passed. `plan.md` does not queue clippy.

`apply_patch` writes go through the same `after_structured_rust_writes` helper. Existing apply_patch tool tests cover add/update/delete/move on non-`.rs` fixtures; they do not start crate-wide cargo clippy. No new apply_patch test was added (wire already calls the helper; argv is proven on the helper + flush spy).

`execute_tool_calls_batch` in `tool_calls.rs` is the session collect point. It does not build cargo clippy argv itself. It calls `flush_batch_clippy_and_tests_for`, which uses `clippy_argv` per file.

Did not run `cargo clippy -p xai-grok-shell --lib`, `cargo test -p xai-grok-shell --lib`, `cargo fmt -p`, `cargo clippy --all-targets`, workspace cargo, or `just check`.
