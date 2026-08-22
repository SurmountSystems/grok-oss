//! Plan surfaces: plan chip/preview, plan approval + feedback, and casual
//! plan commenting (incl. the casual-commenting test fixture).
use super::AgentView;
#[cfg(test)]
use super::{ActivePane, InputMode, test_fixtures};
#[cfg(test)]
use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::app_view::InputOutcome;
use crate::views::file_search::line_viewer::LineViewerState;
use crate::views::list_pane::ListItem;
use crate::views::plan_approval_view::{
    PlanApprovalFocus, PlanApprovalViewState, PlanComment, PlanPromptIntent, PlanReviewSource,
};
use crate::views::prompt_widget::{EnterOutcome, PromptEvent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// Telemetry for every way a plan review resolves ("build", "abandon",
/// "revise").
fn log_plan_submit(action: &str) {
    use xai_grok_telemetry::events::PlanSubmit;
    use xai_grok_telemetry::session_ctx::log_event;
    log_event(PlanSubmit {
        action: action.to_string(),
    });
}

/// `A` and `?` arrive as the shifted char, with or without the SHIFT modifier
/// depending on the terminal.
fn matches_shifted_char(key: &KeyEvent, ch: char) -> bool {
    key.code == KeyCode::Char(ch)
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
}
impl AgentView {
    /// When plan approval is open, attach a PNG path to the plan composer so
    /// approve / revise / clarify can drain it on the same multimodal path as
    /// a pasted screenshot. Returns true if a chip was inserted.
    ///
    /// No-op when plan approval is not open, the path is not a readable image,
    /// or the prompt rejects the insert (policy / capacity). Callers still
    /// keep the on-disk PNG and toast the path.
    pub(crate) fn try_attach_tui_screenshot_for_plan(&mut self, path: &std::path::Path) -> bool {
        if self.plan_approval_view.is_none() {
            return false;
        }
        let Some(img) = crate::prompt_images::try_read_image_from_path(&path.to_string_lossy())
        else {
            return false;
        };
        self.prompt.insert_image(img).is_ok()
    }

    /// Resolve the absolute path to the plan file for this session.
    fn plan_file_path(&self) -> Option<std::path::PathBuf> {
        let session_id = self.session.session_id.as_ref()?;
        let cwd_str = self.session.cwd.to_string_lossy().into_owned();
        let encoded_cwd = urlencoding::encode(&cwd_str);
        Some(
            xai_grok_shell::util::grok_home::grok_home()
                .join("sessions")
                .join(encoded_cwd.as_ref())
                .join(session_id.0.as_ref())
                .join("plan.md"),
        )
    }
    /// Whether the current line viewer is showing a plan preview.
    pub(super) fn is_plan_viewer(&self) -> bool {
        self.line_viewer.as_ref().is_some_and(|v| {
            v.kind == crate::views::file_search::line_viewer::LineViewerKind::PlanPreview
        })
    }
    /// Whether the user is currently composing a comment via the prompt
    /// input inside the *casual* plan preview (the modal opened with no
    /// `plan_approval_view`). Mirrors the `pav.focus == Commenting`
    /// check used by the plan-approval path so the prompt/footer
    /// behaves identically across both modes.
    pub(super) fn is_casual_commenting(&self) -> bool {
        self.plan_approval_view.is_none()
            && self.is_plan_viewer()
            && self.casual_commenting_range.is_some()
    }
    /// Whether the prompt "auto" (LLM classifier mode) flag should render.
    /// Extracted for unit testing the precedence: auto shows only when the
    /// session is in auto mode and neither yolo (always-approve wins) nor plan
    /// is active.
    pub(super) fn auto_flag_visible(&self, effective_plan: bool) -> bool {
        self.session.is_auto() && !self.session.is_yolo() && !effective_plan
    }

    /// Whether the prompt "context-only" flag should render. Diagnostic: no
    /// tools. Hidden under plan, always-approve, or auto (those win).
    pub(super) fn context_only_flag_visible(&self, effective_plan: bool) -> bool {
        self.session.is_context_only() && !effective_plan
    }

    /// Labels the composer paints for permission/session mode. Plan wins over
    /// always-approve, auto, and context-only. Always-approve and auto win over
    /// context-only.
    #[cfg(test)]
    pub(super) fn composer_permission_flag_labels(
        &self,
        effective_plan: bool,
    ) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if effective_plan {
            labels.push("plan");
        }
        if self.session.is_yolo() && !effective_plan {
            labels.push("always-approve");
        }
        if self.auto_flag_visible(effective_plan) {
            labels.push("auto");
        }
        if self.context_only_flag_visible(effective_plan) {
            labels.push("context-only");
        }
        labels
    }

    /// Whether plan content is available for preview.
    fn plan_preview_available(&self) -> bool {
        self.plan_body_for_preview().is_some()
    }
    /// Whether the "plan" status-bar chip should be rendered.
    ///
    /// Visible while plan mode is active, or always when the user has set
    /// `show_plan_chip = true` in `pager.toml`. Hidden by default once the
    /// user exits plan mode.
    pub(super) fn should_show_plan_chip(
        &self,
        appearance: &crate::appearance::AppearanceConfig,
    ) -> bool {
        (self.plan_mode_active || appearance.show_plan_chip) && self.plan_preview_available()
    }

    /// Effective plan-mode flag for UI decisions (approval park, idle CTAs).
    ///
    /// Matches Shift+Tab / mode dispatch: `plan_mode_pending` wins while a
    /// mode change is in flight. `Some(false)` means leaving plan mode (after
    /// Approve / Quit) even if `plan_mode_active` is still true until the shell
    /// `CurrentModeUpdate` lands, so we must not re-park decision CTAs.
    pub(crate) fn effectively_in_plan_mode(&self) -> bool {
        self.plan_mode_pending.unwrap_or(self.plan_mode_active)
    }

    /// Whether idle / draw / `/view-plan` may arm decision CTAs (Approve strip,
    /// "Plan ready", local idle park).
    ///
    /// Requires effective plan mode, no sticky post-decision suppress, and no
    /// open Revise/Clarify rewrite. After Approve/Quit, `plan_decision_resolved`
    /// stays true until a new `exit_plan_mode` present so `CurrentModeUpdate`
    /// clearing pending while shell plan mode is still on cannot re-park the
    /// same plan. After Revise/Clarify, `plan_feedback_in_flight` blocks idle
    /// "Plan written" chrome until the agent re-presents.
    pub(crate) fn should_arm_plan_decision_chrome(&self) -> bool {
        self.effectively_in_plan_mode()
            && !self.plan_decision_resolved
            && self.plan_feedback_in_flight.is_none()
    }

    /// New `exit_plan_mode` present re-arms decision CTAs after Approve/Quit
    /// and after Revise/Clarify in-flight.
    pub(crate) fn clear_plan_loop_flags_for_new_present(&mut self) {
        self.plan_decision_resolved = false;
        self.plan_feedback_in_flight = None;
        self.persist_plan_decision_resolved_flag(false);
    }

    /// Apply `plan_decision_resolved` from this session's `plan_mode.json`.
    /// New process after Approve/Quit must not re-present Plan ready.
    pub(crate) fn apply_persisted_plan_decision_on_load(&mut self) {
        let Some(sid) = self.session.session_id.as_ref().map(|s| s.0.to_string()) else {
            return;
        };
        let cwd = self.session.cwd.to_string_lossy();
        if xai_grok_shell::session::plan_mode::load_plan_decision_resolved(&cwd, &sid) {
            self.plan_decision_resolved = true;
        }
    }

    fn persist_plan_decision_resolved_flag(&self, resolved: bool) {
        let Some(sid) = self.session.session_id.as_ref().map(|s| s.0.to_string()) else {
            return;
        };
        let cwd = self.session.cwd.to_string_lossy();
        xai_grok_shell::session::plan_mode::persist_plan_decision_resolved(&cwd, &sid, resolved);
    }

    fn grok_oss_store_for_plan_choice(&self) -> Option<xai_grok_shell::grok_oss::GrokOssStore> {
        let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
        xai_grok_shell::grok_oss::try_open_from_token_economy_config(&cfg)
    }

    fn record_explicit_plan_choice(
        &self,
        choice: crate::views::file_search::line_viewer::RecordedPlanChoice,
    ) {
        let Some(sid) = self.session.session_id.as_ref().map(|s| s.0.to_string()) else {
            return;
        };
        let Some(store) = self.grok_oss_store_for_plan_choice() else {
            return;
        };
        let db_choice = match choice {
            crate::views::file_search::line_viewer::RecordedPlanChoice::Approve => {
                xai_grok_shell::grok_oss::PlanRecordedChoice::Approve
            }
            crate::views::file_search::line_viewer::RecordedPlanChoice::Comment => {
                xai_grok_shell::grok_oss::PlanRecordedChoice::Comment
            }
            crate::views::file_search::line_viewer::RecordedPlanChoice::Revise => {
                xai_grok_shell::grok_oss::PlanRecordedChoice::Revise
            }
            crate::views::file_search::line_viewer::RecordedPlanChoice::Exit => {
                xai_grok_shell::grok_oss::PlanRecordedChoice::Exit
            }
        };
        if let Err(e) = store.insert_plan_recorded_choice(
            &sid,
            xai_grok_shell::grok_oss::SESSION_PLAN_IDENTITY,
            db_choice,
        ) {
            tracing::debug!(error = %e, "plan_recorded_choice insert failed (fail-open)");
        }
    }

    fn recorded_plan_choice_for_paint(
        &self,
    ) -> Option<crate::views::file_search::line_viewer::RecordedPlanChoice> {
        let sid = self.session.session_id.as_ref().map(|s| s.0.to_string())?;
        let store = self.grok_oss_store_for_plan_choice()?;
        let row = match store
            .latest_plan_recorded_choice(&sid, xai_grok_shell::grok_oss::SESSION_PLAN_IDENTITY)
        {
            Ok(row) => row?,
            Err(e) => {
                tracing::debug!(error = %e, "plan_recorded_choice load failed (fail-open)");
                return None;
            }
        };
        let paint = match row.choice {
            xai_grok_shell::grok_oss::PlanRecordedChoice::Approve => {
                crate::views::file_search::line_viewer::RecordedPlanChoice::Approve
            }
            xai_grok_shell::grok_oss::PlanRecordedChoice::Comment => {
                crate::views::file_search::line_viewer::RecordedPlanChoice::Comment
            }
            xai_grok_shell::grok_oss::PlanRecordedChoice::Revise => {
                crate::views::file_search::line_viewer::RecordedPlanChoice::Revise
            }
            xai_grok_shell::grok_oss::PlanRecordedChoice::Exit => {
                crate::views::file_search::line_viewer::RecordedPlanChoice::Exit
            }
        };
        match paint {
            crate::views::file_search::line_viewer::RecordedPlanChoice::Approve
            | crate::views::file_search::line_viewer::RecordedPlanChoice::Exit => {
                self.plan_decision_resolved.then_some(paint)
            }
            crate::views::file_search::line_viewer::RecordedPlanChoice::Comment
            | crate::views::file_search::line_viewer::RecordedPlanChoice::Revise => Some(paint),
        }
    }

    /// Status chrome for the plan decision loop.
    ///
    /// Open side panel uses `plan_approval_status_label`. Shut panel does not
    /// paint `PLAN_READY_STATUS`: the composer is send-armed, and Plan ready
    /// would look like a review park that is not on screen. Idle leftover
    /// `plan.md` is view-only until `/view-plan` or a live present docks.
    /// Idle Revise/Clarify wait uses `PLAN_REVISING_STATUS` /
    /// `PLAN_WAITING_UPDATED_STATUS`. Busy rewrite yields `None` so real turn
    /// status can paint. Never returns `PLAN_IDLE_REVIEW_STATUS` while the
    /// panel is shut or feedback is in flight.
    pub(crate) fn plan_loop_status_label(&self) -> Option<&'static str> {
        use crate::views::plan_approval_view::plan_approval_status_label;
        if let Some(ref pav) = self.plan_approval_view {
            if self.line_viewer.is_some() {
                return Some(plan_approval_status_label(pav.has_plan));
            }
            return None;
        }
        if let Some(in_flight) = self.plan_feedback_in_flight {
            if self.session.state.is_turn_running() {
                return None;
            }
            return Some(in_flight.status_label());
        }
        None
    }

    fn inline_plan_content(&self) -> Option<&str> {
        self.plan_approval_view
            .as_ref()
            .filter(|p| p.source == PlanReviewSource::Inline)
            .and_then(|p| p.plan_content.as_deref())
            .filter(|s| !s.trim().is_empty())
    }
    /// Resolve the plan body for the line-viewer preview.
    ///
    /// Prefers content carried on the approval request (inline plan-creation or
    /// the shell-read file body), then falls back to the on-disk plan file.
    /// Request body first keeps file-backed previews working when the path
    /// resolution fails or the file disappears between intercept and open.
    pub(super) fn plan_body_for_preview(&self) -> Option<String> {
        if let Some(content) = self
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.plan_content.as_deref())
            .filter(|s| !s.trim().is_empty())
        {
            return Some(content.to_owned());
        }
        if let Some(content) = self
            .latest_inline_plan_content
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            return Some(content.to_owned());
        }
        self.plan_file_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .filter(|s| !s.trim().is_empty())
    }
    /// `/view-plan` and the plan status / chip click.
    ///
    /// Reopens a live `exit_plan_mode` waiter when one is parked. Does not
    /// invent a second local view-only panel whose Approve leaves the tool
    /// waiting. With no live park, falls through to the saved preview (and
    /// may park a local idle decision when chrome should arm).
    pub(crate) fn open_plan_from_view_plan_or_status(&mut self) {
        if self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.response_tx.is_some() || !p.is_local_idle_decision)
        {
            self.reopen_plan_approval();
            return;
        }
        if self.plan_approval_view.is_some() {
            self.reopen_plan_approval();
            return;
        }
        self.show_plan_preview();
    }

    /// Open the plan preview when content exists, or when plan approval is
    /// parked with an empty body (so the decision surface always pops).
    pub(crate) fn show_plan_preview_if_available(&mut self) {
        if self.plan_preview_available() || self.plan_approval_view.is_some() {
            self.show_plan_preview();
        }
    }
    /// Show the plan in the line viewer overlay or a "no plan" toast.
    ///
    /// When plan approval is parked without a body, opens a placeholder
    /// preview so the user always sees a decision surface (a/s/q) instead of
    /// a dead "Waiting on plan approval" line with a no-op Tab:plan.
    pub fn show_plan_preview(&mut self) {
        // Park before open so feedback_active / footer CTAs arm on this open.
        self.park_local_idle_plan_decision_if_needed();
        let body = self.plan_body_for_preview();
        let approval_empty = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| !p.has_plan);
        let Some(mut viewer) = (if let Some(content) = body {
            LineViewerState::open_markdown_content("plan.md", content, None)
        } else if approval_empty {
            LineViewerState::open_markdown_content(
                "plan.md",
                crate::views::plan_approval_view::EMPTY_PLAN_PLACEHOLDER.to_owned(),
                None,
            )
        } else if let Some(plan_path) = self.plan_file_path() {
            LineViewerState::open_markdown(&plan_path, None)
        } else {
            None
        }) else {
            self.show_toast("No plan written yet.");
            return;
        };
        viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
        viewer.title_override = Some(if approval_empty {
            "plan.md (empty)".to_string()
        } else {
            "plan.md".to_string()
        });
        viewer.fullscreen = crate::appearance::cache::load_plan_approval_force_modal();
        {
            let recorded = self.recorded_plan_choice_for_paint();
            let plan = viewer.plan_mut();
            plan.show_action_buttons = true;
            plan.recorded_choice = recorded;
            // Live park still owns ACP Approve. After Approve/Quit, the four
            // idle CTAs still paint; feedback_active stays false so we do
            // not re-arm Plan ready.
            if self.plan_approval_view.is_none() && !self.should_arm_plan_decision_chrome() {
                plan.feedback_active = false;
            } else {
                plan.feedback_active = self.plan_approval_view.is_some();
            }
        }
        if let Some(ref pav) = self.plan_approval_view
            && !pav.comments.is_empty()
        {
            viewer.rebuild_with_comments(&pav.comments);
        } else if !self.plan_comments.is_empty() {
            viewer.rebuild_with_comments(&self.plan_comments);
        }
        self.line_viewer = Some(viewer);
    }

    /// Park a local idle decision when plan mode is on with a plan body and
    /// there is no `plan_approval_view` yet. Does not invent a second park:
    /// no-op when already parked, when chrome must not arm, or when no body.
    pub(crate) fn park_local_idle_plan_decision_if_needed(&mut self) {
        if self.plan_approval_view.is_some() {
            return;
        }
        if !self.should_arm_plan_decision_chrome() {
            return;
        }
        if !self.plan_preview_available() {
            return;
        }
        let body = self.plan_body_for_preview();
        let stashed = self.prompt.stash();
        let mut pav =
            crate::views::plan_approval_view::PlanApprovalViewState::for_idle_decision(body);
        pav.stashed_prompt = stashed;
        self.plan_approval_view = Some(pav);
    }

    /// Drop leftover plan-approval chrome after a turn ends, but never
    /// stale-cancel a live reverse-request or a local idle park that still
    /// needs a decision.
    pub(crate) fn dismiss_plan_approval_after_turn_if_stale(&mut self) {
        let still_awaiting = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.response_tx.is_some());
        if still_awaiting {
            return;
        }
        let keep_local_idle = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.is_local_idle_decision && self.should_arm_plan_decision_chrome());
        if keep_local_idle {
            return;
        }
        if let Some(mut pav) = self.plan_approval_view.take() {
            let _ = pav.send_stale_cancel();
            self.plan_next_comment_id = pav.next_comment_id;
            self.restore_stashed_prompt_unless_composer_has_text(pav.stashed_prompt);
            self.line_viewer = None;
        }
    }

    /// Keep a visible mid-type draft. Park-time stash is a backup for an
    /// empty composer, not a license to wipe later typing.
    fn restore_stashed_prompt_unless_composer_has_text(
        &mut self,
        stashed: crate::views::prompt_widget::StashedPrompt,
    ) {
        if self.prompt.text().trim().is_empty() {
            self.prompt.restore(stashed);
        }
    }

    /// After a turn ends, leftover `plan.md` is view-only. Do not invent a
    /// local idle park that paints Plan ready while the composer is
    /// Enter:send. `/view-plan` still parks via `show_plan_preview`. Live
    /// `exit_plan_mode` still docks. Resume restore parks a waiter without
    /// docking and without Plan ready chrome.
    pub(crate) fn surface_idle_plan_review_if_needed(&mut self) {
        let _ = self;
    }

    /// Honest toast when a follow-up is queued while Revise/Clarify is in flight.
    pub(crate) fn maybe_toast_plan_feedback_queue(&mut self) {
        if self.plan_feedback_in_flight.is_some() {
            self.show_toast(crate::views::plan_approval_view::PLAN_FEEDBACK_QUEUE_TOAST);
        }
    }

    /// Test fixture: drive the agent into casual-commenting state
    /// (line viewer open in plan-preview mode + `casual_commenting_range`
    /// armed) so the `Event::Paste` plan-feedback arm at ~1539 is
    /// reachable from a unit test without spawning the real
    /// keystroke pipeline. Consolidates three field mutations into
    /// one helper so a future refactor of casual-commenting state
    /// only has to update this fixture rather than every test that
    /// reaches into the fields by name.
    #[cfg(test)]
    pub(crate) fn enter_casual_commenting_for_test(&mut self) {
        let mut viewer =
            crate::views::file_search::line_viewer::LineViewerState::open_markdown_content(
                "test.md",
                "hello\n".to_owned(),
                None,
            )
            .expect("fixture must open the line viewer");
        viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
        self.line_viewer = Some(viewer);
        self.casual_commenting_range = Some(0..1);
    }
    pub(crate) fn approve_plan(&mut self) -> InputOutcome {
        // Image chips insert `[Image #N]` into the buffer. Those tokens are
        // not review notes. Strip them before deciding implement text.
        let notes = self.prompt.text_without_image_chips();
        let notes = notes.trim();
        let images = self.prompt.drain_images();
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        self.record_explicit_plan_choice(
            crate::views::file_search::line_viewer::RecordedPlanChoice::Approve,
        );
        let review_comments = if !pav.comments.is_empty() || !notes.is_empty() {
            let formatted = pav.format_feedback((!notes.is_empty()).then_some(notes));
            if formatted.trim().is_empty() {
                None
            } else {
                Some(format!(
                    "The user approved the plan with the following review comments:\n\n{}",
                    formatted
                ))
            }
        } else {
            None
        };
        let sent_acp = pav.send_approved();
        self.close_plan_review(pav, "build");
        // Idle (no waiter) must start implement. Images-only must not
        // Interject empty text. Live-waiter Approve continues via the
        // shell tool result; notes/images still Interject when present.
        let implement = crate::views::plan_approval_view::PLAN_APPROVED_IMPLEMENT_MESSAGE;
        let text = match (review_comments, sent_acp) {
            (Some(notes), false) => format!("{implement}\n\n{notes}"),
            (Some(notes), true) => notes,
            (None, false) => implement.to_string(),
            (None, true) => String::new(),
        };
        if !text.is_empty() || !images.is_empty() {
            return InputOutcome::Action(Action::Interject { text, images });
        }
        InputOutcome::Changed
    }
    pub(crate) fn abandon_plan(&mut self) -> InputOutcome {
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        self.record_explicit_plan_choice(
            crate::views::file_search::line_viewer::RecordedPlanChoice::Exit,
        );
        pav.send_abandoned();
        self.close_plan_review(pav, "abandon");
        InputOutcome::Changed
    }
    /// Shared teardown for the two plan-review decisions that end the
    /// review (approve and abandon). The shell leaves plan mode as a
    /// result, but its confirming `CurrentModeUpdate("default")` is
    /// fire-and-forget and only arrives after the exit tool runs — so
    /// flip the mode indicator optimistically here (a lost update would
    /// otherwise leave the badge stuck on "plan"), restore the
    /// pre-review UI, and log the decision.
    ///
    /// Not for the revision path (`send_plan_feedback`): the shell
    /// stays in plan mode there, so the indicator must stay on.
    fn close_plan_review(&mut self, pav: PlanApprovalViewState, action: &'static str) {
        self.plan_mode_pending = Some(false);
        // Sticky until a new `exit_plan_mode` present: survives pending clear
        // when the shell still reports plan mode. Persist so rebuild does not
        // re-present leftover plan.md.
        self.plan_decision_resolved = true;
        self.persist_plan_decision_resolved_flag(true);
        self.latest_inline_plan_content = None;
        self.plan_next_comment_id = pav.next_comment_id;
        self.restore_stashed_prompt_unless_composer_has_text(pav.stashed_prompt);
        self.line_viewer = None;
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
        log_plan_submit(action);
    }
    /// Live Revise submit (Enter after notes, or a test that needs the same
    /// path). Footer Revise only arms the box via `focus_plan_prompt`.
    pub(crate) fn send_plan_feedback(&mut self, feedback: Option<String>) -> InputOutcome {
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        self.record_explicit_plan_choice(
            crate::views::file_search::line_viewer::RecordedPlanChoice::Revise,
        );
        let formatted = pav.format_feedback(feedback.as_deref());
        let to_send = if formatted.trim().is_empty() {
            feedback
        } else {
            Some(formatted)
        };
        // Always push a human line so the transcript is not barren after
        // decisive Revise (empty freeform still shows intent).
        let human_line = to_send
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                crate::views::plan_approval_view::PLAN_REVISE_HUMAN_LINE.to_string()
            });
        self.scrollback
            .push_block(crate::scrollback::RenderBlock::user_prompt(human_line));
        let sent_acp = pav.send_cancelled(to_send.clone());
        if pav.source == PlanReviewSource::Inline {
            self.latest_inline_plan_content = None;
        }
        self.plan_next_comment_id = pav.next_comment_id;
        // Drop pre-panel stash: do not restore ghost draft into the busy
        // composer (Enter:queue with leftover text while rewrite runs).
        let _ = pav.stashed_prompt;
        // Drain chips before clearing the composer. `set_text("")` drops
        // the textarea image elements, and a later drain would be empty.
        let images = self.prompt.drain_images();
        self.prompt.set_text("");
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        // Block idle "Plan written" / local idle re-park until re-present.
        self.plan_feedback_in_flight =
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Revising);
        tracing::info!(
            status = self.plan_loop_status_label().unwrap_or(""),
            "plan revise in flight"
        );
        self.show_toast("Plan revision sent.");
        log_plan_submit("revise");
        // Local idle or dead reverse-request channel: Interject so the agent
        // rewrites plan.md and calls exit_plan_mode again (never barren wait).
        // Live ACP still Interjects when the composer holds images so those
        // bytes are not dropped as `images: vec![]`.
        if pav.is_local_idle_decision || !sent_acp || !images.is_empty() {
            let feedback_block = to_send
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n\nOperator feedback:\n{s}"))
                .unwrap_or_default();
            let text = format!(
                "The user requested plan revisions. Update plan.md from the conversation\
                 {feedback_block}\n\nWhen the plan is ready, call exit_plan_mode again to \
                 present it for approval."
            );
            return InputOutcome::Action(Action::Interject { text, images });
        }
        InputOutcome::Changed
    }

    /// Submit a clarifying question (ACP `"questions"`) — not a plan rewrite.
    pub(crate) fn send_plan_questions(&mut self, feedback: Option<String>) -> InputOutcome {
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        let formatted = pav.format_feedback(feedback.as_deref());
        let to_send = if formatted.trim().is_empty() {
            feedback
        } else {
            Some(formatted)
        };
        let human_line = to_send
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if let Some(line) = human_line {
            self.scrollback
                .push_block(crate::scrollback::RenderBlock::user_prompt(line));
        }
        pav.send_questions(to_send.clone());
        if pav.source == PlanReviewSource::Inline {
            self.latest_inline_plan_content = None;
        }
        self.plan_next_comment_id = pav.next_comment_id;
        // Drain chips before restoring the pre-panel stash, same as
        // Approve / Revise: a later drain would be empty.
        let images = self.prompt.drain_images();
        self.prompt.restore(pav.stashed_prompt);
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        // Block idle decision chrome until re-present (same loop as revise).
        self.plan_feedback_in_flight =
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Clarifying);
        tracing::info!(
            status = self.plan_loop_status_label().unwrap_or(""),
            "plan clarify in flight"
        );
        self.show_toast("Clarify sent — answers without rewriting the plan.");
        log_plan_submit("question");
        if !images.is_empty() {
            return InputOutcome::Action(Action::Interject {
                text: to_send.unwrap_or_default(),
                images,
            });
        }
        InputOutcome::Changed
    }

    /// Focus the plan-approval prompt with a specific freeform intent.
    pub(crate) fn focus_plan_prompt(&mut self, intent: PlanPromptIntent) -> InputOutcome {
        if self.plan_approval_view.is_none() {
            return InputOutcome::Changed;
        }
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = intent;
        }
        let recorded = match intent {
            PlanPromptIntent::Comment => {
                Some(crate::views::file_search::line_viewer::RecordedPlanChoice::Comment)
            }
            PlanPromptIntent::Revise => {
                Some(crate::views::file_search::line_viewer::RecordedPlanChoice::Revise)
            }
            PlanPromptIntent::Questions | PlanPromptIntent::ApproveNotes => None,
        };
        if let Some(choice) = recorded {
            self.record_explicit_plan_choice(choice);
        }
        InputOutcome::Changed
    }

    pub(crate) fn reopen_plan_approval(&mut self) {
        let keep_draft = !self.prompt.text().trim().is_empty();
        let live_cursor = self.prompt.cursor();
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.stashed_prompt = self.prompt.stash();
            pav.focus = if keep_draft {
                PlanApprovalFocus::Prompt
            } else {
                PlanApprovalFocus::Preview
            };
        }
        if keep_draft {
            self.prompt.set_cursor(live_cursor);
        } else {
            self.prompt.set_text("");
        }
        self.show_plan_preview_if_available();
        if self.line_viewer.is_none() {
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Prompt;
            }
        } else if let Some(ref mut viewer) = self.line_viewer {
            viewer.plan_mut().feedback_active = true;
        }
    }
    /// Discard an in-progress comment draft: clear the prompt text and
    /// drop the selected line range + pending edit + stashed feedback.
    /// Used whenever focus leaves the prompt without an explicit save
    /// or cancel (e.g. Tab back to Preview, click into the modal).
    fn discard_in_progress_comment(&mut self) {
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.commenting_range = None;
            pav.editing_comment_id = None;
            pav.stashed_feedback_prompt = None;
        }
        self.prompt.set_text("");
    }
    /// Submit the live composer as a normal agent prompt. Does not Approve
    /// or Revise a parked plan.
    pub(super) fn send_composer_as_normal_prompt(&mut self) -> InputOutcome {
        if let Some(text) = self.prompt.try_send() {
            let action = self.prompt_input_mode.send_action(text);
            self.prompt_input_mode = super::PromptInputMode::Normal;
            return InputOutcome::Action(action);
        }
        InputOutcome::Changed
    }

    pub(super) fn handle_plan_feedback_key(&mut self, key: &KeyEvent) -> InputOutcome {
        if crate::input::key::is_paste_key(key) {
            let clipboard_text = crate::app::actions::ClipboardTextRead::from_result(
                crate::clipboard::system_clipboard_read_text(),
            );
            return self.handle_paste_key_deferred(clipboard_text);
        }
        let is_commenting = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
        if crate::input::key::RowWalk::from_key(key).is_some() {
            let focus = self.plan_approval_view.as_ref().map(|p| p.focus);
            match focus {
                Some(PlanApprovalFocus::Prompt) | Some(PlanApprovalFocus::Commenting) => {
                    if self.line_viewer.is_none() {
                        self.show_plan_preview_if_available();
                    }
                    if let Some(ref mut pav) = self.plan_approval_view {
                        pav.focus = PlanApprovalFocus::Preview;
                    }
                    if let Some(ref mut viewer) = self.line_viewer {
                        viewer.plan_mut().feedback_active = true;
                    }
                }
                Some(PlanApprovalFocus::Preview) => {
                    if let Some(ref mut pav) = self.plan_approval_view {
                        pav.focus = PlanApprovalFocus::Prompt;
                        if pav.prompt_intent == PlanPromptIntent::Revise {
                            pav.prompt_intent = PlanPromptIntent::Comment;
                        }
                    }
                }
                None => {}
            }
            if is_commenting {
                self.discard_in_progress_comment();
            }
            return InputOutcome::Changed;
        }
        if key.code == KeyCode::Esc {
            if self.prompt.file_search_visible() {
                self.prompt.file_search.clear_context();
                return InputOutcome::Changed;
            }
            if is_commenting {
                let stashed = if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Preview;
                    pav.editing_comment_id = None;
                    pav.commenting_range = None;
                    pav.stashed_feedback_prompt.take()
                } else {
                    None
                };
                if let Some(stashed) = stashed {
                    self.prompt.restore(stashed);
                } else {
                    self.prompt.set_text("");
                }
                return InputOutcome::Changed;
            }
            // Esc dismisses the plan pane. It does not Approve, Exit, or
            // wipe a mid-compose draft.
            self.cancel_line_viewer();
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Preview;
            }
            return InputOutcome::Changed;
        }
        // Empty-composer Ctrl+C exits plan approval (same outcome as the
        // Exit button). Non-empty falls through so the composer can clear
        // the draft first; a second empty Ctrl+C then abandons.
        if crate::key!('c', CONTROL).matches(key)
            && self.prompt.text().is_empty()
            && self.prompt.images.is_empty()
        {
            return self.abandon_plan();
        }
        // Letter CTA keys (`a` Approve, `A` Notes, `s` Revise, `q` Exit) must
        // type. Approve is the clickable button. `?` is not a letter.
        let empty_prompt =
            self.prompt.text().trim().is_empty() && !self.prompt.file_search_visible();
        if !is_commenting && empty_prompt && matches_shifted_char(key, '?') {
            return self.focus_plan_prompt(PlanPromptIntent::Questions);
        }
        match self.prompt.route_enter(key) {
            EnterOutcome::NewlineInserted => return InputOutcome::Changed,
            EnterOutcome::Submit => {
                let panel_open = self.line_viewer.is_some();
                if is_commenting {
                    if panel_open {
                        return self.save_plan_comment();
                    }
                    // Line-comment save needs the pane. Shut panel: send
                    // the buffer as a normal prompt so Enter cannot wipe it.
                    return self.send_composer_as_normal_prompt();
                }
                let text = self.prompt.text().to_string();
                let has_comments = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| !pav.comments.is_empty());
                let prompt_focused = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Prompt);
                let intent = self
                    .plan_approval_view
                    .as_ref()
                    .map(|pav| pav.prompt_intent)
                    .unwrap_or_default();
                if prompt_focused {
                    if text.trim().is_empty() && !has_comments {
                        let toast = match intent {
                            PlanPromptIntent::Questions => {
                                "Type a question, then press Enter. Click Approve to approve."
                            }
                            PlanPromptIntent::ApproveNotes => {
                                "Type notes, then press Enter. Click Approve to approve without notes."
                            }
                            PlanPromptIntent::Revise => {
                                "Type revision notes, then press Enter. Click Approve to approve."
                            }
                            PlanPromptIntent::Comment => {
                                "Type a comment, then click Approve, Clarify, or Revise."
                            }
                        };
                        self.show_toast(toast);
                        return InputOutcome::Changed;
                    }
                    if !panel_open && !text.trim().is_empty() {
                        return self.send_composer_as_normal_prompt();
                    }
                    let freeform = if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    };
                    return match intent {
                        PlanPromptIntent::Questions => self.send_plan_questions(freeform),
                        PlanPromptIntent::ApproveNotes => self.approve_plan(),
                        PlanPromptIntent::Revise => self.send_plan_feedback(freeform),
                        PlanPromptIntent::Comment => self.send_composer_as_normal_prompt(),
                    };
                }
                return self.send_composer_as_normal_prompt();
            }
            EnterOutcome::PassThrough => {}
        }
        match self.prompt.handle_key(key) {
            PromptEvent::Edited => {
                if let Some(req) = self.prompt.pending_viewer_request.take() {
                    self.open_line_viewer(&req.path, req.initial_range);
                }
                self.persist_unsent_composer_draft();
                InputOutcome::Changed
            }
            PromptEvent::Ignored => InputOutcome::Changed,
        }
    }
    pub(super) fn enter_plan_commenting(&mut self) -> InputOutcome {
        let viewer = match self.line_viewer.as_mut() {
            Some(v) => v,
            None => return InputOutcome::Changed,
        };
        if let Some(vi) = viewer.list_state.selected_index() {
            let pi = viewer.list_state.to_physical(vi);
            if let Some(comment_id) = viewer.lines.get(pi).and_then(|item| item.comment_id())
                && let Some(pav) = self.plan_approval_view.as_mut()
                && let Some(comment) = pav.comments.iter().find(|c| c.id == comment_id)
            {
                let comment_text = comment.text.clone();
                let comment_range = comment.line_range.clone();
                pav.stashed_feedback_prompt = Some(self.prompt.stash());
                pav.editing_comment_id = Some(comment_id);
                pav.commenting_range = Some(comment_range);
                pav.focus = PlanApprovalFocus::Commenting;
                self.prompt.set_text(&comment_text);
                return InputOutcome::Changed;
            }
        }
        let range = viewer.selected_line_range();
        let Some(range) = range else {
            return InputOutcome::Changed;
        };
        if viewer.list_state.visual_mode {
            let start_vi = viewer.list_state.multi_range().map(|r| r.start);
            if let Some(start_vi) = start_vi {
                let start_pi = viewer.list_state.to_physical(start_vi);
                let start_id = viewer.lines.get(start_pi).map(|l| l.stable_id());
                viewer.list_state.exit_visual_mode();
                if let Some(id) = start_id {
                    viewer.list_state.select_by_id(id);
                }
            } else {
                viewer.list_state.exit_visual_mode();
            }
        }
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.stashed_feedback_prompt = Some(self.prompt.stash());
            pav.commenting_range = Some(range);
            pav.editing_comment_id = None;
            pav.focus = PlanApprovalFocus::Commenting;
        }
        self.prompt.set_text("");
        InputOutcome::Changed
    }
    fn save_plan_comment(&mut self) -> InputOutcome {
        let text = self.prompt.text().to_string();
        if text.trim().is_empty() {
            return InputOutcome::Changed;
        }
        let pav = match self.plan_approval_view.as_mut() {
            Some(pav) => pav,
            None => return InputOutcome::Changed,
        };
        let range = match pav.commenting_range.take() {
            Some(r) => r,
            None => return InputOutcome::Changed,
        };
        if let Some(edit_id) = pav.editing_comment_id.take() {
            if let Some(comment) = pav.comments.iter_mut().find(|c| c.id == edit_id) {
                comment.text = text;
                comment.line_range = range;
            }
        } else {
            let id = pav.next_comment_id;
            pav.next_comment_id += 1;
            pav.comments.push(PlanComment {
                id,
                line_range: range,
                text,
            });
        }
        pav.focus = PlanApprovalFocus::Preview;
        let comments = pav.comments.clone();
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.rebuild_with_comments(&comments);
        }
        // Keep chips attached during the comment draft. Restore / set_text
        // would drop them otherwise.
        let kept_images = self.prompt.drain_images();
        if let Some(stashed) = pav.stashed_feedback_prompt.take() {
            self.prompt.restore(stashed);
        } else {
            self.prompt.set_text("");
        }
        for image in kept_images {
            let _ = self.prompt.insert_image(image);
        }
        InputOutcome::Changed
    }
    pub(super) fn delete_plan_comment_at_cursor(&mut self) -> InputOutcome {
        let viewer = match self.line_viewer.as_ref() {
            Some(v) => v,
            None => return InputOutcome::Changed,
        };
        let vi = match viewer.list_state.selected_index() {
            Some(vi) => vi,
            None => return InputOutcome::Changed,
        };
        let pi = viewer.list_state.to_physical(vi);
        let comment_id = match viewer.lines.get(pi).and_then(|item| item.comment_id()) {
            Some(id) => id,
            None => return InputOutcome::Changed,
        };
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.comments.retain(|c| c.id != comment_id);
            let comments = pav.comments.clone();
            if let Some(ref mut viewer) = self.line_viewer {
                viewer.rebuild_with_comments(&comments);
            }
        }
        InputOutcome::Changed
    }
    /// Enter casual commenting mode from the plan preview.
    ///
    /// If the cursor is on a comment line, enter edit mode for that comment.
    /// If the cursor is on a source line, capture the line range and enter
    /// new-comment mode.
    pub(super) fn enter_casual_plan_commenting(&mut self) -> InputOutcome {
        let viewer = match self.line_viewer.as_mut() {
            Some(v) => v,
            None => return InputOutcome::Changed,
        };
        if let Some(vi) = viewer.list_state.selected_index() {
            let pi = viewer.list_state.to_physical(vi);
            if let Some(comment_id) = viewer.lines.get(pi).and_then(|item| item.comment_id())
                && let Some(comment) = self.plan_comments.iter().find(|c| c.id == comment_id)
            {
                let comment_text = comment.text.clone();
                let comment_range = comment.line_range.clone();
                if self.casual_stashed_prompt.is_none() {
                    self.casual_stashed_prompt = Some(self.prompt.stash());
                }
                self.casual_editing_comment_id = Some(comment_id);
                self.casual_commenting_range = Some(comment_range);
                self.prompt.set_text(&comment_text);
                return InputOutcome::Changed;
            }
        }
        let range = viewer.selected_line_range();
        let Some(range) = range else {
            return InputOutcome::Changed;
        };
        if viewer.list_state.visual_mode {
            let start_vi = viewer.list_state.multi_range().map(|r| r.start);
            if let Some(start_vi) = start_vi {
                let start_pi = viewer.list_state.to_physical(start_vi);
                let start_id = viewer.lines.get(start_pi).map(|l| l.stable_id());
                viewer.list_state.exit_visual_mode();
                if let Some(id) = start_id {
                    viewer.list_state.select_by_id(id);
                }
            } else {
                viewer.list_state.exit_visual_mode();
            }
        }
        if self.casual_stashed_prompt.is_none() {
            self.casual_stashed_prompt = Some(self.prompt.stash());
        }
        self.casual_commenting_range = Some(range);
        self.casual_editing_comment_id = None;
        self.prompt.set_text("");
        InputOutcome::Changed
    }
    /// Save the current casual comment (new or edited) and rebuild the viewer.
    pub(super) fn save_casual_plan_comment(&mut self) -> InputOutcome {
        let text = self.prompt.text().to_owned();
        if text.trim().is_empty() {
            return self.cancel_casual_plan_commenting();
        }
        let range = match self.casual_commenting_range.take() {
            Some(r) => r,
            None => return self.cancel_casual_plan_commenting(),
        };
        if let Some(edit_id) = self.casual_editing_comment_id.take() {
            if let Some(comment) = self.plan_comments.iter_mut().find(|c| c.id == edit_id) {
                comment.text = text;
                comment.line_range = range;
            }
        } else {
            let id = self.plan_next_comment_id;
            self.plan_next_comment_id += 1;
            self.plan_comments.push(PlanComment {
                id,
                line_range: range,
                text,
            });
        }
        if let Some(stashed) = self.casual_stashed_prompt.take() {
            self.prompt.restore(stashed);
        } else {
            self.prompt.set_text("");
        }
        let comments = self.plan_comments.clone();
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.rebuild_with_comments(&comments);
        }
        InputOutcome::Changed
    }
    /// Cancel casual plan commenting without saving.
    pub(super) fn cancel_casual_plan_commenting(&mut self) -> InputOutcome {
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
        if let Some(stashed) = self.casual_stashed_prompt.take() {
            self.prompt.restore(stashed);
        } else {
            self.prompt.set_text("");
        }
        InputOutcome::Changed
    }
    /// Key handler used while the user is composing a casual plan
    /// comment via the prompt input. Mirrors `handle_plan_feedback_key`
    /// (which serves the plan-approval Commenting focus) so the UX is
    /// identical: Enter saves, Esc cancels, Tab cancels back to the
    /// modal, and everything else routes to the prompt textarea.
    pub(super) fn handle_casual_plan_feedback_key(&mut self, key: &KeyEvent) -> InputOutcome {
        if key.code == KeyCode::Esc {
            if self.prompt.file_search_visible() {
                self.prompt.file_search.clear_context();
                return InputOutcome::Changed;
            }
            return self.cancel_casual_plan_commenting();
        }
        match self.prompt.route_enter(key) {
            EnterOutcome::NewlineInserted => return InputOutcome::Changed,
            EnterOutcome::Submit => return self.save_casual_plan_comment(),
            EnterOutcome::PassThrough => {}
        }
        if key.code == KeyCode::Tab && key.modifiers.is_empty() {
            return self.cancel_casual_plan_commenting();
        }
        match self.prompt.handle_key(key) {
            PromptEvent::Edited => {
                if let Some(req) = self.prompt.pending_viewer_request.take() {
                    self.open_line_viewer(&req.path, req.initial_range);
                }
                InputOutcome::Changed
            }
            PromptEvent::Ignored => InputOutcome::Changed,
        }
    }
    /// Delete the casual comment under the cursor in the plan preview.
    pub(super) fn delete_casual_plan_comment_at_cursor(&mut self) -> InputOutcome {
        let viewer = match self.line_viewer.as_ref() {
            Some(v) => v,
            None => return InputOutcome::Unchanged,
        };
        let vi = match viewer.list_state.selected_index() {
            Some(vi) => vi,
            None => return InputOutcome::Unchanged,
        };
        let pi = viewer.list_state.to_physical(vi);
        let comment_id = match viewer.lines.get(pi).and_then(|item| item.comment_id()) {
            Some(id) => id,
            None => return InputOutcome::Unchanged,
        };
        self.plan_comments.retain(|c| c.id != comment_id);
        let comments = self.plan_comments.clone();
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.rebuild_with_comments(&comments);
        }
        InputOutcome::Changed
    }
    pub(super) fn send_casual_plan_comments(&mut self) -> InputOutcome {
        if self.plan_comments.is_empty() {
            self.show_toast("No comments to send.");
            return InputOutcome::Changed;
        }
        let plan_content = self.inline_plan_content().map(str::to_owned).or_else(|| {
            let path = self.plan_file_path()?;
            std::fs::read_to_string(path).ok()
        });
        let body = crate::views::plan_approval_view::format_plan_comments(
            &self.plan_comments,
            plan_content.as_deref(),
        );
        let text = format!("Plan feedback:\n\n{body}");
        self.plan_comments.clear();
        self.plan_next_comment_id = 0;
        self.cancel_line_viewer();
        self.show_toast("Plan feedback sent.");
        InputOutcome::Action(Action::SendPrompt(text))
    }
}
#[cfg(test)]
mod prompt_flag_tests {
    use super::test_fixtures::make_agent;
    /// The prompt "auto" (classifier) mode flag shows only when the session is
    /// in Auto and neither yolo (always-approve wins) nor plan is active.
    #[test]
    fn auto_flag_visible_precedence() {
        let mut agent = make_agent();
        assert!(!agent.auto_flag_visible(false));
        agent.session.auto_mode = true;
        assert!(agent.auto_flag_visible(false));
        assert!(!agent.auto_flag_visible(true));
        agent.session.yolo_mode = true;
        assert!(!agent.auto_flag_visible(false));
        agent.session.yolo_mode = false;
        assert!(agent.auto_flag_visible(false));
    }

