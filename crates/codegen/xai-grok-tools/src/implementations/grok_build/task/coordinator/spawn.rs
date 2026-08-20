//! Spawn admission: reparenting, duplicate checks, and the admit decision.

use tokio::sync::oneshot;

use super::super::admission::{
    AdmissionDecision, AdmissionError, ImplementLoopReviewAdmit,
    admit_implement_loop_review_description, is_implement_loop_review_description,
};
use super::super::coordinator_state::PendingChild;
use super::super::types::{SubagentOwner, SubagentRequest, SubagentResult, SubagentSpawnRequest};
use super::queue::{QueuedCaller, QueuedSpawn, StartOrigin};
use super::{
    ChildRunOutput, ChildRunner, LimitedSpawnOrigin, SubagentCoordinator, SubagentLimitDecision,
    SubagentLimitNotice,
};

impl<R: ChildRunner> SubagentCoordinator<R> {
    pub(super) fn handle_spawn(&mut self, command: SubagentSpawnRequest) {
        let SubagentSpawnRequest {
            mut request,
            result_tx,
        } = command;
        if let Err(rejection) = self.reparent_nested_spawn(&mut request) {
            self.reject_queries_waiting_for_spawn(&request.id);
            let _ = result_tx.send(rejection);
            return;
        }
        // Late Task spawn after user Stop (detached TaskTool background).
        if !request.owner.is_workflow()
            && self
                .spawn_blocked_sessions
                .contains(&request.parent_session_id)
        {
            self.reject_queries_waiting_for_spawn(&request.id);
            let _ = result_tx.send(rejected_spawn_result(
                &request.id,
                "parent session is stopped",
                true,
            ));
            return;
        }
        let id = request.id.clone();
        // Attach waits that arrived in the fire-and-forget window before
        // this Spawn was processed. Visibility is the live child if this
        // id already exists, not a later duplicate request.
        self.attach_queries_waiting_for_spawn(&id, &request);
        if self.pending.contains_key(&id)
            || self.active.contains_key(&id)
            || self.completed.contains_key(&id)
            || self.queued.contains_id(&id)
        {
            let _ = result_tx.send(rejected_spawn_result(
                &id,
                &format!("Subagent id '{id}' already exists"),
                false,
            ));
            return;
        }
        if let Some(existing_id) = self.live_same_description(&request) {
            self.reject_queries_waiting_for_spawn(&request.id);
            let _ = result_tx.send(rejected_spawn_result(
                &id,
                &format!(
                    "A live subagent with the same description already exists ('{existing_id}')"
                ),
                false,
            ));
            return;
        }
        // Token Economy implement-loop effort is thoroughness. Distinct
        // Review descriptions still count as extra Review rows. Live
        // `/implement --effort` is on the request. No operator-ask bit:
        // one live Review description at any setting.
        let implement_loop_effort = request.implement_loop_effort_or_default();
        if !request.owner.is_workflow()
            && admit_implement_loop_review_description(
                implement_loop_effort,
                false,
                self.live_review_descriptions(&request),
                &request.description,
            ) == ImplementLoopReviewAdmit::Reject
        {
            self.reject_queries_waiting_for_spawn(&request.id);
            let _ = result_tx.send(rejected_spawn_result(
                &id,
                &format!(
                    "Implement-loop effort {implement_loop_effort} admits one Review description unless the operator asked for more"
                ),
                false,
            ));
            return;
        }
        let running = self.session_running_count(&request.parent_session_id);
        match self.admission.admit(&request, running) {
            AdmissionDecision::Start => {
                self.start_child(*request, Some(result_tx), StartOrigin::Direct)
            }
            AdmissionDecision::Enqueue => {
                debug_assert!(
                    !request.owner.is_workflow(),
                    "workflow spawns bypass admission and must never queue"
                );
                tracing::info!(
                    subagent_id = %request.id,
                    parent_session_id = %request.parent_session_id,
                    running,
                    "subagent queued at the concurrent limit"
                );
                self.notify_limit(
                    &request,
                    SubagentLimitDecision::QueuedAtConcurrentLimit {
                        limit: self.admission.max_concurrent(),
                    },
                );
                let deadline = request
                    .awaits_in_foreground()
                    .then(|| tokio::time::Instant::now() + self.config.foreground_budget);
                self.queued.push_back(QueuedSpawn {
                    request,
                    queued_at: tokio::time::Instant::now(),
                    caller: QueuedCaller::Awaiting {
                        result_tx,
                        deadline,
                    },
                });
            }
            AdmissionDecision::Reject(error) => {
                self.notify_limit(
                    &request,
                    match &error {
                        AdmissionError::ConcurrentLimitReached { limit } => {
                            SubagentLimitDecision::RejectedAtConcurrentLimit { limit: *limit }
                        }
                    },
                );
                let result = SubagentResult {
                    success: false,
                    error: Some(error.message()),
                    subagent_id: id.clone(),
                    child_session_id: id,
                    ..Default::default()
                };
                self.finish_never_started(
                    *request,
                    Some(result_tx),
                    result,
                    std::time::Instant::now(),
                );
            }
        }
    }

