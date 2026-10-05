//! Spawn admission: reparenting, duplicate checks, and the admit decision.

use tokio::sync::oneshot;

use super::super::admission::{
    AdmissionDecision, AdmissionError, ImplementLoopReviewAdmit,
    admit_implement_loop_review_description, is_implement_loop_review_description,
};
use super::super::coordinator_state::PendingChild;
use super::super::types::{SubagentOwner, SubagentRequest, SubagentResult, SubagentSpawnRequest};
use super::graph::NestedSpawner;
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
            mut registered_tx,
            admitted_tx,
        } = command;
        // Registration is a side channel: a background caller still gets its terminal result on
        // `result_tx`. The scheduler actor needs the signal to tell "admitted" from a pre-start
        // reject before it deletes a one-shot. `admitted_tx` is the same moment for
        // `spawn_registered`, including a pre-start reject.
        let start_ack = if request.run_in_background {
            BackgroundStartAck::OnRegister
        } else {
            BackgroundStartAck::Hold
        };
        if start_ack == BackgroundStartAck::Hold {
            registered_tx = None;
        }
        let spawner = match self.reparent_nested_spawn(&mut request) {
            Ok(spawner) => spawner,
            Err(rejection) => {
                self.reject_queries_waiting_for_spawn(&request.id);
                reply_admitted_err(admitted_tx, &rejection);
                let _ = result_tx.send(rejection);
                return;
            }
        };
        // Late Task spawn after user Stop (detached TaskTool background).
        if !request.owner.is_workflow()
            && self
                .spawn_blocked_sessions
                .contains(&request.parent_session_id)
        {
            self.reject_queries_waiting_for_spawn(&request.id);
            reply_rejected(
                admitted_tx,
                result_tx,
                rejected_spawn_result(&request.id, "parent session is stopped", true),
            );
            return;
        }
        let id = request.id.clone();
        if !request.owner.is_workflow()
            && request.description != "goal achievement skeptic"
            && self.live_same_description(&request)
        {
            self.reject_queries_waiting_for_spawn(&id);
            reply_rejected(
                admitted_tx,
                result_tx,
                rejected_spawn_result(
                    &id,
                    &format!(
                        "A live subagent already has description '{}'. Wait for it to finish, or use a distinct description.",
                        request.description
                    ),
                    false,
                ),
            );
            return;
        }
        if self.pending.contains_key(&id)
            || self.active.contains_key(&id)
            || self.completed.contains_key(&id)
            || self.queued.contains_id(&id)
        {
            // The live child, not this duplicate request, owns parked waits.
            self.attach_queries_waiting_for_spawn(&id, &request);
            reply_rejected(
                admitted_tx,
                result_tx,
                rejected_spawn_result(&id, &format!("Subagent id '{id}' already exists"), false),
            );
            return;
        }
        // `spawn()` waits on the child result. Reject a live source here so
        // that wait is not the source's exit (or the shared finish signal).
        if let Some(resume_id) = request
            .resume_from
            .as_deref()
            .filter(|id| xai_tool_types::is_not_sentinel(id))
            && self.resume_target_is_still_running(resume_id, &request.parent_session_id)
        {
            self.reject_queries_waiting_for_spawn(&id);
            reply_rejected(
                admitted_tx,
                result_tx,
                rejected_spawn_result(
                    &id,
                    &format!(
                        "Cannot resume from subagent '{resume_id}': it is still running. \
                         Wait for it to complete before resuming."
                    ),
                    false,
                ),
            );
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
            reply_rejected(
                admitted_tx,
                result_tx,
                rejected_spawn_result(
                    &id,
                    &format!(
                        "Implement-loop effort {implement_loop_effort} admits one Review description unless the operator asked for more"
                    ),
                    false,
                ),
            );
            return;
        }
        // Capture before `insert_nested` moves `spawner`.
        let spawner_session_id = spawner.as_ref().map(|nested| nested.session_id.clone());
        // The node must exist before any record that can be looked up by `id`.
        match spawner {
            Some(spawner) => {
                // Unreachable while reparent only names spawners in `active`.
                if self
                    .graph
                    .insert_nested(&id, &request.parent_session_id, spawner)
                    .is_err()
                {
                    self.reject_queries_waiting_for_spawn(&id);
                    reply_rejected(
                        admitted_tx,
                        result_tx,
                        rejected_spawn_result(
                            &id,
                            "parent subagent lineage is unknown; refusing to spawn",
                            true,
                        ),
                    );
                    return;
                }
            }
            None => self
                .graph
                .insert_root_child(&id, &request.parent_session_id),
        }
        self.attach_queries_waiting_for_spawn(&id, &request);
        self.inherit_resume_subagent_type(request.as_mut());
        let running = self.session_running_count(&request.parent_session_id);
        match self.admission.admit(&request, running) {
            AdmissionDecision::Start => {
                self.start_child(
                    *request,
                    Some(result_tx),
                    registered_tx,
                    StartOrigin::Direct,
                    None,
                    spawner_session_id,
                    None,
                    None,
                );
                reply_admitted(admitted_tx);
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
                // Keep `result_tx` so cancel/completion of a queued background
                // child still resolves `spawn()`. Registration is a side channel.
                let deadline = request
                    .awaits_in_foreground()
                    .then(|| tokio::time::Instant::now() + self.config.foreground_budget);
                let agent_address = xai_message_delivery_core::mint_child_address(
                    request.owner.is_workflow(),
                    uuid::Uuid::new_v4().as_u128(),
                );
                self.queued.push_back(QueuedSpawn {
                    request,
                    queued_at: tokio::time::Instant::now(),
                    caller: QueuedCaller::Awaiting {
                        result_tx,
                        deadline,
                    },
                    agent_address,
                    spawner_session_id,
                    wake_origin: None,
                    wake: None,
                });
                // `Hold` already cleared `registered_tx`; fire iff the policy
                // left a signal.
                if let Some(tx) = registered_tx {
                    let _ = tx.send(());
                }
                reply_admitted(admitted_tx);
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
                let result = SubagentResult::failed(id.clone(), id, error.message());
                reply_admitted_err(admitted_tx, &result);
                self.finish_never_started(
                    *request,
                    Some(result_tx),
                    result,
                    std::time::Instant::now(),
                );
            }
        }
    }

    /// Another live task-owned child on this parent already has this description.
    /// The reserved panel description is exempt at the call site.
    fn live_same_description(&self, request: &SubagentRequest) -> bool {
        let matches = |other: &SubagentRequest| {
            !other.owner.is_workflow()
                && other.id != request.id
                && other.parent_session_id == request.parent_session_id
                && other.description == request.description
        };
        self.pending.values().any(|child| matches(&child.request))
            || self.active.values().any(|child| matches(&child.request))
            || self.queued.iter().any(|queued| matches(&queued.request))
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

    /// Re-key a nested spawn (its parent is itself a subagent) to the root
    /// session; the spawn graph keeps the ancestors' authority over it.
    fn reparent_nested_spawn(
        &self,
        request: &mut SubagentRequest,
    ) -> Result<Option<NestedSpawner>, SubagentResult> {
        let Some(spawner) = self.active_child_for_session(&request.parent_session_id) else {
            return Ok(None);
        };
        if spawner.cancellation.is_cancelled() {
            // The parent subagent is being torn down, so its late child
            // would be orphaned against the closed scope.
            return Err(rejected_spawn_result(
                &request.id,
                "parent subagent is being torn down",
                true,
            ));
        }
        let root_parent = spawner.request.parent_session_id.clone();
        let spawner_session_id = std::mem::replace(&mut request.parent_session_id, root_parent);
        // The request's flag becomes root-scoped; the spawner's wish lives on the graph node.
        let surface_completion = std::mem::replace(&mut request.surface_completion, false);
        // Nested children keep workflow lineage after reparent so
        // ParentSession Stop does not kill in-flight workflow work.
        if !request.owner.is_workflow()
            && let Some(run_id) = spawner.request.owner.workflow_run_id()
        {
            request.owner = SubagentOwner::workflow(run_id);
        }
        if request.runtime_overrides.loop_task_id.is_none() {
            request.runtime_overrides.loop_task_id =
                spawner.request.runtime_overrides.loop_task_id.clone();
        }
        Ok(Some(NestedSpawner {
            child_id: spawner.request.id.clone(),
            session_id: spawner_session_id,
            surface_completion,
        }))
    }

    /// Counts are computed here, not at call sites: a queued spawn counts
    /// itself in `queue_depth` (the notice fires before the push), a rejected
    /// spawn does not.
    pub(super) fn notify_limit(&self, request: &SubagentRequest, decision: SubagentLimitDecision) {
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
        mut request: SubagentRequest,
        spawn_reply: Option<oneshot::Sender<SubagentResult>>,
        result: SubagentResult,
        since: std::time::Instant,
    ) {
        let id = request.id.clone();
        drop(request.spawn_root.take_span());
        self.pending.insert(
            id.clone(),
            PendingChild {
                started_at: since,
                cancellation: request.cancel_token.clone(),
                spawn_reply,
                foreground_deadline: None,
                handle_only: request.run_in_background,
                explicitly_killed: false,
                disposition: Default::default(),
                launched: false,
                attempt_id: xai_message_delivery_core::AttemptId::mint(
                    uuid::Uuid::new_v4().as_u128(),
                ),
                generation: super::ActiveChildGeneration::new(),
                agent_address: None,
                spawner_session_id: None,
                wake_of: None,
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

    /// In-memory resume source wins over the caller's default type before admission.
    fn inherit_resume_subagent_type(&self, request: &mut SubagentRequest) {
        let Some(resume_id) = request
            .resume_from
            .as_deref()
            .filter(|id| xai_tool_types::is_not_sentinel(id))
        else {
            return;
        };
        if let Some(source) = self.completed.get(resume_id) {
            if source.request.parent_session_id == request.parent_session_id {
                request.subagent_type = source.request.subagent_type.clone();
            }
            return;
        }
        if let Some(subagent_type) = self
            .runner
            .durable_resume_type(resume_id, &request.parent_session_id)
        {
            request.subagent_type = subagent_type;
        }
    }
}

/// Who should see a registration signal versus the terminal `result_tx` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BackgroundStartAck {
    /// Fire `registered_tx` once the child is pending or queued.
    OnRegister,
    /// Leave `result_tx` for completion or a foreground-budget handoff.
    Hold,
}

fn reply_admitted(admitted_tx: Option<oneshot::Sender<Result<(), String>>>) {
    if let Some(tx) = admitted_tx {
        let _ = tx.send(Ok(()));
    }
}

fn reply_admitted_err(
    admitted_tx: Option<oneshot::Sender<Result<(), String>>>,
    result: &SubagentResult,
) {
    if let Some(tx) = admitted_tx {
        let message = result
            .error
            .clone()
            .unwrap_or_else(|| "spawn rejected".to_owned());
        let _ = tx.send(Err(message));
    }
}

fn reply_rejected(
    admitted_tx: Option<oneshot::Sender<Result<(), String>>>,
    result_tx: oneshot::Sender<SubagentResult>,
    result: SubagentResult,
) {
    reply_admitted_err(admitted_tx, &result);
    let _ = result_tx.send(result);
}

/// A spawn refused before it ever became a child record.
fn rejected_spawn_result(id: &str, error: &str, cancelled: bool) -> SubagentResult {
    if cancelled {
        SubagentResult::cancelled(id, id, error)
    } else {
        SubagentResult::failed(id, id, error)
    }
}
