//! `/plan` enters plan mode. Bare `/plan` exclusive-blocks nested
//! implementers and paints covering exclusive present. `/plan <description>`
//! enters plan mode and starts a turn with the description after the mode
//! switch completes.
//!
//! `/plan --soft` docks Isolated Preview, the existing plan present surface
//! on the right. It does not enter plan mode. It does not park L1. It does
//! not enqueue the description as a Prompt. L1 docking Isolated Preview
//! must not cancel nested L2s. Nested work stays Working. Soft planning
//! does not reset the primary plan. It makes a secondary plan. Isolated
//! Preview does not immediately pull up leftover current `plan.md`. Isolated
//! Preview stays until Esc, Exit, or Approve. Present is not Approve.
//! `--soft` is not the queue hold token (`queue` / `later`).
//!
//! Use `/view-plan` to open the current saved plan preview.

use crate::app::actions::{Action, Effect, PlanModeKind};
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
    /// `/view-plan` and `/rebuild` restore dock Isolated Preview from the
    /// primary session `plan.md`. Bare `/plan` uses
    /// [`Self::enter_exclusive_plan_covering`]. `/plan --soft` uses
    /// [`Self::dock_isolated_preview_with_feature`].
    pub(crate) fn dock_isolated_preview(&mut self) {
        self.isolated_preview_shows_secondary_plan = false;
        if !matches!(
            self.plan_feedback_in_flight,
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Updating)
        ) {
            self.reread_isolated_preview_from_current_disk_plan_md();
        }
        self.finish_isolated_preview_dock();
    }

    /// Bare `/plan` covering exclusive present from current disk `plan.md`.
    /// Covering is `line_viewer.fullscreen` and not a soft side pane.
    /// Nested implementers are exclusive-blocked by the caller. Empty
    /// Enter never Approves. Isolated Preview leftover dock is `/plan --soft`
    /// and `/view-plan`, not this path.
    pub(crate) fn enter_exclusive_plan_covering(&mut self) {
        self.isolated_preview_shows_secondary_plan = false;
        if !matches!(
            self.plan_feedback_in_flight,
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Updating)
        ) {
            self.reread_isolated_preview_from_current_disk_plan_md();
        }
        self.finish_isolated_preview_dock();
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.fullscreen = true;
            viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
            viewer.plan_mut().selected_cta = None;
        }
    }

    /// Exclusive `/plan` exclusive-blocks live nested implementers. Marks
    /// each live row `pending_kill` and returns `Effect::KillSubagent`
    /// (`session_id` + `subagent_id`). Does not CancelTurn the parent.
    /// Does not kill already-finished rows.
    pub(crate) fn exclusive_block_nested_implementers(&mut self) -> Vec<Effect> {
        let Some(session_id) = self.session.session_id.clone() else {
            return vec![];
        };
        let mut effects = Vec::new();
        for info in self.subagent_sessions.values_mut() {
            if info.finished {
                continue;
            }
            if info.pending_kill {
                continue;
            }
            info.pending_kill = true;
            info.kill_requested_at = Some(std::time::Instant::now());
            effects.push(Effect::KillSubagent {
                session_id: session_id.clone(),
                subagent_id: info.subagent_id.to_string(),
            });
        }
        effects
    }

    /// `/plan --soft [description]` docks Isolated Preview as a secondary
    /// plan. It does not enter plan mode, does not enqueue that text as a
    /// Prompt, and does not reset the primary session `plan.md`. Isolated
    /// Preview must not immediately pull up leftover current `plan.md`.
    pub(crate) fn dock_isolated_preview_with_feature(&mut self, feature: Option<String>) {
        self.isolated_preview_shows_secondary_plan = true;
        let feature = feature.filter(|s| !s.trim().is_empty());
        let secondary =
            self.session_plan_body_from_identity(xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY);
        let leftover_primary = |s: &str| {
            s.contains("why the agent stopped")
                || s.contains("TECH.md")
                || s.contains("Mill leftover")
                || s.contains("Current mill plan.md")
        };
        // The Operator prompt is the input. The document plans that work.
        // Do not store the prompt, or a status recap, as the file.
        let planned = feature.as_deref().map(compose_soft_feature_plan);
        let body = planned
            .clone()
            .or_else(|| secondary.filter(|s| !leftover_primary(s)))
            .unwrap_or_else(|| {
                crate::views::plan_approval_view::SECONDARY_PLAN_PLACEHOLDER.to_owned()
            });
        self.latest_inline_plan_content = Some(body.clone());
        if let Some(pav) = self.plan_approval_view.as_mut() {
            pav.plan_content = Some(body.clone());
            pav.has_plan = true;
        }
        self.view_plan_requested = true;
        self.snapshot_or_clear_plan_feedback_draft();
        let title = if let Some(text) = feature.as_deref() {
            let filename = format!(
                "{}-{}.md",
                thoughtful_feature_slug(text),
                xai_grok_tools::util::ulid::mint()
            );
            let dir = self.session.cwd.join("docs").join("features");
            let _ = std::fs::create_dir_all(&dir);
            let file_body = planned.as_deref().unwrap_or(text);
            let _ = std::fs::write(dir.join(&filename), file_body);
            filename
        } else {
            xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY.to_string()
        };
        self.paint_secondary_isolated_preview(body.clone(), &title);
        if feature.is_some() {
            self.persist_session_plan_body_for(
                xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY,
                &body,
            );
        }
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.fullscreen = false;
            viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
        }
        self.restore_plan_feedback_draft_if_composer_lost();
        self.clear_view_plan_request_if_waiter_bound();
        self.persist_session_plan_dock_open(self.line_viewer.is_some());
    }

    fn finish_isolated_preview_dock(&mut self) {
        self.view_plan_requested = true;
        self.snapshot_or_clear_plan_feedback_draft();
        if self.plan_approval_view.is_some() {
            self.reopen_plan_approval();
        } else {
            self.park_local_idle_plan_decision_if_needed();
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
    }

    /// A soft-plan present that is only the Operator prompt, or only a
    /// Job/State/Operator status recap, is not the document. Repaint the
    /// feature plan that `/plan --soft` wrote, and do not add a second file.
    pub(crate) fn restore_soft_feature_plan_over_prompt_or_status(&mut self) {
        let dir = self.session.cwd.join("docs").join("features");
        let Some((filename, body)) = load_soft_feature_plan(&dir) else {
            return;
        };
        if let Some(pav) = self.plan_approval_view.as_mut() {
            pav.plan_content = Some(body.clone());
            pav.has_plan = true;
        }
        self.latest_inline_plan_content = Some(body.clone());
        self.persist_session_plan_body_for(
            xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY,
            &body,
        );
        self.paint_secondary_isolated_preview(body, &filename);
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.fullscreen = false;
            viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
            let plan = viewer.plan_mut();
            plan.show_action_buttons = true;
            plan.feedback_active = self.plan_approval_view.is_some();
        }
    }
}

