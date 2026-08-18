//! `ExitPlanMode` tool — new architecture (`Tool` trait).
//!
//! Signals that the agent has finished planning and is ready for the user to
//! review and approve the plan. The tool reads the plan file from disk (it does
//! NOT accept plan content as input) and surfaces it via:
//!
//! 1. A `PlanModeExited` **notification** sent to the gateway/client, carrying
//!    the plan content so the client can present it for user approval.
//! 2. A structured **`ExitPlanModeOutput`** returned to the model, containing
//!    the plan content (or an empty-plan message).
//!
//! The actual approval flow (yes/no with feedback, context clear, mode
//! transition) happens on the client side — this tool just says "I'm done,
//! here's the plan."
//!
//! ## Plan File
//!
//! The plan file path defaults to `.grok/plan.md` relative to the session
//! `Cwd`. The tool reads it via the `FileSystem` resource (the same async FS
//! abstraction used by `ReadFile` and `SearchReplace`).

pub mod types;

pub use types::{ExitPlanModeExtRequest, ExitPlanModeExtResponse};

use crate::notification::types::PlanModeExited;
use crate::types::output::ExitPlanModeOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{FileSystem, NotificationHandle, require_plan_file_path};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Input for the `ExitPlanMode` tool.
///
/// Empty object — the plan is read from the plan file on disk, NOT passed as
/// a parameter. This ensures the user sees exactly what was written to disk,
/// preventing divergence between the model's in-context plan and the actual
/// file content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExitPlanModeInput {}

/// `ExitPlanMode` tool.
///
/// Reads the plan file from disk and signals to the orchestration layer that
/// the agent is done planning. The client receives a `PlanModeExited`
/// notification with the plan content and is responsible for presenting the
/// approval UI.
///
/// Params: `()` — no per-tool configuration.
#[derive(Debug, Default)]
pub struct ExitPlanModeTool;

impl crate::types::tool_metadata::ToolMetadata for ExitPlanModeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::ExitPlan
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn emitted_notifications(&self) -> &'static [&'static str] {
        &["PlanModeExited"]
    }

    fn description_template(&self) -> &str {
        r#"Exit plan mode and present your plan to the user for review.

Use this after you have finished writing your plan to the plan file in plan mode.
Tool success means the plan was presented (soft-park / panel), not that the operator
approved it. Do not implement until a real plan panel decision (Approve / Revise /
Clarify / Quit) or an explicit freeform approve."#
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        // ExitPlanMode can only exist if EnterPlanMode is also registered —
        // exiting plan mode without the ability to enter would be nonsensical.
        use crate::implementations::grok_build::enter_plan_mode::EnterPlanModeTool;
        Expr::Value(ToolRequirement::Tool {
            namespace: crate::types::tool_metadata::ToolMetadata::tool_namespace(
                &EnterPlanModeTool,
            )
            .to_string(),
            id: xai_tool_runtime::Tool::id(&EnterPlanModeTool).to_string(),
            if_params: None,
        })
    }
}

