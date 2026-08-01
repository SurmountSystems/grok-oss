//! TodoWrite — new-architecture implementation.
//!
//! Reuses the core logic (`validate_no_duplicate_ids`, `apply_replace`,
//! `apply_merge`, `summarize_todo_state`) from the old `implementations::todo`
//! module. State is stored as `State<TodoState>` in Resources instead of
//! `ToolState.todo_state`.

use std::fmt::Write;

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::output::{TodoWriteOutput, TodoWriteSuccess};
use crate::types::requirements::{Expr, ToolRequirement};
#[allow(unused_imports)]
use crate::types::resources::{SharedResources, State};
use crate::types::tool::{ToolKind, ToolNamespace};

#[derive(thiserror::Error, Debug)]
pub enum TodoError {
    #[error("Missing Todo content in mode: {0}")]
    MissingTodoContent(String),

    #[error("Missing Todo ID in mode: {0}")]
    MissingTodoID(String),

    #[error("Duplicate Todo ID in response: {0}")]
    DuplicateTodoID(String),

    /// Size not in {1, 2}, or size set on a parent that has/will have children.
    #[error("{0}")]
    InvalidSize(String),
}

/// Allowed Fibonacci leaf sizes (only atomic work leaves).
pub const VALID_TODO_SIZES: &[u8] = &[1, 2];

/// Validate a first-class or meta-derived size: only 1 or 2 when set.
pub fn validate_todo_size_value(size: u8) -> Result<u8, String> {
    if VALID_TODO_SIZES.contains(&size) {
        Ok(size)
    } else {
        Err(format!(
            "Invalid todo size {size}: only 1 or 2 allowed (Fibonacci leaves). \
             Split larger work into children."
        ))
    }
}

/// Resolve size from the explicit field, falling back to `meta.size` JSON number.
///
/// When the field is omitted, a numeric `meta.size` is accepted and will be
/// normalized onto the item's first-class `size` field by callers.
pub fn resolve_todo_size(
    explicit: Option<u8>,
    meta: &Option<serde_json::Value>,
) -> Result<Option<u8>, String> {
    if let Some(n) = explicit {
        return validate_todo_size_value(n).map(Some);
    }
    let Some(meta) = meta.as_ref() else {
        return Ok(None);
    };
    let Some(raw) = meta.get("size") else {
        return Ok(None);
    };
    let n = if let Some(u) = raw.as_u64() {
        u8::try_from(u).map_err(|_| {
            format!(
                "Invalid todo size {u}: only 1 or 2 allowed (Fibonacci leaves). \
                 Split larger work into children."
            )
        })?
    } else if let Some(i) = raw.as_i64() {
        u8::try_from(i).map_err(|_| {
            format!(
                "Invalid todo size {i}: only 1 or 2 allowed (Fibonacci leaves). \
                 Split larger work into children."
            )
        })?
    } else {
        return Err("Invalid todo meta.size: expected a JSON number 1 or 2.".to_owned());
    };
    validate_todo_size_value(n).map(Some)
}

/// `meta.parentId` string when present.
pub fn todo_parent_id(item: &TodoItem) -> Option<&str> {
    item.meta
        .as_ref()
        .and_then(|m| m.get("parentId"))
        .and_then(|v| v.as_str())
}

/// True when any active item (or update in `updates`) lists `id` as `parentId`.
pub fn todo_id_has_children(state: &TodoState, id: &str, updates: &[TodoUpdate]) -> bool {
    if state
        .todo_items()
        .any(|item| todo_parent_id(item) == Some(id))
    {
        return true;
    }
    updates.iter().any(|u| {
        u.meta
            .as_ref()
            .and_then(|m| m.get("parentId"))
            .and_then(|v| v.as_str())
            == Some(id)
    })
}

/// Leaf-weighted progress for the session board.
///
/// - **Points mode** (any non-cancelled leaf has `size`): only leaves with an
///   explicit size contribute; parents never count even if size is set.
/// - **Legacy count mode** (no sized leaves): `completed`/`total` match the
///   status-bar item counts (all non-cancelled items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoProgress {
    /// Completed units (leaf points in points mode; item count in legacy).
    pub completed: u32,
    /// Total units (non-cancelled).
    pub total: u32,
    /// Completed leaves counted toward progress.
    pub leaves_done: u32,
    /// Non-cancelled leaves counted toward progress.
    pub leaves_total: u32,
    /// True when progress is leaf-size weighted.
    pub points_mode: bool,
}

impl TodoProgress {
    /// Percent complete 0–100 (0 when total is 0).
    pub fn pct(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            (self.completed * 100) / self.total
        }
    }

    /// One-line summary for tool output / prompts.
    pub fn summary_line(&self) -> String {
        if self.total == 0 {
            return "Progress: none".into();
        }
        if self.points_mode {
            format!(
                "Progress: {}/{} pts ({}% · {}/{} leaves)",
                self.completed,
                self.total,
                self.pct(),
                self.leaves_done,
                self.leaves_total
            )
        } else {
            format!(
                "Progress: {}/{} ({}%)",
                self.completed,
                self.total,
                self.pct()
            )
        }
    }
}

/// Compute leaf-only (or legacy count) progress from the active board.
pub fn compute_leaf_progress(state: &TodoState) -> TodoProgress {
    use std::collections::HashSet;

    let parent_ids: HashSet<&str> = state.todo_items().filter_map(todo_parent_id).collect();

    // Leaves: not referenced as anyone's parentId, and not cancelled.
    let leaves: Vec<(&TodoId, &TodoItem)> = state
        .todo_items_with_ids()
        .filter(|(id, item)| {
            !parent_ids.contains(id.as_str()) && !matches!(item.status, TodoStatus::Cancelled)
        })
        .collect();

    let any_sized = leaves.iter().any(|(_, item)| item.size.is_some());

    if any_sized {
        let mut completed = 0u32;
        let mut total = 0u32;
        let mut leaves_done = 0u32;
        let mut leaves_total = 0u32;
        for (_, item) in &leaves {
            // Points mode: only explicit sizes; parent size already excluded
            // (parents are not leaves). Unsized leaves ignored for points.
            let Some(sz) = item.size else {
                continue;
            };
            total += u32::from(sz);
            leaves_total += 1;
            if matches!(item.status, TodoStatus::Completed) {
                completed += u32::from(sz);
                leaves_done += 1;
            }
        }
        TodoProgress {
            completed,
            total,
            leaves_done,
            leaves_total,
            points_mode: true,
        }
    } else {
        // Legacy: all non-cancelled items (matches status-bar badge counts).
        let mut completed = 0u32;
        let mut total = 0u32;
        for item in state.todo_items() {
            if matches!(item.status, TodoStatus::Cancelled) {
                continue;
            }
            total += 1;
            if matches!(item.status, TodoStatus::Completed) {
                completed += 1;
            }
        }
        let leaves_total = leaves.len() as u32;
        let leaves_done = leaves
            .iter()
            .filter(|(_, i)| matches!(i.status, TodoStatus::Completed))
            .count() as u32;
        TodoProgress {
            completed,
            total,
            leaves_done,
            leaves_total,
            points_mode: false,
        }
    }
}

/// Validate size rules on a write batch (before apply).
///
/// - size ∈ {1, 2} when set (field or meta.size)
/// - reject size on an id that already has children or gains children in batch
fn validate_write_sizes(state: &TodoState, updates: &[TodoUpdate]) -> Result<(), TodoError> {
    for u in updates {
        let size = resolve_todo_size(u.size, &u.meta).map_err(TodoError::InvalidSize)?;
        if size.is_some() && todo_id_has_children(state, &u.id, updates) {
            return Err(TodoError::InvalidSize(format!(
                "Todo \"{}\" has children — omit size on parents/containers \
                 (only leaf sizes 1|2 count toward progress).",
                u.id
            )));
        }
    }
    Ok(())
}

/// Clear `size` on any item that is a parent (referenced as `meta.parentId`).
///
/// Used after merge/replace so a former leaf that gains children does not keep
/// a zombie size (tool progress already ignores parents; this keeps state honest).
fn clear_sizes_on_parents(state: &mut TodoState) {
    use std::collections::HashSet;
    let parent_ids: HashSet<String> = state
        .todo_items()
        .filter_map(todo_parent_id)
        .map(str::to_owned)
        .collect();
    for pid in parent_ids {
        if let Some(item) = state.todos.get_mut(&pid)
            && item.size.is_some()
        {
            item.size = None;
        }
    }
}

pub(crate) fn validate_no_duplicate_ids(updates: &[TodoUpdate]) -> Result<(), TodoError> {
    use std::collections::HashSet;
    let mut seen = HashSet::with_capacity(updates.len());
    if let Some(dup) = updates.iter().map(|u| &u.id).find(|id| !seen.insert(*id)) {
        return Err(TodoError::DuplicateTodoID(dup.to_owned()));
    }
    Ok(())
}

/// Id prefixes owned by skills / session layers. On `merge=false` full replace,
/// items with these prefixes are **kept unless mentioned** in the replace
/// payload (so a skill cannot silently wipe foreign namespaces).
pub const PROTECTED_TODO_PREFIXES: &[&str] = &[
    "plan:",
    "impl:",
    "pr-",
    "recon:",
    "residual:",
    "ask:",
    "feat:",
    "bug:",
];

/// True when `id` starts with a protected skill/session namespace prefix.
pub fn is_protected_todo_id(id: &str) -> bool {
    PROTECTED_TODO_PREFIXES
        .iter()
        .any(|prefix| id.starts_with(prefix))
}

/// Prefix for auto-seeded user-ask todos (`ask:<prompt_id>`).
pub const ASK_TODO_PREFIX: &str = "ask:";

/// Max open `ask:*` todos kept on the board (oldest pruned first).
pub const MAX_ASK_TODOS: usize = 20;

/// Max entries kept in the off-board `cleared_todos` archive (oldest dropped).
pub const MAX_CLEARED_TODOS: usize = 200;

/// Truncate ask content for the board (chars, not bytes).
pub const ASK_CONTENT_MAX_CHARS: usize = 120;

/// Build a stable protected id for a user ask from its prompt id.
pub fn ask_todo_id(prompt_id: &str) -> String {
    let slug: String = prompt_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    let slug = if slug.is_empty() { "turn".into() } else { slug };
    format!("{ASK_TODO_PREFIX}{slug}")
}

