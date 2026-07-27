# Cleared todos archive (2026-07-25)

**Slice:** when todos leave the active board, keep a capped off-pane archive
on `TodoState` so drops are still tracked without cluttering the UI.

## Goal

`merge: false` replace and ask-cap prune used to delete items with no product
trail. Archive drops for recovery/audit while the main todo pane, ACP Plan
wire, and prompt summaries stay **active-only**.

## What shipped

| Piece | Behavior |
|-------|----------|
| `TodoState.cleared_todos` | `VecDeque<ClearedTodo>`, cap **`MAX_CLEARED_TODOS` = 200** (oldest dropped) |
| `ClearedTodo` | `id`, `snapshot` (`TodoItem`), `reason`, `cleared_at` (RFC3339), optional `work_ulid` (minted ULID at archive) |
| `ClearedReason` | `ReplaceUnmentioned` \| `AskPrune` |
| `apply_replace` | Before clear: archive unprotected unmentioned items |
| `prune_old_ask_todos` | On `shift_remove`: archive with `AskPrune` |
| Active APIs | `todo_items*`, `is_empty`, `has_id`, tool `output.todos`, `summarize_todo_state` — **active only** |
| Protected prefixes | Unmentioned protected ids still keep-unless-mentioned; **not** archived |
| Serde | Field on same `grok_build.Todo` resource; `#[serde(default)]` for legacy payloads |

Not in this slice: dedicated archive UI pane, session-dir `cleared_todos.jsonl`.
`work_ulid` is now minted at `push_cleared` (join key; not UUID mass rewrite).

## Key path

`crates/codegen/xai-grok-tools/src/implementations/grok_build/todo/mod.rs`

## Verify

```bash
cargo test -p xai-grok-tools --lib implementations::grok_build::todo
```
