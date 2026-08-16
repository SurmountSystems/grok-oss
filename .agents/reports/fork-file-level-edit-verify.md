# File-level infer-from-path verify (docs + catalog)

Docs and catalog only. Did not edit `rust_edit_verify.rs` or the
search_replace / apply_patch / bash reject product files.

## What FORK says now

Replaced leftover: the old mash ("Agentic fmt/clippy ACP" plus
thoughtful todo tracking) is gone. Thoughtful todo tracking stays its
own residual bullet under Still open, labeled as session board hygiene
and **not** file-level edit verify.

Product checkbox + named subsection **File-level infer-from-path
verify**: after ACP `search_replace` / `apply_patch`, the edit tool
infers from the path. A `.rs` file is formatted and linted as that
file. Argv must include the written path. Not `cargo clippy -p
<crate> --lib`, not `cargo fmt -p`, not `just check`. Other extensions
do not get Rust cargo. Command-running tool still rejects crate-wide
cargo. Kill switch: `GROK_SKIP_EDIT_VERIFY=1`. A restack that drops
`util/rust_edit_verify.rs` or the named tests is a failed land. Extra
restack-droppable class, not one of the seven numbered land classes.

## Tests (already on disk; argv asserts file path)

Confirmed in `crates/codegen/xai-grok-tools/src/util/rust_edit_verify.rs`:
format argv is `rustfmt` plus the absolute `.rs` paths (not `cargo
fmt -p`). Lint argv is `clippy-driver` plus the written path (not
`cargo`, not `-p`, not `--lib`).

Key `fn`s enrolled in FORK + catalog:

- `rustfmt_argv_edition_2024_config_and_absolute_files`
- `clippy_argv_lints_the_edited_file_not_crate_lib`
- `clippy_argv_includes_bin_path_not_package_lib`
- `clippy_argv_includes_integration_test_path_not_package_lib`
- `clippy_argv_is_file_level_not_package_lib`
- `several_rust_writes_run_file_level_clippy_per_file`

Command-tool reject (`dangerous_cargo` filter):

- `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_test_package_lib_filter_is_not_refused`
- parser-only: `honest_package_lib_filter_is_not_refused`,
  `listed_file_fmt_is_not_refused`, `env_prefixed_fmt_all_is_refused`

Ran (env `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`,
`TMPDIR=/home/hunter/.cache/grok-oss-tmp`):

```
cargo test -p xai-grok-tools --lib rust_edit_verify
```

25 passed, 0 failed.

```
cargo test -p xai-grok-tools --lib dangerous_cargo
```

11 passed, 0 failed.

Did not run crate-wide clippy, `cargo test -p xai-grok-shell --lib`,
or `just check`.

## Pin paths

- Project `AGENTS.md` hard constraint 3a (product pointer) and **3b**
  (2026-08-15: do not prove product work with crate-wide cargo via
  subagents; one agent per job; no duplicate red-test pairs; no mop
  swarm for this feature; proof is named fixture tests).
- Host `~/.grok/AGENTS.md` § *Do not prove product work with crate-wide
  cargo via subagents* (token-efficiency area) and the short product
  pointer under *Structured Rust edits*.
- Catalog:
  `doc/dev/upstream-regression-filters.md` § *File-level infer-from-path
  verify*.

## Left to the product implementer

- Own `rust_edit_verify.rs` and the search_replace / apply_patch /
  bash reject product wiring.
- Keep file-level argv (path in argv; not crate-wide cargo).
- I did not add tests to the helper (tests were already there and
  green).
