# Explore: multi-track / also guard (2026-08-07)

Board: `feat:multi-track-also-guard`. Residual: `RESIDUAL.md` §2i.
Process pins exist; **no mechanical product bind/guard shipped**.

## 1. Todo / session board model

| Piece | Path / symbol |
|-------|----------------|
| Tool | `TodoWriteTool` — `crates/codegen/xai-grok-tools/src/implementations/grok_build/todo/mod.rs` |
| Input | `TodoWriteInput { merge: bool (default true), todos: Vec<TodoUpdate> }` |
| Item | `TodoItem { content, priority, status, meta?, size? }` |
| Status | `TodoStatus::{Pending, InProgress, Completed, Cancelled}` (`snake_case` wire) |
| Merge | `apply_merge` — partial by id; omitted fields keep prior (status can flip freely) |
| Replace | `apply_replace` when `merge: false` — **keep-unless-mentioned** for `PROTECTED_TODO_PREFIXES` |
| Prefixes | `plan:`, `impl:`, `pr-`, `recon:`, `residual:`, `ask:`, `feat:`, `bug:` |
| Size | first-class `size` 1\|2 only; `meta.size` fallback; parents reject size |
| Meta keys (documented) | `kind`, `parentId`, `namespace`, `size` — **no agent/task bind key** |
| Progress | `compute_leaf_progress` / `TodoProgress` on tool success |
| State | `TodoState` Resource `grok_build.Todo`; shell plan file + session restore |
| Shell re-export | `xai-grok-shell/src/tools/todo.rs` |
| UI | `xai-grok-pager/src/views/todo_pane.rs`; Clear finished via shell `extensions/todo.rs` |
| User ask seed | `seed_ask_todo` → `ask:<prompt_id>` **pending** (does not touch other rows) |

Research: `doc/dev/research/todo-levels-product-2026-07-24.md`, fib plan residual §0.

## 2. Subagent / task IDs today

| Piece | Path / symbol |
|-------|----------------|
| Spawn | `TaskTool` + `TaskToolInput` (`xai-tool-types` `task.rs`) |
| Id | UUID v7; request field `task_id` optional; result **`subagent_id`** (child session id) |
| Wait / kill | `get_task_output` (`task_ids[]`), `kill_task` (`task_id`) |
| Live state | `SubagentCoordinator` + `SubagentSnapshot` / `SubagentSnapshotStatus` (Running, …) |
| Owner enum | `SubagentOwner::{Task, Workflow}` — **not** todo board owner |
| UI tasks | `tasks_pane.rs` `running_count`; click model/timer opens subagent |

**No product field links `TodoItem` ↔ `subagent_id`.** Process law only: parent inventories `task_id`s + board owners in chat/prompt.

Closest heuristic (not binding): turn-end **TodoGate** partitions first N `in_progress` as “backed” by **count** of live tasks/subagents (`CollectedTodoGateInput.backing_task_count`, insertion order). See `trace_classifier/mod.rs` `partition_todos`, shell `evaluate_todo_gate`, tests `turn_end_guard_tests.rs`. Opt-in (`TodoGateConfig.enabled` default false).

## 3. Where demote can happen (product)

**No code path demotes `in_progress` → `pending` on new user message.**

Abandonment is model behavior: parent calls `todo_write` with `status: pending` (or wipe). `TodoState::update` accepts any status. New-user path only **adds** `ask:*` via `seed_ask_todo` (pending; merge-only).

Also free: `merge: false` drop of unprotected ids; explicit `cancelled`/`completed`.

## 4. Sticky / status chrome for running agents

Already shipped (reuse, not invent):

- Terminal title `TitleItem::Agents` → `"N agents"` when `busy_agent_count > 1` (`notifications/title.rs` `format_busy_agents_title_part`)
- Post-turn title keeps busy if L2 children live (`dispatch/prompt.rs` + `agent_has_running_title_subagents`)
- Queue hold while any **background subagent** live; status e.g. `N subagent(s) still running · M queued — Interject to force` (user-guide `16-subagents`, research `queue-hold-background-subagents-2026-07-24.md`)
- Modal copy `"{} subagents running"` (`views/modal.rs`); tasks pane running counts; dashboard “N agents · M working”
- Goal bail regex `PATTERN_AGENTS_IN_FLIGHT` (`goal_stop_detector.rs`) — narrative only

**Gap vs residual:** no sticky that fires specifically on **new user message while multi-track live** that names board tracks or blocks demote; title agents item is global busy chrome, not also-guard.

## 5. Smallest shippable vertical vs full invent

**Reuse (already exists):**

1. **`meta` extensibility** — document `meta.taskId` / `meta.agentId` (or first-class optional field later); round-trip already works.
2. **`apply_merge` / `TodoState::update`** — single choke point to **reject** `InProgress→Pending` when bound id still `Running` (or when unbound but any live child and multi `in_progress` — weaker).
3. **`SubagentCoordinator` query** — true live check by id (beats TodoGate count heuristic).
4. **TodoGate + backing count** — soft nudge vertical only; already knows “unbacked in_progress”; could extend reminder for multi-track abandonment (no hard block).
5. **Busy chrome + queue hold + title Agents** — sticky “N agents running” on new message ≈ re-surface existing hold/title cues; optional toast on prompt enqueue while `running_count > 0`.

**Smallest vertical (recommended order):**

1. **Hard demote guard (minimal):** in `todo_write` merge, refuse demoting `in_progress`→`pending` when `meta.taskId` is set **and** coordinator says still running; allow `cancelled`/`completed` always; error text plain English. Teach spawn path (prompt) to set `meta.taskId` after Task returns `subagent_id` (agent contract + optional later auto-bind).
2. **Optional UI sticky:** reuse queue-hold / title Agents when user enqueues while subagents live (copy: N agents still running; do not park first track).
3. **Not in v1:** full todo↔agent UI tree, auto-bind on every Task without meta, kill-on-demote, parent HITL product block of freeform model status (hard tool reject is enough).

**Full invent (defer):** bidirectional product bind always-on, board demote blocked for *all* live tracks without meta, multi-parent dashboard ownership, process inventory automation.

## Honesty

Do **not** claim multi-track also-guard shipped. Process dual-pin only until (1) bind field + (2) merge reject on live bind land with red/green tests.