    /// Composer chrome must show `context-only` when that mode is on (yolo and
    /// auto off, not plan). Switch-in is otherwise toast-only.
    #[test]
    fn composer_permission_flags_include_context_only() {
        let mut agent = make_agent();
        agent.session.context_only_mode = true;
        let labels = agent.composer_permission_flag_labels(false);
        assert!(
            labels.contains(&"context-only"),
            "composer flags must include context-only when that mode is on, got {labels:?}"
        );
        assert!(!labels.contains(&"plan"));
        assert!(!labels.contains(&"always-approve"));
        assert!(!labels.contains(&"auto"));
        assert!(
            !agent
                .composer_permission_flag_labels(true)
                .contains(&"context-only"),
            "plan chrome wins; do not stack context-only under plan"
        );
        agent.session.yolo_mode = true;
        assert!(
            !agent
                .composer_permission_flag_labels(false)
                .contains(&"context-only"),
            "always-approve wins over context-only"
        );
        agent.session.yolo_mode = false;
        agent.session.auto_mode = true;
        assert!(
            !agent
                .composer_permission_flag_labels(false)
                .contains(&"context-only"),
            "auto wins over context-only"
        );
    }
}
#[cfg(test)]
mod plan_chip_tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::agent::{AgentId, AgentSession, AgentState};
    use crate::appearance::AppearanceConfig;
    use crate::scrollback::state::ScrollbackState;
    fn make_agent() -> AgentView {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        AgentView::new(
            AgentSession {
                id: AgentId(0),
                acp_tx: tx,
                session_id: None,
                models: ModelState::default(),
                state: AgentState::Idle,
                tracker: crate::acp::tracker::AcpUpdateTracker::new(),
                cwd: std::path::PathBuf::from("/tmp"),
                is_worktree: false,
                forked_from: None,
                pending_prompts: std::collections::VecDeque::new(),
                next_queue_id: 0,
                yolo_mode: false,
                auto_mode: false,
                context_only_mode: false,
                prompt_history: Vec::new(),
                prompt_history_loading: false,
                loading_replay: false,
                restore_degree: None,
                rate_limited: false,
                model_incompatible: false,
                credit_limit_blocked: false,
                free_usage_blocked: false,
                available_commands: Vec::new(),
                available_commands_generation: 0,
                available_tools: None,
                model_switch_pending: false,
                user_model_preference: None,
                deferred_model_switch: None,
                bg_tasks: std::collections::BTreeMap::new(),
                bg_tool_call_to_task: std::collections::HashMap::new(),
                scheduled_tasks: std::collections::HashMap::new(),
                in_flight_prompt: None,
                compact_held_prompt: None,
                current_prompt_id: None,
                created_via_new: false,
                session_notes: crate::app::agent::SessionNotes::default(),
            },
            ScrollbackState::new(),
        )
    }
    #[test]
    fn plan_chip_hidden_after_exit_by_default() {
        let mut agent = make_agent();
        agent.plan_mode_active = false;
        let appearance = AppearanceConfig::default();
        assert!(!appearance.show_plan_chip);
        assert!(!agent.should_show_plan_chip(&appearance));
    }
    #[test]
    fn plan_chip_visible_while_plan_mode_active() {
        let mut agent = make_agent();
        agent.plan_mode_active = true;
        let appearance = AppearanceConfig::default();
        assert!(!agent.should_show_plan_chip(&appearance));
    }
    #[test]
    fn plan_chip_visible_when_config_overrides() {
        let mut agent = make_agent();
        agent.plan_mode_active = false;
        let appearance = AppearanceConfig {
            show_plan_chip: true,
            ..Default::default()
        };
        assert!(!agent.should_show_plan_chip(&appearance));
    }
    #[test]
    fn set_input_mode_vim_empty_prompt_switches_to_scrollback_and_j_selects_next() {
        crate::appearance::cache::set_simple_mode(true);
        let mut agent = make_agent();
        agent.vim_mode = true;
        agent.set_active_pane(ActivePane::Prompt, true);
        agent.set_input_mode(InputMode::Vim);
        assert_eq!(agent.active_pane, ActivePane::Scrollback);
        assert!(!agent.is_simple_mode());
        let registry = ActionRegistry::defaults();
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let outcome = agent.handle_scrollback_key(&j, &registry);
        assert!(matches!(outcome, InputOutcome::Action(Action::SelectNext)));
    }
    #[test]
    fn set_input_mode_vim_nonempty_prompt_keeps_pane() {
        let mut agent = make_agent();
        agent.set_active_pane(ActivePane::Prompt, true);
        agent.prompt.set_text("draft");
        agent.set_input_mode(InputMode::Vim);
        assert_eq!(agent.active_pane, ActivePane::Prompt);
    }
    #[test]
    fn set_input_mode_simple_from_scrollback_leaves_pane_unchanged() {
        let mut agent = make_agent();
        agent.vim_mode = true;
        agent.set_active_pane(ActivePane::Scrollback, true);
        agent.set_input_mode(InputMode::Simple);
        assert_eq!(agent.active_pane, ActivePane::Scrollback);
        assert!(agent.is_simple_mode());
        let registry = ActionRegistry::defaults();
        let x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let outcome = agent.handle_scrollback_key(&x, &registry);
        assert_eq!(agent.active_pane, ActivePane::Scrollback);
        assert!(matches!(outcome, InputOutcome::Unchanged));
    }
    #[test]
    fn new_agent_respects_persisted_simple_mode_for_mode_and_pane() {
        crate::appearance::cache::set_simple_mode(true);
        let a1 = make_agent();
        assert!(a1.is_simple_mode());
        assert_eq!(a1.active_pane, ActivePane::Prompt);
        crate::appearance::cache::set_simple_mode(false);
        let a2 = make_agent();
        assert!(!a2.is_simple_mode());
        assert_eq!(a2.active_pane, ActivePane::Scrollback);
    }
    #[test]
    fn set_input_mode_reconciles_pane_orthogonal_to_active_modal_field() {
        let mut agent = make_agent();
        agent.set_active_pane(ActivePane::Prompt, true);
        agent.active_modal = None;
        agent.set_input_mode(InputMode::Vim);
        assert_eq!(agent.active_pane, ActivePane::Scrollback);
        assert!(agent.active_modal.is_none());
    }
    #[test]
    fn scrollback_j_with_vim_mode_off_forwards_to_prompt() {
        crate::appearance::cache::set_vim_mode(false);
        let mut agent = make_agent();
        agent.vim_mode = false;
        agent.set_active_pane(ActivePane::Scrollback, true);
        let registry = ActionRegistry::defaults();
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let outcome = agent.handle_scrollback_key(&j, &registry);
        assert!(
            matches!(
                outcome,
                InputOutcome::ActionThenForward(Action::FocusPrompt)
            ),
            "vim-off: bare 'j' in scrollback must forward to prompt; got {outcome:?}"
        );
    }
    #[test]
    fn scrollback_j_with_vim_mode_on_selects_next() {
        crate::appearance::cache::set_vim_mode(true);
        let mut agent = make_agent();
        agent.vim_mode = true;
        agent.set_active_pane(ActivePane::Scrollback, true);
        let registry = ActionRegistry::defaults();
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let outcome = agent.handle_scrollback_key(&j, &registry);
        assert!(
            matches!(outcome, InputOutcome::Action(Action::SelectNext)),
            "vim-on: bare 'j' in scrollback must dispatch SelectNext; got {outcome:?}"
        );
    }
    #[test]
    fn scrollback_arrow_down_works_in_both_modes() {
        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let mut a_off = make_agent();
        a_off.vim_mode = false;
        a_off.set_active_pane(ActivePane::Scrollback, true);
        assert!(matches!(
            a_off.handle_scrollback_key(&down, &registry),
            InputOutcome::Action(Action::SelectNext)
        ));
        let mut a_on = make_agent();
        a_on.vim_mode = true;
        a_on.set_active_pane(ActivePane::Scrollback, true);
        assert!(matches!(
            a_on.handle_scrollback_key(&down, &registry),
            InputOutcome::Action(Action::SelectNext)
        ));
    }
}
#[cfg(test)]
mod plan_approval_enter_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};
    fn enter_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }
    fn agent_with_revise_prompt() -> AgentView {
        let mut agent = make_agent();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-1".into(),
            plan_content: Some("# Plan\n\n## Step 1\nDo something".into()),
        };
        let mut pav = crate::views::plan_approval_view::PlanApprovalViewState::new(
            request,
            crate::views::prompt_widget::StashedPrompt {
                text: String::new(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        );
        pav.focus = PlanApprovalFocus::Prompt;
        agent.plan_approval_view = Some(pav);
        agent.prompt.set_text("");
        agent
    }
    #[test]
    fn empty_enter_on_revise_prompt_does_not_approve() {
        let mut agent = agent_with_revise_prompt();
        let outcome = agent.handle_plan_feedback_key(&enter_key());
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(
            agent.plan_approval_view.is_some(),
            "empty Enter must leave plan approval open"
        );
        assert_eq!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Type revision notes, then press Enter. Click Approve to approve.")
        );
    }
    #[test]
    fn enter_with_revision_text_requests_changes() {
        let mut agent = agent_with_revise_prompt();
        agent.show_plan_preview();
        assert!(
            agent.line_viewer.is_some(),
            "fixture: Revise-on-Enter needs the open decision surface"
        );
        agent.prompt.set_text("please use auth middleware");
        let outcome = agent.handle_plan_feedback_key(&enter_key());
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(agent.plan_approval_view.is_none());
        assert_eq!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }
    #[test]
    fn empty_enter_with_pending_comments_still_requests_changes() {
        let mut agent = agent_with_revise_prompt();
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.comments.push(PlanComment {
                id: 1,
                line_range: 0..1,
                text: "nit".into(),
            });
        }
        let outcome = agent.handle_plan_feedback_key(&enter_key());
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(agent.plan_approval_view.is_none());
        assert_eq!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }
    #[test]
    fn s_on_empty_prompt_decisively_revises() {
        let mut agent = agent_with_revise_prompt();
        let s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&s);
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(
            agent.plan_approval_view.is_some(),
            "letter s must type, not submit empty Revise"
        );
        assert_eq!(agent.prompt.text(), "s");
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }

    #[test]
    fn question_mark_on_empty_prompt_focuses_clarify() {
        let mut agent = agent_with_revise_prompt();
        let q = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&q);
        assert!(matches!(outcome, InputOutcome::Changed));
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("clarify stays parked for typed input");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(pav.prompt_intent, PlanPromptIntent::Questions);
    }

    #[test]
    fn capital_a_on_empty_prompt_focuses_notes() {
        let mut agent = agent_with_revise_prompt();
        let a = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(matches!(outcome, InputOutcome::Changed));
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("capital A must type, not arm Notes");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_ne!(pav.prompt_intent, PlanPromptIntent::ApproveNotes);
        assert_eq!(agent.prompt.text(), "A");
    }

    #[test]
    fn a_on_empty_revise_prompt_approves() {
        let mut agent = agent_with_revise_prompt();
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(
            agent.plan_approval_view.is_some(),
            "letter a must type, not Approve"
        );
        assert_eq!(agent.prompt.text(), "a");
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }
    #[test]
    fn a_with_pending_comments_still_approves() {
        let mut agent = agent_with_revise_prompt();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.comments.push(PlanComment {
                id: 1,
                line_range: 0..1,
                text: "nit".into(),
            });
        }
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(
            agent.plan_approval_view.is_some(),
            "letter a must type even when comments exist"
        );
        assert_eq!(agent.prompt.text(), "a");
        assert!(matches!(outcome, InputOutcome::Changed));
    }

    fn assert_enter_did_not_silently_discard(
        agent: &AgentView,
        outcome: &InputOutcome,
        original: &str,
    ) {
        let composer = agent.prompt.text();
        let comments_have_original = agent
            .plan_approval_view
            .as_ref()
            .is_some_and(|pav| pav.comments.iter().any(|c| c.text.contains(original)))
            || agent
                .plan_comments
                .iter()
                .any(|c| c.text.contains(original));
        let transcript_has_original = agent.scrollback.iter_entries().any(|(_, e)| {
            matches!(
                &e.block,
                crate::scrollback::RenderBlock::UserPrompt(u) if u.text.contains(original)
            )
        });
        let sent_original = match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => text.contains(original),
            InputOutcome::Action(Action::Interject { text, .. }) => text.contains(original),
            InputOutcome::Action(Action::SendPromptNow { text, .. }) => text.contains(original),
            _ => false,
        };
        assert!(
            sent_original
                || composer.contains(original)
                || comments_have_original
                || transcript_has_original,
            "Enter must not delete a non-empty plan-mode composer with no transcript line and no plan comment; outcome={outcome:?} composer={composer:?}"
        );
        if composer.trim().is_empty() {
            assert!(
                sent_original || comments_have_original || transcript_has_original,
                "empty composer after Enter must leave the prompt visible as a send, a comment, or a transcript line"
            );
        }
    }

    /// Named contract: plan mode + shut panel + non-empty composer + Enter
    /// must not vanish the buffer. Send like a normal prompt, or keep the
    /// draft visible. Never silent discard.
    #[test]
    fn plan_mode_enter_on_nonempty_composer_does_not_silently_discard() {
        const DRAFT: &str =
            "archive Telegram Desktop chats from this tree and keep media filenames";
        let mut agent = agent_with_revise_prompt();
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none(), "fixture: plan panel is shut");
        agent.prompt.set_text(DRAFT);
        agent.prompt.set_cursor(DRAFT.len());

        let outcome = agent.handle_input(
            &crossterm::event::Event::Key(enter_key()),
            &ActionRegistry::defaults(),
        );
        assert_enter_did_not_silently_discard(&agent, &outcome, DRAFT);
        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains(DRAFT),
                    "shut panel Enter must send like a normal prompt, got {text:?}"
                );
            }
            other => {
                panic!("plan mode with the panel shut must SendPrompt on Enter, got {other:?}")
            }
        }
        assert!(
            agent.plan_approval_view.is_some(),
            "normal send must not Approve or Revise the parked plan"
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent."),
            "shut panel Enter must not steal the draft as Revise notes"
        );

        let mut typed = agent_with_revise_prompt();
        typed.plan_mode_active = true;
        typed.set_active_pane(ActivePane::Prompt, true);
        for ch in DRAFT.chars() {
            let _ = typed.handle_input(
                &crossterm::event::Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
                &ActionRegistry::defaults(),
            );
        }
        let typed_outcome = typed.handle_input(
            &crossterm::event::Event::Key(enter_key()),
            &ActionRegistry::defaults(),
        );
        assert_enter_did_not_silently_discard(&typed, &typed_outcome, DRAFT);
        match typed_outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains(DRAFT),
                    "typed Enter must SendPrompt, got {text:?}"
                );
            }
            other => panic!("typed Enter with a shut panel must SendPrompt, got {other:?}"),
        }

        let mut no_park = make_agent();
        no_park.plan_mode_active = true;
        no_park.set_active_pane(ActivePane::Prompt, true);
        no_park.prompt.set_text(DRAFT);
        let no_park_outcome = no_park.handle_input(
            &crossterm::event::Event::Key(enter_key()),
            &ActionRegistry::defaults(),
        );
        assert_enter_did_not_silently_discard(&no_park, &no_park_outcome, DRAFT);
        match no_park_outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(text.contains(DRAFT));
            }
            other => panic!("plan mode without a panel must SendPrompt, got {other:?}"),
        }

        // Direct plan-feedback Enter with the panel shut must not Revise.
        let mut feedback = agent_with_revise_prompt();
        feedback.plan_mode_active = true;
        feedback.set_active_pane(ActivePane::Prompt, true);
        feedback.prompt.set_text(DRAFT);
        assert!(feedback.line_viewer.is_none());
        let feedback_outcome = feedback.handle_plan_feedback_key(&enter_key());
        assert_enter_did_not_silently_discard(&feedback, &feedback_outcome, DRAFT);
        match feedback_outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains(DRAFT),
                    "shut panel plan-feedback Enter must send like a normal prompt, got {text:?}"
                );
            }
            other => panic!(
                "shut panel plan-feedback Enter must SendPrompt, not Revise; got {other:?} toast={:?}",
                feedback.toast.as_ref().map(|(m, _)| m.as_str())
            ),
        }
        assert!(
            feedback.plan_approval_view.is_some(),
            "shut panel Enter must leave the parked plan"
        );
        assert_ne!(
            feedback.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }

    /// Idle Comment hub: a non-empty composer is comment/send, not Approve.
    #[test]
    fn idle_plan_comment_intent_nonempty_enter_is_send_not_approve() {
        const DRAFT: &str = "please keep the join order from the archive index";
        let mut agent = agent_with_revise_prompt();
        agent.plan_mode_active = true;
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.prompt_intent = PlanPromptIntent::Comment;
            pav.focus = PlanApprovalFocus::Prompt;
        }
        agent.prompt.set_text(DRAFT);
        let outcome = agent.handle_plan_feedback_key(&enter_key());
        assert_enter_did_not_silently_discard(&agent, &outcome, DRAFT);
        assert!(
            agent.plan_approval_view.is_some(),
            "Comment Enter must not Approve"
        );
        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains(DRAFT),
                    "Comment Enter must send, got {text:?}"
                );
            }
            InputOutcome::Changed if agent.prompt.text().contains(DRAFT) => {}
            other => panic!(
                "Comment Enter must send or keep the draft, got {other:?} composer={:?}",
                agent.prompt.text()
            ),
        }
    }
}
/// The mode indicator renders
/// `plan_mode_pending.unwrap_or(plan_mode_active)`, and the shell's
/// confirming `CurrentModeUpdate("default")` only arrives after the exit
/// tool runs (and can be lost entirely). Resolving the review with a
/// decision must therefore optimistically clear the effective plan mode
/// on BOTH decision paths — approve and abandon.
#[cfg(test)]
mod plan_approval_optimistic_mode_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use agent_client_protocol as acp;
    fn agent_in_plan_mode_with_approval() -> (
        AgentView,
        tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>>,
    ) {
        let mut agent = make_agent();
        agent.plan_mode_active = true;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-1".into(),
            plan_content: Some("# Plan\n\n## Step 1\nDo something".into()),
        };
        let pav = crate::views::plan_approval_view::PlanApprovalViewState::new(
            request,
            agent.prompt.stash(),
            tx,
        );
        agent.plan_approval_view = Some(pav);
        (agent, rx)
    }
    fn effective_plan_mode(agent: &AgentView) -> bool {
        agent.plan_mode_pending.unwrap_or(agent.plan_mode_active)
    }
    #[test]
    fn approve_plan_optimistically_clears_plan_mode() {
        let (mut agent, mut rx) = agent_in_plan_mode_with_approval();
        assert!(effective_plan_mode(&agent));
        agent.approve_plan();
        assert_eq!(agent.plan_mode_pending, Some(false));
        assert!(
            !effective_plan_mode(&agent),
            "indicator must leave plan mode immediately on approve, \
             not wait for the shell's CurrentModeUpdate"
        );
        let raw = rx
            .try_recv()
            .expect("approval response must be sent")
            .expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).unwrap();
        assert_eq!(parsed["outcome"], "approved");
    }
    /// Approve with review comments takes the early `Action::Interject`
    /// return — the optimistic clear must happen before that branch.
    #[test]
    fn approve_plan_with_comments_still_clears_plan_mode() {
        let (mut agent, _rx) = agent_in_plan_mode_with_approval();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.comments
                .push(crate::views::plan_approval_view::PlanComment {
                    id: 1,
                    line_range: 1..2,
                    text: "use the existing helper".into(),
                });
        }
        let outcome = agent.approve_plan();
        assert!(matches!(
            outcome,
            InputOutcome::Action(Action::Interject { .. })
        ));
        assert_eq!(agent.plan_mode_pending, Some(false));
        assert!(!effective_plan_mode(&agent));
    }
    #[test]
    fn abandon_plan_optimistically_clears_plan_mode() {
        let (mut agent, _rx) = agent_in_plan_mode_with_approval();
        agent.abandon_plan();
        assert_eq!(agent.plan_mode_pending, Some(false));
        assert!(!effective_plan_mode(&agent));
    }
}

