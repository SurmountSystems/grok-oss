# Process mop: auto-compact todo-board preserve

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Role:** `[process-mop]` L2  
**Slice:** AutoCompactCompleted must not wipe the UI todo board  
**Primary report:** `.agents/reports/bug-auto-compact-wipes-todos.md`

## Scope

Mop only. Did not race settings/, spend/status, welcome, title.rs, session_startup, ledger, grok_oss, or builder.rs.

Slice file (actual path):  
`crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs`

Named test: `auto_compact_completed_preserves_todo_board`

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| 1. fmt | `cargo fmt -p xai-grok-pager` | **0** |
| 2. clippy lib | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **0** |
| 3. named test | `cargo test -p xai-grok-pager --lib -- auto_compact_completed_preserves_todo_board` | **0** |

Clippy was `--lib` only, as ordered. Other writers can leave `--all-targets` red.

## Test output

```text
test app::acp_handler::tests::subagents::auto_compact_completed_preserves_todo_board ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8797 filtered out
```

## Fallout

None. Fmt did not rewrite the slice. Clippy lib was clean. The named test passed. No product edit in this mop.

The AutoCompactCompleted arm still refreshes context used and does **not** call `update_todos(Vec::new())`.

## Honesty

- Live TUI stays old until a successful rebuild and full quit/reopen.
- Did not run `--all-targets` clippy.
- Did not re-run nearby AutoCompact tests. Only the named filter.