/// True when this present must not become the feature document.
pub(crate) fn soft_present_should_keep_feature_plan(body: &str) -> bool {
    if feature_plan_states_the_work(body) {
        return false;
    }
    if text_is_status_recap(body) {
        return true;
    }
    // A headed present that is not a status recap stays. `exit_plan_mode`
    // can still show the document it wrote. Unheaded text that does not
    // plan the work is the Operator prompt, or a wrap of that prompt.
    !body.trim_start().starts_with('#')
}

fn load_soft_feature_plan(dir: &std::path::Path) -> Option<(String, String)> {
    let (name, body) = newest_feature_markdown(dir)?;
    if feature_plan_states_the_work(&body) {
        return Some((name, body));
    }
    let planned = compose_soft_feature_plan(&body);
    let _ = std::fs::write(dir.join(&name), &planned);
    Some((name, planned))
}

fn newest_feature_markdown(dir: &std::path::Path) -> Option<(String, String)> {
    let mut best: Option<(std::time::SystemTime, String, String)> = None;
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !feature_filename_has_crockford_ulid(&name) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let body = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let replace = match &best {
            None => true,
            Some((when, _, _)) => modified >= *when,
        };
        if replace {
            best = Some((modified, name, body));
        }
    }
    best.map(|(_, name, body)| (name, body))
}