/// Empty-composer Ctrl+C abandons plan approval (same as panel `q` / Quit).
/// Restored from origin/main catalog names after the 1.0.3 restack dropped them.
#[cfg(test)]
mod plan_approval_ctrl_c_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::views::prompt_widget::StashedPrompt;
    use agent_client_protocol as acp;
    use crossterm::event::Event;
    use xai_acp_lib::AcpResult;

    fn install_plan_approval(
        agent: &mut AgentView,
        plan_content: &str,
    ) -> tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-ctrl-c-abandon".into(),
            plan_content: Some(plan_content.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            StashedPrompt {
                text: "original chat".into(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        ));
        rx
    }

    fn assert_abandoned(mut rx: tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>>) {
        let resp = rx.try_recv().expect("abandon response");
        let raw = resp.expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(
            parsed["outcome"], "abandoned",
            "must abandon like panel q / Quit; got {parsed:?}"
        );
    }

    /// Named contract: empty-composer Ctrl+C while plan approval is soft-parked
    /// must quit plan approval (same outcome as soft-park mouse Quit / panel `q`),
    /// not swallow as a no-op.
    #[test]
    fn soft_park_empty_ctrl_c_abandons_plan_approval() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park Ctrl+C quit");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
            pav.stashed_prompt = StashedPrompt::default();
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none(), "soft-park has no side panel");

        let registry = ActionRegistry::defaults();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let outcome = agent.handle_input(&Event::Key(ctrl_c), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "empty Ctrl+C must be consumed as plan quit; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "empty Ctrl+C must clear plan_approval_view (not soft-park no-op)"
        );
        assert!(
            agent.plan_decision_resolved,
            "Ctrl+C abandon must set the same sticky as q / Quit"
        );
        assert_abandoned(rx);
    }

    /// Empty Ctrl+C with plan side panel open (Preview) must also abandon —
    /// the panel path used to return Changed and swallow the chord.
    #[test]
    fn plan_panel_empty_ctrl_c_abandons_plan_approval() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Panel Ctrl+C quit");
        agent.show_plan_preview();
        assert!(agent.line_viewer.is_some(), "panel requires line_viewer");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);

        let registry = ActionRegistry::defaults();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let outcome = agent.handle_input(&Event::Key(ctrl_c), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "panel empty Ctrl+C must abandon; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "panel empty Ctrl+C must clear plan approval"
        );
        assert!(
            agent.plan_decision_resolved,
            "panel Ctrl+C abandon must set the same sticky as q / Quit"
        );
        assert_abandoned(rx);
    }

    /// Non-empty plan composer: Ctrl+C clears draft first (composer contract),
    /// keeps plan approval open. Second empty Ctrl+C then abandons.
    #[test]
    fn plan_approval_ctrl_c_clears_draft_then_second_abandons() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Ctrl+C clear then quit");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.stashed_prompt = StashedPrompt::default();
        }
        agent.prompt.set_text("draft notes");
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none());

        let registry = ActionRegistry::defaults();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let first = agent.handle_input(&Event::Key(ctrl_c), &registry);
        assert!(
            matches!(first, InputOutcome::Changed),
            "first Ctrl+C with draft must clear; got {first:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "first Ctrl+C must not abandon while draft existed"
        );
        assert!(
            !agent.plan_decision_resolved,
            "first Ctrl+C must not mark plan decided while draft existed"
        );
        assert!(
            agent.prompt.text().is_empty(),
            "first Ctrl+C must clear composer draft"
        );

        let second = agent.handle_input(&Event::Key(ctrl_c), &registry);
        assert!(
            matches!(second, InputOutcome::Changed | InputOutcome::Action(_)),
            "second empty Ctrl+C must abandon; got {second:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert!(
            agent.plan_decision_resolved,
            "second Ctrl+C abandon must set the same sticky as q / Quit"
        );
        assert_abandoned(rx);
    }
}