impl xai_tool_runtime::Tool for ExitPlanModeTool {
    type Args = ExitPlanModeInput;
    type Output = ExitPlanModeOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("exit_plan_mode").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "exit_plan_mode",
            crate::types::tool_metadata::ToolMetadata::sanitized_description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.exit_plan_mode", skip_all)]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        _input: ExitPlanModeInput,
    ) -> Result<ExitPlanModeOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let (plan_file_path, plan_content) = {
            let res = resources.lock().await;

            let (plan_path, plan_file_path_display) = require_plan_file_path(&res)?;

            // Read the plan file from disk via the FileSystem abstraction.
            let content = if let Some(fs) = res.get::<FileSystem>() {
                match fs.0.read_file(&plan_path).await {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if text.trim().is_empty() {
                            None
                        } else {
                            Some(text)
                        }
                    }
                    Err(_) => None,
                }
            } else {
                // Fallback: try tokio::fs if no FileSystem resource is available.
                match tokio::fs::read_to_string(&plan_path).await {
                    Ok(text) if !text.trim().is_empty() => Some(text),
                    _ => None,
                }
            };

            (plan_file_path_display, content)
        };

        // Notify the gateway / client.
        {
            let res = resources.lock().await;
            if let Some(handle) = res.get::<NotificationHandle>() {
                handle.0.send_plan_mode_exited(PlanModeExited {
                    tool_call_id: ctx.call_id.as_str().to_owned(),
                    plan_content: plan_content.clone(),
                    plan_file_path: plan_file_path.clone(),
                });
            }
        }

        match plan_content {
            Some(content) => {
                tracing::info!(
                    plan_chars = content.len(),
                    "Presenting plan for operator review (not approval)"
                );

                // Body is always re-read from disk in this tool run. This tool
                // result is presentation only: shell intercept + plan panel CTAs
                // own real approval. Never claim "approved" / "start coding"
                // here (dogfood false auto-approve). Never teach freeform chat
                // approve (product CTAs only).
                let message = format!(
                    "Plan presented for operator review at {plan_file_path}. \
                     This tool result is NOT operator approval — do not implement yet. \
                     Wait for a real plan panel decision (Approve / Revise / Clarify / Quit). \
                     Do not treat freeform chat or always-approve permission mode as plan \
                     approval. The plan body below was re-read from disk at present time; \
                     prefer it over earlier draft plan titles."
                );

                Ok(ExitPlanModeOutput::PlanReady {
                    message,
                    plan_content: content,
                    plan_file_path,
                })
            }
            None => {
                tracing::info!("Presenting empty/missing plan file for operator review");

                Ok(ExitPlanModeOutput::EmptyPlan {
                    message: format!(
                        "Plan presented for operator review at {plan_file_path}, but no plan \
                         content was found. This tool result is NOT operator approval — do not \
                         implement yet. Wait for a real plan panel decision."
                    ),
                    plan_file_path,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer::local::LocalFs;
    use crate::types::output::ToolOutput;
    use crate::types::resources::{Cwd, PlanFilePath, Resources};
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn resources_with_cwd(cwd: &std::path::Path) -> Resources {
        let mut resources = Resources::new();
        resources.insert(Cwd(cwd.to_path_buf()));
        resources.insert(FileSystem(Arc::new(LocalFs)));
        resources
    }

    #[test]
    fn tool_name_and_description() {
        let tool = ExitPlanModeTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "exit_plan_mode");
        let desc = crate::types::tool_metadata::ToolMetadata::description_template(&tool);
        assert!(desc.contains("Exit plan mode"));
        assert!(desc.contains("plan file"));
    }

    #[test]
    fn tool_is_read_only() {
        assert!(xai_tool_runtime::Tool::capabilities(&ExitPlanModeTool).is_read_only);
    }

    #[test]
    fn tool_kind_is_exit_plan() {
        assert_eq!(
            crate::types::tool_metadata::ToolMetadata::kind(&ExitPlanModeTool),
            ToolKind::ExitPlan
        );
    }

    #[tokio::test]
    async fn exit_with_plan_content() {
        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(
            plan_dir.join("plan.md"),
            "# My Plan\n\n1. Do thing A\n2. Do thing B\n",
        )
        .unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        match result {
            ExitPlanModeOutput::PlanReady {
                ref message,
                ref plan_content,
                ref plan_file_path,
            } => {
                assert!(
                    message.contains("NOT operator approval")
                        || message.contains("not operator approval"),
                    "tool must not claim approval; got {message:?}"
                );
                assert!(
                    !message.to_lowercase().contains("has been approved"),
                    "tool must not claim plan approved: {message:?}"
                );
                assert!(
                    !message.to_lowercase().contains("start coding"),
                    "tool must not tell model to start coding: {message:?}"
                );
                assert!(
                    message.contains("re-read from disk at present time"),
                    "agent handoff must name disk re-read; got {message:?}"
                );
                assert!(plan_content.contains("Do thing A"));
                assert!(plan_content.contains("Do thing B"));
                // Cwd fallback now displays the resolved absolute path (shared resolver).
                assert!(plan_file_path.ends_with(".grok/plan.md"));
                assert!(
                    message.contains(plan_file_path.as_str()) || message.contains(".grok/plan.md"),
                    "message must point at plan file path; got {message:?}"
                );
            }
            other => panic!("Expected PlanReady, got {:?}", other),
        }
    }

    /// Named contract: tool body is read at run time (present). A rewrite of
    /// plan.md between an earlier park snapshot and tool execution must surface
    /// the new body in the model-facing result, not a frozen A.
    #[tokio::test]
    async fn exit_plan_mode_reads_current_disk_not_earlier_draft() {
        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        let plan_path = plan_dir.join("plan.md");
        std::fs::write(&plan_path, "# Plan A\nold_token_economy_marker\n").unwrap();

        // Simulate rewrite while approval was parked (before tool runs).
        std::fs::write(&plan_path, "# Plan B\nsurmount_team_usage_first\n").unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        match &result {
            ExitPlanModeOutput::PlanReady {
                plan_content,
                message,
                ..
            } => {
                assert!(
                    plan_content.contains("surmount_team_usage_first"),
                    "tool must re-read disk B at run time; got {plan_content:?}"
                );
                assert!(
                    !plan_content.contains("old_token_economy_marker"),
                    "tool must not return frozen draft A; got {plan_content:?}"
                );
                assert!(
                    message.contains("re-read from disk at present time"),
                    "model must be told body is present-time disk; got {message:?}"
                );
                assert!(
                    !message.to_lowercase().contains("approved"),
                    "present-time re-read must not claim approval: {message:?}"
                );
            }
            other => panic!("Expected PlanReady, got {other:?}"),
        }
        let prompt: ToolOutput = result.into();
        let text = prompt.to_prompt_format();
        assert!(
            text.contains("surmount_team_usage_first"),
            "model prompt must embed current disk plan; got {text:?}"
        );
        assert!(
            !text.contains("old_token_economy_marker"),
            "model prompt must not embed frozen draft A; got {text:?}"
        );
    }

    #[tokio::test]
    async fn exit_with_empty_plan_file() {
        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join("plan.md"), "   \n  \n").unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        match result {
            ExitPlanModeOutput::EmptyPlan { ref message, .. } => {
                assert!(
                    message.contains("NOT operator approval")
                        || message.contains("not operator approval"),
                    "empty plan must not claim approval: {message:?}"
                );
                assert!(
                    !message.to_lowercase().contains("exit approved"),
                    "must not say exit approved: {message:?}"
                );
                assert!(message.contains("no plan content") || message.contains("No plan content"));
            }
            other => panic!("Expected EmptyPlan, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn exit_with_missing_plan_file() {
        let tmp = TempDir::new().unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        match result {
            ExitPlanModeOutput::EmptyPlan { ref message, .. } => {
                assert!(
                    message.contains("NOT operator approval")
                        || message.contains("not operator approval"),
                    "missing plan must not claim approval: {message:?}"
                );
                assert!(
                    !message.to_lowercase().contains("exit approved"),
                    "must not say exit approved: {message:?}"
                );
            }
            other => panic!("Expected EmptyPlan, got {:?}", other),
        }
    }

    /// Named contract: bare tool success must never teach the model that the
    /// operator approved (dogfood: false "Plan auto-approved" after soft-park).
    #[tokio::test]
    async fn exit_plan_mode_tool_result_does_not_claim_operator_approval() {
        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join("plan.md"), "# Plan\nstep\n").unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let result = xai_tool_runtime::Tool::run(
            &ExitPlanModeTool,
            test_ctx_with_call_id(shared, "t-no-approve"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        let output: ToolOutput = result.into();
        let prompt = output.to_prompt_format();
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("not operator approval") || lower.contains("not operator approve"),
            "must state present ≠ approve: {prompt:?}"
        );
        assert!(
            !lower.contains("has been approved")
                && !lower.contains("plan mode exit approved")
                && !lower.contains("you can now start coding"),
            "must not claim approval or start coding: {prompt:?}"
        );
        assert!(
            lower.contains("plan panel decision") || lower.contains("approve / revise"),
            "must point at plan panel CTAs: {prompt:?}"
        );
        assert!(
            !lower.contains("explicit freeform approve"),
            "must not teach freeform chat approve: {prompt:?}"
        );
        assert!(
            lower.contains("always-approve") || lower.contains("permission mode"),
            "must separate always-approve permission mode from plan Approve: {prompt:?}"
        );
        assert!(
            prompt.contains("step"),
            "must still embed plan body: {prompt:?}"
        );
    }

    #[tokio::test]
    async fn sends_plan_mode_exited_notification_with_content() {
        use crate::notification::types::{ToolNotification, ToolNotificationHandle};

        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join("plan.md"), "The plan").unwrap();

        let (handle, mut rx) = ToolNotificationHandle::channel();
        let mut resources = resources_with_cwd(tmp.path());
        resources.insert(NotificationHandle(handle));
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "call-99"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        let notification = rx.try_recv().expect("should have received a notification");
        match notification {
            ToolNotification::PlanModeExited(exited) => {
                assert_eq!(exited.tool_call_id, "call-99");
                assert_eq!(exited.plan_content, Some("The plan".to_string()));
                assert!(exited.plan_file_path.ends_with(".grok/plan.md"));
            }
            other => panic!("Expected PlanModeExited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn works_without_notification_handle() {
        let tmp = TempDir::new().unwrap();
        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn prompt_format_includes_plan_content() {
        let tmp = TempDir::new().unwrap();
        let plan_dir = tmp.path().join(".grok");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join("plan.md"), "Step 1\nStep 2").unwrap();

        let resources = resources_with_cwd(tmp.path());
        let shared = resources.into_shared();
        let tool = ExitPlanModeTool;

        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(shared, "test-call"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        let output: ToolOutput = result.into();
        let prompt = output.to_prompt_format();
        assert!(
            prompt.contains("NOT operator approval") || prompt.contains("not operator approval"),
            "prompt must not claim approval: {prompt:?}"
        );
        assert!(!prompt.to_lowercase().contains("has been approved"));
        assert!(prompt.contains("saved at:"));
        assert!(prompt.contains("re-read from disk at present time"));
        assert!(prompt.contains("Step 1"));
        assert!(prompt.contains("Step 2"));
        assert!(prompt.contains(".grok/plan.md"));
        assert!(prompt.contains("## Plan:"));
    }

    // -- PlanFilePath resource tests --

    #[tokio::test]
    async fn reads_from_plan_file_path_resource() {
        let tmp = TempDir::new().unwrap();
        let plan_file = tmp.path().join("session-plan.md");
        std::fs::write(&plan_file, "# Session Plan\nDo X then Y").unwrap();

        let mut resources = Resources::new();
        resources.insert(Cwd(tmp.path().to_path_buf()));
        resources.insert(FileSystem(Arc::new(LocalFs)));
        resources.insert(PlanFilePath(plan_file.clone()));
        let shared = resources.into_shared();

        let result = xai_tool_runtime::Tool::run(
            &ExitPlanModeTool,
            test_ctx_with_call_id(shared, "t1"),
            ExitPlanModeInput {},
        )
        .await
        .unwrap();

        match result {
            ExitPlanModeOutput::PlanReady {
                ref plan_content,
                ref plan_file_path,
                ..
            } => {
                assert!(plan_content.contains("Do X then Y"));
                assert_eq!(plan_file_path, &plan_file.display().to_string());
            }
            other => panic!("Expected PlanReady, got {:?}", other),
        }
    }
}