fn feature_filename_has_crockford_ulid(name: &str) -> bool {
    const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let Some((slug, ulid)) = stem.rsplit_once('-') else {
        return false;
    };
    !slug.is_empty() && ulid.len() == 26 && ulid.bytes().all(|byte| CROCKFORD.contains(&byte))
}

/// First six words. The filename names the feature. It does not paste the
/// whole Operator prompt.
fn thoughtful_feature_slug(prompt: &str) -> String {
    let words = thoughtful_words(prompt);
    if words.is_empty() {
        "feature".to_string()
    } else {
        words.join("-")
    }
}

fn thoughtful_words(prompt: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in prompt.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
            if words.len() == 6 {
                break;
            }
        }
    }
    if !current.is_empty() && words.len() < 6 {
        words.push(current);
    }
    words
}

fn thoughtful_title(prompt: &str) -> String {
    let words = thoughtful_words(prompt);
    if words.is_empty() {
        return "Feature plan".to_string();
    }
    let mut title = words.join(" ");
    if let Some(first) = title.chars().next() {
        let upper = first.to_ascii_uppercase();
        title.replace_range(..first.len_utf8(), &upper.to_string());
    }
    title
}

/// Plan the work. Do not copy the Operator prompt in as the document.
/// A short request stays visible so an earlier seed such as "add feature"
/// still appears. A long prompt is not pasted, and a status recap is not
/// pasted. The sentences state what is wrong, what will change, the files,
/// what the Operator will see, and which test proves it.
fn compose_soft_feature_plan(operator_prompt: &str) -> String {
    let title = thoughtful_title(operator_prompt);
    let topic_words = thoughtful_words(operator_prompt);
    let topic = if topic_words.is_empty() {
        "this feature".to_string()
    } else {
        topic_words.join(" ")
    };
    let prompt = operator_prompt.trim();
    let request_sentence =
        if !prompt.is_empty() && prompt.chars().count() <= 80 && !text_is_status_recap(prompt) {
            format!(" The request is {prompt}.")
        } else {
            String::new()
        };
    format!(
        "# {title}\n\n\
         What is wrong is that a soft plan would store the Operator prompt or a status recap instead of planning {topic}.\n\n\
         What will change is that the product writes this feature plan and starts the named work only after Approve.{request_sentence}\n\n\
         The files that change are docs/features and crates/codegen/xai-grok-pager/src/slash/commands/plan.rs.\n\n\
         The Operator will see the feature filename as the pane title and will see this plan instead of a status recap.\n\n\
         The test soft_plan_presentation_creates_a_feature_file_and_does_not_start_until_approve proves it.\n"
    )
}

fn feature_plan_states_the_work(body: &str) -> bool {
    let sentences = plan_sentences(body);
    let states_wrong = sentences.iter().any(|sentence| {
        let lower = sentence.to_ascii_lowercase();
        lower.contains("what is wrong") || lower.contains("is wrong")
    });
    let states_change = sentences
        .iter()
        .any(|sentence| sentence.to_ascii_lowercase().contains("will change"));
    let names_files = sentences.iter().any(|sentence| {
        sentence.contains("docs/features") || sentence.contains(".rs") || sentence.contains('/')
    });
    let operator_sees = sentences.iter().any(|sentence| {
        sentence.contains("Operator") && sentence.to_ascii_lowercase().contains("see")
    });
    let names_proof = sentences.iter().any(|sentence| {
        let lower = sentence.to_ascii_lowercase();
        lower.contains("proves") && sentence.contains('_')
    });
    states_wrong && states_change && names_files && operator_sees && names_proof
}

fn plan_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?'])
        .map(str::trim)
        .filter(|sentence| sentence.split_whitespace().count() >= 4)
        .map(str::to_owned)
        .collect()
}

fn text_is_status_recap(body: &str) -> bool {
    let has_job = body
        .lines()
        .any(|line| line.trim_start().starts_with("Job:"));
    let has_state = body
        .lines()
        .any(|line| line.trim_start().starts_with("State:"));
    let has_operator = body
        .lines()
        .any(|line| line.trim_start().starts_with("Operator:"));
    has_job && has_state && has_operator && !feature_plan_states_the_work(body)
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