/// Truncate user text for an ask todo content field.
pub fn truncate_ask_content(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_owned();
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Upsert one `ask:*` item and prune oldest asks beyond [`MAX_ASK_TODOS`].
///
/// Merge-only semantics: does not clear non-ask todos. Returns `true` only when
/// membership, content, or prune actually changed the board.
pub fn seed_ask_todo(state: &mut TodoState, prompt_id: &str, content: &str) -> bool {
    let content = truncate_ask_content(content, ASK_CONTENT_MAX_CHARS);
    if content.is_empty() {
        return false;
    }
    let id = ask_todo_id(prompt_id);
    let mut changed = false;
    if state.has_id(&id) {
        let prior = state
            .todo_items_with_ids()
            .find(|(i, _)| i.as_str() == id.as_str())
            .map(|(_, item)| item.content.clone());
        if prior.as_deref() != Some(content.as_str()) {
            let _ = state.update(&id, Some(&content), None, None, None, None);
            changed = true;
        }
    } else {
        state.push(
            id.clone(),
            TodoItem {
                content,
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: Some(serde_json::json!({
                    "kind": "work",
                    "namespace": "ask",
                })),
                size: None,
            },
        );
        changed = true;
    }
    let before_len = state.todo_items().count();
    prune_old_ask_todos(state, MAX_ASK_TODOS);
    if state.todo_items().count() != before_len {
        changed = true;
    }
    changed
}

/// Remove **completed** and **cancelled** items from the active board.
///
/// Each removed row is appended to [`TodoState::cleared_todos`] with
/// [`ClearedReason::UserClearCompleted`]. Pending and in-progress items stay.
/// Protected-prefix ids that are finished are cleared like any other done row
/// (open protected work is untouched).
///
/// Returns how many items were archived (0 = no-op).
///
/// This is the human **Clear finished** path — not `merge: false` wipe and not the
/// pane `h` hide-done view filter.
pub fn clear_completed_todos(state: &mut TodoState) -> usize {
    let ids: Vec<TodoId> = state
        .todo_items_with_ids()
        .filter(|(_, item)| matches!(item.status, TodoStatus::Completed | TodoStatus::Cancelled))
        .map(|(id, _)| id.clone())
        .collect();
    let mut n = 0usize;
    for id in ids {
        if let Some(snapshot) = state.todos.shift_remove(&id) {
            state.push_cleared(id, snapshot, ClearedReason::UserClearCompleted);
            n += 1;
        }
    }
    n
}

/// Drop oldest `ask:*` items (by insertion order) when over `max_asks`.
/// Prefers pruning completed/cancelled asks first, then oldest pending.
/// Dropped asks are appended to the capped [`TodoState::cleared_todos`] archive.
pub fn prune_old_ask_todos(state: &mut TodoState, max_asks: usize) {
    let ask_ids: Vec<TodoId> = state
        .todo_items_with_ids()
        .filter(|(id, _)| id.starts_with(ASK_TODO_PREFIX))
        .map(|(id, _)| id.clone())
        .collect();
    if ask_ids.len() <= max_asks {
        return;
    }
    let mut removable: Vec<TodoId> = ask_ids
        .iter()
        .filter(|id| {
            state
                .todo_items_with_ids()
                .find(|(i, _)| *i == *id)
                .map(|(_, item)| {
                    matches!(item.status, TodoStatus::Completed | TodoStatus::Cancelled)
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    for id in &ask_ids {
        if removable.len() >= ask_ids.len().saturating_sub(max_asks) {
            break;
        }
        if !removable.contains(id) {
            removable.push(id.clone());
        }
    }
    let to_remove = ask_ids.len().saturating_sub(max_asks);
    for id in removable.into_iter().take(to_remove) {
        if let Some(item) = state.todos.shift_remove(&id) {
            state.push_cleared(id, item, ClearedReason::AskPrune);
        }
    }
}

/// Choose the `plan.json` snapshot after compaction.
///
/// Prefer the live Resources `TodoState`. Never force an empty wipe when the
/// live board still has items (that was the historical lie on compact).
pub fn plan_json_snapshot_after_compact(live: Option<&TodoState>) -> TodoState {
    live.cloned().unwrap_or_default()
}

/// Resolve durable todo state on session resume.
///
/// Prefer non-empty Resources / `resources_state.json`. When tool state wins,
/// still **union in** any `ask:*` items present only in `plan.json` (backstop
/// when asks were mirrored to plan but not yet flushed to Resources). Fall back
/// to a non-empty `plan.json` when tool state is missing or empty.
///
/// Returns `(state, needs_tool_state_persist)` — the bool is true when the
/// caller should write Resources / `resources_state.json` (seed from plan or
/// ask union changed the tool snapshot).
pub fn effective_todo_state_on_resume(
    from_tool_state: Option<TodoState>,
    from_plan_json: Option<TodoState>,
) -> Option<(TodoState, bool)> {
    match from_tool_state {
        Some(mut tool) if !tool.is_empty() => {
            let mut merged_asks = false;
            if let Some(plan) = from_plan_json.as_ref() {
                for (id, item) in plan.todo_items_with_ids() {
                    if id.starts_with(ASK_TODO_PREFIX) && !tool.has_id(id) {
                        tool.push(id.clone(), item.clone());
                        merged_asks = true;
                    }
                }
            }
            Some((tool, merged_asks))
        }
        _ => from_plan_json.filter(|p| !p.is_empty()).map(|p| (p, true)), // tool empty → seed from plan; must persist tool_state
    }
}

/// True when `text` is slash-command shaped (leading `/` after trim) so ask
/// auto-seed should skip — builtins and skill slash seed their own boards.
pub fn is_slash_shaped_user_text(text: &str) -> bool {
    text.trim_start().starts_with('/')
}

/// Build a [`TodoItem`] from a write update (replace or insert-on-merge).
///
/// Callers must have already validated sizes via [`validate_write_sizes`].
fn item_from_update(u: &TodoUpdate) -> TodoItem {
    let content = if u.has_no_content() {
        u.id.clone()
    } else {
        // has_no_content is false ⇒ content is Some and non-empty.
        u.content.clone().unwrap()
    };
    // Prefer field; fall back to meta.size (validated earlier).
    let size = resolve_todo_size(u.size, &u.meta).ok().flatten();
    TodoItem {
        content,
        priority: u.priority.unwrap_or_default(),
        status: u.status.unwrap_or(TodoStatus::Pending),
        meta: u.meta.clone(),
        size,
    }
}

/// `merge=false`: the incoming list replaces the existing todo state, except
/// **protected-prefix** items (`plan:`, `impl:`, `pr-`, `recon:`, `residual:`,
/// `ask:`, `feat:`, `bug:`) that are **not** listed in `updates` are preserved
/// (keep-unless-mentioned).
/// Unprotected (or otherwise non-preserved) items that leave the active board
/// are appended to the capped [`TodoState::cleared_todos`] archive.
/// If `content` is omitted for an item, the `id` is used as a fallback.
/// If `status` is omitted, it defaults to `Pending`.
/// Optional `priority` / `meta` / `size` on each update are applied when present.
///
/// Returns the number of unprotected items archived by this replace.
pub(crate) fn apply_replace(
    state: &mut TodoState,
    updates: &[TodoUpdate],
) -> Result<usize, TodoError> {
    use std::collections::HashSet;
    let mentioned: HashSet<&str> = updates.iter().map(|u| u.id.as_str()).collect();
    // Snapshot protected items not in the replace set before clear.
    let preserved: Vec<(TodoId, TodoItem)> = state
        .todo_items_with_ids()
        .filter(|(id, _)| is_protected_todo_id(id) && !mentioned.contains(id.as_str()))
        .map(|(id, item)| (id.clone(), item.clone()))
        .collect();
    // Archive drops: unmentioned items that are not keep-unless-mentioned.
    // Mentioned ids stay on the board (replaced by payload); unmentioned
    // protected ids are re-attached — only unprotected unmentioned leave.
    let dropped: Vec<(TodoId, TodoItem)> = state
        .todo_items_with_ids()
        .filter(|(id, _)| !mentioned.contains(id.as_str()) && !is_protected_todo_id(id))
        .map(|(id, item)| (id.clone(), item.clone()))
        .collect();
    let dropped_count = dropped.len();
    for (id, item) in dropped {
        state.push_cleared(id, item, ClearedReason::ReplaceUnmentioned);
    }

    state.clear();
    for u in updates {
        state.push(u.id.clone(), item_from_update(u));
    }
    // Re-attach unmentioned protected items (order: after the replace set).
    for (id, item) in preserved {
        if !state.has_id(&id) {
            state.push(id, item);
        }
    }
    clear_sizes_on_parents(state);
    Ok(dropped_count)
}

/// `merge=true`: updates are merged into the existing state.
/// - **Existing items**: `content` / `priority` / `meta` / `size` are optional —
///   if omitted the previous value is kept. This lets the model mark an item
///   from `in_progress` → `completed` without echoing the content back.
/// - **New items** (id not yet in state): if `content` is omitted the `id`
///   is used as a fallback so the tool never errors on a merge call. This
///   makes the tool resilient to state being lost between calls.
pub(crate) fn apply_merge(state: &mut TodoState, updates: &[TodoUpdate]) -> Result<(), TodoError> {
    for u in updates {
        // `None` = omit size; `Some(v)` = set size to v (field or meta.size).
        let size_patch: Option<Option<u8>> =
            if u.size.is_some() || u.meta.as_ref().and_then(|m| m.get("size")).is_some() {
                // Validated by validate_write_sizes before apply.
                Some(resolve_todo_size(u.size, &u.meta).ok().flatten())
            } else {
                None
            };
        if state.update(
            &u.id,
            u.content.as_deref(),
            u.status,
            u.priority,
            u.meta.clone(),
            size_patch,
        ) {
            // Existing item – partial update succeeded, content was optional.
            continue;
        }
        state.push(u.id.clone(), item_from_update(u));
    }
    clear_sizes_on_parents(state);
    Ok(())
}

pub(crate) fn summarize_todo_state(state: &TodoState) -> String {
    if state.is_empty() {
        "No tasks currently tracked.".into()
    } else {
        let mut out = String::new();
        for (id, t) in state.todo_items_with_ids() {
            let size_tag = t.size.map(|s| format!(" size={s}")).unwrap_or_default();
            writeln!(
                &mut out,
                "- {} {id}: {}{size_tag}",
                t.status.tag(),
                t.content
            )
            .ok();
        }
        let progress = compute_leaf_progress(state);
        if progress.total > 0 {
            writeln!(&mut out, "{}", progress.summary_line()).ok();
        }
        out
    }
}

pub type TodoId = String;

// diff from acp: default to medium
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoPriority {
    High,
    #[default]
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl TodoStatus {
    pub const fn tag(&self) -> &str {
        match self {
            Self::Pending => "[pending]",
            Self::InProgress => "[in_progress]",
            Self::Completed => "[completed]",
            Self::Cancelled => "[cancelled]",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    #[serde(default)]
    pub priority: TodoPriority,
    pub status: TodoStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    /// Fibonacci leaf size: only **1** or **2** when set. Parents/containers
    /// omit this; only leaves contribute to weighted progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u8>,
}

/// Why an item left the active board and entered [`TodoState::cleared_todos`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClearedReason {
    /// Dropped by `merge: false` replace (unmentioned, not keep-unless-mentioned).
    ReplaceUnmentioned,
    /// Dropped by [`prune_old_ask_todos`] when over the ask cap.
    AskPrune,
    /// Operator cleared completed/cancelled rows from the live board
    /// (todo pane **Clear finished**, key, or `/clear-completed-todos`).
    UserClearCompleted,
}

/// Snapshot of a todo that left the active board (off-pane archive).
///
/// Not shown on the main todo pane or ACP Plan wire. `work_ulid` joins the
/// archive row to session-scoped work (usage.jsonl etc.) when
/// [`TodoState::session_work_ulid`] is set; otherwise a fresh ULID is minted
/// as a per-clear event id (still unique, not cross-log joinable).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearedTodo {
    pub id: TodoId,
    /// Content / status / priority / meta at drop time.
    pub snapshot: TodoItem,
    pub reason: ClearedReason,
    /// RFC3339 UTC timestamp when the item was archived.
    pub cleared_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_ulid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoState {
    todos: IndexMap<TodoId, TodoItem>,
    /// Capped ring of items dropped from the active board (not shown in UI).
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    cleared_todos: std::collections::VecDeque<ClearedTodo>,
    /// Session-scoped work join ULID (from `{session_dir}/work_ulid` / spawn).
    /// When set, [`Self::push_cleared`] stamps archive rows with this value
    /// so they join usage.jsonl for the same work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_work_ulid: Option<String>,
}

crate::register_resource!("grok_build", "Todo", TodoState);

impl TodoState {
    pub fn push(&mut self, id: TodoId, todo: TodoItem) {
        self.todos.insert(id, todo);
    }

    /// Prefer an explicit session work ULID for archive join with usage rows.
    pub fn set_session_work_ulid(&mut self, work_ulid: Option<String>) {
        self.session_work_ulid = work_ulid;
    }

    /// Append one entry to the off-board archive, enforcing [`MAX_CLEARED_TODOS`].
    ///
    /// `work_ulid` is the session work id when [`Self::session_work_ulid`] is
    /// set; otherwise a new ULID is minted (per-clear event id).
    pub fn push_cleared(&mut self, id: TodoId, snapshot: TodoItem, reason: ClearedReason) {
        let work_ulid = self
            .session_work_ulid
            .clone()
            .or_else(|| Some(crate::util::ulid::mint()));
        self.cleared_todos.push_back(ClearedTodo {
            id,
            snapshot,
            reason,
            cleared_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            work_ulid,
        });
        while self.cleared_todos.len() > MAX_CLEARED_TODOS {
            self.cleared_todos.pop_front();
        }
    }

    /// Clear the **active** board only. Archive history is preserved.
    pub fn clear(&mut self) {
        self.todos.clear();
    }

    /// Partial update of an existing item. Returns `false` if `id` is unknown.
    ///
    /// Omitted fields (`None`) leave the prior value unchanged. Empty-string
    /// `content` is treated as omitted (does not wipe).
    ///
    /// `size`: `None` = omit (keep prior); `Some(v)` = set `todo.size` to `v`
    /// (including `Some(None)` to clear).
    pub fn update(
        &mut self,
        id: &TodoId,
        content: Option<&str>,
        status: Option<TodoStatus>,
        priority: Option<TodoPriority>,
        meta: Option<serde_json::Value>,
        size: Option<Option<u8>>,
    ) -> bool {
        let Some(todo) = self.todos.get_mut(id) else {
            return false;
        };
        if let Some(content) = content
            && !content.is_empty()
        {
            todo.content = content.into();
        }
        if let Some(status) = status {
            todo.status = status;
        }
        if let Some(priority) = priority {
            todo.priority = priority;
        }
        if let Some(meta) = meta {
            todo.meta = Some(meta);
        }
        if let Some(sz) = size {
            todo.size = sz;
        }
        true
    }

    /// Active board items only (excludes [`Self::cleared_todos`]).
    pub fn todo_items(&self) -> impl Iterator<Item = &TodoItem> + '_ {
        self.todos.values()
    }

    /// Active board items with ids only (excludes archive).
    pub fn todo_items_with_ids(&self) -> impl Iterator<Item = (&TodoId, &TodoItem)> + '_ {
        self.todos.iter()
    }

    /// Off-board archive (oldest first). Not part of the live Plan / UI list.
    pub fn cleared_todos(&self) -> impl Iterator<Item = &ClearedTodo> + '_ {
        self.cleared_todos.iter()
    }

    pub fn cleared_len(&self) -> usize {
        self.cleared_todos.len()
    }

    /// True when the **active** board has no items (archive may still be non-empty).
    pub fn is_empty(&self) -> bool {
        self.todos.is_empty()
    }

    pub fn has_id(&self, id: &str) -> bool {
        self.todos.contains_key(id)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoUpdate {
    #[schemars(description = "Unique identifier for the todo item")]
    pub id: String,

    #[schemars(description = "The description/content of the todo item")]
    pub content: Option<String>,

    #[schemars(
        description = "The status of the todo item: pending, in_progress, completed, or cancelled"
    )]
    pub status: Option<TodoStatus>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional priority: high, medium, or low")]
    pub priority: Option<TodoPriority>,

    /// Optional metadata object for multi-level session boards.
    ///
    /// Documented keys (others allowed):
    /// - `kind`: `residual` | `phase` | `work` | `child`
    /// - `parentId`: id of a parent todo when nesting levels
    /// - `namespace`: owning skill/session prefix (e.g. `plan`, `impl`)
    /// - `size`: Fibonacci leaf size 1|2 (fallback when top-level `size` omitted)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Optional metadata JSON object. Documented keys: kind (residual|phase|work|child), parentId, namespace."
    )]
    pub meta: Option<serde_json::Value>,

    /// Optional Fibonacci leaf size: **only 1 or 2**. Larger work must be split
    /// into children. Parents/containers omit size (size on a parent is rejected).
    /// When omitted, a numeric `meta.size` is accepted and normalized here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Optional Fibonacci leaf size: only 1 or 2. Larger work must split into children. Parents omit size."
    )]
    pub size: Option<u8>,
}

