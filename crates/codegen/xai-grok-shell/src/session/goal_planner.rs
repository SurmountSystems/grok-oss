//! Mirrors [`crate::session::goal_classifier`] but is fail-CLOSED: any failure pauses the goal.
//! At goal creation the runner spawns four roles in order: research, recon, plan writer, then plan reviewer.
//! The plan writer writes a structured plan to the [`GoalTracker::plan_path`](crate::session::goal_tracker::GoalTracker::plan_path) file.
//! Research, recon, and review files sit next to that plan path. The spawn is hidden behind [`GoalPlannerSpawner`] so tests can inject a deterministic spawner.

#![allow(dead_code)]

use crate::session::events::{Event, GoalPlannerFailClosedReason, GoalRoleModelFailOpenReason};
use crate::session::goal_role_tools::RoleToolNames;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use xai_grok_session_events::EventWriter;
use xai_grok_tools::implementations::grok_build::task::backend::{ChannelBackend, SubagentBackend};
use xai_grok_tools::implementations::grok_build::task::types::{
    SubagentOwner, SubagentRequest, SubagentRuntimeOverrides,
};

// Shared per-role model override and spawn-and-retry-once fail-open wrapper

/// The role's configured `agent_type` selects only the harness, threaded as [`SubagentRuntimeOverrides::harness_agent_type`].
/// The subagent_type stays fixed so the role keeps a capable toolset on whichever harness is chosen.
/// The three role spawners and the parent-side `describe_subagent_type` probe all read it, so the gated/probed toolset matches the spawned one.
pub(crate) const GOAL_ROLE_SUBAGENT_TYPE: &str = "general-purpose";
pub(crate) const GOAL_ROLE_AWAIT_BUDGET_EXCEEDED: &str =
    "goal role subagent exceeded foreground wait budget";

/// The planner is aborting regardless, so this is a bound on cleanup, not a correctness gate.
const GOAL_PLANNER_CANCEL_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// `None`/`None` inherits the current model and the session harness (the historic `SubagentRuntimeOverrides::default()` behavior).
/// When either field is `Some` the pair is "explicit".
/// The spawn-and-retry-once wrapper ([`spawn_with_fail_open_retry`]) retries on the current model and session harness if the first attempt fails.
#[derive(Debug, Clone, Default)]
pub(crate) struct RoleSpawnOverride {
    /// Resolved, post-auth, post-fail-open model id, or `None` to inherit.
    pub model: Option<String>,
    /// Resolved harness `agent_type`, applied REGARDLESS of the parent agent.
    /// Its `AgentDefinition` decides the spawned subagent's harness flavor (system prompt and toolset).
    /// NOT a subagent type: the subagent_type stays fixed at [`GOAL_ROLE_SUBAGENT_TYPE`].
    pub agent_type: Option<String>,
}

impl RoleSpawnOverride {
    /// `true` when a configured `{model, agent_type}` pair was committed (so a spawn failure triggers the one current-model retry).
    pub(crate) fn is_explicit(&self) -> bool {
        self.model.is_some() || self.agent_type.is_some()
    }
}

/// The model id a `/goal` role actually runs on: the committed override's model when present, else the inherited parent.
/// Reported as `GoalPlannerFired.model_id`/`GoalStrategistFired.model_id` so a committed remote override isn't under-reported as the parent model.
/// Not used for the classifier: its skeptic pool has no single representative model.
pub(crate) fn effective_role_model_id<'a>(
    override_model: Option<&'a str>,
    parent: &'a str,
) -> &'a str {
    override_model.unwrap_or(parent)
}

/// The explicit-pair retry re-runs on the default/parent toolset.
/// The caller renders both up front and [`spawn_with_fail_open_retry`] picks the matching one per attempt.
/// On the inherit path no retry occurs and `fallback` is never read, so callers on that path may leave it empty.
pub(crate) struct RoleRenderedPrompt {
    /// Rendered for the role's RESOLVED toolset (the first/only attempt).
    pub primary: String,
    /// Rendered for the DEFAULT/parent toolset the explicit-pair fail-open retry falls back to.
    pub fallback: String,
}

/// Implemented by both `goal_planner::SpawnError` and `goal_classifier::SpawnError` (structurally identical).
/// That lets the generic wrapper exclude non-retryable errors without coupling to one concrete error type.
pub(crate) trait RetryableSpawnError {
    /// `true` when the explicit-pair fail-open retry may re-run the spawn on the current model.
    /// A cancellation (user / turn abort) must propagate as-is so the planner pauses as `Aborted` instead of silently re-running.
    fn is_retryable(&self) -> bool;
}

impl RetryableSpawnError for SpawnError {
    fn is_retryable(&self) -> bool {
        match self {
            SpawnError::Transport(_) => true,
            SpawnError::Runtime { cancelled, .. } => !cancelled,
            SpawnError::Interrupted => false,
        }
    }
}

/// If that returns a retryable `Err`, the wrapper emits `GoalRoleModelFailOpen { reason: spawn_failed }`.
/// A cancellation propagates as-is (no retry), so a bad configured pair can never change the failure behavior.
/// The second arg is the harness override (`None` inherits the session harness), NOT a subagent type.
pub(crate) async fn spawn_with_fail_open_retry<E, F, Fut>(
    role: &'static str,
    skeptic_idx: Option<u32>,
    override_: &RoleSpawnOverride,
    events: Option<&EventWriter>,
    prompt: RoleRenderedPrompt,
    mut spawn: F,
) -> Result<String, E>
where
    E: RetryableSpawnError,
    F: FnMut(Option<String>, Option<String>, String) -> Fut,
    Fut: std::future::Future<Output = Result<String, E>>,
{
    // Inherit path: single attempt on the current model and session harness; move the prompt
    if !override_.is_explicit() {
        return spawn(None, None, prompt.primary).await;
    }
    let first = spawn(
        override_.model.clone(),
        override_.agent_type.clone(),
        prompt.primary,
    )
    .await;
    let should_retry = match &first {
        Ok(_) => false,
        Err(e) => e.is_retryable(),
    };
    if !should_retry {
        return first;
    }
    if let Some(ev) = events {
        ev.emit(Event::GoalRoleModelFailOpen {
            role,
            skeptic_idx,
            reason: GoalRoleModelFailOpenReason::SpawnFailed.as_const_str(),
        });
    }
    // Retry on the session harness, using the matching `fallback` render so the prompt names the toolset the retry actually runs on
    spawn(None, None, prompt.fallback).await
}

// Constants

/// Telemetry value on `GoalPlannerFired`.
/// It does not cap user-initiated `/goal resume` retries, which re-run the planner unbounded.
pub(crate) const GOAL_PLANNER_MAX_RUNS: u32 = 1;

/// Research, recon, the plan writer, and the plan reviewer share this subagent type so each keeps a capable toolset.
const GOAL_PLANNER_SUBAGENT_TYPE: &str = GOAL_ROLE_SUBAGENT_TYPE;

const GOAL_PLANNER_SUBAGENT_DESCRIPTION: &str = "goal plan writer";

const GOAL_RESEARCH_PROMPT_TEMPLATE: &str = include_str!("templates/goal_research_prompt.md");
const GOAL_RECON_PROMPT_TEMPLATE: &str = include_str!("templates/goal_recon_prompt.md");
const GOAL_PLANNER_PROMPT_TEMPLATE: &str = include_str!("templates/goal_planner_prompt.md");
const GOAL_PLAN_REVIEW_PROMPT_TEMPLATE: &str = include_str!("templates/goal_plan_review_prompt.md");

/// One goal-creation role. The runner requests each role as its own spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoalCreationRole {
    Research,
    Recon,
    PlanWriter,
    Reviewer,
}

impl GoalCreationRole {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Research => "goal research specialist",
            Self::Recon => "goal recon specialist",
            Self::PlanWriter => GOAL_PLANNER_SUBAGENT_DESCRIPTION,
            Self::Reviewer => "goal plan reviewer",
        }
    }

    fn fail_open_label(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Recon => "recon",
            Self::PlanWriter => "planner",
            Self::Reviewer => "plan_reviewer",
        }
    }
}

/// Sibling of `plan.md` or `plan-<id>.md`: `research.md` / `research-<id>.md`, and the same for recon and review.
pub(crate) fn goal_creation_artifact(plan_file: &Path, role_stem: &str) -> PathBuf {
    let file_name = plan_file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "plan.md".to_string());
    let renamed = match file_name.strip_prefix("plan") {
        Some(rest) => format!("{role_stem}{rest}"),
        None => format!("{role_stem}.md"),
    };
    plan_file.with_file_name(renamed)
}

// Outcome and spawner abstraction

