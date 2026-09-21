//! Soft spawn `write_paths` assignment reminder.
//!
//! After each tool call, nested agents see which live siblings were
//! assigned paths via `write_paths`. That assignment is not a hard
//! exclusive lock. The hard lock is only the in-flight edit-tool call.

use crate::implementations::editor_infra::per_path_write_lock::format_soft_assignment_reminder;
use crate::types::output::ToolOutput;
use crate::types::resources::{OwnerSessionId, SharedResources};
use crate::types::tool::Reminder;

/// Cross-cutting reminder: "L2 X is assigned these paths."
pub struct WritePathsAssignmentReminder;

#[async_trait::async_trait]
impl Reminder for WritePathsAssignmentReminder {
    async fn collect_reminders(
        &self,
        resources: SharedResources,
        _tool_output: &ToolOutput,
    ) -> Vec<String> {
        let except = {
            let res = resources.lock().await;
            res.get::<OwnerSessionId>().map(|owner| owner.0.clone())
        };
        format_soft_assignment_reminder(except.as_deref())
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::editor_infra::per_path_write_lock::{
        release_holder, try_reserve_writes,
    };
    use crate::types::resources::Resources;

    #[tokio::test]
    async fn soft_lock_reminder_is_observable_on_a_sibling_tool_call() {
        // Operator: layer/L2 write_paths claims must be a soft lock. Other
        // agents get an automated reminder that a sibling is working on that
        // path.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("plan.md");
        std::fs::write(&path, "x\n").unwrap();
        let first = format!("l2-{}", path.display());
        try_reserve_writes([&path], &first);

        let mut resources = Resources::new();
        resources.insert(OwnerSessionId("sibling-writer".to_string()));
        let reminders = WritePathsAssignmentReminder
            .collect_reminders(resources.into_shared(), &ToolOutput::Text("ok".into()))
            .await;
        let joined = reminders.join("\n");
        assert!(
            joined.contains(&format!("L2 {first} is assigned these paths")),
            "reminder must name the assigned sibling: {joined}"
        );
        assert!(
            joined.contains("plan.md"),
            "reminder must name the file: {joined}"
        );
        release_holder(&first);
    }
}