impl TodoUpdate {
    /// True when the update carries no meaningful content (None or empty string).
    fn has_no_content(&self) -> bool {
        self.content.as_deref().is_none_or(str::is_empty)
    }
}

const fn default_merge() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoWriteInput {
    /// When true (the default), merge the provided todos into the existing
    /// list by id (partial updates are allowed — leave unchanged fields
    /// undefined). When explicitly set to false, the provided todos replace
    /// the existing list, except protected-prefix ids (`plan:`, `impl:`,
    /// `pr-`, `recon:`, `residual:`, `ask:`, `feat:`, `bug:`) that are not
    /// mentioned are kept.
    #[serde(
        default = "default_merge",
        deserialize_with = "crate::types::schema::deserialize_lenient_bool"
    )]
    #[schemars(
        description = "Optional. When true (default), merges the provided todos into the existing list by id — send only the items you are changing, and to flip status without changing content send just id + status. When false, the provided todos replace the existing list. Protected-prefix ids (plan:, impl:, pr-, recon:, residual:, ask:, feat:, bug:) not mentioned in the replace set are preserved so foreign namespaces are not silently wiped. Prefer merge:true always; avoid casual full replace."
    )]
    pub merge: bool,

    #[schemars(
        description = "Array of todo items to write. Prefer namespaced ids. Fib leaves size 1|2 only; parents unsized."
    )]
    pub todos: Vec<TodoUpdate>,
}

/// New-architecture `TodoWrite` tool.
///
/// State: `State<TodoState>` — persisted across calls via Resources serde.
/// Params: `()` — no per-tool configuration.
#[derive(Debug, Default)]
pub struct TodoWriteTool;

impl crate::types::tool_metadata::ToolMetadata for TodoWriteTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Plan
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Create and manage a structured task list. The user sees this list live — it is your primary way to show progress.

Use for any task with 3+ steps. Skip for trivial single-step work.