/// G1 / plan-pane letter-A: CTA accelerators must not eat letters while the
/// composer or plan box can receive text. Empty `a` Approves is superseded.
#[cfg(test)]
mod plan_pane_letter_a_contract_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::app::app_view::InputOutcome;
    use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};
    use crate::views::prompt_widget::StashedPrompt;
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;

    fn install_parked_plan(agent: &mut AgentView, plan_content: &str) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-letter-a".into(),
            plan_content: Some(plan_content.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            StashedPrompt::default(),
            tx,
        ));
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
    }

    fn type_key(agent: &mut AgentView, key: KeyEvent) -> InputOutcome {
        agent.handle_input(&Event::Key(key), &ActionRegistry::defaults())
    }

    fn type_chars(agent: &mut AgentView, text: &str) {
        for ch in text.chars() {
            let modifiers = if ch.is_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            let _ = type_key(agent, KeyEvent::new(KeyCode::Char(ch), modifiers));
        }
    }

    /// Empty main composer while plan review is parked: `also` must type.
    /// Empty-prompt `a` must not Approve.
    #[test]
    fn plan_prompt_letter_a_inserts_when_composing() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nType also");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.prompt.text().is_empty());

        type_chars(&mut agent, "also");

        assert!(
            agent.plan_approval_view.is_some(),
            "letter a must type into the composer, not Approve"
        );
        assert!(
            !agent.plan_decision_resolved,
            "typing also must not decide the plan"
        );
        assert_eq!(
            agent.prompt.text(),
            "also",
            "the operator must be able to type also into the main prompt, got {:?}",
            agent.prompt.text()
        );

        // Same contract in the plan pane box (revise composer).
        let mut box_agent = make_agent();
        install_parked_plan(&mut box_agent, "# Plan\n\nType also in box");
        if let Some(ref mut pav) = box_agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        type_chars(&mut box_agent, "also");
        assert!(
            box_agent.plan_approval_view.is_some(),
            "letter a in the plan box must type, not Approve"
        );
        assert_eq!(
            box_agent.prompt.text(),
            "also",
            "the operator must be able to type also into the plan pane box, got {:?}",
            box_agent.prompt.text()
        );
    }

    /// Capital A is not Notes. `Also` must type into the main prompt.
    #[test]
    fn plan_prompt_capital_a_inserts_also() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nType Also");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }

        type_chars(&mut agent, "Also");

        assert!(
            agent.plan_approval_view.is_some(),
            "capital A must type, not arm Notes"
        );
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_ne!(
            pav.prompt_intent,
            PlanPromptIntent::ApproveNotes,
            "capital A must not switch the box to Notes"
        );
        assert_eq!(
            agent.prompt.text(),
            "Also",
            "the operator must be able to type Also into the main prompt, got {:?}",
            agent.prompt.text()
        );

        let mut box_agent = make_agent();
        install_parked_plan(&mut box_agent, "# Plan\n\nType Also in box");
        if let Some(ref mut pav) = box_agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        type_chars(&mut box_agent, "Also");
        assert!(
            box_agent.plan_approval_view.is_some(),
            "capital A in the plan box must type, not arm Notes"
        );
        assert_eq!(
            box_agent.prompt.text(),
            "Also",
            "the operator must be able to type Also into the plan pane box, got {:?}",
            box_agent.prompt.text()
        );
    }

    /// Plan box (revise composer): Ctrl+Enter must not wipe an unsent buffer
    /// and must not submit that buffer unless the product gesture is send.
    /// Ctrl+Enter is not the plan-box send gesture.
    #[test]
    fn plan_box_ctrl_enter_does_not_wipe_unsent() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nRevise box");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("unsent notes");
        agent.prompt.set_cursor(12);

        let outcome = type_key(
            &mut agent,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        );
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Ctrl+Enter must be handled; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Ctrl+Enter must not submit the unsent revise buffer"
        );
        assert!(
            agent.prompt.text().contains("unsent notes"),
            "Ctrl+Enter must not wipe the unsent buffer, got {:?}",
            agent.prompt.text()
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent."),
            "Ctrl+Enter is not the plan-box send gesture"
        );

        // Commenting box (Add a comment on this line): same no-wipe rule.
        let mut comment_agent = make_agent();
        install_parked_plan(&mut comment_agent, "# Plan\n\nComment box");
        comment_agent.show_plan_preview();
        if let Some(ref mut pav) = comment_agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Commenting;
            pav.commenting_range = Some(0..1);
        }
        comment_agent.prompt.set_text("unsent comment");
        comment_agent.prompt.set_cursor(14);
        let _ = type_key(
            &mut comment_agent,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        );
        assert!(
            comment_agent.plan_approval_view.is_some(),
            "Ctrl+Enter must not submit the unsent comment buffer"
        );
        assert!(
            comment_agent.prompt.text().contains("unsent comment"),
            "Ctrl+Enter must not wipe the comment box, got {:?}",
            comment_agent.prompt.text()
        );
    }

    /// Plan box (revise composer): Shift+Enter must not wipe an unsent buffer.
    /// Newline is the real gesture; the unsent text stays.
    #[test]
    fn plan_box_shift_enter_does_not_wipe_unsent() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nRevise box");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("unsent notes");
        agent.prompt.set_cursor(12);

        let outcome = type_key(
            &mut agent,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        );
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Shift+Enter must be handled; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Shift+Enter must not submit the unsent revise buffer"
        );
        assert!(
            agent.prompt.text().contains("unsent notes"),
            "Shift+Enter must not wipe the unsent buffer, got {:?}",
            agent.prompt.text()
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent."),
            "Shift+Enter is newline, not send"
        );

        let mut comment_agent = make_agent();
        install_parked_plan(&mut comment_agent, "# Plan\n\nComment box");
        comment_agent.show_plan_preview();
        if let Some(ref mut pav) = comment_agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Commenting;
            pav.commenting_range = Some(0..1);
        }
        comment_agent.prompt.set_text("unsent comment");
        comment_agent.prompt.set_cursor(14);
        let _ = type_key(
            &mut comment_agent,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        );
        assert!(
            comment_agent.plan_approval_view.is_some(),
            "Shift+Enter must not submit the unsent comment buffer"
        );
        assert!(
            comment_agent.prompt.text().contains("unsent comment"),
            "Shift+Enter must not wipe the comment box, got {:?}",
            comment_agent.prompt.text()
        );
    }

    /// Footer Revise (and `s` if kept) arms the revise box and waits.
    /// It must not submit empty revise.
    #[test]
    fn revise_cta_arms_composer_does_not_submit_empty() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nRevise CTA");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        {
            let viewer = agent.line_viewer.as_mut().expect("plan pane open");
            viewer.plan_mut().send_button_area = Some(Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Revise click must be handled; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Revise must not submit empty revise"
        );
        assert!(
            !agent.plan_decision_resolved,
            "empty Revise must not resolve the plan"
        );
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_eq!(
            pav.focus,
            PlanApprovalFocus::Prompt,
            "Revise must focus the plan box"
        );
        assert_eq!(
            pav.prompt_intent,
            PlanPromptIntent::Revise,
            "Revise must arm revise mode and wait"
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent."),
            "empty Revise must not pretend text was sent"
        );
    }

    /// Idle Comment focuses the plan prompt as the comment composer.
    #[test]
    fn comment_cta_focuses_comment_composer() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nComment CTA");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }
        {
            let viewer = agent.line_viewer.as_mut().expect("plan pane open");
            viewer.plan_mut().comment_button_area = Some(Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Comment click must be handled; got {outcome:?}"
        );
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("Comment must leave review parked");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(pav.prompt_intent, PlanPromptIntent::Comment);
        assert!(!agent.plan_decision_resolved);
    }

    /// Bare Approve with a live waiter sends ACP `"approved"` so the shell
    /// can continue the same turn with the implement-facing tool result.
    /// It must not Interject the present-only "do not implement" body.
    #[test]
    fn bare_approve_with_live_waiter_sends_approved() {
        let mut agent = make_agent();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-live-approve".into(),
            plan_content: Some("# Plan\n\nBare Approve".into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            StashedPrompt::default(),
            tx,
        ));
        let outcome = agent.approve_plan();
        assert!(
            agent.plan_approval_view.is_none(),
            "bare Approve must close the review"
        );
        assert!(agent.plan_decision_resolved);
        match outcome {
            InputOutcome::Changed => {}
            InputOutcome::Action(Action::Interject { ref text, .. }) => {
                assert!(
                    !text.contains("do not implement yet")
                        && !text.contains("NOT operator approval"),
                    "live-waiter Approve must not Interject present-only text: {text:?}"
                );
            }
            other => panic!("bare live-waiter Approve must send approved; got {other:?}"),
        }
        let raw = rx
            .try_recv()
            .expect("bare Approve must send ACP approved")
            .expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).unwrap();
        assert_eq!(parsed["outcome"], "approved");
    }

    /// Local idle Approve (no live waiter) must start an implement turn.
    #[test]
    fn bare_idle_approve_interjects_implement_message() {
        let mut agent = make_agent();
        agent.plan_approval_view = Some(PlanApprovalViewState::for_idle_decision(Some(
            "# Plan\n\nIdle Approve".into(),
        )));
        let outcome = agent.approve_plan();
        assert!(
            agent.plan_approval_view.is_none(),
            "idle Approve must close the review"
        );
        assert!(agent.plan_decision_resolved);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("The user approved the plan. Implement"),
                    "idle Approve must start implement: {text:?}"
                );
                assert!(
                    !text.contains("do not implement yet")
                        && !text.contains("NOT operator approval"),
                    "idle Approve must not use present-only text: {text:?}"
                );
            }
            other => panic!("idle Approve must Interject implement; got {other:?}"),
        }
    }

    /// Idle Approve with images only is still a real Approve. Interject
    /// must carry the implement sentence plus the chips, not empty text.
    #[test]
    fn idle_approve_with_images_interjects_implement_message() {
        let mut agent = make_agent();
        agent.plan_approval_view = Some(PlanApprovalViewState::for_idle_decision(Some(
            "# Plan\n\nIdle Approve with image".into(),
        )));
        agent
            .prompt
            .insert_image(super::test_fixtures::test_pasted_image())
            .expect("fixture image must attach");
        let outcome = agent.approve_plan();
        assert!(
            agent.plan_approval_view.is_none(),
            "idle Approve must close the review"
        );
        assert!(agent.plan_decision_resolved);
        match outcome {
            InputOutcome::Action(Action::Interject { text, images }) => {
                assert!(
                    text.contains("The user approved the plan. Implement"),
                    "idle Approve with images must start implement, not empty Interject: {text:?}"
                );
                assert!(
                    !text.contains("do not implement yet")
                        && !text.contains("NOT operator approval"),
                    "idle Approve must not use present-only text: {text:?}"
                );
                assert_eq!(
                    images.len(),
                    1,
                    "idle Approve must keep the attached image on the implement turn"
                );
            }
            other => {
                panic!("idle Approve with images must Interject implement + images; got {other:?}")
            }
        }
    }

    /// Comment plus Approve implements and sends the typed notes.
    #[test]
    fn comment_plus_approve_implements_with_notes() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nApprove with comment");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Comment;
        }
        agent.prompt.set_text("use the existing helper");
        {
            let viewer = agent.line_viewer.as_mut().expect("plan pane open");
            viewer.plan_mut().approve_button_area = Some(Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            agent.plan_approval_view.is_none(),
            "comment plus Approve must decide the plan"
        );
        assert!(agent.plan_decision_resolved);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("approved the plan") && text.contains("use the existing helper"),
                    "Approve with comment must send the notes; got {text:?}"
                );
            }
            other => panic!("comment plus Approve must Interject notes; got {other:?}"),
        }
    }

    /// Comment plus Clarify is read-only answers, not a rewrite.
    #[test]
    fn comment_plus_clarify_sends_questions_not_rewrite() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nClarify with comment");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Comment;
        }
        agent.prompt.set_text("what about auth?");
        {
            let viewer = agent.line_viewer.as_mut().expect("plan pane open");
            viewer.plan_mut().questions_button_area = Some(Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Clarify click must be handled; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Clarifying)
        );
        assert_eq!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Clarify sent — answers without rewriting the plan.")
        );
    }

    /// Comment plus Revise rewrites the plan with the typed notes.
    #[test]
    fn comment_plus_revise_rewrites_plan() {
        let mut agent = make_agent();
        install_parked_plan(&mut agent, "# Plan\n\nRevise with comment");
        agent.show_plan_preview();
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Comment;
        }
        agent.prompt.set_text("split step two");
        {
            let viewer = agent.line_viewer.as_mut().expect("plan pane open");
            viewer.plan_mut().send_button_area = Some(Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "comment plus Revise must be handled; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "comment plus Revise must send the rewrite"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(crate::views::plan_approval_view::PlanFeedbackInFlight::Revising)
        );
        assert_eq!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
    }
}

