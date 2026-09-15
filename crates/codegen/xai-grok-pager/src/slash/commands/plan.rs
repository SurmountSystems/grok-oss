//! `/plan` enters plan mode. `/plan <description>` enters plan mode and starts
//! a turn with the description after the mode switch completes.
//!
//! `/plan --soft` docks Isolated Preview, the existing plan present surface
//! on the right. It does not enter plan mode. It does not park L1. It does
//! not enqueue the description as a Prompt. L1 docking Isolated Preview
//! must not cancel nested L2s. Nested work stays Working. Present is not
//! Approve. `--soft` is not the queue hold token (`queue` / `later`).
//!
//! Use `/view-plan` to open the current saved plan preview.

use crate::app::actions::{Action, PlanModeKind};
use crate::app::agent_view::AgentView;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::queue_schedule::{plan_command_text, queue_later_command, split_schedule_token};

/// Enter plan mode.
pub struct PlanCommand;

impl SlashCommand for PlanCommand {
    fn name(&self) -> &str {
        "plan"
    }

    fn description(&self) -> &str {
        "Enter plan mode, or /plan --soft to dock Isolated Preview"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn offered_when_session_less(&self) -> bool {
        // The dashboard offers `/plan` to start the next spawned agent in
        // plan mode (intercepted in `dispatch_dashboard_dispatch_slash`).
        true
    }

    fn usage(&self) -> &str {
        "/plan [--soft] [queue|later] [description]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[description]")
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let (hold, rest) = split_schedule_token(args);
        if hold {
            return queue_later_command(plan_command_text(rest));
        }
        let (soft, trimmed) = split_soft_flag(rest);
        if soft {
            // `/plan --soft` docks Isolated Preview. This is not
            // EnterPlanMode and not `/view-plan` ShowPlan. Dispatch must
            // not enter plan mode, must not park L1, and must not enqueue
            // the description as a Prompt. Nested L2s stay Working.
            // Present is not Approve.
            return CommandResult::Action(Action::DockIsolatedPreview {
                description: if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                },
            });
        }
        if trimmed.is_empty() {
            return CommandResult::Action(Action::SetPlanMode(PlanModeKind::On));
        }
        CommandResult::Action(Action::EnterPlanMode {
            description: Some(trimmed.to_string()),
        })
    }
}

/// Leading `--soft` token. Not `--softer` and not a description that happens
/// to start with that string without a token boundary.
fn split_soft_flag(args: &str) -> (bool, &str) {
    let trimmed = args.trim();
    if trimmed == "--soft" {
        return (true, "");
    }
    if let Some(rest) = trimmed.strip_prefix("--soft")
        && rest.starts_with(char::is_whitespace)
    {
        return (true, rest.trim_start());
    }
    (false, trimmed)
}

/// Session sidecar: Isolated Preview was docked when this TUI persisted
/// for `/rebuild`. Resume must not auto-dock leftover `plan.md`. This file
/// means the pane was open and must come back after relaunch. rebuild persist
/// plus load docks Isolated Preview again.
const ISOLATED_PREVIEW_OPEN_FILE: &str = "isolated_preview_open";

fn isolated_preview_open_path(cwd: &str, session_id: &str) -> Option<std::path::PathBuf> {
    xai_grok_shell::session::unsent_prompt_draft::unsent_prompt_draft_path(cwd, session_id)
        .and_then(|p| p.parent().map(|dir| dir.join(ISOLATED_PREVIEW_OPEN_FILE)))
}

