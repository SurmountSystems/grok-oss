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
pub const PROTECTED_TODO_PREFIXES: &[&str] = &["plan:", "impl:", "pr-", "recon:", "residual:"];

/// True when `id` starts with a protected skill/session namespace prefix.
pub fn is_protected_todo_id(id: &str) -> bool {
    PROTECTED_TODO_PREFIXES
        .iter()
        .any(|prefix| id.starts_with(prefix))
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
/// **protected-prefix** items (`plan:`, `impl:`, `pr-`, `recon:`, `residual:`)
/// that are **not** listed in `updates` are preserved (keep-unless-mentioned).
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
    /// `pr-`, `recon:`, `residual:`) that are not mentioned are kept.
    #[serde(
        default = "default_merge",
        deserialize_with = "crate::types::schema::deserialize_lenient_bool"
    )]
    #[schemars(
        description = "Optional. When true (default), merges the provided todos into the existing list by id — send only the items you are changing, and to flip status without changing content send just id + status. When false, the provided todos replace the existing list. Protected-prefix ids (plan:, impl:, pr-, recon:, residual:) not mentioned in the replace set are preserved so foreign namespaces are not silently wiped."
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

        // Seed mixed board: plan + recon + plain.
        let seed = TodoWriteInput {
            merge: false,
            todos: vec![
                make_update("plan:1", Some("Plan step"), Some(TodoStatus::Pending)),
                make_update(
                    "recon:inventory",
                    Some("Inventory crates"),
                    Some(TodoStatus::InProgress),
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
        assert!(!is_protected_todo_id("1"));
        assert!(!is_protected_todo_id("scratch"));
        assert!(!is_protected_todo_id("planning")); // not plan: prefix
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
