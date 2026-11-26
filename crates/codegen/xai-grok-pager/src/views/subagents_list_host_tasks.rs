//! Subagents list rows from the session's live host tasks.
//!
//! A header that says newer ids exist is not a row. Each running host task
//! is one row: id, description, and elapsed time. A completed task is not a
//! running row. The session-context sentence that says agents are running
//! uses this same set.

use std::collections::HashSet;
use std::time::Duration;

/// One host task the session already has. `completed` wins over a stale
/// `running` bit so a finished id does not stay on the running list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTask {
    pub id: String,
    pub description: String,
    pub elapsed: Duration,
    pub running: bool,
    pub completed: bool,
}

/// One painted Subagents row. `label` shows the id, the description, and the
/// elapsed time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentsListRow {
    pub id: String,
    pub description: String,
    pub elapsed: Duration,
    pub label: String,
}

/// Running host tasks. A completed id is absent even when `running` is still set.
pub fn running_host_tasks(tasks: &[HostTask]) -> Vec<&HostTask> {
    let mut seen = HashSet::<&str>::new();
    let mut out = Vec::new();
    for task in tasks {
        if task.completed || !task.running {
            continue;
        }
        if task.id.is_empty() {
            continue;
        }
        if seen.insert(task.id.as_str()) {
            out.push(task);
        }
    }
    out
}

fn row_from_task(task: &HostTask) -> SubagentsListRow {
    let elapsed_text = crate::util::format_duration(task.elapsed);
    let label = format!("{} {} {}", task.id, task.description, elapsed_text);
    SubagentsListRow {
        id: task.id.clone(),
        description: task.description.clone(),
        elapsed: task.elapsed,
        label,
    }
}

/// One row per live host task. Completed ids are not rows.
pub fn subagents_list_rows(tasks: &[HostTask]) -> Vec<SubagentsListRow> {
    running_host_tasks(tasks)
        .into_iter()
        .map(row_from_task)
        .collect()
}

/// Session-context sentence for the same running set the list paints.
/// Empty when nothing is running. A completed id is not named.
pub fn session_context_agents_running_sentence(tasks: &[HostTask]) -> String {
    let rows = subagents_list_rows(tasks);
    if rows.is_empty() {
        return String::new();
    }
    let body = rows
        .iter()
        .map(|row| {
            let elapsed = crate::util::format_duration(row.elapsed);
            format!("{} ({}, {})", row.id, row.description, elapsed)
        })
        .collect::<Vec<_>>()
        .join("; ");
    if rows.len() == 1 {
        format!("1 agent is running: {body}.")
    } else {
        format!("{} agents are running: {body}.", rows.len())
    }
}

/// Fixture only. These values describe two live host tasks and one completed
/// id. Constructing them does not start or resume a host job.
#[cfg(test)]
pub fn grub_fontforge_live_host_tasks_fixture() -> Vec<HostTask> {
    vec![
        HostTask {
            id: "01a2165b-8168-7629-88d8-2c9f585c0eb6".to_string(),
            description: "Coordinator for the grub fontforge job".to_string(),
            elapsed: Duration::from_secs(14 * 60 + 51),
            running: true,
            completed: false,
        },
        HostTask {
            id: "01a21664-fa98-750b-b5c5-03519147d2b7".to_string(),
            description: "Implementer reading the grub hook".to_string(),
            elapsed: Duration::from_secs(4 * 60 + 41),
            running: true,
            completed: false,
        },
        HostTask {
            id: "completed-grub-not-a-running-row".to_string(),
            description: "Completed grub fontforge pass".to_string(),
            elapsed: Duration::from_secs(30 * 60),
            running: true,
            completed: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_NOT_A_ROW: &str = "Two newer IDs exist beyond the L2";

    #[test]
    fn live_host_task_produces_a_subagents_row() {
        let tasks = grub_fontforge_live_host_tasks_fixture();
        let rows = subagents_list_rows(&tasks);
        assert!(
            !rows.is_empty(),
            "a live host task produces no Subagents row"
        );
        assert_eq!(rows.len(), 2, "one row per live host task, got {rows:?}");
        let coordinator = &rows[0];
        assert_eq!(coordinator.id, "01a2165b-8168-7629-88d8-2c9f585c0eb6");
        assert!(coordinator.description.contains("fontforge"));
        assert_eq!(coordinator.elapsed, Duration::from_secs(14 * 60 + 51));
        assert!(coordinator.label.contains(&coordinator.id));
        assert!(coordinator.label.contains("fontforge"));
        assert!(coordinator.label.contains("14m51s"));
        let implementer = &rows[1];
        assert_eq!(implementer.id, "01a21664-fa98-750b-b5c5-03519147d2b7");
        assert!(implementer.description.contains("grub hook"));
        assert_eq!(implementer.elapsed, Duration::from_secs(4 * 60 + 41));
        assert!(implementer.label.contains(&implementer.id));
        assert!(implementer.label.contains("grub hook"));
        assert!(implementer.label.contains("4m41s"));
        assert!(
            rows.iter()
                .all(|row| row.id != "completed-grub-not-a-running-row")
        );
        assert!(
            rows.iter().all(|row| row.label != HEADER_NOT_A_ROW),
            "a header is not a Subagents row"
        );
        let sentence = session_context_agents_running_sentence(&tasks);
        assert_eq!(
            sentence,
            session_context_agents_running_sentence(
                &running_host_tasks(&tasks)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            )
        );
        assert!(sentence.starts_with("2 agents are running:"));
        assert!(sentence.contains("01a2165b-8168-7629-88d8-2c9f585c0eb6"));
        assert!(sentence.contains("01a21664-fa98-750b-b5c5-03519147d2b7"));
        assert!(sentence.contains("14m51s"));
        assert!(sentence.contains("4m41s"));
        assert!(!sentence.contains("completed-grub-not-a-running-row"));
        assert_ne!(sentence, HEADER_NOT_A_ROW);
    }
}
