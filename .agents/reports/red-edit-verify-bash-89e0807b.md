# RED: bash refuses crate-wide cargo argv (not implemented)

Wave 1 red only. No product refuse logic was added.

## Files changed

- `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/mod.rs`
  - Test helper: `make_tracking_resources_with` (existing memory intercept tests still use `failing()`).
  - New tests next to `implement_memory_snapshot_intercept_does_not_spawn_shell`.

`util/mod.rs` was not left changed. `rust_edit_verify.rs` was not edited.

## Named tests

All under `implementations::grok_build::bash::tests`:

Refuse (must fail today: host must not spawn cargo):

1. `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell` — `cargo fmt --all`
2. `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell` — `cargo fmt -p xai-grok-pager`
3. `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy --all-targets`
4. `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy -p xai-grok-pager --all-targets -- -D warnings`
5. `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell` — `cargo clippy --workspace`
6. `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell` — `cargo test --workspace`
7. `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell` — `cargo nextest run`

Allowed (already green; honest argv still reaches the terminal mock):

8. `dangerous_cargo_test_package_lib_filter_is_not_refused` — `cargo test -p xai-grok-tools --lib implement_memory_snapshot_intercept`

Contract: refuse is `ToolError` or a non-zero foreground result, a message that mentions cargo and refuse / do not run crate-wide cargo, and `TrackingTerminal` is **not** called.

## RED

Exact command:

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib -- dangerous_cargo_ -- --nocapture
```

Exit code: **101**

Result: **1 passed, 7 failed** (3201 filtered out). Finished in 0.01s after compile.

Fail reason (all seven refuse tests, same assert):

```
dangerous cargo must not reach TerminalBackend (cargo must not spawn): <argv>
```

at `bash/mod.rs` `assert_dangerous_cargo_refused` (TrackingTerminal `called == true`). Refuse is not implemented; `BashTool::run` still forwards those commands to the terminal backend.

Allowed test passed: honest `cargo test -p … --lib <filter>` still reaches `TerminalBackend`.

### Compile note (other L3, not this slice)

A straight `cargo test -p xai-grok-tools --lib` does not compile today: `util/rust_edit_verify.rs` test imports (`classify_edit_path`, `clippy_argv`, …) have no product items yet. That is the other specialist's compile-red. To observe **this** runtime red, `pub mod rust_edit_verify` was unplugged for one compile, the filter above was run, then the module line was restored. Do not treat that unplug as product work.

## What was not implemented

- No bash intercept / `try_parse_*` for cargo argv.
- No refuse message in `BashTool::run`.
- No rust_edit_verify product helpers.
- No clippy `--all-targets`, `cargo fmt --all`, `cargo fmt -p` without a file list, workspace test, or nextest --workspace.
- No git add / commit / push.
