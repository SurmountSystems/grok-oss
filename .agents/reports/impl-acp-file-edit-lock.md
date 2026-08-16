# ACP per-path write lock

## What landed

ACP `search_replace`, `apply_patch`, and `write` take a per-path write
lock automatically as part of the tool call. There is no lock argument
on the schema. A successful write does not mention the lock.

If another agent already holds that path, the tool returns an execution
error that names the holder and the file. It does not write. It does
not wait. It does not show a human steal, skip, or wait menu. Agents
are expected to talk to each other (wait, hand off, or pick another
path).

The lock is process-wide, keyed on a normalized path. It is held for
the rest of the tool call, including file-level infer-from-path verify
on a written `.rs` file. `GROK_SKIP_EDIT_VERIFY=1` still skips only
that verify. The lock releases when the call returns, so a later call
on the same path can write.

Holder identity is `OwnerSessionId` when present, else the session id,
else the tool-call id.

OpenCode `edit` is not wired. That file was reserved for the
relative-path fixer (its report was not on disk when this slice
started). Hashline edit is not in this slice.

The unused FIFO waiter in `file_operation_lock.rs` is unchanged. This
lock is fail-fast and names the holder.

## Files

- `crates/codegen/xai-grok-tools/src/implementations/editor_infra/per_path_write_lock.rs`
- `crates/codegen/xai-grok-tools/src/implementations/editor_infra/per_path_write_lock_tests.rs`
- `crates/codegen/xai-grok-tools/src/implementations/editor_infra/mod.rs`
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/search_replace/mod.rs`
- `crates/codegen/xai-grok-tools/src/implementations/codex/apply_patch/tool.rs`
- `crates/codegen/xai-grok-tools/src/implementations/opencode/write/mod.rs`
- `FORK.md` (subsection **ACP per-path write lock**)
- `RESIDUAL.md` (lock bullet now shipped, leftover named)

Did not edit `crates/codegen/xai-grok-tools/src/implementations/opencode/edit.rs`.

## Red (observed fail, before product wiring)

Named tests existed. The lock table could hold a path. The three ACP
tools still wrote.

```text
cargo test -p xai-grok-tools --lib per_path_write_lock
```

Fail reason (before the three tools called `acquire_for_tool`):

- `two_agents_cannot_write_the_same_path_at_once` expected a tool
  error and got `EditsApplied` (`shared.txt` overwritten to
  `changed by b`).
- `search_replace_apply_patch_and_write_all_take_the_lock` expected
  `search_replace` to fail while held and got `EditsApplied`.
- `held_path_error_names_holder_and_file_without_a_steal_skip_wait_menu`
  expected a tool error and got `EditsApplied`.

Happy-path and release tests already passed (no lock mentioned; a
later call could write because nothing was held after return).

## Green (same filter, after wiring)

```text
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-tools --lib per_path_write_lock
```

```text
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 3241 filtered out
```

Named contracts:

- `two_agents_cannot_write_the_same_path_at_once`
- `happy_path_first_writer_succeeds_silently`
- `lock_releases_after_the_tool_call_so_a_later_call_can_write`
- `search_replace_apply_patch_and_write_all_take_the_lock`
- `held_path_error_names_holder_and_file_without_a_steal_skip_wait_menu`

## Leftovers

- OpenCode `edit` does not take the lock. Call
  `editor_infra::per_path_write_lock::acquire_for_tool` after that
  file is free.
- Hashline structured edit does not take the lock.
- No new inter-subagent message bus. The tool error is the signal.
  Agents already talk; they can wait, hand off, or pick another path.