/// `Planned` carries the path the planner wrote (always the input `plan_file`).
/// `FailClosed` carries the reason; every variant pauses the goal at the call site.
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "`latency_ms` is consumed by future telemetry")]
pub(crate) enum GoalPlannerOutcome {
    Planned {
        plan_file: PathBuf,
        latency_ms: u64,
    },
    /// Produced only by the planner's [`ChannelSpawner`], whose `cancel_token` is wired to user steering / Send Now through `start_planner_run`.
    /// When that token fires mid-spawn the attempt returns `Interrupted` so the loop replans.
    /// The strategist and summarizer spawners pass a fresh, never-cancelled token, so their equivalent cancel arms are unreachable in practice.
    Interrupted,
    FailClosed {
        reason: GoalPlannerFailClosedReason,
        latency_ms: u64,
    },
}

/// Production uses [`ChannelSpawner`]; tests use `MockSpawner` (defined in the tests module).
#[async_trait::async_trait]
pub(crate) trait GoalPlannerSpawner: Send + Sync {
    /// Spawn one goal-creation role under `id` and return the terminal response when it finishes.
    async fn spawn_planner(
        &self,
        id: &str,
        role: GoalCreationRole,
        prompt: RoleRenderedPrompt,
    ) -> Result<String, SpawnError>;
}

/// Deliberately mirrored from the verifier rather than shared.
#[derive(Debug)]
pub(crate) enum SpawnError {
    Transport(String),
    Runtime { message: String, cancelled: bool },
    Interrupted,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(d) => write!(f, "subagent transport error: {d}"),
            Self::Runtime { message, cancelled } => {
                write!(
                    f,
                    "subagent runtime error (cancelled={cancelled}): {message}"
                )
            }
            Self::Interrupted => f.write_str("planner interrupted by user steering"),
        }
    }
}

// Terminal-token parse

/// A malformed token isn't fatal on its own; the runner gates on the plan file's presence.
pub(crate) fn parse_terminal_response(text: &str) -> bool {
    text.trim() == "Done"
}

// Production spawner

pub(crate) struct ChannelSpawner {
    pub(crate) event_tx: tokio::sync::mpsc::UnboundedSender<
        xai_grok_tools::implementations::grok_build::task::types::SubagentEvent,
    >,
    pub(crate) foreground_wait:
        Option<xai_grok_tools::implementations::grok_build::task::types::SubagentForegroundWait>,
    pub(crate) parent_session_id: String,
    pub(crate) parent_prompt_id: Option<String>,
    pub(crate) cwd: Option<String>,
    /// Trace-artifact sink and resolved `task` tool name; `None` disables recording.
    /// See [`super::goal_classifier::record_subagent_trace`].
    pub(crate) trace_sink: Option<(xai_chat_state::ChatStateHandle, String)>,
    /// Resolved per-role model and toolset override.
    /// Default (inherit) keeps the historic `::default()` spawn behavior.
    pub(crate) role_override: RoleSpawnOverride,
    pub(crate) cancel_token: tokio_util::sync::CancellationToken,
    /// Event sink for the spawn-and-retry-once fail-open telemetry; `None` in tests / when no event log is wired.
    pub(crate) events: Option<EventWriter>,
}

#[async_trait::async_trait]
impl GoalPlannerSpawner for ChannelSpawner {
    async fn spawn_planner(
        &self,
        id: &str,
        role: GoalCreationRole,
        prompt: RoleRenderedPrompt,
    ) -> Result<String, SpawnError> {
        let description = role.description();
        // Clone the primary render for the trace pair only when tracing; the wrapper moves each render into its attempt (no other clone)
        let trace_prompt = self.trace_sink.as_ref().map(|_| prompt.primary.clone());
        let outcome = spawn_with_fail_open_retry(
            role.fail_open_label(),
            None,
            &self.role_override,
            self.events.as_ref(),
            prompt,
            |model, harness, prompt| self.send_one(id, description, prompt, model, harness),
        )
        .await;

        // Trace the FINAL attempt's output / runtime error (transport errors carry no subagent output)
        match &outcome {
            Ok(text) => crate::session::goal_classifier::record_subagent_trace(
                self.trace_sink.as_ref(),
                id,
                GOAL_PLANNER_SUBAGENT_TYPE,
                description,
                trace_prompt.as_deref(),
                text,
            ),
            Err(SpawnError::Runtime { message, .. }) => {
                crate::session::goal_classifier::record_subagent_trace(
                    self.trace_sink.as_ref(),
                    id,
                    GOAL_PLANNER_SUBAGENT_TYPE,
                    description,
                    trace_prompt.as_deref(),
                    message,
                )
            }
            Err(SpawnError::Transport(_) | SpawnError::Interrupted) => {}
        }
        outcome
    }
}

impl ChannelSpawner {
    /// Send one spawn (model and harness override resolved by the caller) and await its terminal result.
    /// The fail-open wrapper calls this once or twice (retry on the current model and session harness).
    async fn send_one(
        &self,
        id: &str,
        description: &str,
        prompt: String,
        model: Option<String>,
        harness_agent_type: Option<String>,
    ) -> Result<String, SpawnError> {
        let request = SubagentRequest {
            id: id.to_string(),
            prompt,
            description: description.to_string(),
            subagent_type: GOAL_PLANNER_SUBAGENT_TYPE.to_string(),
            parent_session_id: self.parent_session_id.clone(),
            parent_prompt_id: self.parent_prompt_id.clone(),
            resume_from: None,
            cwd: self.cwd.clone(),
            runtime_overrides: SubagentRuntimeOverrides {
                model,
                harness_agent_type,
                once_run: true,
                ..Default::default()
            },
            run_in_background: false,
            // Harness-internal: never surface to the model's idle reminder.
            surface_completion: false,
            // Goal roles are never auto-backgrounded: the planner runs until it finishes or the user interrupts.
            await_to_completion: true,
            fork_context: true,
            owner: SubagentOwner::Task,
            implement_loop_effort: None,
            cancel_token: self.cancel_token.clone(),
            spawn_root: Default::default(),
            tool_call_id: None,
        };
        let backend = ChannelBackend::new(self.event_tx.clone());
        let cancel = self.cancel_token.clone();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                let _ = tokio::time::timeout(
                    GOAL_PLANNER_CANCEL_ACK_TIMEOUT,
                    backend.cancel(id),
                )
                .await;
                return Err(SpawnError::Interrupted);
            }
            result = backend.spawn_with_foreground_wait(request, self.foreground_wait.as_ref()) => {
                result.map_err(|error| SpawnError::Transport(error.to_string()))?
            }
        };
        // Unreachable with `await_to_completion`; `cancelled: true` keeps the fail-open retry from spawning beside the orphan.
        if result.backgrounded {
            return Err(SpawnError::Runtime {
                message: "engine bug: goal role subagent was auto-backgrounded despite await_to_completion".into(),
                cancelled: true,
            });
        }
        if !result.success {
            let message = result.error.unwrap_or_else(|| "unknown error".to_string());
            return Err(SpawnError::Runtime {
                message,
                cancelled: result.cancelled,
            });
        }
        Ok(result.output.to_string())
    }
}

// Runner

pub(crate) struct GoalPlannerInputs<'a> {
    pub objective: &'a str,
    pub context: &'a str,
    pub plan_file: &'a Path,
    pub attempt: u32,
    pub model_id: &'a str,
    /// Resolved tool names for the planner role's prompt placeholders (`{READ_TOOL}`/`{SEARCH_TOOL}`/`{LIST_TOOL}`/`{WRITE_TOOL}`).
    /// Built parent-side from the planner's resolved toolset.
    pub tool_names: &'a RoleToolNames,
    /// Default/parent-toolset tool names used to render the fail-open RETRY prompt.
    /// On the inherit path this equals `tool_names`.
    pub inherit_tool_names: &'a RoleToolNames,
}

