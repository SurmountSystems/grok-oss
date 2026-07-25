# Session notes channel (not a pending prompt) — 2026-07-24

## Goal

Operator can leave mid-session notes (especially while subagents / plan /
queue hold run) **without** enqueueing a user turn that hijacks the agent.

## Shipped (v1)

| Piece | Detail |
|-------|--------|
| Store | `SessionNote` / `SessionNotes` on `AgentSession` — id, timestamp, text, optional tags |
| Slash | `/note [text] [#tag…]` — add; bare `/note` or `/notes` — list as system block |
| Dispatch | `Action::AddSessionNote` / `ShowNotes` — **no** ACP effects, **no** `pending_prompts` |
| List UI | System block via `/note`; notes count row on `/tasks` system block |
| Docs | user-guide `04-slash-commands` § `/note`; cross-links from `16-subagents`, `20-background-tasks` |
| Residual | Closed former “notes channel” residual (was #6 / historically #7) |

## How the user invokes a note

```text
/note check hold gate when children finish
/note follow up on flake PATH #ci
/note
/notes
```

- Requires an active session.
- Composer is cleared; text is **not** sent as a prompt and **not** queued.
- Full TUI: toast `Note saved (N): …`
- Minimal: short system line with the same confirmation.

## Non-goals (still later)

- Promote note → pending prompt or todo
- Interactive Notes group inside the full-TUI tasks pane
- Persistence across session resume / disk L2 join note auto-ingest
- Replacing on-disk agent join notes (`grok-impl-summary-*`, explore maps)

## Key paths

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/agent.rs` | `SessionNote`, `SessionNotes`, `parse_note_input` |
| `crates/codegen/xai-grok-pager/src/slash/commands/note.rs` | `/note` / `/notes` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/notes.rs` | add / show dispatchers |
| `crates/codegen/xai-grok-pager/src/app/status_blocks.rs` | `notes_block_text`; `/tasks` count row |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | `AddSessionNote`, `ShowNotes` |

## Verify

```bash
cargo test -p xai-grok-pager --lib note
```

## Dual-pin

- [`FORK.md`](../../../FORK.md) product line
- [`RESIDUAL.md`](../../../RESIDUAL.md) moved to resolved
- User-guide as above