/// `/screenshot` / F9 capture auto-attaches the PNG into the plan composer
/// when plan approval is open (same multimodal drain as paste).
#[cfg(test)]
mod tui_screenshot_plan_attach_tests {
    use super::test_fixtures::make_agent;
    use super::*;

    fn install_plan_approval(agent: &mut AgentView, plan_content: &str) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-screenshot".into(),
            plan_content: Some(plan_content.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            agent.prompt.stash(),
            tx,
        ));
    }

    fn test_png_bytes() -> Vec<u8> {
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(8, 8, image::Rgba([128, 64, 32, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("encode png");
        buf
    }

    /// Soft follow-up: `/screenshot` / F9 capture auto-attaches the PNG into
    /// the plan composer when plan approval is open (same multimodal drain as paste).
    #[test]
    fn try_attach_tui_screenshot_for_plan_when_approval_open() {
        let mut agent = make_agent();
        install_plan_approval(&mut agent, "# Plan\n\nAttach shot");
        assert!(agent.plan_approval_view.is_some());
        assert!(
            agent.prompt.images.is_empty(),
            "composer starts without images"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("tui-shot.png");
        std::fs::write(&png_path, test_png_bytes()).unwrap();

        assert!(
            agent.try_attach_tui_screenshot_for_plan(&png_path),
            "must attach readable PNG while plan approval is open"
        );
        assert_eq!(
            agent.prompt.images.len(),
            1,
            "plan composer must hold one image chip for multimodal drain"
        );
        let attached = &agent.prompt.images[0];
        assert_eq!(attached.mime_type, "image/png");
        assert_eq!(
            attached.source_path.as_deref(),
            Some(png_path.as_path()),
            "source_path must be the capture path for preview/display"
        );
    }

    /// Named contract: outside plan approval, capture path is toast-only —
    /// do not invent a chip on the normal chat composer.
    #[test]
    fn try_attach_tui_screenshot_skips_when_no_plan_approval() {
        let mut agent = make_agent();
        assert!(agent.plan_approval_view.is_none());

        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("tui-shot.png");
        std::fs::write(&png_path, test_png_bytes()).unwrap();

        assert!(
            !agent.try_attach_tui_screenshot_for_plan(&png_path),
            "must not attach when plan approval is closed"
        );
        assert!(
            agent.prompt.images.is_empty(),
            "chat composer must stay clean"
        );
    }
}

/// Sticky Approve/Quit + Revising/Clarify in-flight chrome after the 1.0.3
/// restack dropped `plan_decision_resolved` and `plan_feedback_in_flight`.
///
/// These tests name the FORK contract. They do not draw `render.rs` (pause
/// chips still own that file). Status copy is the helper
/// [`AgentView::plan_loop_status_label`], not the live turn-row paint.
#[cfg(test)]
mod plan_sticky_and_revising_chrome_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::app::agent::AgentState;
    use crate::views::plan_approval_view::{
        PLAN_IDLE_REVIEW_STATUS, PLAN_READY_STATUS, PLAN_REVISING_STATUS,
        PLAN_WAITING_UPDATED_STATUS, PlanFeedbackInFlight,
    };

    fn park_exit_plan_mode(agent: &mut AgentView, body: &str) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-1".into(),
            plan_content: Some(body.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            agent.prompt.stash(),
            tx,
        ));
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.show_plan_preview_if_available();
    }

    fn present_new_exit_plan_mode(agent: &mut AgentView, body: &str) {
        // Same sticky clear as `handle_exit_plan_mode` on a new present.
        agent.clear_plan_loop_flags_for_new_present();
        park_exit_plan_mode(agent, body);
    }

    /// After Approve, `CurrentModeUpdate` clearing pending while plan mode
    /// is still active must not re-arm decision chrome.
    #[test]
    fn after_approve_current_mode_clears_pending_still_in_plan_does_not_repark() {
        let mut agent = make_agent();
        park_exit_plan_mode(
            &mut agent,
            "# Workflow\n\nWorkflow status: approved and implemented (2026-08-10)\n",
        );
        agent.latest_inline_plan_content =
            Some("# Workflow\n\nWorkflow status: approved and implemented (2026-08-10)\n".into());

        let _ = agent.approve_plan();
        assert!(agent.plan_approval_view.is_none());
        assert!(
            agent.plan_decision_resolved,
            "approve must set plan_decision_resolved"
        );

        // detect_plan_mode_change: every CurrentModeUpdate clears pending.
        agent.plan_mode_pending = None;
        agent.plan_mode_active = true;
        assert!(
            agent.effectively_in_plan_mode(),
            "fixture: effective plan mode is true after pending clear"
        );
        assert!(
            !agent.should_arm_plan_decision_chrome(),
            "sticky resolved must block Approve / Plan ready re-arm"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS),
            "must not re-arm idle Plan written. Click or /view-plan after Approve"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "must not re-arm Plan ready after Approve"
        );
    }

    /// Disk body that already says the plan was implemented must not re-arm.
    #[test]
    fn approved_and_implemented_plan_body_does_not_repark_after_decide() {
        let mut agent = make_agent();
        park_exit_plan_mode(
            &mut agent,
            "# Done\n\nWorkflow status: approved and implemented\n",
        );
        agent.latest_inline_plan_content =
            Some("# Done\n\nWorkflow status: approved and implemented\n".into());

        let _ = agent.approve_plan();
        agent.plan_mode_pending = None;
        agent.plan_mode_active = true;

        assert!(agent.plan_decision_resolved);
        assert!(
            !agent.should_arm_plan_decision_chrome(),
            "approved-and-implemented body must not re-arm decision CTAs"
        );
    }

    /// Quit is also a decisive close: same sticky, no re-arm.
    #[test]
    fn after_quit_current_mode_clears_pending_still_in_plan_does_not_repark() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Quit me\n\nBody\n");

        let _ = agent.abandon_plan();
        assert!(agent.plan_approval_view.is_none());
        assert!(
            agent.plan_decision_resolved,
            "quit must set plan_decision_resolved"
        );
        agent.plan_mode_pending = None;
        agent.plan_mode_active = true;
        assert!(!agent.should_arm_plan_decision_chrome());
    }

    /// New `exit_plan_mode` present after a prior decide re-arms CTAs.
    #[test]
    fn new_exit_plan_mode_present_clears_decision_resolved_and_parks() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# First plan\n\nDo A\n");
        let _ = agent.approve_plan();
        assert!(agent.plan_decision_resolved);
        assert!(agent.plan_approval_view.is_none());

        present_new_exit_plan_mode(&mut agent, "# Second plan\n\nDo B\n");

        assert!(
            agent.plan_approval_view.is_some(),
            "new present must park decision chrome"
        );
        assert!(
            !agent.plan_decision_resolved,
            "new present must clear sticky resolved"
        );
        assert!(
            agent.plan_feedback_in_flight.is_none(),
            "new present must clear revise/clarify in-flight"
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.plan_ref().is_some_and(|p| p.feedback_active)),
            "new present panel must arm approval CTAs"
        );
    }

    /// After Revise unparks, do not re-arm idle decision chrome while
    /// `plan_feedback_in_flight` is set.
    #[test]
    fn after_revise_in_flight_surface_does_not_rearm_idle_ctas() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Revise then re-present\n\nBody\n");
        agent.latest_inline_plan_content = Some("# Revise then re-present\n\nBody\n".into());

        let _ = agent.send_plan_feedback(None);
        assert!(
            agent.plan_approval_view.is_none(),
            "revise must clear park immediately"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Revising),
            "revise must mark feedback in flight"
        );
        assert!(agent.effectively_in_plan_mode());
        assert!(
            !agent.should_arm_plan_decision_chrome(),
            "in-flight revise must block decision chrome arming"
        );
    }

    /// After Revise, idle status is Revising plan..., not Plan written.
    #[test]
    fn after_revise_status_is_revising_not_plan_written_click_or_view() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Rewrite in flight\n\nBody\n");

        let _ = agent.send_plan_feedback(None);
        assert!(agent.plan_approval_view.is_none());
        assert!(agent.plan_feedback_in_flight.is_some());
        assert_eq!(
            agent.plan_loop_status_label(),
            Some(PLAN_REVISING_STATUS),
            "idle revise-in-flight status must be Revising plan..."
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS)
        );

        // Busy rewrite: helper yields no exclusive Revising chip so real
        // turn status can paint.
        agent.session.state = AgentState::TurnRunning;
        assert!(
            agent.plan_loop_status_label().is_none()
                || agent.plan_loop_status_label() != Some(PLAN_IDLE_REVIEW_STATUS),
            "busy rewrite must not paint idle Plan written. Click or /view-plan"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS)
        );
    }

    /// After Clarify unparks, status is Waiting for updated plan...
    #[test]
    fn after_clarify_status_is_waiting_for_updated_plan() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Clarify me\n\nBody\n");

        let _ = agent.send_plan_questions(Some("what about auth?".into()));
        assert!(agent.plan_approval_view.is_none());
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Clarifying)
        );
        assert_eq!(
            agent.plan_loop_status_label(),
            Some(PLAN_WAITING_UPDATED_STATUS)
        );
        assert!(!agent.should_arm_plan_decision_chrome());
    }

    /// New `exit_plan_mode` present after revise-in-flight clears the flag
    /// and arms CTAs once.
    #[test]
    fn re_present_after_revise_clears_in_flight_and_arms_ctas() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# First draft\n\nA\n");
        let _ = agent.send_plan_feedback(None);
        assert!(agent.plan_feedback_in_flight.is_some());
        assert!(agent.plan_approval_view.is_none());

        present_new_exit_plan_mode(&mut agent, "# Second draft\n\nB\n");

        assert!(
            agent.plan_feedback_in_flight.is_none(),
            "new present must clear revise-in-flight"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "new present must park decision chrome"
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.plan_ref().is_some_and(|p| p.feedback_active)),
            "new present panel must arm approval CTAs"
        );
        assert!(
            agent.should_arm_plan_decision_chrome() || agent.plan_approval_view.is_some(),
            "after re-present, decision surface is live"
        );
    }

    /// `exit_plan_mode` present is review chrome, not an operator Approve.
    #[test]
    fn exit_plan_mode_present_is_not_operator_approve() {
        let mut agent = make_agent();
        present_new_exit_plan_mode(&mut agent, "# Review me\n\nBody\n");
        assert!(
            !agent.plan_decision_resolved,
            "a present must not set plan_decision_resolved"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(
            agent.should_arm_plan_decision_chrome() || agent.plan_approval_view.is_some(),
            "present arms review CTAs; it does not approve"
        );
        assert_eq!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
            "parked present status must be Plan ready. Side panel open"
        );
    }

    /// Status "Side panel open" is only honest when the plan viewer is open.
    #[test]
    fn plan_loop_status_does_not_claim_side_panel_when_viewer_closed() {
        let mut agent = make_agent();
        present_new_exit_plan_mode(&mut agent, "# Review me\n\nBody\n");
        assert!(agent.line_viewer.is_some());
        agent.line_viewer = None;
        assert_ne!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
            "must not say the side panel is open when the pane is closed"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "shut pane must not paint Plan ready while the composer is send-armed"
        );
    }

    /// `/view-plan` after Approve still paints Approve / Comment / Revise / Exit.
    #[test]
    fn view_plan_after_resolved_still_paints_four_idle_ctas() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Done\n\nAlready approved\n");
        let _ = agent.approve_plan();
        assert!(agent.plan_decision_resolved);
        assert!(agent.plan_approval_view.is_none());
        agent.latest_inline_plan_content = Some("# Done\n\nAlready approved\n".into());
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;

        agent.open_plan_from_view_plan_or_status();
        assert!(
            agent.plan_approval_view.is_none(),
            "/view-plan after decide must not invent a third park"
        );
        assert!(
            !agent.should_arm_plan_decision_chrome(),
            "/view-plan after decide must not re-arm Plan ready"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "/view-plan after decide must not paint shut-pane Plan ready"
        );

        let viewer = agent
            .line_viewer
            .as_mut()
            .expect("/view-plan must open the pane");
        let full = ratatui::layout::Rect::new(0, 0, 80, 24);
        let mut buf = ratatui::buffer::Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        crate::views::file_search::line_viewer::render_line_viewer(
            &mut buf,
            full,
            viewer,
            std::path::Path::new("/tmp"),
            &theme,
            0,
        );
        let modal = viewer.last_modal_area.expect("view-plan footer");
        let mut footer = String::new();
        for x in modal.x..modal.x + modal.width {
            footer.push_str(buf[(x, modal.y + modal.height.saturating_sub(1))].symbol());
        }
        let lower = footer.to_ascii_lowercase();
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "/view-plan after resolved must name {needle}; got {footer:?}"
            );
        }
        assert!(
            !lower.contains("c comment") && !lower.contains("y copy plan"),
            "/view-plan after resolved must not stay casual comment+copy; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(plan.approve_button_area.is_some());
        assert!(plan.comment_button_area.is_some());
        assert!(plan.send_button_area.is_some());
        assert!(plan.abandon_button_area.is_some());
    }

    /// Clicking Approve on view-plan after a recorded Approve must not
    /// re-park Plan ready and must not send a second `approved`.
    #[test]
    fn view_plan_approve_click_when_already_decided_does_not_repark() {
        let mut agent = make_agent();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-already-decided".into(),
            plan_content: Some("# Done\n\nBody\n".into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            agent.prompt.stash(),
            tx,
        ));
        agent.plan_mode_active = true;
        agent.show_plan_preview_if_available();
        let first = agent.approve_plan();
        let resp = rx
            .try_recv()
            .expect("first Approve must complete the waiter");
        let raw = resp.expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(parsed["outcome"], "approved");
        assert!(
            !matches!(first, crate::app::app_view::InputOutcome::Action(_)),
            "live-waiter Approve must not Interject a second implement turn; got {first:?}"
        );

        agent.latest_inline_plan_content = Some("# Done\n\nBody\n".into());
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.open_plan_from_view_plan_or_status();
        assert!(agent.plan_decision_resolved);
        assert!(agent.plan_approval_view.is_none());

        {
            let viewer = agent.line_viewer.as_mut().expect("pane open");
            viewer.plan_mut().approve_button_area = Some(ratatui::layout::Rect::new(10, 20, 8, 1));
            viewer.last_modal_area = Some(ratatui::layout::Rect::new(0, 0, 80, 24));
        }
        let click = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            agent.plan_approval_view.is_none(),
            "second Approve must not invent a park"
        );
        assert!(agent.plan_decision_resolved);
        assert!(
            !agent.should_arm_plan_decision_chrome(),
            "second Approve must not re-arm Plan ready"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "second Approve must not paint Plan ready"
        );
        assert!(
            !matches!(outcome, crate::app::app_view::InputOutcome::Action(_)),
            "already-decided Approve must not Interject or send a second approved; got {outcome:?}"
        );
        assert!(agent.line_viewer.is_some(), "view-plan pane stays open");
    }
}

