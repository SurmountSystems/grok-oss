# Clippy question_mark — per-path write lock

## Files changed

- `crates/codegen/xai-grok-tools/src/implementations/editor_infra/per_path_write_lock.rs`

Replaced the `match` on `try_acquire_write` in `try_acquire_writes` with:

```rust
let guard = try_acquire_write(&key, holder)?;
guards.push(guard);
```

Lock behavior is unchanged (same `?` error path).

## Command(s) run

```
cargo test -p xai-grok-tools --lib per_path_write_lock
```

Env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

File-level edit verify also ran `clippy-driver` on that `.rs` file alone (expected fail: standalone compile, missing crate deps) and `cargo test -p xai-grok-tools --lib implementations::editor_infra::per_path_write_lock` (ok).

Did **not** run crate-wide `cargo clippy --workspace` or `just check`.

## Result

**Pass.** `cargo test -p xai-grok-tools --lib per_path_write_lock`: 9 passed, 0 failed.