/// Fail-CLOSED: any failure path returns `FailClosed { reason }` and the caller pauses the goal.
pub(crate) async fn run_goal_planner(
    spawner: Arc<dyn GoalPlannerSpawner>,
    inputs: GoalPlannerInputs<'_>,
    emit_event: &dyn Fn(Event),
) -> GoalPlannerOutcome {
    let started = std::time::Instant::now();
    emit_event(Event::GoalPlannerFired {
        attempt: inputs.attempt,
        max_runs: GOAL_PLANNER_MAX_RUNS,
        model_id: inputs.model_id.to_string(),
    });

    if let Some(parent) = inputs.plan_file.parent()
        && let Err(err) = tokio::fs::create_dir_all(parent).await
    {
        tracing::warn!(
            parent = %parent.display(),
            error = %err,
            "goal planner: failed to pre-create plan-file parent dir; failing closed",
        );
        return record_fail_closed(
            GoalPlannerFailClosedReason::FileWriteFailed,
            inputs.attempt,
            started,
            emit_event,
        );
    }

    let plan_file_str = inputs.plan_file.to_string_lossy();
    let research_file = goal_creation_artifact(inputs.plan_file, "research");
    let recon_file = goal_creation_artifact(inputs.plan_file, "recon");
    let review_file = goal_creation_artifact(inputs.plan_file, "review");
    let research_file_str = research_file.to_string_lossy();
    let recon_file_str = recon_file.to_string_lossy();
    let review_file_str = review_file.to_string_lossy();

    // Research and recon do not read each other's files. Separate spawns, separate prompts.
    // Research is requested first so a cancel retries this attempt before the later roles.
    let research_prompt = goal_creation_prompt(
        GOAL_RESEARCH_PROMPT_TEMPLATE,
        &[("{RESEARCH_FILE}", research_file_str.as_ref())],
        inputs.tool_names,
        inputs.inherit_tool_names,
        inputs.objective,
        inputs.context,
    );
    let research_result =
        spawn_goal_creation_role(&spawner, GoalCreationRole::Research, research_prompt).await;
    if let Some(stop) = outcome_if_spawn_stopped(
        research_result,
        inputs.plan_file,
        plan_file_str.as_ref(),
        inputs.attempt,
        started,
        emit_event,
    )
    .await
    {
        return stop;
    }
    if !non_empty_plan_file(&research_file).await {
        tracing::info!(
            research_file = %research_file_str,
            "goal planner: research file missing or empty; failing closed",
        );
        return record_fail_closed(
            GoalPlannerFailClosedReason::MissingPlan,
            inputs.attempt,
            started,
            emit_event,
        );
    }

    let recon_prompt = goal_creation_prompt(
        GOAL_RECON_PROMPT_TEMPLATE,
        &[("{RECON_FILE}", recon_file_str.as_ref())],
        inputs.tool_names,
        inputs.inherit_tool_names,
        inputs.objective,
        inputs.context,
    );
    let recon_result =
        spawn_goal_creation_role(&spawner, GoalCreationRole::Recon, recon_prompt).await;
    if let Some(stop) = outcome_if_spawn_stopped(
        recon_result,
        inputs.plan_file,
        plan_file_str.as_ref(),
        inputs.attempt,
        started,
        emit_event,
    )
    .await
    {
        return stop;
    }
    if !non_empty_plan_file(&recon_file).await {
        tracing::info!(
            recon_file = %recon_file_str,
            "goal planner: recon file missing or empty; failing closed",
        );
        return record_fail_closed(
            GoalPlannerFailClosedReason::MissingPlan,
            inputs.attempt,
            started,
            emit_event,
        );
    }

    // Both artifact paths exist. The plan writer reads those two paths and writes only the plan.
    let plan_prompt = goal_creation_prompt(
        GOAL_PLANNER_PROMPT_TEMPLATE,
        &[
            ("{PLAN_FILE}", plan_file_str.as_ref()),
            ("{RESEARCH_FILE}", research_file_str.as_ref()),
            ("{RECON_FILE}", recon_file_str.as_ref()),
        ],
        inputs.tool_names,
        inputs.inherit_tool_names,
        inputs.objective,
        inputs.context,
    );
    let plan_result =
        spawn_goal_creation_role(&spawner, GoalCreationRole::PlanWriter, plan_prompt).await;
    if !non_empty_plan_file(inputs.plan_file).await {
        if let Some(stop) = outcome_if_spawn_stopped(
            plan_result,
            inputs.plan_file,
            plan_file_str.as_ref(),
            inputs.attempt,
            started,
            emit_event,
        )
        .await
        {
            return stop;
        }
        tracing::info!(
            plan_file = %plan_file_str,
            "goal planner: plan file missing or empty; failing closed",
        );
        return record_fail_closed(
            GoalPlannerFailClosedReason::MissingPlan,
            inputs.attempt,
            started,
            emit_event,
        );
    }

    // The plan file and the research file exist. The reviewer writes a review and does not rewrite the plan.
    let review_prompt = goal_creation_prompt(
        GOAL_PLAN_REVIEW_PROMPT_TEMPLATE,
        &[
            ("{REVIEW_FILE}", review_file_str.as_ref()),
            ("{PLAN_FILE}", plan_file_str.as_ref()),
            ("{RESEARCH_FILE}", research_file_str.as_ref()),
            ("{RECON_FILE}", recon_file_str.as_ref()),
        ],
        inputs.tool_names,
        inputs.inherit_tool_names,
        inputs.objective,
        inputs.context,
    );
    let review_result =
        spawn_goal_creation_role(&spawner, GoalCreationRole::Reviewer, review_prompt).await;
    if non_empty_plan_file(inputs.plan_file).await {
        // A reviewer stop must not discard a plan that is already on disk.
        let _ = review_result;
        return planned_outcome(inputs.plan_file, inputs.attempt, started, emit_event);
    }
    if let Some(stop) = outcome_if_spawn_stopped(
        review_result,
        inputs.plan_file,
        plan_file_str.as_ref(),
        inputs.attempt,
        started,
        emit_event,
    )
    .await
    {
        return stop;
    }
    tracing::info!(
        plan_file = %plan_file_str,
        "goal planner: plan file missing or empty after review; failing closed",
    );
    record_fail_closed(
        GoalPlannerFailClosedReason::MissingPlan,
        inputs.attempt,
        started,
        emit_event,
    )
}

/// Render one role prompt. Path tokens are real paths, not file bodies.
fn goal_creation_prompt(
    template: &str,
    path_replacements: &[(&str, &str)],
    tool_names: &RoleToolNames,
    inherit_tool_names: &RoleToolNames,
    objective: &str,
    context: &str,
) -> RoleRenderedPrompt {
    let render = |names: &RoleToolNames| {
        let mut with_paths = template.to_string();
        for (token, value) in path_replacements {
            with_paths = with_paths.replace(token, value);
        }
        let rendered = names.apply(&with_paths);
        let mut full = String::with_capacity(rendered.len() + objective.len() + context.len() + 64);
        full.push_str(&rendered);
        full.push_str("\n\nOBJECTIVE:\n");
        full.push_str(objective);
        full.push_str("\n\nCONTEXT:\n");
        full.push_str(context);
        full.push('\n');
        full
    };
    RoleRenderedPrompt {
        primary: render(tool_names),
        fallback: render(inherit_tool_names),
    }
}

async fn spawn_goal_creation_role(
    spawner: &Arc<dyn GoalPlannerSpawner>,
    role: GoalCreationRole,
    prompt: RoleRenderedPrompt,
) -> Result<String, SpawnError> {
    let spawn_id = uuid::Uuid::now_v7().to_string();
    spawner.spawn_planner(&spawn_id, role, prompt).await
}

/// `None` means the spawn returned `Ok` and the caller should check that role's file.
/// A non-empty plan file on a stop is Planned, same as a single writer that died after the write.
async fn outcome_if_spawn_stopped(
    result: Result<String, SpawnError>,
    plan_file: &Path,
    plan_file_str: &str,
    attempt: u32,
    started: std::time::Instant,
    emit_event: &dyn Fn(Event),
) -> Option<GoalPlannerOutcome> {
    let err = match result {
        Ok(_) => return None,
        Err(err) => err,
    };
    match err {
        SpawnError::Transport(detail) => {
            // Nested writer can drop the channel after `{PLAN_FILE}` is already
            // on disk. A written plan is Planned, not "No plan was produced."
            if non_empty_plan_file(plan_file).await {
                tracing::info!(
                    plan_file = %plan_file_str,
                    error = %detail,
                    "goal planner: transport stop after the plan file was written; accepting it",
                );
                return Some(planned_outcome(plan_file, attempt, started, emit_event));
            }
            if spawn_error_is_repeating_sentence_stop(&detail) {
                tracing::info!(
                    error = %detail,
                    "goal planner: repeating-sentence transport with no plan; treating as interrupt so the harness can retry",
                );
                return Some(GoalPlannerOutcome::Interrupted);
            }
            tracing::warn!(error = %detail, "goal planner: transport error; failing closed");
            Some(record_fail_closed(
                GoalPlannerFailClosedReason::Transport,
                attempt,
                started,
                emit_event,
            ))
        }
        SpawnError::Interrupted => {
            // A finished writer shares this token with `finish_child`, which cancels it
            // when the writer stops sampling. The select can report Interrupted after
            // `{PLAN_FILE}` is already on disk. A non-empty file counts; a missing file
            // is a real interrupt and the caller may replan.
            Some(
                accept_written_plan_or_else(
                    plan_file,
                    attempt,
                    started,
                    emit_event,
                    GoalPlannerOutcome::Interrupted,
                )
                .await,
            )
        }
        SpawnError::Runtime { message, cancelled } => {
            // The child's turn error (repeating-sentence stop, sampling-window
            // death) arrives here as Runtime. `finish_child` can also cancel the
            // shared token after a successful write so the spawn result wins
            // this arm instead of Interrupted. A non-empty `{PLAN_FILE}` is the
            // plan; do not fail-closed and delete it.
            if non_empty_plan_file(plan_file).await {
                tracing::info!(
                    plan_file = %plan_file_str,
                    cancelled,
                    "goal planner: runtime stop after the plan file was written; accepting it",
                );
                return Some(planned_outcome(plan_file, attempt, started, emit_event));
            }
            if !cancelled && spawn_error_is_repeating_sentence_stop(&message) {
                tracing::info!(
                    error = %message,
                    "goal planner: repeating-sentence stop with no plan; treating as interrupt so the harness can retry",
                );
                return Some(GoalPlannerOutcome::Interrupted);
            }
            let reason = if cancelled {
                GoalPlannerFailClosedReason::Aborted
            } else {
                GoalPlannerFailClosedReason::Runtime
            };
            tracing::warn!(
                error = %message,
                cancelled,
                "goal planner: subagent runtime error; failing closed",
            );
            Some(record_fail_closed(reason, attempt, started, emit_event))
        }
    }
}

