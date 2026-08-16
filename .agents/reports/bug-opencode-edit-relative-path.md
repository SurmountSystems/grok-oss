# OpenCode edit relative path (`relative_path_resolution`)

## Named contract

A relative path such as `src/lib.rs` passed to the OpenCode `edit` tool must
resolve against the session working directory (`Cwd`). The tool must apply
the replacement at that resolved file, and `EditsApplied.absolute_path` must
be that absolute path.

After file-level infer-from-path verify, a structured write of a `.rs` file
is rustfmt'd before the tool returns. On-disk bytes after an edit of
`src/lib.rs` are the rustfmt (edition 2024) form of the replacement, not the
raw `new_string` with the leftover newline.

## Red (before the test expectation change)

- Test: `implementations::opencode::edit::tests::relative_path_resolution`
- Command:

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-tools --lib relative_path_resolution
```

- Result: FAIL
- Fail reason: the relative path already resolved and the edit applied. The
  test then compared on-disk bytes to the unformatted replacement
  `"fn main() { /* edited */ }\n"`. rustfmt edition 2024 rewrites that
  snippet to `"fn main() { /* edited */\n}\n"`. Panic:

```
assertion `left == right` failed
  left: "fn main() { /* edited */\n}\n"
 right: "fn main() { /* edited */ }\n"
```

Path resolution itself was not the failure. `applied.absolute_path` matched
`{tmpdir}/src/lib.rs` (that assert ran first and passed).

Evidence that rustfmt, not path join, produces those bytes: the same snippet
run through `rustfmt --edition 2024` (default, and with this repo's
`rustfmt.toml`) yields the left-hand string.

The named contract of this test (resolve a relative path against `Cwd`) did
not change. The on-disk contract for `.rs` writes did change when file-level
infer-from-path verify landed. The content assert was updated to that
rustfmt output. The path assert was not loosened.

## Green (same command)

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-tools --lib relative_path_resolution
```

- Result: PASS
- `implementations::opencode::edit::tests::relative_path_resolution` ... ok
- `implementations::opencode::write::tests::relative_path_resolution` ... ok
  (same filter; not this slice)

## Files touched

- `crates/codegen/xai-grok-tools/src/implementations/opencode/edit/mod.rs`
  (test `relative_path_resolution` content expectation only)

No product change in `handle_replacement` / `resolve_model_path`. Relative
join against `Cwd` was already correct. No `xai-grok-shell` edits. No ACP
file lock. No token economy.

## Leftovers

- Mid-run the crate failed to compile because the ACP lock implementer
  briefly declared `mod per_path_write_lock_tests` before that file existed
  and had a `dunce::simplified(...).into_owned()` type error. That race
  cleared. Not this slice.
- `per_path_write_lock_tests.rs` still warns on an unused `ApplyPatchOutput`
  import. Lock implementer owns that file.
- File-level verify `clippy-driver` on this module alone cannot resolve
  `crate::...` (expected for a non-root file). Named fixture tests are the
  proof for this slice.