Prefer merge: true upsert only (never casually wipe with merge: false). Fibonacci work leaves size 1 or 2 only — anything larger must split into children; parents/containers omit size. Progress totals only leaf sizes. Prefer namespaced ids (plan:, impl:, feat:, bug:, …) and meta.kind + parentId for structure."#
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for TodoWriteTool {
    type Args = TodoWriteInput;
    type Output = TodoWriteOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("todo_write").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "todo_write",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(
        name = "new_tool.todo_write",
        skip_all,
        fields(merge = %input.merge, todo_count = input.todos.len())
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: TodoWriteInput,
    ) -> Result<TodoWriteOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        // Validate IDs upfront — return an error-as-output variant so the
        // Python side can distinguish this from infra errors.
        if let Err(TodoError::DuplicateTodoID(id)) = validate_no_duplicate_ids(&input.todos) {
            return Ok(TodoWriteOutput::DuplicateId(format!(
                "Duplicate todo ID in request: \"{id}\". Each todo item must have a unique ID."
            )));
        }

        let (summary_for_prompt, todos, state_snapshot, progress, warning);
        {
            let mut res = resources.lock().await;
            // Join cleared_todos with usage.jsonl when session work ULID is known.
            let session_wu = res
                .get::<crate::types::resources::SessionWorkUlid>()
                .map(|w| w.0.clone());
            let todo_state = res.get_or_default::<State<TodoState>>();
            if todo_state.0.session_work_ulid.is_none() {
                if let Some(wu) = session_wu {
                    todo_state.0.set_session_work_ulid(Some(wu));
                }
            }

            // Auto-upgrade to merge when the model forgot `merge: true` but
            // clearly intended a partial update: state already has items and
            // every update targets an existing ID without providing content.
            let effective_merge = input.merge
                || (!todo_state.0.is_empty()
                    && !input.todos.is_empty()
                    && input
                        .todos
                        .iter()
                        .all(|u| u.has_no_content() && todo_state.0.has_id(&u.id)));

            // Size validation before mutate (uses current graph + batch).
            if let Err(TodoError::InvalidSize(msg)) =
                validate_write_sizes(&todo_state.0, &input.todos)
            {
                return Ok(TodoWriteOutput::InvalidArgument(msg));
            }

            let mut archived = 0usize;
            if effective_merge {
                apply_merge(&mut todo_state.0, &input.todos)?;
            } else {
                archived = apply_replace(&mut todo_state.0, &input.todos)?;
            }
            // Parent sizes cleared inside apply_merge / apply_replace.

            progress = compute_leaf_progress(&todo_state.0);
            summary_for_prompt = summarize_todo_state(&todo_state.0);
            todos = todo_state.0.todo_items().cloned().collect::<Vec<_>>();
            state_snapshot = todo_state.0.clone();
            warning = if !effective_merge && archived > 0 {
                Some(format!(
                    "merge:false archived {archived} unprotected todo(s) not in the replace set. \
                     Prefer merge:true upsert; protected prefixes are kept unless mentioned."
                ))
            } else {
                None
            };
        }

        Ok(TodoWriteOutput::TodosUpdated(TodoWriteSuccess {
            summary_for_prompt,
            todos,
            state: state_snapshot,
            progress,
            warning,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::output::TodoWriteOutput;
    use crate::types::resources::Resources;
    use crate::types::tool_metadata::test_ctx;

    // -- Helpers --

    fn make_update(id: &str, content: Option<&str>, status: Option<TodoStatus>) -> TodoUpdate {
        TodoUpdate {
            id: id.to_owned(),
            content: content.map(str::to_owned),
            status,
            priority: None,
            meta: None,
            size: None,
        }
    }

    fn make_update_with_meta(
        id: &str,
        content: Option<&str>,
        status: Option<TodoStatus>,
        priority: Option<TodoPriority>,
        meta: Option<serde_json::Value>,
    ) -> TodoUpdate {
        TodoUpdate {
            id: id.to_owned(),
            content: content.map(str::to_owned),
            status,
            priority,
            meta,
            size: None,
        }
    }

    fn make_update_with_size(
        id: &str,
        content: Option<&str>,
        status: Option<TodoStatus>,
        size: Option<u8>,
    ) -> TodoUpdate {
        TodoUpdate {
            id: id.to_owned(),
            content: content.map(str::to_owned),
            status,
            priority: None,
            meta: None,
            size,
        }
    }

    /// Unwrap a `TodoWriteOutput` expecting the `TodosUpdated` variant.
    fn expect_success(output: TodoWriteOutput) -> TodoWriteSuccess {
        match output {
            TodoWriteOutput::TodosUpdated(s) => s,
            other => panic!("expected TodosUpdated, got {other:?}"),
        }
    }

    // -- Tests --

    #[test]
    fn name_and_description() {
        use crate::types::tool_metadata::ToolMetadata;
        let tool = TodoWriteTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "todo_write");
        assert!(ToolMetadata::description_template(&tool).contains("task list"));
    }

    #[tokio::test]
    async fn replace_mode_creates_items() {
        let tool = TodoWriteTool;
        let resources = Resources::new();

        let input = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("1", Some("Task A"), Some(TodoStatus::Pending)),
                make_update("2", Some("Task B"), Some(TodoStatus::InProgress)),
            ],
        };

        let shared = resources.into_shared();
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.len(), 2);
        assert!(output.summary_for_prompt.contains("Task A"));
        assert!(output.summary_for_prompt.contains("Task B"));

        // State persists in Resources
        let res = shared.lock().await;
        let state = res.get::<State<TodoState>>().unwrap();
        assert_eq!(state.0.todo_items().count(), 2);
    }

    #[tokio::test]
    async fn replace_clears_previous_state() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        // Seed initial state
        let input1 = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "old",
                Some("Old task"),
                Some(TodoStatus::Completed),
            )],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input1)
            .await
            .unwrap();

        // Replace with new
        let input2 = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "new",
                Some("New task"),
                Some(TodoStatus::Pending),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input2)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.len(), 1);
        assert!(output.summary_for_prompt.contains("New task"));
        assert!(!output.summary_for_prompt.contains("Old task"));
    }

    #[tokio::test]
    async fn merge_mode_updates_existing() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        // Create initial items
        let input1 = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("1", Some("Build project"), Some(TodoStatus::InProgress)),
                make_update("2", Some("Run tests"), Some(TodoStatus::Pending)),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input1)
            .await
            .unwrap();

        // Merge: mark item 1 completed (no content), add item 3
        let input2 = TodoWriteInput {
            merge: true,
            todos: vec![
                make_update("1", None, Some(TodoStatus::Completed)),
                make_update("3", Some("Deploy"), Some(TodoStatus::Pending)),
            ],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input2)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.len(), 3);

        // Item 1 content preserved, status updated
        let item1 = output
            .todos
            .iter()
            .find(|t| t.content == "Build project")
            .unwrap();
        assert_eq!(item1.status, TodoStatus::Completed);
    }

    #[tokio::test]
    async fn merge_with_lost_state_uses_id_fallback() {
        let tool = TodoWriteTool;
        let resources = Resources::new();

        // Merge into empty state — should not error
        let input = TodoWriteInput {
            merge: true,
            todos: vec![make_update("explore", None, Some(TodoStatus::Completed))],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.len(), 1);
        // Id used as fallback content
        assert_eq!(output.todos[0].content, "explore");
        assert_eq!(output.todos[0].status, TodoStatus::Completed);
    }

    #[tokio::test]
    async fn duplicate_ids_rejected() {
        let tool = TodoWriteTool;
        let resources = Resources::new();

        let input = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("dup", Some("A"), Some(TodoStatus::Pending)),
                make_update("dup", Some("B"), Some(TodoStatus::Pending)),
            ],
        };
        let result = xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
            .await
            .unwrap();
        assert!(
            matches!(result, TodoWriteOutput::DuplicateId(ref msg) if msg.contains("dup")),
            "expected DuplicateId variant, got {result:?}"
        );
    }

    #[tokio::test]
    async fn empty_todos_shows_no_tasks_message() {
        let tool = TodoWriteTool;
        let resources = Resources::new();

        let input = TodoWriteInput {
            merge: false,
            todos: vec![],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
                .await
                .unwrap(),
        );
        assert!(output.summary_for_prompt.contains("No tasks"));
        assert!(output.todos.is_empty());
    }

    #[tokio::test]
    async fn state_output_includes_snapshot() {
        let tool = TodoWriteTool;
        let resources = Resources::new();

        let input = TodoWriteInput {
            merge: false,
            todos: vec![make_update("1", Some("Task"), Some(TodoStatus::Pending))],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
                .await
                .unwrap(),
        );

        // state field should match what's in Resources
        assert!(!output.state.is_empty());
        assert_eq!(output.state.todo_items().count(), 1);
    }

    #[tokio::test]
    async fn state_serialization_roundtrip() {
        let tool = TodoWriteTool;
        let mut resources = Resources::new();
        resources.register_state::<TodoState>();

        // Create some state
        let input = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("1", Some("First"), Some(TodoStatus::Completed)),
                make_update("2", Some("Second"), Some(TodoStatus::InProgress)),
            ],
        };
        let shared = resources.into_shared();
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input)
            .await
            .unwrap();

        // Serialize
        let res = shared.lock().await;
        let snapshot = res.serialize();
        let state_map = snapshot.get("state").unwrap();
        assert!(
            state_map.get("grok_build.Todo").is_some(),
            "TodoState should serialize under 'grok_build.Todo'"
        );

        // Deserialize into fresh Resources
        let mut resources2 = Resources::new();
        resources2.register_state::<TodoState>();
        let data: std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        > = serde_json::from_value(snapshot).unwrap();
        resources2.load_from(data);

        // Verify state was restored
        let restored = resources2.get::<State<TodoState>>().unwrap();
        assert_eq!(restored.0.todo_items().count(), 2);
        let items: Vec<_> = restored.0.todo_items().collect();
        assert_eq!(items[0].content, "First");
        assert_eq!(items[0].status, TodoStatus::Completed);
        assert_eq!(items[1].content, "Second");
        assert_eq!(items[1].status, TodoStatus::InProgress);
    }

    fn seed_state(items: &[(&str, &str, TodoStatus)]) -> TodoState {
        let mut state = TodoState::default();
        for (id, content, status) in items {
            state.push(
                id.to_string(),
                TodoItem {
                    content: content.to_string(),
                    priority: TodoPriority::default(),
                    status: *status,
                    meta: None,
                    size: None,
                },
            );
        }
        state
    }

    fn get_item<'a>(state: &'a TodoState, id: &str) -> &'a TodoItem {
        state
            .todo_items_with_ids()
            .find(|(i, _)| *i == id)
            .map(|(_, item)| item)
            .unwrap_or_else(|| panic!("item {id} not found in state"))
    }

    // ── replace (merge=false) ────────────────────────────────────────

    #[test]
    fn replace_without_content_falls_back_to_id() {
        let mut state = TodoState::default();
        let updates = vec![make_update(
            "build_project",
            None,
            Some(TodoStatus::Pending),
        )];
        apply_replace(&mut state, &updates).unwrap();

        let item = get_item(&state, "build_project");
        assert_eq!(item.content, "build_project"); // id used as fallback
        assert_eq!(item.status, TodoStatus::Pending);
    }

    #[test]
    fn replace_without_content_or_status_defaults() {
        let mut state = TodoState::default();
        let updates = vec![make_update("task_1", None, None)];
        apply_replace(&mut state, &updates).unwrap();

        let item = get_item(&state, "task_1");
        assert_eq!(item.content, "task_1");
        assert_eq!(item.status, TodoStatus::Pending);
    }

    #[test]
    fn replace_with_content_succeeds() {
        let mut state = TodoState::default();
        let updates = vec![
            make_update("1", Some("Task A"), Some(TodoStatus::Pending)),
            make_update("2", Some("Task B"), Some(TodoStatus::InProgress)),
        ];
        apply_replace(&mut state, &updates).unwrap();

        assert_eq!(get_item(&state, "1").content, "Task A");
        assert_eq!(get_item(&state, "1").status, TodoStatus::Pending);
        assert_eq!(get_item(&state, "2").content, "Task B");
        assert_eq!(get_item(&state, "2").status, TodoStatus::InProgress);
    }

    #[test]
    fn replace_clears_previous_state_unit() {
        let mut state = seed_state(&[("old", "Old task", TodoStatus::Completed)]);
        let updates = vec![make_update(
            "new",
            Some("New task"),
            Some(TodoStatus::Pending),
        )];
        apply_replace(&mut state, &updates).unwrap();

        // Old item is gone.
        assert!(!state.todo_items_with_ids().any(|(id, _)| *id == "old"));
        assert_eq!(get_item(&state, "new").content, "New task");
    }

    // ── merge (merge=true) ───────────────────────────────────────────

    #[test]
    fn merge_existing_item_status_only() {
        // The core use-case: mark in_progress → completed without sending content.
        let mut state = seed_state(&[("1", "Build the project", TodoStatus::InProgress)]);
        let updates = vec![make_update("1", None, Some(TodoStatus::Completed))];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "1");
        assert_eq!(item.status, TodoStatus::Completed);
        assert_eq!(item.content, "Build the project"); // unchanged
    }

    #[test]
    fn merge_existing_item_content_and_status() {
        let mut state = seed_state(&[("1", "Old text", TodoStatus::Pending)]);
        let updates = vec![make_update(
            "1",
            Some("New text"),
            Some(TodoStatus::InProgress),
        )];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "1");
        assert_eq!(item.content, "New text");
        assert_eq!(item.status, TodoStatus::InProgress);
    }

    #[test]
    fn merge_existing_item_no_fields_is_noop() {
        let mut state = seed_state(&[("1", "Keep me", TodoStatus::Pending)]);
        let updates = vec![make_update("1", None, None)];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "1");
        assert_eq!(item.content, "Keep me");
        assert_eq!(item.status, TodoStatus::Pending);
    }

    #[test]
    fn merge_new_item_without_content_uses_id_fallback() {
        // When state is empty (e.g. lost between calls) and content is None,
        // the id is used as fallback content instead of erroring.
        let mut state = TodoState::default();
        let updates = vec![make_update(
            "explore_codebase",
            None,
            Some(TodoStatus::Completed),
        )];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "explore_codebase");
        assert_eq!(item.content, "explore_codebase"); // id used as fallback
        assert_eq!(item.status, TodoStatus::Completed);
    }

    #[test]
    fn merge_new_item_without_content_or_status_defaults_to_pending() {
        let mut state = TodoState::default();
        let updates = vec![make_update("task_1", None, None)];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "task_1");
        assert_eq!(item.content, "task_1");
        assert_eq!(item.status, TodoStatus::Pending);
    }

    #[test]
    fn merge_new_item_with_content_succeeds() {
        let mut state = TodoState::default();
        let updates = vec![make_update(
            "1",
            Some("Fresh task"),
            Some(TodoStatus::Pending),
        )];
        apply_merge(&mut state, &updates).unwrap();

        assert_eq!(get_item(&state, "1").content, "Fresh task");
    }

    #[test]
    fn merge_mixed_existing_and_new() {
        let mut state = seed_state(&[("exist", "Existing task", TodoStatus::InProgress)]);
        let updates = vec![
            // Update existing — content omitted, just flip status.
            make_update("exist", None, Some(TodoStatus::Completed)),
            // Brand-new item — content required.
            make_update("fresh", Some("New task"), Some(TodoStatus::Pending)),
        ];
        apply_merge(&mut state, &updates).unwrap();

        let existing = get_item(&state, "exist");
        assert_eq!(existing.status, TodoStatus::Completed);
        assert_eq!(existing.content, "Existing task"); // preserved

        let fresh = get_item(&state, "fresh");
        assert_eq!(fresh.content, "New task");
        assert_eq!(fresh.status, TodoStatus::Pending);
    }

    // ── duplicate id validation ──────────────────────────────────────

    #[test]
    fn duplicate_ids_rejected_unit() {
        let updates = vec![
            make_update("dup", Some("A"), Some(TodoStatus::Pending)),
            make_update("dup", Some("B"), Some(TodoStatus::Pending)),
        ];
        let err = validate_no_duplicate_ids(&updates).unwrap_err();
        assert!(matches!(err, TodoError::DuplicateTodoID(ref id) if id == "dup"));
    }

    #[test]
    fn unique_ids_accepted() {
        let updates = vec![
            make_update("a", Some("A"), Some(TodoStatus::Pending)),
            make_update("b", Some("B"), Some(TodoStatus::Pending)),
        ];
        validate_no_duplicate_ids(&updates).unwrap();
    }

    // ── regression: missing merge=true auto-upgrade ────────────────────

    #[tokio::test]
    async fn missing_merge_flag_auto_upgrades_when_status_only() {
        // Regression: status-only update without merge=true must not wipe content.
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        // Create todos with content
        let input1 = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("1", Some("Explore codebase"), Some(TodoStatus::InProgress)),
                make_update("2", Some("Review tools"), Some(TodoStatus::Pending)),
                make_update("3", Some("Write tests"), Some(TodoStatus::Pending)),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input1)
            .await
            .unwrap();

        // Status-only update without merge=true
        let input2 = TodoWriteInput {
            merge: false, // model forgot merge: true
            todos: vec![
                make_update("1", None, Some(TodoStatus::Completed)),
                make_update("2", None, Some(TodoStatus::Completed)),
                make_update("3", None, Some(TodoStatus::InProgress)),
            ],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input2)
                .await
                .unwrap(),
        );

        // Content must be preserved, not replaced with id fallback.
        assert_eq!(output.todos.len(), 3);
        assert_eq!(output.todos[0].content, "Explore codebase");
        assert_eq!(output.todos[0].status, TodoStatus::Completed);
        assert_eq!(output.todos[1].content, "Review tools");
        assert_eq!(output.todos[1].status, TodoStatus::Completed);
        assert_eq!(output.todos[2].content, "Write tests");
        assert_eq!(output.todos[2].status, TodoStatus::InProgress);
    }

    // ── regression: merge with null content should never error ────────

    #[test]
    fn merge_after_replace_status_update_with_null_content() {
        // Reproduces the exact scenario from the bug report:
        // 1. Replace creates 3 items
        // 2. Merge updates 2 items with content=null, status changed
        let mut state = TodoState::default();

        // Step 1: replace (merge=false)
        let initial = vec![
            make_update(
                "explore_codebase",
                Some("Explore django/db/backends/sqlite3/"),
                Some(TodoStatus::InProgress),
            ),
            make_update(
                "analyze_and_propose",
                Some("Analyze current SQLite min version"),
                Some(TodoStatus::Pending),
            ),
            make_update(
                "implementation",
                Some("Update version checks"),
                Some(TodoStatus::Pending),
            ),
        ];
        apply_replace(&mut state, &initial).unwrap();

        // Step 2: merge (merge=true) — content=null, just status changes
        let updates = vec![
            make_update("explore_codebase", None, Some(TodoStatus::Completed)),
            make_update("analyze_and_propose", None, Some(TodoStatus::InProgress)),
        ];
        apply_merge(&mut state, &updates).unwrap();

        // Statuses flipped, content preserved from step 1.
        assert_eq!(
            get_item(&state, "explore_codebase").status,
            TodoStatus::Completed
        );
        assert_eq!(
            get_item(&state, "explore_codebase").content,
            "Explore django/db/backends/sqlite3/"
        );
        assert_eq!(
            get_item(&state, "analyze_and_propose").status,
            TodoStatus::InProgress
        );
        assert_eq!(
            get_item(&state, "analyze_and_propose").content,
            "Analyze current SQLite min version"
        );
        // Third item unchanged.
        assert_eq!(
            get_item(&state, "implementation").status,
            TodoStatus::Pending
        );
    }

    // ── regression: empty-string content must not wipe existing content ──

    #[test]
    fn merge_existing_item_empty_string_content_preserves_original() {
        // Model sends content: "" instead of omitting it. Must not wipe.
        let mut state = seed_state(&[("1", "Build the project", TodoStatus::InProgress)]);
        let updates = vec![make_update("1", Some(""), Some(TodoStatus::Completed))];
        apply_merge(&mut state, &updates).unwrap();

        let item = get_item(&state, "1");
        assert_eq!(item.status, TodoStatus::Completed);
        assert_eq!(item.content, "Build the project"); // unchanged
    }

    #[test]
    fn replace_empty_string_content_falls_back_to_id() {
        let mut state = TodoState::default();
        let updates = vec![make_update("task_1", Some(""), Some(TodoStatus::Pending))];
        apply_replace(&mut state, &updates).unwrap();

        assert_eq!(get_item(&state, "task_1").content, "task_1");
    }

    #[test]
    fn merge_new_item_empty_string_content_falls_back_to_id() {
        let mut state = TodoState::default();
        let updates = vec![make_update("task_1", Some(""), Some(TodoStatus::Pending))];
        apply_merge(&mut state, &updates).unwrap();

        assert_eq!(get_item(&state, "task_1").content, "task_1");
    }

    #[test]
    fn merge_with_null_content_and_lost_state() {
        // Same scenario but state was lost between calls (empty state).
        // The tool should still not error — falls back to id as content.
        let mut state = TodoState::default();

        let updates = vec![
            make_update("explore_codebase", None, Some(TodoStatus::Completed)),
            make_update("analyze_and_propose", None, Some(TodoStatus::InProgress)),
        ];
        apply_merge(&mut state, &updates).unwrap();

        assert_eq!(
            get_item(&state, "explore_codebase").content,
            "explore_codebase"
        );
        assert_eq!(
            get_item(&state, "explore_codebase").status,
            TodoStatus::Completed
        );
        assert_eq!(
            get_item(&state, "analyze_and_propose").content,
            "analyze_and_propose"
        );
        assert_eq!(
            get_item(&state, "analyze_and_propose").status,
            TodoStatus::InProgress
        );
    }

    // ── priority + meta write path ───────────────────────────────────

    #[tokio::test]
    async fn meta_and_priority_round_trip_via_todo_write() {
        let tool = TodoWriteTool;
        let mut resources = Resources::new();
        resources.register_state::<TodoState>();
        let shared = resources.into_shared();

        let meta = serde_json::json!({
            "kind": "phase",
            "parentId": "plan:root",
            "namespace": "impl"
        });
        let input = TodoWriteInput {
            merge: false,
            todos: vec![make_update_with_meta(
                "impl:1",
                Some("Wire meta fields"),
                Some(TodoStatus::InProgress),
                Some(TodoPriority::High),
                Some(meta.clone()),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.len(), 1);
        assert_eq!(output.todos[0].priority, TodoPriority::High);
        assert_eq!(output.todos[0].meta, Some(meta.clone()));

        // Merge status-only must preserve priority + meta.
        let input2 = TodoWriteInput {
            merge: true,
            todos: vec![make_update("impl:1", None, Some(TodoStatus::Completed))],
        };
        let output2 = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), input2)
                .await
                .unwrap(),
        );
        let item = output2
            .todos
            .iter()
            .find(|t| t.content == "Wire meta fields")
            .expect("content preserved");
        assert_eq!(item.status, TodoStatus::Completed);
        assert_eq!(item.priority, TodoPriority::High);
        assert_eq!(item.meta, Some(meta));

        // Resources state still holds meta after serialize/load.
        {
            let res = shared.lock().await;
            let snapshot = res.serialize();
            drop(res);
            let mut resources2 = Resources::new();
            resources2.register_state::<TodoState>();
            let data: std::collections::HashMap<
                String,
                std::collections::HashMap<String, serde_json::Value>,
            > = serde_json::from_value(snapshot).unwrap();
            resources2.load_from(data);
            let restored = resources2.get::<State<TodoState>>().unwrap();
            let item = get_item(&restored.0, "impl:1");
            assert_eq!(item.priority, TodoPriority::High);
            assert_eq!(
                item.meta.as_ref().and_then(|m| m.get("kind")),
                Some(&serde_json::json!("phase"))
            );
        }
    }

    #[tokio::test]
    async fn merge_false_preserves_foreign_prefix_items_not_in_replace_set() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        // Seed mixed board: plan + recon + feat + bug + plain.
        let seed = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("plan:1", Some("Plan step"), Some(TodoStatus::Pending)),
                make_update(
                    "recon:inventory",
                    Some("Inventory crates"),
                    Some(TodoStatus::InProgress),
                ),
                make_update(
                    "feat:my-idea",
                    Some("Feature suggestion"),
                    Some(TodoStatus::Pending),
                ),
                make_update(
                    "bug:repro",
                    Some("User-reported bug"),
                    Some(TodoStatus::Pending),
                ),
                make_update("scratch", Some("Ephemeral"), Some(TodoStatus::Pending)),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
            .await
            .unwrap();

        // Implement skill opens with merge:false and only its own ids.
        let replace = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "impl:1",
                Some("Do the slice"),
                Some(TodoStatus::InProgress),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), replace)
                .await
                .unwrap(),
        );

        let ids: Vec<_> = output
            .state
            .todo_items_with_ids()
            .map(|(id, _)| id.as_str())
            .collect();
        assert!(ids.contains(&"plan:1"), "plan:* must survive: {ids:?}");
        assert!(
            ids.contains(&"recon:inventory"),
            "recon:* must survive: {ids:?}"
        );
        assert!(
            ids.contains(&"feat:my-idea"),
            "feat:* must survive: {ids:?}"
        );
        assert!(ids.contains(&"bug:repro"), "bug:* must survive: {ids:?}");
        assert!(ids.contains(&"impl:1"), "new impl item present: {ids:?}");
        assert!(
            !ids.contains(&"scratch"),
            "unprotected unmentioned id is dropped: {ids:?}"
        );
        assert_eq!(get_item(&output.state, "plan:1").content, "Plan step");
        assert_eq!(
            get_item(&output.state, "recon:inventory").content,
            "Inventory crates"
        );
        assert_eq!(
            get_item(&output.state, "feat:my-idea").content,
            "Feature suggestion"
        );
        assert_eq!(
            get_item(&output.state, "bug:repro").content,
            "User-reported bug"
        );
    }

    #[tokio::test]
    async fn merge_false_can_replace_protected_when_mentioned() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        let seed = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "plan:1",
                Some("Old plan text"),
                Some(TodoStatus::Pending),
            )],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
            .await
            .unwrap();

        let replace = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "plan:1",
                Some("Updated plan text"),
                Some(TodoStatus::Completed),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), replace)
                .await
                .unwrap(),
        );
        assert_eq!(output.state.todo_items().count(), 1);
        assert_eq!(
            get_item(&output.state, "plan:1").content,
            "Updated plan text"
        );
        assert_eq!(
            get_item(&output.state, "plan:1").status,
            TodoStatus::Completed
        );
    }

    #[test]
    fn old_callers_json_without_priority_or_meta_still_deserialize() {
        // Legacy callers send only id/content/status.
        let json = serde_json::json!({
            "merge": true,
            "todos": [
                {"id": "1", "content": "Legacy task", "status": "pending"}
            ]
        });
        let input: TodoWriteInput = serde_json::from_value(json).unwrap();
        assert!(input.merge);
        assert_eq!(input.todos.len(), 1);
        assert_eq!(input.todos[0].id, "1");
        assert_eq!(input.todos[0].content.as_deref(), Some("Legacy task"));
        assert_eq!(input.todos[0].status, Some(TodoStatus::Pending));
        assert_eq!(input.todos[0].priority, None);
        assert_eq!(input.todos[0].meta, None);

        let mut state = TodoState::default();
        apply_merge(&mut state, &input.todos).unwrap();
        let item = get_item(&state, "1");
        assert_eq!(item.content, "Legacy task");
        assert_eq!(item.priority, TodoPriority::Medium);
        assert_eq!(item.meta, None);
    }

    #[test]
    fn protected_prefix_helpers() {
        assert!(is_protected_todo_id("plan:1"));
        assert!(is_protected_todo_id("impl:slice"));
        assert!(is_protected_todo_id("pr-3:fix"));
        assert!(is_protected_todo_id("recon:map"));
        assert!(is_protected_todo_id("residual:open"));
        assert!(is_protected_todo_id("ask:turn-1"));
        assert!(is_protected_todo_id("feat:my-idea"));
        assert!(is_protected_todo_id("bug:repro"));
        assert!(!is_protected_todo_id("1"));
        assert!(!is_protected_todo_id("scratch"));
        assert!(!is_protected_todo_id("planning")); // not plan: prefix
        assert!(!is_protected_todo_id("asking")); // not ask: prefix
        assert!(!is_protected_todo_id("feature")); // not feat: prefix
        assert!(!is_protected_todo_id("bugs")); // not bug: prefix
    }

    #[test]
    fn plan_json_snapshot_after_compact_keeps_live_board() {
        let mut live = TodoState::default();
        live.push(
            "impl:1".into(),
            TodoItem {
                content: "still open".into(),
                priority: TodoPriority::High,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        let snap = plan_json_snapshot_after_compact(Some(&live));
        assert!(!snap.is_empty());
        assert!(snap.has_id("impl:1"));
        // Empty live → empty snapshot is honest (not a forced wipe of non-empty).
        assert!(plan_json_snapshot_after_compact(None).is_empty());
        assert!(plan_json_snapshot_after_compact(Some(&TodoState::default())).is_empty());
    }

    #[test]
    fn effective_todo_state_on_resume_prefers_tool_state() {
        let mut tool = TodoState::default();
        tool.push(
            "impl:a".into(),
            TodoItem {
                content: "from tool".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        let mut plan = TodoState::default();
        plan.push(
            "impl:b".into(),
            TodoItem {
                content: "from plan".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        let (got, need_persist) =
            effective_todo_state_on_resume(Some(tool.clone()), Some(plan.clone())).unwrap();
        assert!(got.has_id("impl:a"));
        assert!(!got.has_id("impl:b"));
        assert!(!need_persist, "no ask merge → no forced tool_state rewrite");

        let (from_plan, need_seed) =
            effective_todo_state_on_resume(Some(TodoState::default()), Some(plan.clone())).unwrap();
        assert!(from_plan.has_id("impl:b"));
        assert!(need_seed, "plan fallback must flag tool_state persist");

        let (from_plan_only, need_seed2) =
            effective_todo_state_on_resume(None, Some(plan)).unwrap();
        assert!(from_plan_only.has_id("impl:b"));
        assert!(need_seed2);

        assert!(effective_todo_state_on_resume(None, None).is_none());
        assert!(
            effective_todo_state_on_resume(Some(TodoState::default()), Some(TodoState::default()))
                .is_none()
        );
    }

    #[test]
    fn effective_todo_state_on_resume_unions_asks_from_plan() {
        let mut tool = TodoState::default();
        tool.push(
            "impl:a".into(),
            TodoItem {
                content: "from tool".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        let mut plan = TodoState::default();
        plan.push(
            "impl:a".into(),
            TodoItem {
                content: "stale plan impl".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        plan.push(
            ask_todo_id("turn-1"),
            TodoItem {
                content: "user ask only on plan".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: None,
            },
        );
        let (got, need_persist) = effective_todo_state_on_resume(Some(tool), Some(plan)).unwrap();
        assert!(got.has_id("impl:a"));
        assert_eq!(
            got.todo_items_with_ids()
                .find(|(id, _)| *id == "impl:a")
                .unwrap()
                .1
                .content,
            "from tool",
            "tool content wins for non-ask"
        );
        assert!(got.has_id(&ask_todo_id("turn-1")), "ask from plan survives");
        assert!(need_persist, "ask union requires tool_state flush");
    }

    #[test]
    fn seed_ask_todo_false_when_unchanged() {
        let mut state = TodoState::default();
        assert!(seed_ask_todo(&mut state, "t1", "same question"));
        assert!(!seed_ask_todo(&mut state, "t1", "same question"));
        assert!(seed_ask_todo(&mut state, "t1", "edited question"));
    }

    #[test]
    fn seed_ask_survives_with_nonempty_tool_board() {
        // Regression: asks must land on the same board as prior impl:* so
        // resume (tool_state preferred) keeps them after a pure-text turn.
        let mut tool = TodoState::default();
        tool.push(
            "impl:1".into(),
            TodoItem {
                content: "prior work".into(),
                priority: TodoPriority::High,
                status: TodoStatus::InProgress,
                meta: None,
                size: None,
            },
        );
        assert!(seed_ask_todo(
            &mut tool,
            "user-abc",
            "please also fix resume durability"
        ));
        let ask_id = ask_todo_id("user-abc");
        assert!(tool.has_id("impl:1"));
        assert!(tool.has_id(&ask_id));

        let (restored, need) =
            effective_todo_state_on_resume(Some(tool.clone()), Some(TodoState::default())).unwrap();
        assert!(restored.has_id(&ask_id));
        assert!(restored.has_id("impl:1"));
        assert!(!need);

        // Broken history: tool has only impl, plan has ask → union + persist flag.
        let mut tool_stale = TodoState::default();
        tool_stale.push(
            "impl:1".into(),
            TodoItem {
                content: "prior work".into(),
                priority: TodoPriority::High,
                status: TodoStatus::InProgress,
                meta: None,
                size: None,
            },
        );
        let mut plan_with_ask = TodoState::default();
        seed_ask_todo(
            &mut plan_with_ask,
            "user-abc",
            "please also fix resume durability",
        );
        let (merged, need_flush) =
            effective_todo_state_on_resume(Some(tool_stale), Some(plan_with_ask)).unwrap();
        assert!(merged.has_id(&ask_id));
        assert!(need_flush);
    }

    #[test]
    fn is_slash_shaped_user_text_helper() {
        assert!(is_slash_shaped_user_text("/resume"));
        assert!(is_slash_shaped_user_text("  /implement foo"));
        assert!(!is_slash_shaped_user_text("please /mention something"));
        assert!(!is_slash_shaped_user_text("fix the board"));
    }

    #[test]
    fn seed_ask_todo_and_protect_on_merge_false() {
        let mut state = TodoState::default();
        assert!(seed_ask_todo(
            &mut state,
            "user-turn-abc",
            "Please fix the resume board after compact"
        ));
        let ask_id = ask_todo_id("user-turn-abc");
        assert!(state.has_id(&ask_id));
        assert!(is_protected_todo_id(&ask_id));

        // merge:false with only impl:* must keep unmentioned ask:*
        apply_replace(
            &mut state,
            &[make_update(
                "impl:1",
                Some("do work"),
                Some(TodoStatus::Pending),
            )],
        )
        .unwrap();
        assert!(
            state.has_id(&ask_id),
            "ask:* must survive merge:false keep-unless-mentioned"
        );
        assert!(state.has_id("impl:1"));
    }

    #[test]
    fn seed_ask_todo_prunes_beyond_cap() {
        let mut state = TodoState::default();
        for i in 0..(MAX_ASK_TODOS + 5) {
            assert!(seed_ask_todo(
                &mut state,
                &format!("turn-{i}"),
                &format!("Ask number {i}")
            ));
        }
        let ask_count = state
            .todo_items_with_ids()
            .filter(|(id, _)| id.starts_with(ASK_TODO_PREFIX))
            .count();
        assert_eq!(ask_count, MAX_ASK_TODOS);
    }

    #[test]
    fn truncate_ask_content_ellipsis() {
        let long = "x".repeat(200);
        let t = truncate_ask_content(&long, 10);
        assert_eq!(t.chars().count(), 10);
        assert!(t.ends_with('…'));
        assert_eq!(truncate_ask_content("  hi  ", 120), "hi");
    }

    #[test]
    fn merge_updates_priority_and_meta_on_existing() {
        let mut state = seed_state(&[("1", "Task", TodoStatus::Pending)]);
        let updates = vec![make_update_with_meta(
            "1",
            None,
            Some(TodoStatus::InProgress),
            Some(TodoPriority::Low),
            Some(serde_json::json!({"kind": "work"})),
        )];
        apply_merge(&mut state, &updates).unwrap();
        let item = get_item(&state, "1");
        assert_eq!(item.content, "Task");
        assert_eq!(item.status, TodoStatus::InProgress);
        assert_eq!(item.priority, TodoPriority::Low);
        assert_eq!(
            item.meta.as_ref().and_then(|m| m.get("kind")),
            Some(&serde_json::json!("work"))
        );
    }

    // ── cleared_todos archive (merge:false drops + ask prune) ─────────

    #[test]
    fn merge_false_archives_unprotected_drops() {
        let mut state = seed_state(&[
            ("scratch", "Ephemeral work", TodoStatus::Pending),
            ("plan:1", "Keep me", TodoStatus::Pending),
            ("other", "Also drop", TodoStatus::InProgress),
        ]);
        apply_replace(
            &mut state,
            &[make_update(
                "impl:1",
                Some("new work"),
                Some(TodoStatus::Pending),
            )],
        )
        .unwrap();

        // Active board: new impl + preserved plan.
        assert!(state.has_id("impl:1"));
        assert!(state.has_id("plan:1"));
        assert!(!state.has_id("scratch"));
        assert!(!state.has_id("other"));

        let cleared: Vec<_> = state.cleared_todos().collect();
        assert_eq!(cleared.len(), 2, "both unprotected drops archived");
        let ids: Vec<&str> = cleared.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"scratch"));
        assert!(ids.contains(&"other"));
        for c in &cleared {
            assert_eq!(c.reason, ClearedReason::ReplaceUnmentioned);
            let wu = c.work_ulid.as_deref().expect("work_ulid minted on clear");
            assert!(
                crate::util::ulid::is_valid(wu),
                "work_ulid must be ULID: {wu}"
            );
            assert!(!c.cleared_at.is_empty());
        }
        // Without session_work_ulid, each clear mints its own id (not shared).
        let a = cleared[0].work_ulid.as_ref().unwrap();
        let b = cleared[1].work_ulid.as_ref().unwrap();
        assert_ne!(a, b, "per-clear event ids differ without session work id");
        let scratch = cleared.iter().find(|c| c.id == "scratch").unwrap();
        assert_eq!(scratch.snapshot.content, "Ephemeral work");
        assert_eq!(scratch.snapshot.status, TodoStatus::Pending);
        let other = cleared.iter().find(|c| c.id == "other").unwrap();
        assert_eq!(other.snapshot.content, "Also drop");
        assert_eq!(other.snapshot.status, TodoStatus::InProgress);
    }

    #[test]
    fn cleared_todos_use_session_work_ulid_when_set() {
        let session_wu = crate::util::ulid::mint();
        let mut state = seed_state(&[("scratch", "drop me", TodoStatus::Pending)]);
        state.set_session_work_ulid(Some(session_wu.clone()));
        apply_replace(
            &mut state,
            &[make_update(
                "impl:1",
                Some("new"),
                Some(TodoStatus::Pending),
            )],
        )
        .unwrap();
        let cleared: Vec<_> = state.cleared_todos().collect();
        assert_eq!(cleared.len(), 1);
        assert_eq!(
            cleared[0].work_ulid.as_deref(),
            Some(session_wu.as_str()),
            "archive row joins session work_ulid"
        );
    }

    #[test]
    fn merge_false_protected_keep_does_not_archive() {
        let mut state = seed_state(&[
            ("plan:1", "Plan step", TodoStatus::Pending),
            ("recon:map", "Inventory", TodoStatus::InProgress),
            ("feat:idea", "Feature", TodoStatus::Pending),
            ("bug:repro", "Bug", TodoStatus::Pending),
            ("ask:turn-1", "User ask", TodoStatus::Pending),
            ("scratch", "Gone", TodoStatus::Pending),
        ]);
        apply_replace(
            &mut state,
            &[make_update(
                "impl:1",
                Some("slice"),
                Some(TodoStatus::InProgress),
            )],
        )
        .unwrap();

        for id in [
            "plan:1",
            "recon:map",
            "feat:idea",
            "bug:repro",
            "ask:turn-1",
            "impl:1",
        ] {
            assert!(state.has_id(id), "{id} must stay active");
        }
        assert!(!state.has_id("scratch"));

        let cleared: Vec<_> = state.cleared_todos().map(|c| c.id.as_str()).collect();
        assert_eq!(cleared, vec!["scratch"]);
        // Protected ids must never appear only in archive after keep-unless-mentioned.
        for id in [
            "plan:1",
            "recon:map",
            "feat:idea",
            "bug:repro",
            "ask:turn-1",
        ] {
            assert!(
                !cleared.contains(&id),
                "protected {id} must not be archived when unmentioned"
            );
        }
    }

    #[test]
    fn cleared_todos_cap_drops_oldest() {
        let mut state = TodoState::default();
        // Fill past the cap via direct archive helper.
        for i in 0..(MAX_CLEARED_TODOS + 15) {
            state.push_cleared(
                format!("drop-{i}"),
                TodoItem {
                    content: format!("item {i}"),
                    priority: TodoPriority::Medium,
                    status: TodoStatus::Pending,
                    meta: None,
                    size: None,
                },
                ClearedReason::ReplaceUnmentioned,
            );
        }
        assert_eq!(state.cleared_len(), MAX_CLEARED_TODOS);
        let first = state.cleared_todos().next().unwrap();
        assert_eq!(
            first.id,
            format!("drop-{}", 15),
            "oldest entries pop_front under cap"
        );
        let last = state.cleared_todos().last().unwrap();
        assert_eq!(last.id, format!("drop-{}", MAX_CLEARED_TODOS + 14));
        // Active board still empty.
        assert!(state.is_empty());
        assert_eq!(state.todo_items().count(), 0);
    }

    #[test]
    fn active_list_api_excludes_cleared_items() {
        let mut state = seed_state(&[
            ("keep", "Stay", TodoStatus::Pending),
            ("drop-me", "Archive me", TodoStatus::Completed),
        ]);
        apply_replace(
            &mut state,
            &[make_update("keep", Some("Stay"), Some(TodoStatus::Pending))],
        )
        .unwrap();

        assert_eq!(state.todo_items().count(), 1);
        assert_eq!(state.todo_items_with_ids().count(), 1);
        assert!(state.has_id("keep"));
        assert!(!state.has_id("drop-me"));
        assert_eq!(state.cleared_len(), 1);
        assert_eq!(
            state.cleared_todos().next().unwrap().id,
            "drop-me",
            "archive holds the drop; active iterators do not"
        );

        // Prompt summary + tool output shape use active only.
        let summary = summarize_todo_state(&state);
        assert!(summary.contains("Stay"));
        assert!(!summary.contains("Archive me"));
        assert!(!summary.contains("drop-me"));
    }

    #[tokio::test]
    async fn todo_write_output_todos_are_active_only() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        let seed = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("scratch", Some("Ephemeral"), Some(TodoStatus::Pending)),
                make_update("plan:1", Some("Plan"), Some(TodoStatus::Pending)),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
            .await
            .unwrap();

        let replace = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "impl:1",
                Some("Do work"),
                Some(TodoStatus::InProgress),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), replace)
                .await
                .unwrap(),
        );

        // Live list in tool output: active only (impl + preserved plan).
        assert_eq!(output.todos.len(), 2);
        assert!(
            output
                .todos
                .iter()
                .all(|t| t.content == "Do work" || t.content == "Plan"),
            "cleared scratch must not appear in output.todos: {:?}",
            output.todos
        );
        assert!(!output.summary_for_prompt.contains("Ephemeral"));
        assert!(output.state.cleared_len() >= 1);
        assert!(
            output
                .state
                .cleared_todos()
                .any(|c| c.id == "scratch" && c.snapshot.content == "Ephemeral")
        );
    }

    #[test]
    fn ask_prune_archives_with_ask_prune_reason() {
        let mut state = TodoState::default();
        for i in 0..(MAX_ASK_TODOS + 3) {
            assert!(seed_ask_todo(
                &mut state,
                &format!("turn-{i}"),
                &format!("Ask number {i}")
            ));
        }
        let ask_count = state
            .todo_items_with_ids()
            .filter(|(id, _)| id.starts_with(ASK_TODO_PREFIX))
            .count();
        assert_eq!(ask_count, MAX_ASK_TODOS);
        assert_eq!(state.cleared_len(), 3);
        for c in state.cleared_todos() {
            assert_eq!(c.reason, ClearedReason::AskPrune);
            assert!(c.id.starts_with(ASK_TODO_PREFIX));
        }
    }

    #[test]
    fn cleared_todos_round_trip_via_resources_serde() {
        let mut state = seed_state(&[("scratch", "Gone", TodoStatus::Pending)]);
        apply_replace(
            &mut state,
            &[make_update(
                "impl:1",
                Some("stay"),
                Some(TodoStatus::Pending),
            )],
        )
        .unwrap();
        assert_eq!(state.cleared_len(), 1);

        let json = serde_json::to_value(&state).unwrap();
        assert!(
            json.get("cleared_todos").is_some() || json.get("clearedTodos").is_some(),
            "archive should serialize (got keys: {:?})",
            json.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        let restored: TodoState = serde_json::from_value(json).unwrap();
        assert_eq!(restored.cleared_len(), 1);
        assert_eq!(restored.cleared_todos().next().unwrap().id, "scratch");
        assert!(restored.has_id("impl:1"));
        assert!(!restored.has_id("scratch"));

        // Legacy payload without cleared_todos still deserializes.
        let legacy = serde_json::json!({
            "todos": {
                "1": {
                    "content": "Legacy",
                    "priority": "medium",
                    "status": "pending"
                }
            }
        });
        let from_legacy: TodoState = serde_json::from_value(legacy).unwrap();
        assert!(from_legacy.has_id("1"));
        assert_eq!(from_legacy.cleared_len(), 0);
    }

    // ── clear_completed_todos (operator Clear finished) ────────────────

    #[test]
    fn clear_completed_archives_done_and_cancelled_leaves_open() {
        let mut state = seed_state(&[
            ("open", "Still working", TodoStatus::Pending),
            ("run", "In flight", TodoStatus::InProgress),
            ("done", "Finished", TodoStatus::Completed),
            ("nope", "Dropped", TodoStatus::Cancelled),
            ("feat:shipped", "Protected finished", TodoStatus::Completed),
            ("plan:open", "Protected open", TodoStatus::Pending),
        ]);
        let n = clear_completed_todos(&mut state);
        assert_eq!(n, 3, "completed + cancelled + finished protected");
        assert!(state.has_id("open"));
        assert!(state.has_id("run"));
        assert!(state.has_id("plan:open"));
        assert!(!state.has_id("done"));
        assert!(!state.has_id("nope"));
        assert!(!state.has_id("feat:shipped"));

        let cleared: Vec<_> = state.cleared_todos().collect();
        assert_eq!(cleared.len(), 3);
        for c in &cleared {
            assert_eq!(c.reason, ClearedReason::UserClearCompleted);
            assert!(matches!(
                c.snapshot.status,
                TodoStatus::Completed | TodoStatus::Cancelled
            ));
        }
        let ids: Vec<&str> = cleared.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"done"));
        assert!(ids.contains(&"nope"));
        assert!(ids.contains(&"feat:shipped"));
    }

    #[test]
    fn clear_completed_is_noop_when_nothing_done() {
        let mut state = seed_state(&[
            ("a", "One", TodoStatus::Pending),
            ("b", "Two", TodoStatus::InProgress),
        ]);
        assert_eq!(clear_completed_todos(&mut state), 0);
        assert_eq!(state.todo_items().count(), 2);
        assert_eq!(state.cleared_len(), 0);
    }

    #[test]
    fn clear_completed_reason_serde_round_trip() {
        let mut state = seed_state(&[("done", "x", TodoStatus::Completed)]);
        assert_eq!(clear_completed_todos(&mut state), 1);
        let json = serde_json::to_value(&state).unwrap();
        let restored: TodoState = serde_json::from_value(json).unwrap();
        assert_eq!(restored.cleared_len(), 1);
        assert_eq!(
            restored.cleared_todos().next().unwrap().reason,
            ClearedReason::UserClearCompleted
        );
        // Wire spelling is snake_case.
        let reason_json = serde_json::to_value(ClearedReason::UserClearCompleted).unwrap();
        assert_eq!(reason_json, serde_json::json!("user_clear_completed"));
    }

    #[test]
    fn clear_completed_updates_leaf_progress_badge_math() {
        let mut state = TodoState::default();
        state.push(
            "leaf-done".into(),
            TodoItem {
                content: "done leaf".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Completed,
                meta: None,
                size: Some(2),
            },
        );
        state.push(
            "leaf-open".into(),
            TodoItem {
                content: "open leaf".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
                size: Some(1),
            },
        );
        let before = compute_leaf_progress(&state);
        assert!(before.points_mode);
        assert_eq!(before.completed, 2);
        assert_eq!(before.total, 3);

        assert_eq!(clear_completed_todos(&mut state), 1);
        let after = compute_leaf_progress(&state);
        assert!(after.points_mode);
        assert_eq!(after.completed, 0);
        assert_eq!(after.total, 1);
        assert!(state.has_id("leaf-open"));
        assert!(!state.has_id("leaf-done"));
    }

    // ── Fibonacci size + leaf progress ─────────────────────────────────

    #[test]
    fn validate_todo_size_accepts_1_and_2_rejects_others() {
        assert_eq!(validate_todo_size_value(1).unwrap(), 1);
        assert_eq!(validate_todo_size_value(2).unwrap(), 2);
        for bad in [0u8, 3, 5, 8, 13] {
            let err = validate_todo_size_value(bad).unwrap_err();
            assert!(
                err.contains("only 1 or 2"),
                "expected fib rejection for {bad}, got {err}"
            );
        }
    }

    #[test]
    fn resolve_size_from_meta_when_field_omitted() {
        let meta = Some(serde_json::json!({"size": 2, "kind": "work"}));
        assert_eq!(resolve_todo_size(None, &meta).unwrap(), Some(2));
        // Field wins over meta.
        assert_eq!(resolve_todo_size(Some(1), &meta).unwrap(), Some(1));
        let bad = Some(serde_json::json!({"size": 5}));
        assert!(resolve_todo_size(None, &bad).unwrap_err().contains("5"));
    }

    #[test]
    fn compute_leaf_progress_legacy_counts_without_sizes() {
        let state = seed_state(&[
            ("a", "A", TodoStatus::Completed),
            ("b", "B", TodoStatus::Pending),
            ("c", "C", TodoStatus::Cancelled),
        ]);
        let p = compute_leaf_progress(&state);
        assert!(!p.points_mode);
        assert_eq!(p.completed, 1);
        assert_eq!(p.total, 2); // cancelled excluded
    }

    #[test]
    fn compute_leaf_progress_points_sums_sized_leaves_only() {
        let mut state = TodoState::default();
        // Parent phase (unsized) with two sized children.
        state.push(
            "impl:phase".into(),
            TodoItem {
                content: "Phase".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::InProgress,
                meta: Some(serde_json::json!({"kind": "phase"})),
                size: None,
            },
        );
        state.push(
            "impl:a".into(),
            TodoItem {
                content: "Leaf A".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Completed,
                meta: Some(serde_json::json!({"kind": "work", "parentId": "impl:phase"})),
                size: Some(2),
            },
        );
        state.push(
            "impl:b".into(),
            TodoItem {
                content: "Leaf B".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: Some(serde_json::json!({"kind": "work", "parentId": "impl:phase"})),
                size: Some(1),
            },
        );
        // Unsized leaf ignored in points mode.
        state.push(
            "impl:c".into(),
            TodoItem {
                content: "No size".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: Some(serde_json::json!({"kind": "work", "parentId": "impl:phase"})),
                size: None,
            },
        );
        let p = compute_leaf_progress(&state);
        assert!(p.points_mode);
        assert_eq!(p.completed, 2);
        assert_eq!(p.total, 3); // 2+1; unsized leaf ignored; parent not counted
        assert_eq!(p.leaves_done, 1);
        assert_eq!(p.leaves_total, 2);
    }

    #[test]
    fn compute_leaf_progress_ignores_parent_size_when_has_children() {
        let mut state = TodoState::default();
        // Parent incorrectly sized — still excluded because it has children.
        state.push(
            "parent".into(),
            TodoItem {
                content: "Parent".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Completed,
                meta: Some(serde_json::json!({"kind": "phase"})),
                size: Some(2),
            },
        );
        state.push(
            "child".into(),
            TodoItem {
                content: "Child".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Completed,
                meta: Some(serde_json::json!({"parentId": "parent"})),
                size: Some(1),
            },
        );
        let p = compute_leaf_progress(&state);
        assert!(p.points_mode);
        assert_eq!(p.completed, 1);
        assert_eq!(p.total, 1);
    }

    #[tokio::test]
    async fn todo_write_rejects_size_not_1_or_2() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let input = TodoWriteInput {
            merge: true,
            todos: vec![make_update_with_size(
                "w",
                Some("Too big"),
                Some(TodoStatus::Pending),
                Some(5),
            )],
        };
        let result = xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
            .await
            .unwrap();
        match result {
            TodoWriteOutput::InvalidArgument(msg) => {
                assert!(msg.contains("5"), "got {msg}");
                assert!(msg.contains("1 or 2"), "got {msg}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn todo_write_accepts_size_1_and_2_and_reports_progress() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();
        let input = TodoWriteInput {
            merge: true,
            todos: vec![
                make_update_with_size("a", Some("Small"), Some(TodoStatus::Completed), Some(1)),
                make_update_with_size("b", Some("Medium"), Some(TodoStatus::Pending), Some(2)),
            ],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared), input)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos.iter().filter(|t| t.size == Some(1)).count(), 1);
        assert_eq!(output.todos.iter().filter(|t| t.size == Some(2)).count(), 1);
        assert!(output.progress.points_mode);
        assert_eq!(output.progress.completed, 1);
        assert_eq!(output.progress.total, 3);
        assert!(output.summary_for_prompt.contains("Progress:"));
        assert!(output.summary_for_prompt.contains("pts"));
    }

    #[tokio::test]
    async fn todo_write_normalizes_meta_size_into_field() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let input = TodoWriteInput {
            merge: true,
            todos: vec![make_update_with_meta(
                "m",
                Some("From meta"),
                Some(TodoStatus::Pending),
                None,
                Some(serde_json::json!({"size": 2, "kind": "work"})),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
                .await
                .unwrap(),
        );
        assert_eq!(output.todos[0].size, Some(2));
    }

    #[tokio::test]
    async fn todo_write_rejects_size_on_parent_with_children() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();
        // Seed parent + child
        let seed = TodoWriteInput {
            merge: true,
            todos: vec![
                make_update("parent", Some("Parent"), Some(TodoStatus::Pending)),
                make_update_with_meta(
                    "child",
                    Some("Child"),
                    Some(TodoStatus::Pending),
                    None,
                    Some(serde_json::json!({"parentId": "parent"})),
                ),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
            .await
            .unwrap();

        let bad = TodoWriteInput {
            merge: true,
            todos: vec![make_update_with_size("parent", None, None, Some(2))],
        };
        let result = xai_tool_runtime::Tool::run(&tool, test_ctx(shared), bad)
            .await
            .unwrap();
        match result {
            TodoWriteOutput::InvalidArgument(msg) => {
                assert!(
                    msg.contains("children") || msg.contains("parent"),
                    "got {msg}"
                );
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Same batch: parent sized + child parentId → reject (not deferred clear).
    #[tokio::test]
    async fn todo_write_rejects_same_batch_parent_size_with_child() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let input = TodoWriteInput {
            merge: true,
            todos: vec![
                make_update_with_size("parent", Some("Parent"), Some(TodoStatus::Pending), Some(2)),
                make_update_with_meta(
                    "child",
                    Some("Child"),
                    Some(TodoStatus::Pending),
                    None,
                    Some(serde_json::json!({"parentId": "parent"})),
                ),
            ],
        };
        let result = xai_tool_runtime::Tool::run(&tool, test_ctx(resources.into_shared()), input)
            .await
            .unwrap();
        match result {
            TodoWriteOutput::InvalidArgument(msg) => {
                assert!(
                    msg.contains("children") || msg.contains("parent"),
                    "got {msg}"
                );
            }
            other => panic!("expected InvalidArgument for same-batch parent size, got {other:?}"),
        }
    }

    /// Size first as a leaf, then attach a child → parent size cleared; progress leaf-only.
    #[tokio::test]
    async fn todo_write_clears_parent_size_when_child_attaches_later() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();

        // (1) Write sized leaf.
        let seed = TodoWriteInput {
            merge: true,
            todos: vec![make_update_with_size(
                "parent",
                Some("Will become parent"),
                Some(TodoStatus::Pending),
                Some(2),
            )],
        };
        let first = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
                .await
                .unwrap(),
        );
        assert_eq!(first.todos[0].size, Some(2));
        assert_eq!(first.progress.total, 2);

        // (2) Attach sized child — parent must lose size; points = child only.
        let child = TodoWriteInput {
            merge: true,
            todos: vec![TodoUpdate {
                id: "child".into(),
                content: Some("Leaf child".into()),
                status: Some(TodoStatus::Completed),
                priority: None,
                meta: Some(serde_json::json!({"parentId": "parent"})),
                size: Some(1),
            }],
        };
        let second = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared), child)
                .await
                .unwrap(),
        );
        let parent_item = second
            .state
            .todo_items_with_ids()
            .find(|(id, _)| id.as_str() == "parent")
            .map(|(_, item)| item)
            .expect("parent on board");
        assert_eq!(
            parent_item.size, None,
            "parent size must be cleared when children attach"
        );
        assert!(second.progress.points_mode);
        assert_eq!(second.progress.total, 1, "only child leaf size counts");
        assert_eq!(second.progress.completed, 1);
    }

    #[tokio::test]
    async fn merge_false_warns_when_archiving_unprotected() {
        let tool = TodoWriteTool;
        let resources = Resources::new();
        let shared = resources.into_shared();
        let seed = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("scratch", Some("Wipe me"), Some(TodoStatus::Pending)),
                make_update("impl:keep", Some("Keep"), Some(TodoStatus::Pending)),
            ],
        };
        xai_tool_runtime::Tool::run(&tool, test_ctx(shared.clone()), seed)
            .await
            .unwrap();

        let replace = TodoWriteInput {
            merge: false,
            todos: vec![make_update(
                "impl:keep",
                Some("Keep"),
                Some(TodoStatus::Completed),
            )],
        };
        let output = expect_success(
            xai_tool_runtime::Tool::run(&tool, test_ctx(shared), replace)
                .await
                .unwrap(),
        );
        let warning = output.warning.expect("archive warning");
        assert!(warning.contains("archived"), "{warning}");
        assert!(
            warning.contains("merge:false") || warning.contains("merge: false"),
            "{warning}"
        );
    }
}
