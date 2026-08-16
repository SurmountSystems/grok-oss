# Restore: AutoCompactCompleted must not wipe the todo board

**Date:** 2026-08-13
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Mode:** product restore. Red/green TDD.

This is **not** last-session-on-start and **not** `grok_oss.db`. Scope was the UI todo board plus `AutoCompactCompleted` dispatch only.

L3 `spawn_subagent` was not available in this L2 session. Diagnosis already named the wipe line and the deleted catalog test, so this restore stayed on that seam.

## Contract

FORK: the pager must not clear the UI todo list on `AutoCompactCompleted`. Resources still hold the board. The UI wipe made the list look empty after compact. A later Plan / `todo_write` still replaces the board on the normal ACP Plan path.

Catalog name: `auto_compact_completed_preserves_todo_board`.

## What was wrong

In `session_notification.rs`, the root `x.ai/session_notification` arm for `AutoCompactCompleted` still did:

```rust
refresh_context_used(agent, *tokens_after);
agent.todo.update_todos(Vec::new());
```

The context-bar refresh is correct. The empty `update_todos` is the 1.0.3 restack wipe. The child-session compact path never wiped todos. The catalog test that would have gone red was deleted.

## TDD

### Red (observed before the product edit)

Restored the named test in
`crates/codegen/xai-grok-pager/src/app/acp_handler/tests/subagents.rs`
(same home and same asserts as the last good tree).

```text
cargo test -p xai-grok-pager --lib -- auto_compact_completed_preserves_todo_board -- --nocapture
```

```text
test app::acp_handler::tests::subagents::auto_compact_completed_preserves_todo_board ... FAILED
assertion `left == right` failed: AutoCompactCompleted must not clear the todo board
  left: 0
  right: 2
test result: FAILED. 0 passed; 1 failed
```

The board had two items before dispatch. After `AutoCompactCompleted` it had zero.

### Product fix (smallest)

Removed only `agent.todo.update_todos(Vec::new());`. Left `refresh_context_used`. Restored the comment that compact must keep the UI board.

File: `crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs`

### Green (same test)

```text
cargo test -p xai-grok-pager --lib -- auto_compact_completed_preserves_todo_board -- --nocapture
test app::acp_handler::tests::subagents::auto_compact_completed_preserves_todo_board ... ok
test result: ok. 1 passed; 0 failed
```

Did not weaken the assert. Length stays 2. First item content stays `impl: residual slice`. Context bar still becomes 25_000.

## Verify

| Step | Command | Result |
|------|---------|--------|
| fmt | `cargo fmt -p xai-grok-pager` | exit 0 |
| clippy lib | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | exit 0 |
| clippy all-targets | `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | exit 101 on **untouched** files (settings e2e, edit_highlight bench, doctor_early_dispatch, dispatch/tests/status.rs, diagnostics/fix_tests.rs). Did not race those writers. |
| named + nearby | `cargo test -p xai-grok-pager --lib -- auto_compact_completed_preserves_todo_board ext_session_notification_for_inactive_agent_updates_its_context_used child_compact_completed_updates_subagent_info apply_compaction_completed` | 6 passed, 0 failed |

One mid-verify compile failed because a parallel settings writer added `SettingKind::Enum.supports_preview` before `settings/registry.rs` matched. That race cleared. The AutoCompact tests were re-run after it compiled again.

`rg 'update_todos\(Vec::new\(\)\)'` over `*.rs` is now empty.

## Files touched

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/acp_handler/tests/subagents.rs`

Did not edit status spend, `grok_oss` ledger, `settings/defs.rs`, appearance unread wires, `session_startup.rs`, welcome, or `title.rs`.

## Honesty

- Live TUI stays old until a successful rebuild and full quit/reopen.
- Plan / `todo_write` ACP Plan still replaces the board. That path is unchanged.
- Catalog cheat sheet already names this test. No FORK / residual / cheat-sheet edit in this slice.
