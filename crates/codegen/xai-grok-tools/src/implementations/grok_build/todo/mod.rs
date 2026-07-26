//! TodoWrite — new-architecture implementation.
//!
//! Reuses the core logic (`validate_no_duplicate_ids`, `apply_replace`,
//! `apply_merge`, `summarize_todo_state`) from the old `implementations::todo`
//! module. State is stored as `State<TodoState>` in Resources instead of
//! `ToolState.todo_state`.

use std::fmt::Write;

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
pub const PROTECTED_TODO_PREFIXES: &[&str] =
    &["plan:", "impl:", "pr-", "recon:", "residual:", "ask:", "feat:"];

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
            let _ = state.update(&id, Some(&content), None, None, None);
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

/// Drop oldest `ask:*` items (by insertion order) when over `max_asks`.
/// Prefers pruning completed/cancelled asks first, then oldest pending.
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
        state.todos.shift_remove(&id);
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
fn item_from_update(u: &TodoUpdate) -> TodoItem {
    let content = if u.has_no_content() {
        u.id.clone()
    } else {
        // has_no_content is false ⇒ content is Some and non-empty.
        u.content.clone().unwrap()
    };
    TodoItem {
        content,
        priority: u.priority.unwrap_or_default(),
        status: u.status.unwrap_or(TodoStatus::Pending),
        meta: u.meta.clone(),
    }
}

/// `merge=false`: the incoming list replaces the existing todo state, except
/// **protected-prefix** items (`plan:`, `impl:`, `pr-`, `recon:`, `residual:`,
/// `ask:`, `feat:`) that are **not** listed in `updates` are preserved
/// (keep-unless-mentioned).
/// If `content` is omitted for an item, the `id` is used as a fallback.
/// If `status` is omitted, it defaults to `Pending`.
/// Optional `priority` / `meta` on each update are applied when present.
pub(crate) fn apply_replace(
    state: &mut TodoState,
    updates: &[TodoUpdate],
) -> Result<(), TodoError> {
    use std::collections::HashSet;
    let mentioned: HashSet<&str> = updates.iter().map(|u| u.id.as_str()).collect();
    // Snapshot protected items not in the replace set before clear.
    let preserved: Vec<(TodoId, TodoItem)> = state
        .todo_items_with_ids()
        .filter(|(id, _)| is_protected_todo_id(id) && !mentioned.contains(id.as_str()))
        .map(|(id, item)| (id.clone(), item.clone()))
        .collect();

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
    Ok(())
}

/// `merge=true`: updates are merged into the existing state.
/// - **Existing items**: `content` / `priority` / `meta` are optional — if
///   omitted the previous value is kept. This lets the model mark an item
///   from `in_progress` → `completed` without echoing the content back.
/// - **New items** (id not yet in state): if `content` is omitted the `id`
///   is used as a fallback so the tool never errors on a merge call. This
///   makes the tool resilient to state being lost between calls.
pub(crate) fn apply_merge(state: &mut TodoState, updates: &[TodoUpdate]) -> Result<(), TodoError> {
    for u in updates {
        if state.update(
            &u.id,
            u.content.as_deref(),
            u.status,
            u.priority,
            u.meta.clone(),
        ) {
            // Existing item – partial update succeeded, content was optional.
            continue;
        }
        state.push(u.id.clone(), item_from_update(u));
    }
    Ok(())
}

pub(crate) fn summarize_todo_state(state: &TodoState) -> String {
    if state.is_empty() {
        "No tasks currently tracked.".into()
    } else {
        let mut out = String::new();
        for (id, t) in state.todo_items_with_ids() {
            writeln!(&mut out, "- {} {id}: {}", t.status.tag(), t.content).ok();
        }
        out
    }
}

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoState {
    todos: IndexMap<TodoId, TodoItem>,
}

crate::register_resource!("grok_build", "Todo", TodoState);

impl TodoState {
    pub fn push(&mut self, id: TodoId, todo: TodoItem) {
        self.todos.insert(id, todo);
    }

    pub fn clear(&mut self) {
        self.todos.clear();
    }

    /// Partial update of an existing item. Returns `false` if `id` is unknown.
    ///
    /// Omitted fields (`None`) leave the prior value unchanged. Empty-string
    /// `content` is treated as omitted (does not wipe).
    pub fn update(
        &mut self,
        id: &TodoId,
        content: Option<&str>,
        status: Option<TodoStatus>,
        priority: Option<TodoPriority>,
        meta: Option<serde_json::Value>,
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
        true
    }

    pub fn todo_items(&self) -> impl Iterator<Item = &TodoItem> + '_ {
        self.todos.values()
    }

    pub fn todo_items_with_ids(&self) -> impl Iterator<Item = (&TodoId, &TodoItem)> + '_ {
        self.todos.iter()
    }

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Optional metadata JSON object. Documented keys: kind (residual|phase|work|child), parentId, namespace."
    )]
    pub meta: Option<serde_json::Value>,
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
    /// `pr-`, `recon:`, `residual:`, `ask:`, `feat:`) that are not mentioned
    /// are kept.
    #[serde(
        default = "default_merge",
        deserialize_with = "crate::types::schema::deserialize_lenient_bool"
    )]
    #[schemars(
        description = "Optional. When true (default), merges the provided todos into the existing list by id — send only the items you are changing, and to flip status without changing content send just id + status. When false, the provided todos replace the existing list. Protected-prefix ids (plan:, impl:, pr-, recon:, residual:, ask:, feat:) not mentioned in the replace set are preserved so foreign namespaces are not silently wiped."
    )]
    pub merge: bool,

    #[schemars(description = "Array of todo items to write to the workspace")]
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

Use for any task with 3+ steps. Skip for trivial single-step work."#
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

        let (summary_for_prompt, todos, state_snapshot);
        {
            let mut res = resources.lock().await;
            let todo_state = res.get_or_default::<State<TodoState>>();

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

            if effective_merge {
                apply_merge(&mut todo_state.0, &input.todos)?;
            } else {
                apply_replace(&mut todo_state.0, &input.todos)?;
            }

            summary_for_prompt = summarize_todo_state(&todo_state.0);
            todos = todo_state.0.todo_items().cloned().collect::<Vec<_>>();
            state_snapshot = todo_state.0.clone();
        }

        Ok(TodoWriteOutput::TodosUpdated(TodoWriteSuccess {
            summary_for_prompt,
            todos,
            state: state_snapshot,
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

        // Seed mixed board: plan + recon + feat + plain.
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
        assert!(!is_protected_todo_id("1"));
        assert!(!is_protected_todo_id("scratch"));
        assert!(!is_protected_todo_id("planning")); // not plan: prefix
        assert!(!is_protected_todo_id("asking")); // not ask: prefix
        assert!(!is_protected_todo_id("feature")); // not feat: prefix
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
            },
        );
        plan.push(
            ask_todo_id("turn-1"),
            TodoItem {
                content: "user ask only on plan".into(),
                priority: TodoPriority::Medium,
                status: TodoStatus::Pending,
                meta: None,
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
}