/// Leftover plan-approval chrome after the 1.0.3 restack restore wave:
/// idle local-decision park, honest queue toast, Revise barren-wait landing.
#[cfg(test)]
mod plan_remaining_chrome_leftover_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::views::plan_approval_view::{
        PLAN_FEEDBACK_QUEUE_TOAST, PLAN_REVISE_HUMAN_LINE, PlanFeedbackInFlight,
    };
    use crate::views::prompt_widget::StashedPrompt;

    fn park_exit_plan_mode(agent: &mut AgentView, body: &str) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-leftover".into(),
            plan_content: Some(body.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            StashedPrompt {
                text: "original chat".into(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        ));
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.show_plan_preview_if_available();
    }

    /// When plan mode is idle with a plan body and chrome is not already
    /// armed, park the five-CTA panel (local idle decision, no ACP waiter).
    #[test]
    fn park_local_idle_plan_decision_parks_five_cta_panel() {
        let mut agent = make_agent();
        agent.plan_mode_active = true;
        agent.plan_decision_resolved = false;
        agent.plan_feedback_in_flight = None;
        agent.latest_inline_plan_content = Some("# Idle park\n\nBody\n".into());
        assert!(agent.plan_approval_view.is_none());

        agent.park_local_idle_plan_decision_if_needed();

        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("idle local-decision park must set plan_approval_view");
        assert!(
            pav.is_local_idle_decision,
            "idle park must be a local decision (no reverse-request waiter)"
        );
        assert!(
            pav.has_plan,
            "file-backed / inline body must count as a plan"
        );
        assert!(
            pav.response_tx.is_none(),
            "local idle park has no ACP response channel"
        );

        agent.show_plan_preview();
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.plan_ref().is_some_and(|p| p.feedback_active)),
            "idle park must arm five-CTA approval chrome, not casual view-only"
        );
    }

    /// Sticky Approve / in-flight Revise must not invent a second park.
    #[test]
    fn park_local_idle_plan_decision_skips_when_chrome_must_not_arm() {
        let mut agent = make_agent();
        agent.plan_mode_active = true;
        agent.plan_decision_resolved = true;
        agent.latest_inline_plan_content = Some("# Already decided\n\nBody\n".into());

        agent.park_local_idle_plan_decision_if_needed();
        assert!(
            agent.plan_approval_view.is_none(),
            "must not re-park after a decisive Approve/Quit"
        );

        agent.plan_decision_resolved = false;
        agent.plan_feedback_in_flight = Some(PlanFeedbackInFlight::Revising);
        agent.park_local_idle_plan_decision_if_needed();
        assert!(
            agent.plan_approval_view.is_none(),
            "must not re-park while Revise/Clarify is in flight"
        );
    }

    /// Already-parked live present is a no-op (do not invent a second park).
    #[test]
    fn park_local_idle_plan_decision_skips_when_already_parked() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Already parked\n\nBody\n");
        let before = agent
            .plan_approval_view
            .as_ref()
            .map(|p| p.tool_call_id.clone());

        agent.park_local_idle_plan_decision_if_needed();

        let after = agent
            .plan_approval_view
            .as_ref()
            .map(|p| p.tool_call_id.clone());
        assert_eq!(
            before, after,
            "already-parked live present must keep the same park"
        );
        assert!(
            agent
                .plan_approval_view
                .as_ref()
                .is_some_and(|p| !p.is_local_idle_decision),
            "must not replace a live reverse-request park with local idle"
        );
    }

    /// Second note while rewrite is in flight uses the honest queue toast.
    #[test]
    fn in_flight_followup_shows_plan_feedback_queue_toast() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Queue toast\n\nBody\n");
        let _ = agent.send_plan_feedback(None);
        assert!(agent.plan_feedback_in_flight.is_some());

        agent.maybe_toast_plan_feedback_queue();

        assert_eq!(
            agent.active_toast_message(),
            Some(PLAN_FEEDBACK_QUEUE_TOAST),
            "in-flight follow-up must not pretend it was live Revise/Clarify"
        );
        assert!(
            PLAN_FEEDBACK_QUEUE_TOAST.to_lowercase().contains("queue"),
            "toast must say the note queues; got {PLAN_FEEDBACK_QUEUE_TOAST:?}"
        );
    }

    /// Decisive empty Revise always leaves a human scrollback line.
    #[test]
    fn after_revise_empty_always_pushes_human_scrollback_line() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Empty revise line\n\nBody\n");
        agent.prompt.set_text("");

        let _ = agent.send_plan_feedback(None);

        let human_lines: Vec<String> = agent
            .scrollback
            .iter_entries()
            .filter_map(|(_, e)| match &e.block {
                crate::scrollback::RenderBlock::UserPrompt(u) => Some(u.text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            human_lines
                .iter()
                .any(|t| t.contains(PLAN_REVISE_HUMAN_LINE) || t.to_lowercase().contains("revise")),
            "empty Revise must push a human line; got {human_lines:?}"
        );
        assert!(
            agent.prompt.text().trim().is_empty(),
            "composer must be empty after empty Revise (no Enter:queue ghost draft)"
        );
        assert!(
            !agent.prompt.can_send(),
            "empty composer after Revise must not be sendable (no Enter:queue)"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Revising)
        );
    }

    /// Soft-park Revise with pre-panel stash must not restore ghost draft.
    #[test]
    fn after_revise_clears_composer_no_ghost_stash_draft() {
        let mut agent = make_agent();
        park_exit_plan_mode(&mut agent, "# Stash ghost\n\nBody\n");
        agent.prompt.set_text("rewrite step 2");

        let _ = agent.send_plan_feedback(Some("rewrite step 2".into()));

        assert!(
            agent.prompt.text().trim().is_empty(),
            "must not restore pre-panel draft after Revise; got {:?}",
            agent.prompt.text()
        );
        assert!(!agent.prompt.can_send());
    }
}

