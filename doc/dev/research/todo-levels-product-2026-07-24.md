# Todo levels product surface (2026-07-24)

**Slice:** tool-writable multi-level todos; stop silent wipe of foreign
namespaces. **Shipped** — under RESIDUAL *Not residual*. `allow_worktree` OSS
default-false is also closed (see `task-worktree-pins-2026-07-24.md`).

## Goal

Make the session board first-class enough for multi-skill work without a full
tree UI: write `priority` + `meta` through `todo_write`, and guard
`merge: false` so skill namespaces cannot silently disappear.

## What shipped

### Tool surface (`xai-grok-tools` `todo_write`)

| Field | Before | After |
|-------|--------|-------|
| `TodoItem.priority` | stored only | **writable** via optional `TodoUpdate.priority` |
| `TodoItem.meta` | stored only | **writable** via optional `TodoUpdate.meta` (JSON object) |
| `merge: false` | full wipe | **keep-unless-mentioned** for protected prefixes |

**Documented `meta` keys** (schema description; others allowed):

| Key | Values / meaning |
|-----|------------------|
| `kind` | `residual` \| `phase` \| `work` \| `child` (prefer this for levels) |
| `parentId` | parent todo id when nesting levels |
| `namespace` | owning skill/session prefix (e.g. `plan`, `impl`) |

**Protected id prefixes** (`PROTECTED_TODO_PREFIXES`):

`plan:`, `impl:`, `pr-`, `recon:`, `residual:`, `ask:`, `feat:`, `bug:`

User-reported items (same-turn board + red/green TDD): `feat:<kebab-slug>` for
features, `bug:<kebab-slug>` for bugs/regressions. Session board only — not
durable residual unless campaign-ranked.

On full replace, existing items with those prefixes that are **not** in the
replace payload are re-attached after the new set. Unprotected unmentioned
ids still drop. Mentioned protected ids are replaced by the payload (content
fallback / status defaults unchanged).

### Merge path

`TodoState::update` accepts optional `priority` and `meta`. Omitted fields
leave prior values (same as content/status). New inserts take update fields
with priority defaulting to medium.

### Persistence / compaction

- Full `TodoState` (including `meta` / `priority`) remains in Resources serde
  under `grok_build.Todo` — round-trips across session restore.
- Compaction **reminders** still summarize id/content/status only
  (`TodoSummary`); live board state is not stripped of meta.
- Trace classifier mirrors protect-on-replace + priority/meta for replay
  fidelity.

### UI

Light badge in full-TUI `TodoPane`: when `meta.kind` is a non-empty string,
row content is prefixed with dim `[kind] `. No tree UI.

### Dual-pin

| Layer | Update |
|-------|--------|
| `AGENTS.md` | L1 row: product guard + prefer `meta.kind` + join path |
| `FORK.md` | Hierarchical shipped note |
| `RESIDUAL.md` | Todo levels under *Not residual*; `allow_worktree` OSS default also *Not residual* |
| Host `_SKILL_RULES` | Product keep-unless-mentioned + prefer `meta.kind` |
| Campaign / task-worktree pins | Cross-links |

## Tests (`xai-grok-tools` todo module)

- `meta_and_priority_round_trip_via_todo_write` — write, status-only merge
  preserves, Resources serialize/load keeps meta
- `merge_false_preserves_foreign_prefix_items_not_in_replace_set`
- `merge_false_can_replace_protected_when_mentioned`
- `old_callers_json_without_priority_or_meta_still_deserialize`
- unit: merge priority/meta; prefix helpers
- pager: `list_entry_shows_meta_kind_badge` / plain without kind

## Non-goals (still residual elsewhere)

- Full hierarchical tree UI
- Dedicated L2 “notes channel” pane — **shipped as operator `/note` v1**
  (session-local, not pending prompts; not agent join-note auto-ingest).
  See `notes-channel-2026-07-24.md`
- OpenCode `todowrite` full-replace semantics (positional ids, no namespaces)
- Changing default `allow_worktree` — **done** elsewhere (OSS default false;
  see `task-worktree-pins-2026-07-24.md`)
- Cleared-item recovery UI — **archive ring shipped** (active-only pane still);
  see `cleared-todos-archive-2026-07-25.md`

## Key paths

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-tools/src/implementations/grok_build/todo/mod.rs` | Schema, apply_replace/merge, tests |
| `crates/codegen/xai-grok-shell/src/trace_classifier/mod.rs` | Replay mirror |
| `crates/codegen/xai-grok-pager/src/views/todo_pane.rs` | `[kind]` badge |

## Verify

```bash
cargo test -p xai-grok-tools --lib implementations::grok_build::todo
cargo test -p xai-grok-pager --lib views::todo_pane
```
