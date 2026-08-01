//! Types are canonical in `xai-grok-tools`.
//! This module adds conversions between ACP plan entries and `TodoItem` since `xai-grok-tools` is protocol-agnostic.

pub use xai_grok_tools::implementations::grok_build::todo::TodoId;
pub use xai_grok_tools::implementations::grok_build::todo::TodoItem;
pub use xai_grok_tools::implementations::grok_build::todo::TodoPriority;
pub use xai_grok_tools::implementations::grok_build::todo::TodoState;
pub use xai_grok_tools::implementations::grok_build::todo::TodoStatus;
pub use xai_grok_tools::implementations::grok_build::todo::{
    ASK_CONTENT_MAX_CHARS, ASK_TODO_PREFIX, MAX_ASK_TODOS, PROTECTED_TODO_PREFIXES, ask_todo_id,
    clear_completed_todos, effective_todo_state_on_resume, is_protected_todo_id,
    is_slash_shaped_user_text, plan_json_snapshot_after_compact, prune_old_ask_todos,
    seed_ask_todo, truncate_ask_content,
};
pub use xai_grok_tools::implementations::grok_build::todo::{ClearedReason, ClearedTodo};

use agent_client_protocol as acp;

/// ACP has no `Cancelled` status, so cancelled items are stored as `Completed` with `{"cancelled": true}` in meta.
pub fn todo_item_from_plan_entry(entry: acp::PlanEntry) -> TodoItem {
    let status = match entry.status {
        acp::PlanEntryStatus::Pending => TodoStatus::Pending,
        acp::PlanEntryStatus::InProgress => TodoStatus::InProgress,
        acp::PlanEntryStatus::Completed => {
            if entry
                .meta
                .as_ref()
                .and_then(|m| m.get("cancelled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                TodoStatus::Cancelled
            } else {
                TodoStatus::Completed
            }
        }
        // TODO(acp-0.10): `PlanEntryStatus` is #[non_exhaustive].
        _ => TodoStatus::Pending,
    };
    let meta = entry.meta.map(serde_json::Value::Object);
    // Recover first-class size from meta when ACP Plan carried it.
    let size = meta
        .as_ref()
        .and_then(|m| m.get("size"))
        .and_then(|v| v.as_u64())
        .and_then(|n| u8::try_from(n).ok())
        .filter(|n| *n == 1 || *n == 2);
    TodoItem {
        content: entry.content,
        priority: match entry.priority {
            acp::PlanEntryPriority::High => TodoPriority::High,
            acp::PlanEntryPriority::Medium => TodoPriority::Medium,
            acp::PlanEntryPriority::Low => TodoPriority::Low,
            // TODO(acp-0.10): `PlanEntryPriority` is #[non_exhaustive].
            _ => TodoPriority::Medium,
        },
        status,
        meta,
        size,
    }
}

/// Cancelled items become `Completed` with `{"cancelled": true}` in meta.
pub(crate) fn plan_entry_from_todo_item(item: TodoItem) -> acp::PlanEntry {
    let status = match item.status {
        TodoStatus::Pending => acp::PlanEntryStatus::Pending,
        TodoStatus::InProgress => acp::PlanEntryStatus::InProgress,
        TodoStatus::Completed => acp::PlanEntryStatus::Completed,
        TodoStatus::Cancelled => acp::PlanEntryStatus::Completed,
    };
    let mut meta = item.meta;
    // Stamp board id for parentId leaf detection on the client.
    if let Some(id) = id {
        let mut m = meta.unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = m.as_object_mut() {
            obj.insert("id".into(), serde_json::json!(id));
        }
        meta = Some(m);
    }
    // Preserve size across ACP Plan (no first-class size on PlanEntry).
    if let Some(sz) = item.size {
        let mut m = meta.unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = m.as_object_mut() {
            obj.insert("size".into(), serde_json::json!(sz));
        }
        meta = Some(m);
    }
    if item.status == TodoStatus::Cancelled {
        let mut m = meta.unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = m.as_object_mut() {
            obj.insert("cancelled".into(), true.into());
        }
        meta = Some(m);
    }
    acp::PlanEntry::new(
        item.content,
        match item.priority {
            TodoPriority::High => acp::PlanEntryPriority::High,
            TodoPriority::Medium => acp::PlanEntryPriority::Medium,
            TodoPriority::Low => acp::PlanEntryPriority::Low,
        },
        status,
    )
    .meta(meta.and_then(|v| v.as_object().cloned()))
}