/// Rebuild / resume must not auto-dock plan review. Empty-composer Esc
/// dismisses the pane and keeps the waiter. Not Approve, not Exit.
#[cfg(test)]
mod plan_rebuild_resume_and_esc_dismiss_tests {
    use super::test_fixtures::make_agent;
    use super::*;
    use crate::views::plan_approval_view::{PLAN_IDLE_REVIEW_STATUS, PLAN_READY_STATUS};
    use crate::views::prompt_widget::StashedPrompt;
    use agent_client_protocol as acp;
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;
    use xai_acp_lib::AcpResult;

    fn install_live_park(
        agent: &mut AgentView,
        plan_content: &str,
    ) -> tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-esc-dismiss".into(),
            plan_content: Some(plan_content.into()),
        };
        agent.plan_approval_view = Some(PlanApprovalViewState::new(
            request,
            StashedPrompt::default(),
            tx,
        ));
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.show_plan_preview_if_available();
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        rx
    }

    fn type_esc(agent: &mut AgentView) -> InputOutcome {
        agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        )
    }

    #[test]
    fn empty_composer_esc_dismisses_plan_side_panel_keeps_waiter() {
        let mut agent = make_agent();
        let mut rx = install_live_park(&mut agent, "# Esc dismiss\n\nKeep the waiter\n");
        assert!(agent.line_viewer.is_some(), "fixture: pane is open");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }

        let outcome = type_esc(&mut agent);
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Esc must be consumed as dismiss; got {outcome:?}"
        );
        assert!(
            agent.line_viewer.is_none(),
            "empty-composer Esc must close the plan pane"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Esc dismisses the viewer, not the waiter"
        );
        assert!(
            rx.try_recv().is_err(),
            "Esc must not send an ACP plan outcome"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "closed pane + send-armed composer must not paint Plan ready"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS),
            "shut panel must not use the exclusive click cue"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
            "must not say the side panel is open when the pane is closed"
        );
    }

    #[test]
    fn empty_composer_esc_does_not_abandon_or_approve() {
        let mut agent = make_agent();
        let mut rx = install_live_park(&mut agent, "# Esc is not decide\n\nBody\n");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }

        let _ = type_esc(&mut agent);
        assert!(
            !agent.plan_decision_resolved,
            "Esc dismiss is not Approve and not Exit"
        );
        assert!(agent.plan_approval_view.is_some());
        match rx.try_recv() {
            Err(_) => {}
            Ok(Ok(raw)) => {
                let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
                let outcome = parsed["outcome"].as_str().unwrap_or("");
                assert!(
                    outcome != "abandoned" && outcome != "approved",
                    "Esc must not approve or abandon; got {parsed:?}"
                );
            }
            Ok(Err(err)) => panic!("Esc must not fail the waiter: {err:?}"),
        }
    }

    #[test]
    fn plan_close_button_dismisses_pane_when_parked() {
        let mut agent = make_agent();
        let mut rx = install_live_park(&mut agent, "# Close button\n\nBody\n");
        let close = Rect::new(10, 1, 3, 1);
        {
            let viewer = agent
                .line_viewer
                .as_mut()
                .expect("fixture: parked pane is open");
            viewer.close_button_area = Some(close);
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 20));
            viewer.last_popup_area = Some(Rect::new(0, 0, 80, 16));
        }

        let outcome = agent.handle_input(
            &Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: close.x,
                row: close.y,
                modifiers: KeyModifiers::NONE,
            }),
            &ActionRegistry::defaults(),
        );
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "close (x) must be consumed; got {outcome:?}"
        );
        assert!(
            agent.line_viewer.is_none(),
            "close (x) must dismiss the parked plan pane"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "close (x) keeps the waiter"
        );
        assert!(
            !agent.plan_decision_resolved,
            "close (x) is not Approve and not Exit"
        );
        assert!(
            rx.try_recv().is_err(),
            "close (x) must not send an ACP outcome"
        );
    }

    #[test]
    fn commenting_esc_still_steps_back_to_preview() {
        let mut agent = make_agent();
        let mut rx = install_live_park(&mut agent, "# Comment Esc\n\nBody\n");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Commenting;
        }
        agent.prompt.set_text("a line note");

        let first = type_esc(&mut agent);
        assert!(
            matches!(first, InputOutcome::Changed),
            "first Esc from commenting must step back; got {first:?}"
        );
        assert!(
            agent.line_viewer.is_some(),
            "first Esc from commenting must not close the pane"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Preview),
            "first Esc from commenting must return to Preview"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(rx.try_recv().is_err());
    }
}