/// Write or clear the Isolated Preview dock marker for `/rebuild` restore.
/// This helper is crate-visible so rebuild persist can record the pane.
/// The roundtrip in this file is the part this module can prove. rebuild.rs
/// and load.rs own the persist and load call sites.
pub(crate) fn persist_isolated_preview_open(cwd: &str, session_id: &str, open: bool) {
    let Some(path) = isolated_preview_open_path(cwd, session_id) else {
        return;
    };
    if open {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, b"1\n");
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

/// True when Isolated Preview was docked at the last `/rebuild` persist.
/// Consumes the marker so a later resume without the pane does not dock.
/// This helper is crate-visible so load can dock Isolated Preview again.
pub(crate) fn take_isolated_preview_open(cwd: &str, session_id: &str) -> bool {
    let Some(path) = isolated_preview_open_path(cwd, session_id) else {
        return false;
    };
    let open = path.is_file();
    if open {
        let _ = std::fs::remove_file(&path);
    }
    open
}

impl AgentView {
    /// Dock Isolated Preview on the right. L1 docking Isolated Preview must
    /// not cancel nested L2s. Nested work stays Working. Present is not
    /// Approve. A live Approve waiter, if parked, stays parked.
    ///
    /// `/plan --soft` is the dock for a new feature. `/rebuild` restore uses
    /// this same crate-visible surface.
    pub(crate) fn dock_isolated_preview(&mut self) {
        self.dock_isolated_preview_with_feature(None);
    }

    /// `/plan --soft [description]` docks Isolated Preview. A feature
    /// description seeds Isolated Preview. It does not enter plan mode and
    /// does not enqueue that text as a Prompt.
    pub(crate) fn dock_isolated_preview_with_feature(&mut self, feature: Option<String>) {
        let feature = feature.filter(|s| !s.trim().is_empty());
        if let Some(text) = feature.clone() {
            self.latest_inline_plan_content = Some(text.clone());
            if let Some(pav) = self.plan_approval_view.as_mut()
                && pav
                    .plan_content
                    .as_ref()
                    .is_none_or(|c| c.trim().is_empty())
            {
                pav.plan_content = Some(text);
                pav.has_plan = true;
            }
        } else {
            // Bare `/plan` / `/plan --soft`: current disk plan.md, not leftover
            // Isolated Preview present ("why the agent stopped" / TECH.md).
            self.reread_isolated_preview_from_current_disk_plan_md();
        }
        self.view_plan_requested = true;
        self.snapshot_or_clear_plan_feedback_draft();
        if self.plan_approval_view.is_some() {
            self.reopen_plan_approval();
        } else {
            self.park_local_idle_plan_decision_if_needed();
            // `/plan --soft` docks Isolated Preview even when plan mode is
            // off. After Plan Exit, plan_decision_resolved stays true so we
            // must not invent a live idle park that paints Plan ready.
            if self.plan_approval_view.is_none()
                && !self.plan_decision_resolved
                && self.plan_feedback_in_flight.is_none()
            {
                let stashed = self.prompt.stash();
                let mut pav =
                    crate::views::plan_approval_view::PlanApprovalViewState::for_idle_decision(
                        self.plan_body_for_preview(),
                    );
                pav.stashed_prompt = stashed;
                self.plan_approval_view = Some(pav);
            }
            self.show_plan_preview();
            // After Plan Exit, idle park is not invented (empty Enter never
            // Approves). `/plan` / `/plan --soft` must still dock Isolated
            // Preview. Compact must not swallow that slash.
            if self.line_viewer.is_none()
                && self.plan_decision_resolved
                && let Some(mut viewer) =
                    crate::views::file_search::line_viewer::LineViewerState::open_markdown_content(
                        "plan.md",
                        crate::views::plan_approval_view::EMPTY_PLAN_PLACEHOLDER.to_owned(),
                        None,
                    )
            {
                viewer.fullscreen = false;
                viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
                let plan = viewer.plan_mut();
                plan.show_action_buttons = true;
                plan.feedback_active = false;
                self.line_viewer = Some(viewer);
            }
        }
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.fullscreen = false;
            viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
        }
        self.restore_plan_feedback_draft_if_composer_lost();
        self.clear_view_plan_request_if_waiter_bound();
        self.persist_session_plan_dock_open(self.line_viewer.is_some());
        // Isolated Preview dock must not re-stamp session_plans with the
        // painted body. Present persist writes the live plan. Re-stamping
        // here freezes Isolated Preview over a newer disk plan.md.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn make_ctx_inactive_plan_mode<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot {
                plan_mode_active: false,
                ..PagerLocalSnapshot::default()
            },
        }
    }

    fn make_ctx_active_plan_mode<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot {
                plan_mode_active: true,
                ..PagerLocalSnapshot::default()
            },
        }
    }

    /// `/plan` (no args, not in plan mode) → `SetPlanMode(On)`.
    #[test]
    fn no_args_not_in_plan_dispatches_set_plan_mode_on() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "") {
            CommandResult::Action(Action::SetPlanMode(kind)) => {
                assert_eq!(
                    kind,
                    PlanModeKind::On,
                    "`/plan` (no args, not in plan mode) must dispatch SetPlanMode(On)"
                );
            }
            other => panic!("expected Action::SetPlanMode, got {other:?}"),
        }
    }

    /// `/plan` (no args, already in plan mode) → idempotent `SetPlanMode(On)`.
    #[test]
    fn no_args_already_in_plan_dispatches_set_plan_mode_on() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_active_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "") {
            CommandResult::Action(Action::SetPlanMode(kind)) => {
                assert_eq!(kind, PlanModeKind::On);
            }
            other => panic!("expected Action::SetPlanMode, got {other:?}"),
        }
    }

    /// Whitespace-only → treated as no args.
    #[test]
    fn whitespace_only_arg_not_in_plan_dispatches_set_plan_mode_on() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "   ") {
            CommandResult::Action(Action::SetPlanMode(kind)) => {
                assert_eq!(kind, PlanModeKind::On);
            }
            other => panic!("expected SetPlanMode for whitespace-only arg, got {other:?}"),
        }
    }

    /// `/plan <description>` → `EnterPlanMode` with description.
    #[test]
    fn with_description_keeps_enter_plan_mode_when_not_in_plan() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "Refactor the auth flow") {
            CommandResult::Action(Action::EnterPlanMode { description }) => {
                assert_eq!(
                    description.as_deref(),
                    Some("Refactor the auth flow"),
                    "`/plan <desc>` must dispatch EnterPlanMode with the description"
                );
            }
            CommandResult::Action(Action::DockIsolatedPreview { .. }) => {
                panic!("`/plan <desc>` without --soft is not Isolated Preview")
            }
            other => panic!("expected Action::EnterPlanMode, got {other:?}"),
        }
    }

    /// `/plan <description>` when already in plan mode still emits
    /// `EnterPlanMode`; the dispatcher owns the idempotent mode handling.
    #[test]
    fn with_description_already_in_plan_keeps_enter_plan_mode() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_active_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "something") {
            CommandResult::Action(Action::EnterPlanMode { description }) => {
                assert_eq!(description.as_deref(), Some("something"));
            }
            other => panic!("expected EnterPlanMode, got {other:?}"),
        }
    }

    /// Whitespace is trimmed from the description.
    #[test]
    fn with_description_trims_whitespace() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "  hello world  ") {
            CommandResult::Action(Action::EnterPlanMode { description }) => {
                assert_eq!(description.as_deref(), Some("hello world"));
            }
            other => panic!("expected EnterPlanMode, got {other:?}"),
        }
    }

    /// Named contract: `/plan --soft` docks Isolated Preview. That Action
    /// is not EnterPlanMode and not `/view-plan` ShowPlan. Dispatch must
    /// not enter plan mode.
    #[test]
    fn plan_soft_flag_dispatches_isolated_preview_dock_not_plan_mode() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "--soft") {
            CommandResult::Action(Action::DockIsolatedPreview { description }) => {
                assert!(
                    description.is_none(),
                    "`/plan --soft` with no feature text must not invent a description"
                );
            }
            CommandResult::Action(Action::EnterPlanMode { .. }) => {
                panic!("`/plan --soft` must not return EnterPlanMode")
            }
            CommandResult::Action(Action::SetPlanMode(_)) => {
                panic!("`/plan --soft` must not enter plan mode")
            }
            CommandResult::Action(Action::ShowPlan) => {
                panic!("`/plan --soft` is Isolated Preview, not `/view-plan` ShowPlan")
            }
            other => {
                panic!("`/plan --soft` must dock Isolated Preview, got {other:?}")
            }
        }
    }

    /// Named contract: `--soft` is a flag, not a plan description and not
    /// the queue hold token. The rest seeds Isolated Preview.
    #[test]
    fn plan_soft_flag_is_not_a_description() {
        let cmd = PlanCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx_inactive_plan_mode(&models, &bundle);
        match cmd.run(&mut ctx, "  --soft  ") {
            CommandResult::Action(Action::DockIsolatedPreview { description }) => {
                assert!(description.is_none());
            }
            other => {
                panic!("whitespace around --soft must still dock Isolated Preview, got {other:?}")
            }
        }
        match cmd.run(&mut ctx, "--soft rewrite auth") {
            CommandResult::Action(Action::DockIsolatedPreview { description }) => {
                assert_eq!(
                    description.as_deref(),
                    Some("rewrite auth"),
                    "`--soft` is a flag; the rest seeds Isolated Preview, not a Prompt"
                );
            }
            CommandResult::Action(Action::EnterPlanMode { .. }) => {
                panic!("`/plan --soft <feature>` must not return EnterPlanMode")
            }
            other => {
                panic!("`/plan --soft <feature>` must keep the feature description, got {other:?}")
            }
        }
        match cmd.run(&mut ctx, "queue --soft") {
            CommandResult::QueueLater { .. } => {}
            other => panic!("`queue` is the hold token; `--soft` is not, got {other:?}"),
        }
    }

    /// Named contract: rebuild persist + load docks Isolated Preview again.
    /// This file proves the persist-marker helper roundtrip. rebuild.rs and
    /// load.rs own the persist and load call sites.
    #[serial_test::serial(GROK_HOME)]
    #[test]
    fn persist_isolated_preview_open_roundtrip_rebuild_persist_load_docks_again() {
        let mut fx = crate::test_util::GrokHomeFixture::new();
        let cwd = fx.cwd_str();
        let session_id = "iso-preview-roundtrip";
        fx.write_summary(&cwd, session_id, serde_json::json!({}));

        persist_isolated_preview_open(&cwd, session_id, true);
        assert!(
            take_isolated_preview_open(&cwd, session_id),
            "rebuild persist + load docks Isolated Preview again"
        );
        assert!(
            !take_isolated_preview_open(&cwd, session_id),
            "take consumes the Isolated Preview dock marker so a later resume without the pane does not dock"
        );

        persist_isolated_preview_open(&cwd, session_id, true);
        persist_isolated_preview_open(&cwd, session_id, false);
        assert!(
            !take_isolated_preview_open(&cwd, session_id),
            "persist open=false must clear the Isolated Preview dock marker"
        );
    }
}
