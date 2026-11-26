//! Session feed for the Subagents list.
//!
//! [`AgentView::subagent_sessions`] is the live host task list this session
//! already has. Refresh copies that list into [`TasksPane::set_host_tasks`]
//! and then syncs the pane. A completed id is not a running row.

use std::collections::HashMap;

use super::AgentView;
use crate::app::subagent::SubagentInfo;
use crate::views::subagents_list_host_tasks::HostTask;

impl AgentView {
    /// Copy this session's live host tasks into the Subagents list, then sync.
    ///
    /// Does not start or resume a host job. A finished id, or a stale running
    /// bit with a terminal host status, is stored completed so it is not a
    /// running row.
    pub(crate) fn refresh_subagents_list_from_live_host_tasks(&mut self) {
        self.tasks
            .set_host_tasks(live_host_tasks_from_subagent_sessions(
                &self.subagent_sessions,
            ));
        self.tasks.sync(
            &self.session.bg_tasks,
            &self.subagent_sessions,
            &self.session.scheduled_tasks,
            &self.workflow_runs,
        );
    }
}

fn live_host_tasks_from_subagent_sessions(
    sessions: &HashMap<String, SubagentInfo>,
) -> Vec<HostTask> {
    let mut infos: Vec<&SubagentInfo> = sessions
        .values()
        .filter(|info| info.attempt.workflow_run_id.is_none())
        .filter(|info| !info.child_session_id.is_empty())
        .collect();
    infos.sort_by(|a, b| {
        a.attempt
            .started_at
            .cmp(&b.attempt.started_at)
            .then_with(|| a.child_session_id.cmp(&b.child_session_id))
    });
    infos.into_iter().map(host_task_from_subagent).collect()
}