/// A plan counts only when `{PLAN_FILE}` is a non-empty file. An empty stub does not.
async fn non_empty_plan_file(path: &Path) -> bool {
    match tokio::fs::metadata(path).await {
        Ok(meta) => meta.is_file() && meta.len() > 0,
        Err(_) => false,
    }
}

/// Nested once-run writer turn errors are `Session error: {detail}`. Match the
/// sampler line and the pager chrome headline (`Stopped: repeating sentence`).
fn spawn_error_is_repeating_sentence_stop(message: &str) -> bool {
    let msg = message.to_ascii_lowercase();
    let sampler = xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE
        .trim_end_matches('.')
        .to_ascii_lowercase();
    msg.contains(sampler.as_str()) || msg.contains("repeating sentence")
}

/// `{PLAN_FILE}` already on disk is a finished writer. Otherwise keep `otherwise`.
async fn accept_written_plan_or_else(
    plan_file: &Path,
    attempt: u32,
    started: std::time::Instant,
    emit_event: &dyn Fn(Event),
    otherwise: GoalPlannerOutcome,
) -> GoalPlannerOutcome {
    if non_empty_plan_file(plan_file).await {
        tracing::info!(
            plan_file = %plan_file.display(),
            "goal planner: spawn stopped after the plan file was written; accepting it",
        );
        planned_outcome(plan_file, attempt, started, emit_event)
    } else {
        otherwise
    }
}

fn planned_outcome(
    plan_file: &Path,
    attempt: u32,
    started: std::time::Instant,
    emit_event: &dyn Fn(Event),
) -> GoalPlannerOutcome {
    let latency_ms = started.elapsed().as_millis() as u64;
    emit_event(Event::GoalPlannerCompleted {
        attempt,
        latency_ms,
    });
    GoalPlannerOutcome::Planned {
        plan_file: plan_file.to_path_buf(),
        latency_ms,
    }
}

/// Headline from a planner markdown file (`# Plan: <work>`).
/// `None` when the heading has no headline (`# Plan`) or is not that contract.
pub(crate) fn planned_goal_title(markdown: &str) -> Option<&str> {
    let line = markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let rest = line.strip_prefix('#')?.trim();
    let title = rest.strip_prefix("Plan:")?.trim();
    (!title.is_empty()).then_some(title)
}

/// Read [`planned_goal_title`] from a PLAN_FILE path. `None` if the file is
/// missing, unreadable, or the heading has no headline.
pub(crate) fn planned_goal_title_from_path(path: &Path) -> Option<String> {
    let markdown = std::fs::read_to_string(path).ok()?;
    planned_goal_title(&markdown).map(str::to_owned)
}

