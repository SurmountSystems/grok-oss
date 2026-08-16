# CONFIRM: bash refuses crate-wide cargo argv

Already green. No product edit this turn.

## Command

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-tools --lib -- dangerous_cargo_ -- --nocapture
```

Exit code: **0**

Result: **8 passed; 0 failed; 0 ignored; 0 measured; 3231 filtered out.** Finished in 0.01s (compile already warm: 0.64s).

## Named tests (all under `implementations::grok_build::bash::tests`)

Refuse (still green; cargo is not started):

1. `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell` — `cargo fmt --all`
2. `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell` — `cargo fmt -p xai-grok-pager`
3. `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy --all-targets`
4. `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell` — `cargo clippy -p xai-grok-pager --all-targets -- -D warnings`
5. `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell` — `cargo clippy --workspace`
6. `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell` — `cargo test --workspace`
7. `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell` — `cargo nextest run`

Allowed (still green):

8. `dangerous_cargo_test_package_lib_filter_is_not_refused` — `cargo test -p xai-grok-tools --lib implement_memory_snapshot_intercept`

## What was not changed

- No edits to `bash/mod.rs` or `bash/dangerous_cargo.rs`.
- No `rust_edit_verify.rs`, `search_replace`, `apply_patch`, `tool_calls.rs`, AGENTS, FORK, RESIDUAL, or user-guide.
- Did not compile or test `xai-grok-shell`.
- Did not run `cargo fmt --all`, `cargo clippy --all-targets`, `cargo test --workspace`, `just check`, or crate-wide clippy.
- No git add / commit / push.