fn host_task_from_subagent(info: &SubagentInfo) -> HostTask {
    let running = info.is_running() && !info.has_terminal_host_status();
    HostTask {
        id: info.child_session_id.to_string(),
        description: info.description.to_string(),
        elapsed: info.display_elapsed(),
        running,
        completed: !running,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::super::test_agent_view;
    use crate::app::subagent::{SubagentAttemptInfo, SubagentInfo, SubagentLifecycleState};

    const COORDINATOR_ID: &str = "01a2165b-8168-7629-88d8-2c9f585c0eb6";
    const IMPLEMENTER_ID: &str = "01a21664-fa98-750b-b5c5-03519147d2b7";
    const COMPLETED_ID: &str = "completed-grub-not-a-running-row";
    const HEADER_NOT_A_ROW: &str = "Two newer IDs exist beyond the L2";

    fn elapsed_shown(text: &str, secs: u64) -> bool {
        let primary = crate::util::format_duration(Duration::from_secs(secs));
        let next = crate::util::format_duration(Duration::from_secs(secs.saturating_add(1)));
        text.contains(&primary) || text.contains(&next)
    }

    /// `depth` and `parent_session_id` are the values spawn writes.
    /// An L1 coordinator is depth 1. The implementer it stores is depth 2.
    fn live_host_subagent(
        id: &str,
        description: &str,
        elapsed: Duration,
        depth: u32,
        parent_session_id: Option<&str>,
    ) -> SubagentInfo {
        let started = Instant::now()
            .checked_sub(elapsed)
            .unwrap_or_else(Instant::now);
        SubagentInfo {
            subagent_id: Arc::from(id),
            child_session_id: Arc::from(id),
            description: Arc::from(description),
            subagent_type: Arc::from("general-purpose"),
            finished: false,
            status: None,
            from_occupancy_restore: false,
            attempt: SubagentAttemptInfo {
                lifecycle: SubagentLifecycleState::running_legacy_for_test(),
                persona: None,
                role: None,
                model: None,
                context_source: None,
                resumed_from: None,
                capability_mode: None,
                workflow_run_id: None,
                context_normalized: false,
                parent_prompt_id: None,
                parent_session_id: parent_session_id.map(std::sync::Arc::from),
                depth: Some(depth),
                tokens_past: 0,
                started_at: started,
                last_progress_at: started,
                status: None,
                error: None,
                duration_ms: None,
                tool_calls: None,
                turns: None,
                turn_count: None,
                tool_call_count: None,
                tokens_used: None,
                context_window_tokens: None,
                context_usage_pct: None,
                tools_used: Vec::new(),
                error_count: None,
                activity_label: None,
                is_background: false,
                pending_kill: false,
                kill_requested_at: None,
                scrollback_entry_id: None,
                terminal_entry_id: None,
            },
            completed_attempt_tokens: 0,
            sealed_attempt_tokens: Default::default(),
            prompt: None,
            child_cwd: None,
            worktree_path: None,
            transcript: Default::default(),
        }
    }

    fn completed_host_subagent(id: &str, description: &str) -> SubagentInfo {
        let mut info = live_host_subagent(id, description, Duration::from_secs(30 * 60), 2, None);
        info.finished = true;
        info.status = Some(Arc::from("completed"));
        info.attempt.status = Some(Arc::from("completed"));
        info.attempt.duration_ms = Some(30 * 60 * 1000);
        info
    }

    #[test]
    fn session_refreshes_subagents_list_from_live_host_tasks_and_a_running_task_appears_as_a_row() {
        let mut agent = test_agent_view(Some("l1-session"), std::path::PathBuf::from("/tmp"));
        agent.subagent_sessions.insert(
            COORDINATOR_ID.to_string(),
            live_host_subagent(
                COORDINATOR_ID,
                "Coordinator for the grub fontforge job",
                Duration::from_secs(14 * 60 + 51),
                1,
                Some("l1-session"),
            ),
        );
        agent.subagent_sessions.insert(
            IMPLEMENTER_ID.to_string(),
            live_host_subagent(
                IMPLEMENTER_ID,
                "Implementer reading the grub hook",
                Duration::from_secs(4 * 60 + 41),
                2,
                Some(COORDINATOR_ID),
            ),
        );
        assert_eq!(
            agent.subagent_sessions[COORDINATOR_ID].attempt.depth,
            Some(1),
            "spawn writes depth 1 for the L1 coordinator"
        );
        assert_eq!(
            agent.subagent_sessions[IMPLEMENTER_ID].attempt.depth,
            Some(2),
            "spawn writes depth 2 for the implementer"
        );
        agent.subagent_sessions.insert(
            COMPLETED_ID.to_string(),
            completed_host_subagent(COMPLETED_ID, "Completed grub fontforge pass"),
        );

        agent.refresh_subagents_list_from_live_host_tasks();

        let ids = agent.tasks.listed_agent_session_ids();
        let dock_rows = agent.dock_subagent_rows();
        let dock_ids: Vec<String> = dock_rows
            .iter()
            .map(|(session_id, _, _)| session_id.clone())
            .collect();
        assert!(
            ids.iter().any(|id| id == COORDINATOR_ID),
            "a live host task produces no Subagents row: {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id == IMPLEMENTER_ID),
            "a live host task produces no Subagents row: {ids:?}"
        );
        assert!(
            !ids.iter().any(|id| id == COMPLETED_ID),
            "a completed id is not a running row: {ids:?}"
        );
        assert!(
            dock_ids.iter().any(|id| id == COORDINATOR_ID),
            "dock hides the coordinator: {dock_ids:?}"
        );
        assert!(
            dock_ids.iter().any(|id| id == IMPLEMENTER_ID),
            "a depth-1 coordinator must not hide the depth-2 implementer: {dock_ids:?}"
        );
        assert!(
            !dock_ids.iter().any(|id| id == COMPLETED_ID),
            "a completed id is not a running dock row: {dock_ids:?}"
        );
        let implementer_dock = dock_rows
            .iter()
            .find(|(session_id, _, _)| session_id == IMPLEMENTER_ID)
            .expect("implementer dock row");
        assert!(
            implementer_dock.2.description.contains(IMPLEMENTER_ID),
            "dock line must show the implementer id: {}",
            implementer_dock.2.description
        );
        let sentence = agent.tasks.session_context_agents_running();
        assert!(
            sentence.contains(COORDINATOR_ID) && sentence.contains("fontforge"),
            "coordinator row needs id and description: {sentence}"
        );
        assert!(
            elapsed_shown(&sentence, 14 * 60 + 51),
            "coordinator row needs elapsed time: {sentence}"
        );
        assert!(
            sentence.contains(IMPLEMENTER_ID) && sentence.contains("grub hook"),
            "implementer row needs id and description: {sentence}"
        );
        assert!(
            elapsed_shown(&sentence, 4 * 60 + 41),
            "implementer row needs elapsed time: {sentence}"
        );
        assert!(
            !sentence.contains(COMPLETED_ID),
            "a completed id is not a running row: {sentence}"
        );
        assert_ne!(sentence, HEADER_NOT_A_ROW);
        assert!(sentence.starts_with("2 agents are running:"));
    }
}