fn record_fail_closed(
    reason: GoalPlannerFailClosedReason,
    attempt: u32,
    started: std::time::Instant,
    emit_event: &dyn Fn(Event),
) -> GoalPlannerOutcome {
    let latency_ms = started.elapsed().as_millis() as u64;
    emit_event(Event::GoalPlannerFailClosed {
        reason: reason.as_const_str(),
        attempt,
        latency_ms,
    });
    GoalPlannerOutcome::FailClosed { reason, latency_ms }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::goal_role_tools::tests::{assert_no_tool_placeholders, summary_with};
    use std::sync::{Arc, Mutex};
    use xai_grok_tools::types::tool::ToolKind;

    #[test]
    fn planner_template_default_render_has_no_placeholders() {
        let rendered = RoleToolNames::inherit_defaults().apply(GOAL_PLANNER_PROMPT_TEMPLATE);
        assert_no_tool_placeholders(&rendered);
    }

    #[test]
    fn planner_template_explicit_render_has_no_leftover_placeholders() {
        for summary in [
            summary_with(&[
                (ToolKind::Read, "cursor_read"),
                (ToolKind::ListDir, "cursor_ls"),
                (ToolKind::Search, "cursor_grep"),
                (ToolKind::Write, "cursor_write"),
            ]),
            summary_with(&[
                (ToolKind::Read, "read_file"),
                (ToolKind::ListDir, "list_dir"),
                (ToolKind::Search, "grep"),
                (ToolKind::Write, "write"),
            ]),
        ] {
            let rendered =
                RoleToolNames::from_summary(&summary).apply(GOAL_PLANNER_PROMPT_TEMPLATE);
            assert_no_tool_placeholders(&rendered);
        }
    }

    #[test]
    fn research_template_default_render_names_the_web_tools() {
        // The research mandate is inert if the research specialist cannot see the tool.
        // The plan writer must not carry that instruction.
        let rendered = RoleToolNames::inherit_defaults().apply(GOAL_RESEARCH_PROMPT_TEMPLATE);
        assert!(
            rendered.contains("web_search"),
            "default render must name the web-search tool",
        );
        assert!(rendered.contains("web_fetch"));
        assert_no_tool_placeholders(&rendered);
        let plan = RoleToolNames::inherit_defaults().apply(GOAL_PLANNER_PROMPT_TEMPLATE);
        assert!(
            !plan.contains("web_search"),
            "plan writer must not name the web-search tool",
        );
        assert!(
            !plan.contains("research it with"),
            "plan writer must not contain the web-research instruction",
        );
        assert!(
            !plan.contains("Inspect files"),
            "plan writer must not contain the file-inspection instruction",
        );
    }

    #[test]
    fn research_template_cursor_render_names_the_web_search_tool() {
        // The previously-broken case: on the alternate toolset the web tool is named "WebSearch", so the research prompt must render THAT.
        // Otherwise the weak model never reaches for it and researches from memory.
        let rendered = RoleToolNames::from_summary(&summary_with(&[
            (ToolKind::WebSearch, "WebSearch"),
            (ToolKind::WebFetch, "WebFetch"),
        ]))
        .apply(GOAL_RESEARCH_PROMPT_TEMPLATE);
        assert!(
            rendered.contains("WebSearch"),
            "cursor render must name the cursor web-search tool",
        );
        assert!(rendered.contains("WebFetch"));
        assert_no_tool_placeholders(&rendered);
        let plan = RoleToolNames::from_summary(&summary_with(&[
            (ToolKind::WebSearch, "WebSearch"),
            (ToolKind::WebFetch, "WebFetch"),
        ]))
        .apply(GOAL_PLANNER_PROMPT_TEMPLATE);
        assert!(
            !plan.contains("WebSearch"),
            "plan writer must not name the cursor web-search tool",
        );
        assert!(!plan.contains("research it with"));
        assert!(!plan.contains("Inspect files"));
    }

    #[tokio::test]
    async fn channel_spawner_request_is_harness_internal() {
        use xai_grok_tools::implementations::grok_build::task::types::{
            SubagentEvent, SubagentResult,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let wait_depth = Arc::new(crate::tools::tool_context::BlockingWaitState::new());
        let spawner = ChannelSpawner {
            event_tx: tx,
            foreground_wait: Some(crate::tools::tool_context::subagent_foreground_wait(
                Arc::clone(&wait_depth),
            )),
            parent_session_id: "parent".into(),
            parent_prompt_id: None,
            cwd: None,
            trace_sink: None,
            role_override: RoleSpawnOverride::default(),
            cancel_token: tokio_util::sync::CancellationToken::new(),
            events: None,
        };
        let handle = tokio::spawn(async move {
            let _ = spawner
                .spawn_planner(
                    "plan-id",
                    GoalCreationRole::PlanWriter,
                    role_prompt("prompt"),
                )
                .await;
        });

        let SubagentEvent::Spawn(request) = rx.recv().await.expect("spawn event") else {
            panic!("expected Spawn");
        };
        assert_eq!(wait_depth.depth(), 1);
        assert!(
            !request.surface_completion,
            "planner subagent must not surface to the idle reminder"
        );
        assert!(
            request.await_to_completion,
            "planner subagent must never be auto-backgrounded"
        );
        let _ = request.result_tx.send(SubagentResult::default());
        handle.await.unwrap();
        assert_eq!(wait_depth.depth(), 0);
    }

    #[test]
    fn parse_terminal_response_accepts_done_exact() {
        assert!(parse_terminal_response("Done"));
        assert!(parse_terminal_response("  Done \n"));
        assert!(!parse_terminal_response("done"));
        assert!(!parse_terminal_response("DONE"));
        assert!(!parse_terminal_response("Done."));
        assert!(!parse_terminal_response("Done!"));
        assert!(!parse_terminal_response(""));
    }

    #[test]
    fn planned_goal_title_reads_the_plan_headline() {
        let markdown = "# Plan: Rewrite tests onto production GoalPlan arms\n\n## Goal kind\n";
        assert_eq!(
            planned_goal_title(markdown),
            Some("Rewrite tests onto production GoalPlan arms"),
            "Operator: the goal got lost. PLAN_FILE `# Plan: <work>` is the current planned goal"
        );
        assert_eq!(planned_goal_title("# Plan\n\n1. do it\n"), None);
        assert_eq!(planned_goal_title(""), None);
    }

    /// One recorded goal-creation spawn. File flags are sampled before this spawn writes.
    #[derive(Clone)]
    struct RoleSpawnSeen {
        id: String,
        role: GoalCreationRole,
        prompt: String,
        research_nonempty: bool,
        recon_nonempty: bool,
        plan_nonempty: bool,
    }

    /// Deterministic spawner with knobs for response text, whether to write the plan file, and the file body.
    struct MockSpawner {
        response: Result<String, SpawnError>,
        write_plan: bool,
        plan_body: Vec<u8>,
        plan_file_target: std::path::PathBuf,
        last_prompt: Mutex<Option<String>>,
        seen: Mutex<Vec<RoleSpawnSeen>>,
    }

    impl MockSpawner {
        fn ok_writes(plan_file: &Path, body: &[u8]) -> Self {
            Self {
                response: Ok("Done".to_string()),
                write_plan: true,
                plan_body: body.to_vec(),
                plan_file_target: plan_file.to_path_buf(),
                last_prompt: Mutex::new(None),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn ok_does_not_write(plan_file: &Path) -> Self {
            Self {
                response: Ok("Done".to_string()),
                write_plan: false,
                plan_body: Vec::new(),
                plan_file_target: plan_file.to_path_buf(),
                last_prompt: Mutex::new(None),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn fails(err: SpawnError, plan_file: &Path) -> Self {
            Self {
                response: Err(err),
                write_plan: false,
                plan_body: Vec::new(),
                plan_file_target: plan_file.to_path_buf(),
                last_prompt: Mutex::new(None),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    fn nonempty_file(path: &std::path::Path) -> bool {
        std::fs::metadata(path)
            .map(|meta| meta.is_file() && meta.len() > 0)
            .unwrap_or(false)
    }

    #[async_trait::async_trait]
    impl GoalPlannerSpawner for MockSpawner {
        async fn spawn_planner(
            &self,
            id: &str,
            role: GoalCreationRole,
            prompt: RoleRenderedPrompt,
        ) -> Result<String, SpawnError> {
            let research = goal_creation_artifact(&self.plan_file_target, "research");
            let recon = goal_creation_artifact(&self.plan_file_target, "recon");
            self.seen.lock().unwrap().push(RoleSpawnSeen {
                id: id.to_string(),
                role,
                prompt: prompt.primary.clone(),
                research_nonempty: nonempty_file(&research),
                recon_nonempty: nonempty_file(&recon),
                plan_nonempty: nonempty_file(&self.plan_file_target),
            });
            *self.last_prompt.lock().unwrap() = Some(prompt.primary);
            if self.write_plan {
                // A stop after a requested write still leaves the plan file, so the
                // existing "plan on disk counts" tests stay Planned on the first spawn.
                if self.response.is_err() {
                    let _ = tokio::fs::write(&self.plan_file_target, &self.plan_body).await;
                }
                let dest = match role {
                    GoalCreationRole::Research => research,
                    GoalCreationRole::Recon => recon,
                    GoalCreationRole::PlanWriter => self.plan_file_target.clone(),
                    GoalCreationRole::Reviewer => {
                        goal_creation_artifact(&self.plan_file_target, "review")
                    }
                };
                // An empty body must not create a non-empty research or recon file.
                // The plan writer still writes that empty body so an empty plan fails closed.
                if !self.plan_body.is_empty() || matches!(role, GoalCreationRole::PlanWriter) {
                    let _ = tokio::fs::write(&dest, &self.plan_body).await;
                }
            }
            match &self.response {
                Ok(text) => Ok(text.clone()),
                Err(SpawnError::Transport(s)) => Err(SpawnError::Transport(s.clone())),
                Err(SpawnError::Runtime { message, cancelled }) => Err(SpawnError::Runtime {
                    message: message.clone(),
                    cancelled: *cancelled,
                }),
                Err(SpawnError::Interrupted) => Err(SpawnError::Interrupted),
            }
        }
    }

    fn collect_events() -> (Arc<Mutex<Vec<String>>>, impl Fn(Event) + Send + Sync) {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let log_clone = log.clone();
        let emit = move |e: Event| {
            let tag = match e {
                Event::GoalPlannerFired { .. } => "fired".to_string(),
                Event::GoalPlannerCompleted { .. } => "completed".to_string(),
                Event::GoalPlannerFailClosed { reason, .. } => format!("fail_closed:{reason}"),
                other => format!("other:{other:?}"),
            };
            log_clone.lock().unwrap().push(tag);
        };
        (log, emit)
    }

    fn tmp_plan_file(name: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!(
            "goal-planner-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        tmp.join("plan.md")
    }

    #[tokio::test]
    async fn success_path_emits_fired_and_completed_and_returns_planned() {
        let plan_file = tmp_plan_file("happy");
        let spawner = Arc::new(MockSpawner::ok_writes(
            &plan_file,
            b"# Plan: foo\n\n## Goal kind\n\ncode-change\n",
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(outcome, GoalPlannerOutcome::Planned { .. }));
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2, "{log:?}");
        assert_eq!(log.as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    #[tokio::test]
    async fn missing_plan_file_fails_closed() {
        let plan_file = tmp_plan_file("missing");
        let spawner = Arc::new(MockSpawner::ok_does_not_write(&plan_file));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(
            outcome,
            GoalPlannerOutcome::FailClosed {
                reason: GoalPlannerFailClosedReason::MissingPlan,
                ..
            }
        ));
        let log = log.lock().unwrap();
        assert!(log.iter().any(|t| t == "fail_closed:missing_plan_file"));
    }

    #[tokio::test]
    async fn when_a_goal_plan_writer_finishes_and_the_plan_file_is_present_the_goal_does_not_pause_with_no_plan_was_produced()
     {
        let plan_file = tmp_plan_file("finished-writer");
        let spawner = Arc::new(MockSpawner {
            response: Err(SpawnError::Interrupted),
            write_plan: true,
            plan_body: b"# Plan: vendor KeyOS\n\n## Goal kind\ncode-change\n".to_vec(),
            plan_file_target: plan_file.clone(),
            last_prompt: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "vendor KeyOS dependencies",
                context: "",
                plan_file: &plan_file,
                attempt: 5,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "when a goal plan writer finishes and the plan file is present, the goal does not pause with \"No plan was produced.\" outcome={outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "a plan on disk must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    /// GitHub #234 leftover after #228: the nested once-run writer stop is
    /// `SpawnError::Runtime`, not Interrupted. A written `{PLAN_FILE}` still counts.
    #[tokio::test]
    async fn when_a_goal_plan_writer_stops_on_repeating_sentence_and_the_plan_file_is_present_the_goal_does_not_pause_with_no_plan_was_produced()
     {
        let plan_file = tmp_plan_file("repeat-stop-written");
        let message = format!(
            "Session error: {}",
            xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE
        );
        let spawner = Arc::new(MockSpawner {
            response: Err(SpawnError::Runtime {
                message,
                cancelled: false,
            }),
            write_plan: true,
            plan_body: b"# Plan: vendor KeyOS\n\n## Goal kind\ncode-change\n".to_vec(),
            plan_file_target: plan_file.clone(),
            last_prompt: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "vendor KeyOS dependencies",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "when a goal plan writer stops on a repeating sentence and the plan file is present, the goal does not pause with \"No plan was produced.\" outcome={outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "a plan on disk must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    /// Repeating-sentence stop with no file is the same class as Interrupted:
    /// the harness retries. It is not a Runtime fail-closed.
    #[tokio::test]
    async fn when_a_goal_plan_writer_stops_on_repeating_sentence_without_a_plan_the_harness_retries_instead_of_failing_closed()
     {
        let plan_file = tmp_plan_file("repeat-stop-missing");
        let message = format!(
            "Session error: {}",
            xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE
        );
        let spawner = Arc::new(MockSpawner::fails(
            SpawnError::Runtime {
                message,
                cancelled: false,
            },
            &plan_file,
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Interrupted),
            "a repeating-sentence stop with no plan must retry, not fail closed: {outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "repeating-sentence with no plan must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired"]);
    }

    /// Operator 2026-10-21: Goals are still broken. Repeating-sentence stop on
    /// the plan writer with PLAN_FILE on disk must be Planned, not
    /// "No plan was produced." Transport after write is the production drop.
    #[tokio::test]
    async fn when_a_goal_plan_writer_transport_stops_and_the_plan_file_is_present_the_goal_does_not_pause_with_no_plan_was_produced()
     {
        let plan_file = tmp_plan_file("transport-written");
        let spawner = Arc::new(MockSpawner {
            response: Err(SpawnError::Transport("channel closed".to_string())),
            write_plan: true,
            plan_body: b"# Plan: vendor KeyOS\n\n## Goal kind\ncode-change\n".to_vec(),
            plan_file_target: plan_file.clone(),
            last_prompt: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "vendor KeyOS dependencies",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "Goals are still broken: repeating-sentence stop with PLAN_FILE on disk still reports No plan was produced. \
             transport after write must be Planned, not \"No plan was produced.\" outcome={outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "a plan on disk must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    /// Pager chrome headline is `Stopped: repeating sentence`, not the sampler
    /// constant. Without a file the harness retries.
    #[tokio::test]
    async fn when_a_goal_plan_writer_chrome_repeating_sentence_without_a_plan_the_harness_retries_instead_of_failing_closed()
     {
        let plan_file = tmp_plan_file("chrome-repeat-missing");
        let spawner = Arc::new(MockSpawner::fails(
            SpawnError::Runtime {
                message: "Session error: Stopped: repeating sentence".into(),
                cancelled: false,
            },
            &plan_file,
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Interrupted),
            "Goals are still broken: repeating-sentence stop with PLAN_FILE on disk still reports No plan was produced. \
             chrome repeating sentence with no plan must retry, not fail closed: {outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "repeating-sentence with no plan must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired"]);
    }

    /// Chrome repeating-sentence after `{PLAN_FILE}` is written is Planned.
    #[tokio::test]
    async fn when_a_goal_plan_writer_chrome_repeating_sentence_and_the_plan_file_is_present_the_goal_does_not_pause_with_no_plan_was_produced()
     {
        let plan_file = tmp_plan_file("chrome-repeat-written");
        let spawner = Arc::new(MockSpawner {
            response: Err(SpawnError::Runtime {
                message: "Session error: Stopped: repeating sentence".into(),
                cancelled: false,
            }),
            write_plan: true,
            plan_body: b"# Plan: vendor KeyOS\n\n## Goal kind\ncode-change\n".to_vec(),
            plan_file_target: plan_file.clone(),
            last_prompt: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "vendor KeyOS dependencies",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "Goals are still broken: repeating-sentence stop with PLAN_FILE on disk still reports No plan was produced. \
             chrome repeating sentence with PLAN_FILE on disk must be Planned, not \"No plan was produced.\" outcome={outcome:?}",
        );
        let log = log.lock().unwrap();
        assert!(
            !log.iter().any(|tag| tag.starts_with("fail_closed")),
            "a plan on disk must not fail closed: {log:?}",
        );
        assert_eq!(log.as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    /// `finish_child` cancel after a successful write can land as Runtime
    /// `cancelled: true`. The file still counts.
    #[tokio::test]
    async fn when_a_goal_plan_writer_runtime_cancels_and_the_plan_file_is_present_the_goal_does_not_pause_with_no_plan_was_produced()
     {
        let plan_file = tmp_plan_file("runtime-cancel-written");
        let spawner = Arc::new(MockSpawner {
            response: Err(SpawnError::Runtime {
                message: "Subagent was cancelled".into(),
                cancelled: true,
            }),
            write_plan: true,
            plan_body: b"# Plan: vendor KeyOS\n\n## Goal kind\ncode-change\n".to_vec(),
            plan_file_target: plan_file.clone(),
            last_prompt: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "vendor KeyOS dependencies",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "when a goal plan writer is cancelled after writing the plan file, the goal does not pause with \"No plan was produced.\" outcome={outcome:?}",
        );
        assert_eq!(log.lock().unwrap().as_slice(), ["fired", "completed"]);
        let _ = std::fs::remove_file(&plan_file);
    }

    #[tokio::test]
    async fn transport_error_fails_closed_with_transport_reason() {
        let plan_file = tmp_plan_file("transport");
        let spawner = Arc::new(MockSpawner::fails(
            SpawnError::Transport("channel closed".to_string()),
            &plan_file,
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(
            outcome,
            GoalPlannerOutcome::FailClosed {
                reason: GoalPlannerFailClosedReason::Transport,
                ..
            }
        ));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|t| t == "fail_closed:transport")
        );
    }

    #[tokio::test]
    async fn runtime_cancelled_maps_to_aborted() {
        let plan_file = tmp_plan_file("aborted");
        let spawner = Arc::new(MockSpawner::fails(
            SpawnError::Runtime {
                message: "user aborted".into(),
                cancelled: true,
            },
            &plan_file,
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(
            outcome,
            GoalPlannerOutcome::FailClosed {
                reason: GoalPlannerFailClosedReason::Aborted,
                ..
            }
        ));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|t| t == "fail_closed:aborted")
        );
    }

    #[tokio::test]
    async fn runtime_not_cancelled_maps_to_runtime() {
        let plan_file = tmp_plan_file("runtime");
        let spawner = Arc::new(MockSpawner::fails(
            SpawnError::Runtime {
                message: "subagent crashed".into(),
                cancelled: false,
            },
            &plan_file,
        ));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(
            outcome,
            GoalPlannerOutcome::FailClosed {
                reason: GoalPlannerFailClosedReason::Runtime,
                ..
            }
        ));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|t| t == "fail_closed:runtime")
        );
    }

    #[tokio::test]
    async fn malformed_terminal_with_file_present_still_succeeds() {
        let plan_file = tmp_plan_file("malformed-but-written");
        let mut spawner = MockSpawner::ok_writes(&plan_file, b"# Plan: foo\n");
        spawner.response = Ok("done.".to_string());
        let spawner = Arc::new(spawner);
        let (_, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(outcome, GoalPlannerOutcome::Planned { .. }));
        let _ = std::fs::remove_file(&plan_file);
    }

    #[tokio::test]
    async fn empty_plan_file_fails_closed() {
        let plan_file = tmp_plan_file("empty");
        let spawner = Arc::new(MockSpawner::ok_writes(&plan_file, b""));
        let (log, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(matches!(
            outcome,
            GoalPlannerOutcome::FailClosed {
                reason: GoalPlannerFailClosedReason::MissingPlan,
                ..
            }
        ));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|t| t.starts_with("fail_closed:"))
        );
    }

    #[tokio::test]
    async fn prompt_substitutes_plan_file_path_and_carries_objective() {
        let plan_file = tmp_plan_file("prompt");
        let spawner = Arc::new(MockSpawner::ok_writes(&plan_file, b"# Plan\n"));
        let spawner_obs = spawner.clone();
        let (_, emit) = collect_events();

        let _ = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "implement feature X",
                context: "prior conversation\n",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        let prompt = spawner_obs
            .last_prompt
            .lock()
            .unwrap()
            .clone()
            .expect("prompt captured");
        let expected_path = plan_file.to_string_lossy();
        assert!(
            !prompt.contains("{PLAN_FILE}"),
            "placeholder must be substituted"
        );
        assert!(
            prompt.contains(&*expected_path),
            "rendered prompt must reference plan path"
        );
        assert!(prompt.contains("implement feature X"));
        assert!(prompt.contains("prior conversation"));
        let _ = std::fs::remove_file(&plan_file);
    }

    /// Goal creation requests four distinct roles. The plan-writer template does not
    /// contain the web-research instruction and does not contain the file-inspection instruction.
    #[tokio::test]
    async fn goal_creation_requests_four_distinct_roles_and_plan_writer_does_not_research_or_inspect_files()
     {
        let plan_file = tmp_plan_file("four-roles");
        let plan_body = b"# Plan: four distinct roles\n\n## Goal kind\ncode-change\n";
        let spawner = Arc::new(MockSpawner::ok_writes(&plan_file, plan_body));
        let observed = spawner.clone();
        let (_, emit) = collect_events();

        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "ship the widget named in src/widget.rs",
                context: "notes for the named file",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;

        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "four role spawns with artifacts on disk must plan, outcome={outcome:?}",
        );
        let seen = observed.seen.lock().unwrap().clone();
        let roles: Vec<GoalCreationRole> = seen.iter().map(|row| row.role).collect();
        assert_eq!(
            roles,
            vec![
                GoalCreationRole::Research,
                GoalCreationRole::Recon,
                GoalCreationRole::PlanWriter,
                GoalCreationRole::Reviewer,
            ],
            "goal creation must request four distinct roles in order, not one writer",
        );
        let ids: std::collections::BTreeSet<&str> =
            seen.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            4,
            "the four spawns must use distinct ids: {ids:?}"
        );
        let descriptions: Vec<&str> = roles.iter().map(|role| role.description()).collect();
        assert_eq!(
            descriptions,
            vec![
                "goal research specialist",
                "goal recon specialist",
                "goal plan writer",
                "goal plan reviewer",
            ],
        );

        let plan_writer = seen
            .iter()
            .find(|row| row.role == GoalCreationRole::PlanWriter)
            .expect("plan writer spawn");
        assert!(
            plan_writer.research_nonempty && plan_writer.recon_nonempty,
            "plan writer runs only after the research file and the recon file exist",
        );
        let reviewer = seen
            .iter()
            .find(|row| row.role == GoalCreationRole::Reviewer)
            .expect("reviewer spawn");
        assert!(
            reviewer.plan_nonempty && reviewer.research_nonempty,
            "reviewer runs only after the plan file and the research file exist",
        );

        let research_path = goal_creation_artifact(&plan_file, "research");
        let recon_path = goal_creation_artifact(&plan_file, "recon");
        let review_path = goal_creation_artifact(&plan_file, "review");
        assert!(
            plan_writer
                .prompt
                .contains(&*research_path.to_string_lossy()),
            "plan writer must be given the research path",
        );
        assert!(
            plan_writer.prompt.contains(&*recon_path.to_string_lossy()),
            "plan writer must be given the recon path",
        );
        assert!(
            plan_writer.prompt.contains(&*plan_file.to_string_lossy()),
            "plan writer must be given the plan path",
        );
        assert!(
            !plan_writer.prompt.contains("# Plan: four distinct roles"),
            "do not paste the research or recon file body into the plan writer prompt",
        );
        assert!(
            reviewer.prompt.contains(&*plan_file.to_string_lossy()),
            "reviewer must be given the plan path",
        );
        assert!(
            reviewer.prompt.contains(&*research_path.to_string_lossy()),
            "reviewer must be given the research path",
        );
        assert!(
            !reviewer.prompt.contains("# Plan: four distinct roles"),
            "do not paste file bodies into the reviewer prompt",
        );

        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("research it with"),
            "plan-writer template must not contain the web-research instruction",
        );
        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("WEB_SEARCH_TOOL"),
            "plan-writer template must not contain the web-search tool instruction",
        );
        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("Inspect files"),
            "plan-writer template must not contain the file-inspection instruction",
        );
        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("read/search/list"),
            "plan-writer template must not tell the writer to read/search/list the tree",
        );
        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("SEARCH_TOOL"),
            "plan-writer template must not name the tree search tool",
        );
        assert!(
            !GOAL_PLANNER_PROMPT_TEMPLATE.contains("LIST_TOOL"),
            "plan-writer template must not name the tree list tool",
        );
        assert!(
            !plan_writer.prompt.contains("research it with"),
            "rendered plan writer prompt must not contain the web-research instruction",
        );
        assert!(
            !plan_writer.prompt.contains("Inspect files"),
            "rendered plan writer prompt must not contain the file-inspection instruction",
        );
        assert!(
            GOAL_RESEARCH_PROMPT_TEMPLATE.contains("research it with"),
            "research template keeps the web-research instruction the plan writer dropped",
        );
        assert!(
            GOAL_RECON_PROMPT_TEMPLATE.contains("Inspect files"),
            "recon template keeps the file-inspection instruction the plan writer dropped",
        );
        assert!(
            reviewer.prompt.contains(&*review_path.to_string_lossy()),
            "reviewer must be given its own review path",
        );

        let _ = std::fs::remove_file(&plan_file);
        let _ = std::fs::remove_file(&research_path);
        let _ = std::fs::remove_file(&recon_path);
        let _ = std::fs::remove_file(&review_path);
    }

    // ── RoleSpawnOverride and spawn-and-retry-once wrapper ─────

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A `RoleRenderedPrompt` whose two renders are identical (the inherit / same-toolset case).
    /// Tests that exercise the distinct-fallback path build the struct inline instead.
    fn role_prompt(p: &str) -> RoleRenderedPrompt {
        RoleRenderedPrompt {
            primary: p.to_string(),
            fallback: p.to_string(),
        }
    }

    #[tokio::test]
    async fn channel_spawner_threads_harness_override_to_request() {
        use xai_grok_tools::implementations::grok_build::task::types::{
            SubagentEvent, SubagentResult,
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let spawner = ChannelSpawner {
            event_tx: tx,
            foreground_wait: None,
            parent_session_id: "parent".into(),
            parent_prompt_id: None,
            cwd: None,
            trace_sink: None,
            role_override: RoleSpawnOverride {
                model: Some("cfg-model".into()),
                agent_type: Some("cursor".into()),
            },
            cancel_token: tokio_util::sync::CancellationToken::new(),
            events: None,
        };
        let handle = tokio::spawn(async move {
            let _ = spawner
                .spawn_planner(
                    "plan-id",
                    GoalCreationRole::PlanWriter,
                    role_prompt("prompt"),
                )
                .await;
        });
        let SubagentEvent::Spawn(request) = rx.recv().await.expect("spawn event") else {
            panic!("expected Spawn");
        };
        assert_eq!(
            request.subagent_type, GOAL_PLANNER_SUBAGENT_TYPE,
            "planner always spawns the fixed role subagent type",
        );
        assert_eq!(
            request.runtime_overrides.harness_agent_type.as_deref(),
            Some("cursor"),
            "configured agent_type must thread as the harness override",
        );
        assert_eq!(
            request.runtime_overrides.model.as_deref(),
            Some("cfg-model")
        );
        assert!(
            request.runtime_overrides.once_run,
            "Goal Plan Writer is a disposable once-run role; spawn must mark once_run so the child summarizes and stops instead of compact-and-continue"
        );
        assert!(
            request.fork_context,
            "Goal Plan Writer forks parent history; that fill must not AUTO compact the once-run child"
        );
        // Reply SUCCESS so the explicit pair does NOT trigger a fail-open retry.
        let _ = request.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("ok"),
            ..Default::default()
        });
        handle.await.unwrap();
    }

    #[test]
    fn effective_role_model_id_prefers_override_else_parent() {
        assert_eq!(
            effective_role_model_id(Some("override-model"), "parent-model"),
            "override-model",
        );
        assert_eq!(
            effective_role_model_id(None, "parent-model"),
            "parent-model"
        );
    }

    /// A miss here silently re-runs a cancelled explicit-pair spawn on the current model.
    #[test]
    fn spawn_error_is_retryable_truth_table() {
        assert!(SpawnError::Transport(String::new()).is_retryable());
        assert!(
            SpawnError::Runtime {
                message: String::new(),
                cancelled: false,
            }
            .is_retryable()
        );
        assert!(
            !SpawnError::Runtime {
                message: String::new(),
                cancelled: true,
            }
            .is_retryable()
        );
        assert!(!SpawnError::Interrupted.is_retryable());
    }

    #[test]
    fn role_spawn_override_is_explicit() {
        assert!(!RoleSpawnOverride::default().is_explicit());
        assert!(
            RoleSpawnOverride {
                model: Some("m".into()),
                agent_type: None,
            }
            .is_explicit()
        );
        assert!(
            RoleSpawnOverride {
                model: None,
                agent_type: Some("t".into()),
            }
            .is_explicit()
        );
    }

    #[tokio::test]
    async fn retry_once_on_explicit_failure_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let ov = RoleSpawnOverride {
            model: Some("cfg-model".into()),
            agent_type: Some("cfg-type".into()),
        };
        let out: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "planner",
            None,
            &ov,
            None,
            role_prompt("PROMPT"),
            |model, harness, prompt| {
                let c = c.clone();
                async move {
                    assert_eq!(prompt, "PROMPT");
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        // First attempt uses the configured pair.
                        assert_eq!(model.as_deref(), Some("cfg-model"));
                        assert_eq!(harness.as_deref(), Some("cfg-type"));
                        Err(SpawnError::Transport("boom".into()))
                    } else {
                        // Retry drops the model and harness (inherit session harness)
                        assert_eq!(model, None, "retry must inherit the current model");
                        assert_eq!(harness, None, "retry must inherit the session harness");
                        Ok("retried".into())
                    }
                }
            },
        )
        .await;
        assert_eq!(out.unwrap(), "retried");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "explicit failure retries once"
        );
    }

    #[tokio::test]
    async fn retry_uses_fallback_render_not_primary() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let ov = RoleSpawnOverride {
            model: Some("cfg-model".into()),
            agent_type: Some("cfg-type".into()),
        };
        let out: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "planner",
            None,
            &ov,
            None,
            RoleRenderedPrompt {
                primary: "PRIMARY".to_string(),
                fallback: "FALLBACK".to_string(),
            },
            |_model, _harness, prompt| {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        assert_eq!(prompt, "PRIMARY", "first attempt uses the primary render");
                        Err(SpawnError::Transport("boom".into()))
                    } else {
                        assert_eq!(
                            prompt, "FALLBACK",
                            "retry must use the default-toolset fallback render",
                        );
                        Ok("retried".into())
                    }
                }
            },
        )
        .await;
        assert_eq!(out.unwrap(), "retried");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn inherit_override_never_retries() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let ov = RoleSpawnOverride::default();
        let out: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "planner",
            None,
            &ov,
            None,
            role_prompt("PROMPT"),
            |model, harness, _prompt| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(model, None);
                    assert_eq!(harness, None, "inherit must not pin a harness");
                    Err(SpawnError::Transport("boom".into()))
                }
            },
        )
        .await;
        assert!(out.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "inherit already ran on the current model — no retry",
        );
    }

    #[tokio::test]
    async fn explicit_success_does_not_retry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let ov = RoleSpawnOverride {
            model: Some("m".into()),
            agent_type: None,
        };
        let out: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "skeptic",
            Some(2),
            &ov,
            None,
            role_prompt("PROMPT"),
            |_m, _h, _prompt| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok("ok".into())
                }
            },
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancellation_on_explicit_path_does_not_retry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let ov = RoleSpawnOverride {
            model: Some("cfg-model".into()),
            agent_type: Some("cfg-type".into()),
        };
        let out: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "planner",
            None,
            &ov,
            None,
            role_prompt("PROMPT"),
            |_m, _h, _prompt| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(SpawnError::Runtime {
                        message: "user aborted".into(),
                        cancelled: true,
                    })
                }
            },
        )
        .await;
        assert!(
            matches!(
                out,
                Err(SpawnError::Runtime {
                    cancelled: true,
                    ..
                })
            ),
            "a cancellation must propagate unchanged",
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "cancellation must NOT trigger the inherit retry",
        );
    }

    #[tokio::test]
    async fn fail_open_retry_emits_spawn_failed_event() {
        let dir = tempfile::tempdir().unwrap();
        let writer = xai_grok_session_events::EventWriter::open(dir.path());
        let ov = RoleSpawnOverride {
            model: Some("m".into()),
            agent_type: Some("t".into()),
        };
        // Both attempts fail: wrapper still emits the SpawnFailed reason once.
        let _: Result<String, SpawnError> = spawn_with_fail_open_retry(
            "skeptic",
            Some(1),
            &ov,
            Some(&writer),
            role_prompt("PROMPT"),
            |_m, _h, _prompt| async move { Err::<String, _>(SpawnError::Transport("x".into())) },
        )
        .await;
        let mut body = String::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            body.push_str(&std::fs::read_to_string(entry.unwrap().path()).unwrap_or_default());
        }
        assert!(
            body.contains("goal_role_model_fail_open"),
            "must emit fail-open event: {body}",
        );
        assert!(body.contains("spawn_failed"), "reason must be spawn_failed");
        assert!(body.contains("\"skeptic_idx\":1"), "skeptic_idx carried");
    }

    #[tokio::test]
    async fn planner_retries_to_inherit_instead_of_failing_closed() {
        use xai_grok_tools::implementations::grok_build::task::types::{
            SubagentEvent, SubagentResult,
        };
        let plan_file = tmp_plan_file("retry-failopen");
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // Fake coordinator: explicit-model spawn fails. The inherit retry (model None)
        // writes that role's backtick `.md` path. Writing only the plan file leaves
        // the research file empty, and the runner fail-closes before the plan writer.
        let coord = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let SubagentEvent::Spawn(req) = ev else {
                    continue;
                };
                if req.runtime_overrides.model.is_some() {
                    let _ = req.result_tx.send(SubagentResult {
                        success: false,
                        error: Some("bad configured model".into()),
                        ..Default::default()
                    });
                } else {
                    if let Some(path) = req
                        .prompt
                        .split('`')
                        .skip(1)
                        .step_by(2)
                        .find(|token| token.ends_with(".md"))
                    {
                        if let Some(parent) = std::path::Path::new(path).parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        let _ = tokio::fs::write(path, b"# Plan\n").await;
                    }
                    let _ = req.result_tx.send(SubagentResult {
                        success: true,
                        output: std::sync::Arc::from("Done"),
                        ..Default::default()
                    });
                }
            }
        });
        let spawner = Arc::new(ChannelSpawner {
            event_tx: tx,
            foreground_wait: None,
            parent_session_id: "p".into(),
            parent_prompt_id: None,
            cwd: None,
            trace_sink: None,
            role_override: RoleSpawnOverride {
                model: Some("cfg-model".into()),
                agent_type: Some("general-purpose".into()),
            },
            cancel_token: tokio_util::sync::CancellationToken::new(),
            events: None,
        });
        let (_log, emit) = collect_events();
        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;
        assert!(
            matches!(outcome, GoalPlannerOutcome::Planned { .. }),
            "bad pair must retry to inherit, never fail closed: {outcome:?}",
        );
        drop(coord);
        let _ = std::fs::remove_file(&plan_file);
    }

    #[tokio::test]
    async fn planner_cancellation_pauses_as_aborted_without_retry() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use xai_grok_tools::implementations::grok_build::task::types::{
            SubagentEvent, SubagentResult,
        };
        let plan_file = tmp_plan_file("cancel-aborted");
        let spawns = Arc::new(AtomicUsize::new(0));
        let spawns_coord = spawns.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // Fake coordinator: every spawn reports a cancellation.
        let coord = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let SubagentEvent::Spawn(req) = ev else {
                    continue;
                };
                spawns_coord.fetch_add(1, Ordering::SeqCst);
                let _ = req.result_tx.send(SubagentResult {
                    success: false,
                    cancelled: true,
                    error: Some("user aborted".into()),
                    ..Default::default()
                });
            }
        });
        let spawner = Arc::new(ChannelSpawner {
            event_tx: tx,
            foreground_wait: None,
            parent_session_id: "p".into(),
            parent_prompt_id: None,
            cwd: None,
            trace_sink: None,
            role_override: RoleSpawnOverride {
                model: Some("cfg-model".into()),
                agent_type: Some("general-purpose".into()),
            },
            cancel_token: tokio_util::sync::CancellationToken::new(),
            events: None,
        });
        let (_log, emit) = collect_events();
        let outcome = run_goal_planner(
            spawner,
            GoalPlannerInputs {
                objective: "do X",
                context: "",
                plan_file: &plan_file,
                attempt: 1,
                model_id: "grok-test",
                tool_names: &RoleToolNames::inherit_defaults(),
                inherit_tool_names: &RoleToolNames::inherit_defaults(),
            },
            &emit,
        )
        .await;
        assert!(
            matches!(
                outcome,
                GoalPlannerOutcome::FailClosed {
                    reason: GoalPlannerFailClosedReason::Aborted,
                    ..
                }
            ),
            "a cancelled explicit-pair spawn must fail closed as Aborted: {outcome:?}",
        );
        assert_eq!(
            spawns.load(Ordering::SeqCst),
            1,
            "cancellation must NOT trigger the inherit retry (one spawn only)",
        );
        drop(coord);
        let _ = std::fs::remove_file(&plan_file);
    }
}