    /// Live Task-owned Review-row descriptions on this parent.
    fn live_review_descriptions(&self, request: &SubagentRequest) -> Vec<String> {
        if request.owner.is_workflow() {
            return Vec::new();
        }
        let parent = &request.parent_session_id;
        let collect = |other: &SubagentRequest| {
            !other.owner.is_workflow()
                && other.parent_session_id == *parent
                && other.id != request.id
                && is_implement_loop_review_description(&other.description)
        };
        let mut out = Vec::new();
        for child in self.pending.values() {
            if collect(&child.request) {
                out.push(child.request.description.clone());
            }
        }
        for child in self.active.values() {
            if collect(&child.request) {
                out.push(child.request.description.clone());
            }
        }
        for queued in self.queued.iter() {
            if collect(&queued.request) {
                out.push(queued.request.description.clone());
            }
        }
        out
    }

    /// Live Task-owned child with the same trimmed description on this parent.
    fn live_same_description(&self, request: &SubagentRequest) -> Option<String> {
        if request.owner.is_workflow() {
            return None;
        }
        let desc = request.description.trim();
        if desc.is_empty() {
            return None;
        }
        let matches = |other: &SubagentRequest| {
            !other.owner.is_workflow()
                && other.parent_session_id == request.parent_session_id
                && other.id != request.id
                && other.description.trim() == desc
        };
        self.pending
            .values()
            .find(|child| matches(&child.request))
            .map(|child| child.request.id.clone())
            .or_else(|| {
                self.active
                    .values()
                    .find(|child| matches(&child.request))
                    .map(|child| child.request.id.clone())
            })
            .or_else(|| {
                self.queued
                    .iter()
                    .find(|queued| matches(&queued.request))
                    .map(|queued| queued.request.id.clone())
            })
    }

    /// Re-key a nested spawn (its parent is itself a subagent) to the root
    /// session, inheriting workflow lineage and loop identity; rejects the
    /// spawn when its parent subagent is already being torn down.
    fn reparent_nested_spawn(
        &mut self,
        request: &mut SubagentRequest,
    ) -> Result<(), SubagentResult> {
        let Some((root_parent, loop_task_id, spawner_cancelled, spawner_owner, l2_depth)) = self
            .active
            .values()
            .find(|child| child.child_session_id == request.parent_session_id)
            .map(|child| {
                (
                    child.request.parent_session_id.clone(),
                    child.request.runtime_overrides.loop_task_id.clone(),
                    child.cancellation.is_cancelled(),
                    child.request.owner.clone(),
                    child.request.runtime_overrides.spawn_depth.unwrap_or(1),
                )
            })
        else {
            return Ok(());
        };
        if spawner_cancelled {
            // The parent subagent is being torn down, so its late child
            // would be orphaned against the closed scope.
            return Err(rejected_spawn_result(
                &request.id,
                "parent subagent is being torn down",
                true,
            ));
        }
        self.spawned_by_session
            .insert(request.id.clone(), request.parent_session_id.clone());
        request.runtime_overrides.immediate_parent_session_id =
            Some(request.parent_session_id.clone());
        if request.runtime_overrides.spawn_depth.is_none() {
            request.runtime_overrides.spawn_depth = Some(l2_depth.saturating_add(1));
        }
        request.parent_session_id = root_parent;
        request.surface_completion = false;
        // Nested children keep workflow lineage after reparent so
        // ParentSession Stop does not kill in-flight workflow work.
        if !request.owner.is_workflow()
            && let Some(run_id) = spawner_owner.workflow_run_id()
        {
            request.owner = SubagentOwner::workflow(run_id);
        }
        if request.runtime_overrides.loop_task_id.is_none() {
            request.runtime_overrides.loop_task_id = loop_task_id;
        }
        Ok(())
    }

    /// Counts are computed here, not at call sites: a queued spawn counts
    /// itself in `queue_depth` (the notice fires before the push), a rejected
    /// spawn does not.
    fn notify_limit(&self, request: &SubagentRequest, decision: SubagentLimitDecision) {
        let Some(sink) = &self.config.limit_sink else {
            return;
        };
        let queued = self.session_queued_count(&request.parent_session_id);
        sink(SubagentLimitNotice {
            parent_session_id: request.parent_session_id.clone(),
            decision,
            running: self.session_running_count(&request.parent_session_id),
            queue_depth: match decision {
                SubagentLimitDecision::QueuedAtConcurrentLimit { .. } => queued + 1,
                SubagentLimitDecision::RejectedAtConcurrentLimit { .. } => queued,
            },
            origin: if request.from_scheduler_loop() {
                LimitedSpawnOrigin::SchedulerLoop
            } else {
                LimitedSpawnOrigin::Task
            },
        });
    }

    /// Route a spawn that never reached the runner through `finish_child`,
    /// so waiters resolve and the id stays queryable; `since` anchors the
    /// record's duration.
    pub(super) fn finish_never_started(
        &mut self,
        request: SubagentRequest,
        spawn_reply: Option<oneshot::Sender<SubagentResult>>,
        result: SubagentResult,
        since: std::time::Instant,
    ) {
        let id = request.id.clone();
        self.pending.insert(
            id.clone(),
            PendingChild {
                started_at: since,
                cancellation: request.cancel_token.clone(),
                spawn_reply,
                foreground_deadline: None,
                handle_only: request.run_in_background,
                explicitly_killed: false,
                request,
            },
        );
        self.finish_child(
            &id,
            ChildRunOutput {
                result,
                completion_data: R::CompletionData::default(),
                snapshot_ref: None,
            },
        );
    }
}

/// A spawn refused before it ever became a child record.
fn rejected_spawn_result(id: &str, error: &str, cancelled: bool) -> SubagentResult {
    SubagentResult {
        success: false,
        cancelled,
        error: Some(error.to_owned()),
        subagent_id: id.to_owned(),
        child_session_id: id.to_owned(),
        ..Default::default()
    }
}
