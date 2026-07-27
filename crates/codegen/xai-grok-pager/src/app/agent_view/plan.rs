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
    PlanApprovalFocus, PlanComment, PlanPromptIntent, PlanReviewSource,
};
use crate::views::prompt_widget::{EnterOutcome, PromptEvent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
impl AgentView {
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
        // Plan approval opens as a right-hand side panel (option B) so chat
        // stays visible; casual plan preview keeps the full overlay. Force-
        // modal (`plan_approval_park=modal`) upgrades to fullscreen after
        // reopen in `handle_exit_plan_mode`.
        if self.plan_approval_view.is_some() {
            viewer.side_panel = true;
            viewer.fullscreen = false;
        } else {
            viewer.side_panel = false;
            viewer.fullscreen = true;
        }
        {
            let plan = viewer.plan_mut();
            plan.show_action_buttons = self.plan_approval_view.is_none();
            plan.feedback_active = self.plan_approval_view.is_some();
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
        // Flush composer drafts before taking the view so mouse/`a` approve
        // does not swallow an unsaved line comment or freeform note.
        // Mirrors question-view submit_question_answers → swap_question_freeform.
        self.flush_plan_composer_before_approve();
        // Strip image-chip placeholders so screenshots-only approve does not
        // invent freeform notes like "[Image #1]" (images ride separately).
        let freeform = {
            let text = self.prompt.text_without_image_chips();
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        };
        // Drain screenshots before restore — plan-mode attach rides the same
        // Interject path as review notes (P3).
        let images = self.prompt.drain_images();
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        let review_comments = if !pav.comments.is_empty() || freeform.is_some() {
            let formatted = pav.format_feedback(freeform.as_deref());
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
        pav.send_approved();
        self.latest_inline_plan_content = None;
        self.plan_next_comment_id = pav.next_comment_id;
        self.prompt.restore(pav.stashed_prompt);
        self.line_viewer = None;
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
        {
            use xai_grok_telemetry::events::PlanSubmit;
            use xai_grok_telemetry::session_ctx::log_event;
            log_event(PlanSubmit {
                action: "build".to_string(),
            });
        }
        if let Some(text) = review_comments {
            return InputOutcome::Action(Action::Interject { text, images });
        }
        // Screenshots without text notes still ride with approve so the agent
        // sees visual context on the implement turn.
        if !images.is_empty() {
            return InputOutcome::Action(Action::Interject {
                text: "Screenshot(s) attached with plan approval.".to_owned(),
                images,
            });
        }
        // Approve continues implement shell-side (mid-turn or resume). Local
        // pending stays held until that turn ends and the normal turn-end
        // drain runs — do not race DrainQueue against the implement turn.
        InputOutcome::Changed
    }
    /// Commit any in-progress plan-approval composer text into durable state
    /// before Approve takes `plan_approval_view`.
    ///
    /// - **Commenting** + non-empty draft + range → same as `save_plan_comment`
    ///   (pushes into `pav.comments`, restores stashed freeform into prompt).
    /// - **Commenting** with empty draft → drop the draft range and restore
    ///   any stashed freeform so leftover overall notes are not lost.
    /// - **Prompt** freeform is left in the prompt for the caller to read.
    fn flush_plan_composer_before_approve(&mut self) {
        let Some(focus) = self.plan_approval_view.as_ref().map(|p| p.focus) else {
            return;
        };
        if focus != PlanApprovalFocus::Commenting {
            return;
        }
        let has_draft = !self.prompt.text().trim().is_empty()
            && self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.commenting_range.is_some());
        if has_draft {
            let _ = self.save_plan_comment();
            return;
        }
        // Empty Commenting draft: clear commenting state and restore freeform
        // that was stashed when the user entered line-comment mode.
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
            pav.commenting_range = None;
            pav.editing_comment_id = None;
            if let Some(stashed) = pav.stashed_feedback_prompt.take() {
                self.prompt.restore(stashed);
            } else {
                self.prompt.set_text("");
            }
        }
    }
    pub(crate) fn abandon_plan(&mut self) -> InputOutcome {
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        pav.send_abandoned();
        self.plan_mode_pending = Some(false);
        self.latest_inline_plan_content = None;
        self.plan_next_comment_id = pav.next_comment_id;
        self.prompt.restore(pav.stashed_prompt);
        self.line_viewer = None;
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
        {
            use xai_grok_telemetry::events::PlanSubmit;
            use xai_grok_telemetry::session_ctx::log_event;
            log_event(PlanSubmit {
                action: "abandon".to_string(),
            });
        }
        // Abandon leaves no shell implement/revise turn. If we were idle
        // (resume re-park) with local rows held by the plan-approval gate,
        // promote them now as a clean independent next turn.
        if self.session.state.is_idle() && !self.session.pending_prompts.is_empty() {
            InputOutcome::Action(Action::DrainQueue)
        } else {
            InputOutcome::Changed
        }
    }
    /// Capture the plan line selection to attach to revise/clarify feedback.
    ///
    /// Prefers the live line-viewer selection (cursor or visual range), then
    /// any in-progress `commenting_range` if the viewer is already closed.
    fn plan_selection_for_feedback(&self) -> Option<std::ops::Range<usize>> {
        if let Some(range) = self
            .line_viewer
            .as_ref()
            .and_then(|v| v.selected_line_range())
        {
            return Some(range);
        }
        self.plan_approval_view
            .as_ref()
            .and_then(|p| p.commenting_range.clone())
    }

    pub(crate) fn send_plan_feedback(&mut self, feedback: Option<String>) -> InputOutcome {
        let selection = self.plan_selection_for_feedback();
        // Drain screenshots before restore so they ride with revise (P3).
        let images = self.prompt.drain_images();
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        let formatted = pav.format_feedback_with_selection(feedback.as_deref(), selection.as_ref());
        let to_send = if formatted.trim().is_empty() {
            feedback
        } else {
            Some(formatted)
        };
        if crate::app::minimal_mode_active()
            && let Some(msg) = to_send.as_deref().map(str::trim).filter(|s| !s.is_empty())
        {
            self.scrollback
                .push_block(crate::scrollback::RenderBlock::user_prompt(msg.to_string()));
        }
        pav.send_cancelled(to_send);
        if pav.source == PlanReviewSource::Inline {
            self.latest_inline_plan_content = None;
        }
        self.plan_next_comment_id = pav.next_comment_id;
        self.prompt.restore(pav.stashed_prompt);
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        self.show_toast("Plan revision sent.");
        {
            use xai_grok_telemetry::events::PlanSubmit;
            use xai_grok_telemetry::session_ctx::log_event;
            log_event(PlanSubmit {
                action: "revise".to_string(),
            });
        }
        // Text feedback already went over ACP; screenshots ride as multimodal
        // Interject on the same revise turn (same pattern as approve notes).
        if !images.is_empty() {
            return InputOutcome::Action(Action::Interject {
                text: "Screenshot(s) attached for plan feedback.".to_owned(),
                images,
            });
        }
        // Revise continues plan mode shell-side — do not drain held follow-ups
        // into a competing turn.
        InputOutcome::Changed
    }

    /// Submit a clarifying question (ACP `"questions"`) — not a plan rewrite.
    pub(crate) fn send_plan_questions(&mut self, feedback: Option<String>) -> InputOutcome {
        let selection = self.plan_selection_for_feedback();
        // Drain screenshots before restore so they ride with clarify (P3).
        let images = self.prompt.drain_images();
        let Some(mut pav) = self.plan_approval_view.take() else {
            return InputOutcome::Changed;
        };
        let formatted = pav.format_feedback_with_selection(feedback.as_deref(), selection.as_ref());
        let to_send = if formatted.trim().is_empty() {
            feedback
        } else {
            Some(formatted)
        };
        if crate::app::minimal_mode_active()
            && let Some(msg) = to_send.as_deref().map(str::trim).filter(|s| !s.is_empty())
        {
            self.scrollback
                .push_block(crate::scrollback::RenderBlock::user_prompt(msg.to_string()));
        }
        pav.send_questions(to_send);
        if pav.source == PlanReviewSource::Inline {
            self.latest_inline_plan_content = None;
        }
        self.plan_next_comment_id = pav.next_comment_id;
        self.prompt.restore(pav.stashed_prompt);
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        self.show_toast("Clarifying question sent.");
        {
            use xai_grok_telemetry::events::PlanSubmit;
            use xai_grok_telemetry::session_ctx::log_event;
            log_event(PlanSubmit {
                action: "question".to_string(),
            });
        }
        if !images.is_empty() {
            return InputOutcome::Action(Action::Interject {
                text: "Screenshot(s) attached for plan feedback.".to_owned(),
                images,
            });
        }
        // Clarify/questions stay in plan mode shell-side — same queue gate as revise.
        InputOutcome::Changed
    }

    /// Focus the plan-approval prompt with a specific freeform intent.
    pub(crate) fn focus_plan_prompt(&mut self, intent: PlanPromptIntent) -> InputOutcome {
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = intent;
        }
        InputOutcome::Changed
    }
    pub(crate) fn reopen_plan_approval(&mut self) {
        // Engaging the plan surface: dismiss competing overlays so the plan
        // paints and input routes to the line viewer (soft park left them
        // alone). Opens as a right-hand side panel by default (option B).
        self.active_modal = None;
        self.block_viewer = None;
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.stashed_prompt = self.prompt.stash();
            pav.focus = PlanApprovalFocus::Preview;
        }
        self.prompt.set_text("");
        self.show_plan_preview_if_available();
        if self.line_viewer.is_none() {
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Prompt;
            }
        } else if let Some(ref mut viewer) = self.line_viewer {
            viewer.plan_mut().feedback_active = true;
        }
    }

    /// Push a soft-park plan card into the transcript once per `tool_call_id`
    /// (option C). Body is truncated with CTAs; chat stays usable and the side
    /// panel remains on demand via `/view-plan`.
    pub(crate) fn commit_parked_plan_card(&mut self) {
        let Some(pav) = self.plan_approval_view.as_ref() else {
            return;
        };
        let tool_call_id = pav.tool_call_id.clone();
        if self.plan_card_committed_id.as_deref() == Some(tool_call_id.as_str()) {
            return;
        }
        let body =
            crate::views::plan_approval_view::format_parked_plan_card(pav.plan_content.as_deref());
        self.scrollback
            .push_block(crate::scrollback::block::RenderBlock::agent_message(body));
        self.plan_card_committed_id = Some(tool_call_id);
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
    pub(super) fn handle_plan_feedback_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let is_commenting = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
        // Soft-park / card CTAs (no line viewer open): when Preview focus and
        // the prompt is empty, a/A/s/?/q act without opening the side panel.
        // A non-empty draft keeps character input so typing is never stolen.
        let soft_preview = self.line_viewer.is_none()
            && !is_commenting
            && self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.focus == PlanApprovalFocus::Preview)
            && self.prompt.text().trim().is_empty()
            && self.prompt.images.is_empty();
        if soft_preview {
            if key.code == KeyCode::Char('a') && key.modifiers.is_empty() {
                return self.approve_plan();
            }
            if key.code == KeyCode::Char('A')
                && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
            {
                return self.focus_plan_prompt(PlanPromptIntent::ApproveNotes);
            }
            if key.code == KeyCode::Char('s') && key.modifiers.is_empty() {
                return self.focus_plan_prompt(PlanPromptIntent::Revise);
            }
            if key.code == KeyCode::Char('?') && key.modifiers.is_empty() {
                return self.focus_plan_prompt(PlanPromptIntent::Questions);
            }
            if key.code == KeyCode::Char('q') && key.modifiers.is_empty() {
                return self.abandon_plan();
            }
        }
        if key.code == KeyCode::Tab && key.modifiers.is_empty() {
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
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Preview;
            }
            return InputOutcome::Changed;
        }
        match self.prompt.route_enter(key) {
            EnterOutcome::NewlineInserted => return InputOutcome::Changed,
            EnterOutcome::Submit => {
                if is_commenting {
                    return self.save_plan_comment();
                }
                // Ignore image-chip placeholders when judging empty freeform /
                // building text feedback — chips ride as multimodal images.
                let text = self.prompt.text_without_image_chips();
                let has_comments = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| !pav.comments.is_empty());
                let has_images = !self.prompt.images.is_empty();
                let prompt_focused = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Prompt);
                if prompt_focused {
                    // Empty Enter still approves — but screenshots alone (or
                    // comments) mean the user is submitting content under the
                    // current intent, not empty-approve.
                    if text.trim().is_empty() && !has_comments && !has_images {
                        return self.approve_plan();
                    }
                    let freeform = if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    };
                    let intent = self
                        .plan_approval_view
                        .as_ref()
                        .map(|p| p.prompt_intent)
                        .unwrap_or(PlanPromptIntent::Revise);
                    return match intent {
                        PlanPromptIntent::Questions => self.send_plan_questions(freeform),
                        PlanPromptIntent::Revise => self.send_plan_feedback(freeform),
                        // Freeform stays in the prompt; approve_plan folds it
                        // into the approved + notes Interject path.
                        PlanPromptIntent::ApproveNotes => self.approve_plan(),
                    };
                }
                return InputOutcome::Changed;
            }
            EnterOutcome::PassThrough => {}
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
        if let Some(stashed) = pav.stashed_feedback_prompt.take() {
            self.prompt.restore(stashed);
        } else {
            self.prompt.set_text("");
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
        // Flush in-progress casual line comment before send (same swallow bug
        // as plan approve: mouse/`s` previously only sent already-saved list).
        if self.casual_commenting_range.is_some() && !self.prompt.text().trim().is_empty() {
            let _ = self.save_casual_plan_comment();
        }
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
mod approve_plan_flush_tests {
    use super::*;
    use crate::views::plan_approval_view::{
        ExitPlanModeExtRequest, PlanApprovalFocus, PlanApprovalViewState, PlanPromptIntent,
    };
    use crate::views::prompt_widget::StashedPrompt;
    use agent_client_protocol as acp;
    use xai_acp_lib::AcpResult;

    fn make_agent() -> AgentView {
        test_fixtures::make_agent()
    }

    /// Plan approval parked with a response channel so we can assert outcome.
    fn install_plan_approval(
        agent: &mut AgentView,
        plan_content: &str,
    ) -> tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call-approve-flush".into(),
            plan_content: Some(plan_content.into()),
        };
        let view = PlanApprovalViewState::new(
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
        );
        agent.plan_approval_view = Some(view);
        rx
    }

    fn assert_outcome_approved(
        mut rx: tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>>,
    ) {
        let resp = rx
            .try_recv()
            .expect("should receive exit_plan_mode response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "approved");
    }

    /// Approve while Commenting with a non-empty draft must commit the draft
    /// into the approve Interject (not swallow it when taking the view).
    #[test]
    fn approve_plan_flushes_commenting_draft_into_interject() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\n## Step 1\nDo something");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Commenting;
            pav.commenting_range = Some(3..4);
            pav.stashed_feedback_prompt = Some(StashedPrompt {
                text: String::new(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            });
        }
        agent.prompt.set_text("please add tests for this step");

        let outcome = agent.approve_plan();

        assert!(
            agent.plan_approval_view.is_none(),
            "approve must clear plan_approval_view"
        );
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("please add tests for this step"),
                    "Interject must include flushed line-comment draft; got {text:?}"
                );
                assert!(
                    text.contains("approved the plan with the following review comments"),
                    "Interject must use the approve-with-comments framing; got {text:?}"
                );
            }
            other => panic!("expected Interject with flushed draft, got {other:?}"),
        }
        // Original chat restored after approve (not the draft left in composer).
        assert_eq!(agent.prompt.text(), "original chat");
    }

    /// Approve while Prompt has freeform feedback must fold freeform into
    /// the approve Interject (mouse/`a` path; Enter-on-Prompt still revises).
    #[test]
    fn approve_plan_includes_prompt_freeform_in_interject() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nDo the thing");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            // Also keep a previously saved line comment so freeform is
            // "Additional feedback" in format_feedback.
            pav.comments.push(PlanComment {
                id: 0,
                line_range: 1..2,
                text: "saved earlier".into(),
            });
            pav.next_comment_id = 1;
        }
        agent.prompt.set_text("ship it but watch the edge cases");

        let outcome = agent.approve_plan();

        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("saved earlier"),
                    "Interject must keep already-saved comments; got {text:?}"
                );
                assert!(
                    text.contains("ship it but watch the edge cases"),
                    "Interject must include freeform left in Prompt; got {text:?}"
                );
            }
            other => panic!("expected Interject with freeform, got {other:?}"),
        }
    }

    /// Empty Approve (no draft, no saved comments) still just approves.
    #[test]
    fn approve_plan_empty_still_approves_without_interject() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nempty path");
        agent.plan_approval_view.as_mut().unwrap().focus = PlanApprovalFocus::Preview;
        agent.prompt.set_text("");

        let outcome = agent.approve_plan();

        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty approve must not invent an Interject; got {outcome:?}"
        );
        assert_eq!(agent.prompt.text(), "original chat");
    }

    /// Casual send while composing a new comment must flush the draft first.
    #[test]
    fn send_casual_plan_comments_flushes_in_progress_draft() {
        let mut agent = make_agent();
        agent.enter_casual_commenting_for_test();
        agent.prompt.set_text("casual draft must be sent");
        // No previously saved comments — only the in-progress draft.

        let outcome = agent.send_casual_plan_comments();

        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains("casual draft must be sent"),
                    "casual send must flush composer draft; got {text:?}"
                );
            }
            other => panic!("expected SendPrompt with flushed draft, got {other:?}"),
        }
        assert!(agent.plan_comments.is_empty());
        assert!(agent.casual_commenting_range.is_none());
    }

    fn parse_outcome(
        mut rx: tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>>,
    ) -> serde_json::Value {
        let resp = rx
            .try_recv()
            .expect("should receive exit_plan_mode response");
        let raw = resp.expect("should be Ok");
        serde_json::from_str(raw.0.get()).expect("should be valid JSON")
    }

    /// Questions intent + freeform Enter → ACP `"questions"` (not revise).
    #[test]
    fn send_plan_questions_submits_questions_outcome() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nUse Redis");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Questions;
        }
        agent.prompt.set_text("Why Redis instead of in-memory?");

        // Drive the same path as Prompt Enter with questions intent.
        let freeform = Some(agent.prompt.text().to_string());
        let _ = agent.send_plan_questions(freeform);

        assert!(agent.plan_approval_view.is_none());
        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "questions");
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("Why Redis"),
            "feedback must carry the question; got {:?}",
            parsed["feedback"]
        );
    }

    /// Request-changes path still submits `"cancelled"` (regression).
    #[test]
    fn send_plan_feedback_still_submits_cancelled() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nUse Redis");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("drop Redis");

        let freeform = Some(agent.prompt.text().to_string());
        let _ = agent.send_plan_feedback(freeform);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "cancelled");
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("drop Redis")
        );
    }

    /// P1: freeform revise with a plan line selected must deliver path,
    /// line number, and line text to the agent (not freeform alone).
    #[test]
    fn send_plan_feedback_includes_viewer_selection_path_line_text() {
        let mut agent = make_agent();
        // Line 4 is "Use Redis for sessions" (1-based source lines).
        let plan_body = "# Plan\n\n## Step 1\nUse Redis for sessions\n## Step 2\nShip it";
        let rx = install_plan_approval(&mut agent, plan_body);
        // File-backed is the default exit_plan_mode path.
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;
        agent.show_plan_preview();
        {
            let viewer = agent
                .line_viewer
                .as_mut()
                .expect("plan preview must open a line viewer");
            viewer.prepare_layout(80, 20);
            viewer.set_initial_selection(4..5);
            viewer.prepare_layout(80, 20);
            assert_eq!(
                viewer.selected_line_range(),
                Some(4..5),
                "fixture must leave line 4 selected"
            );
        }
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("rewrite this line");

        let freeform = Some(agent.prompt.text().to_string());
        let _ = agent.send_plan_feedback(freeform);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "cancelled");
        let feedback = parsed["feedback"].as_str().unwrap_or("");
        assert!(
            feedback.contains("@plan.md:4"),
            "feedback must include plan path + line; got {feedback:?}"
        );
        assert!(
            feedback.contains("Use Redis for sessions"),
            "feedback must quote selected line text; got {feedback:?}"
        );
        assert!(
            feedback.contains("rewrite this line"),
            "feedback must keep freeform; got {feedback:?}"
        );
    }

    /// P1: saved line comment on file-backed plan includes path + line text.
    #[test]
    fn send_plan_feedback_file_backed_comment_includes_line_text() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "alpha\nbravo\ncharlie");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.source = PlanReviewSource::FileBacked;
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
            pav.comments.push(PlanComment {
                id: 0,
                line_range: 2..3,
                text: "make this stronger".into(),
            });
            pav.next_comment_id = 1;
        }

        let _ = agent.send_plan_feedback(None);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "cancelled");
        let feedback = parsed["feedback"].as_str().unwrap_or("");
        assert!(
            feedback.contains("@plan.md:2"),
            "must anchor path+line; got {feedback:?}"
        );
        assert!(
            feedback.contains("> bravo"),
            "must quote line text; got {feedback:?}"
        );
        assert!(
            feedback.contains("make this stronger"),
            "must keep comment; got {feedback:?}"
        );
    }

    /// P2: multi-line viewer highlight freeform revise delivers start–end + all
    /// quoted lines (not just the cursor line).
    #[test]
    fn send_plan_feedback_includes_viewer_multiline_selection() {
        let mut agent = make_agent();
        let plan_body =
            "# Plan\n\n## Step 1\nUse Redis for sessions\n## Step 2\nShip it\n## Step 3\nDone";
        let rx = install_plan_approval(&mut agent, plan_body);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;
        agent.show_plan_preview();
        {
            let viewer = agent
                .line_viewer
                .as_mut()
                .expect("plan preview must open a line viewer");
            viewer.prepare_layout(80, 20);
            // Lines 4–6 (half-open 4..7): Redis / Step 2 / Ship it
            viewer.set_initial_selection(4..7);
            viewer.prepare_layout(80, 20);
            assert_eq!(
                viewer.selected_line_range(),
                Some(4..7),
                "fixture must leave multi-line range selected"
            );
        }
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("collapse this block");

        let freeform = Some(agent.prompt.text().to_string());
        let _ = agent.send_plan_feedback(freeform);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "cancelled");
        let feedback = parsed["feedback"].as_str().unwrap_or("");
        assert!(
            feedback.contains("@plan.md:4-6"),
            "feedback must include multi-line range loc; got {feedback:?}"
        );
        assert!(
            feedback.contains("Use Redis for sessions"),
            "must quote first selected line; got {feedback:?}"
        );
        assert!(
            feedback.contains("## Step 2"),
            "must quote middle selected line; got {feedback:?}"
        );
        assert!(
            feedback.contains("Ship it"),
            "must quote last selected line; got {feedback:?}"
        );
        assert!(
            feedback.contains("collapse this block"),
            "must keep freeform; got {feedback:?}"
        );
    }

    /// Select-to-copy: `Y` on plan preview copies the whole plan body (not title).
    #[test]
    fn plan_preview_shift_y_copies_whole_plan_body() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut agent = make_agent();
        let plan_body = "# Plan\n\n## Step 1\nUse Redis for sessions\n## Step 2\nShip it";
        let _rx = install_plan_approval(&mut agent, plan_body);
        agent.show_plan_preview();
        assert!(
            agent.line_viewer.is_some(),
            "plan preview must open line viewer"
        );
        let y = KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT);
        let outcome = agent.handle_line_viewer_key(&y);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Y must be consumed on plan surface; got {outcome:?}"
        );
        let toast = agent.toast.as_ref().map(|(m, _)| m.as_str());
        assert!(
            toast.is_some(),
            "Y must trigger copy toast (clipboard or file fallback)"
        );
        // CTAs still available after copy.
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());
    }

    /// Select-to-copy: `y` on a selected plan line copies that line only.
    #[test]
    fn plan_preview_y_copies_selected_line() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut agent = make_agent();
        let plan_body = "# Plan\n\n## Step 1\nUse Redis for sessions\n## Step 2\nShip it";
        let _rx = install_plan_approval(&mut agent, plan_body);
        agent.show_plan_preview();
        {
            let viewer = agent.line_viewer.as_mut().expect("plan preview");
            viewer.prepare_layout(80, 20);
            viewer.set_initial_selection(4..5);
            viewer.prepare_layout(80, 20);
        }
        let y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let outcome = agent.handle_line_viewer_key(&y);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "y must be consumed; got {outcome:?}"
        );
        assert!(
            agent.toast.is_some(),
            "y must trigger copy toast for selected line"
        );
        // Does not dismiss approval or close viewer.
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());
    }

    /// Option B: plan approval opens as a right-hand side panel (not fullscreen).
    #[test]
    fn plan_approval_opens_as_side_panel_not_fullscreen() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Side panel plan\n\nDo the thing");
        agent.reopen_plan_approval();
        let viewer = agent.line_viewer.as_ref().expect("side panel viewer");
        assert!(viewer.side_panel, "approval reopen must dock as side panel");
        assert!(
            !viewer.fullscreen,
            "approval reopen must not hard-takeover fullscreen"
        );
        assert!(
            viewer.plan_ref().is_some_and(|p| p.feedback_active),
            "side panel must expose approval CTAs"
        );
        // CTAs still work.
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_line_viewer_key(&a);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "a approve must fire from side panel; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
    }

    /// Option B: Ctrl+F enlarges side panel to fullscreen and back.
    #[test]
    fn plan_side_panel_ctrl_f_toggles_fullscreen() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Toggle plan");
        agent.reopen_plan_approval();
        assert!(agent.line_viewer.as_ref().is_some_and(|v| v.side_panel));
        let ctrl_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        let _ = agent.handle_line_viewer_key(&ctrl_f);
        {
            let v = agent.line_viewer.as_ref().unwrap();
            assert!(v.fullscreen);
            assert!(!v.side_panel);
        }
        let _ = agent.handle_line_viewer_key(&ctrl_f);
        {
            let v = agent.line_viewer.as_ref().unwrap();
            assert!(!v.fullscreen);
            assert!(
                v.side_panel,
                "leaving fullscreen must restore plan side panel"
            );
        }
    }

    /// Option C: soft-park commits a transcript card once per tool_call_id.
    #[test]
    fn commit_parked_plan_card_pushes_once_with_ctas() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Card Plan\n\n## Step 1\nDo it");
        assert_eq!(agent.scrollback.len(), 0);
        agent.commit_parked_plan_card();
        assert_eq!(agent.scrollback.len(), 1);
        agent.commit_parked_plan_card(); // dedupe
        assert_eq!(agent.scrollback.len(), 1);
        let text = match &agent.scrollback.entry(0).unwrap().block {
            crate::scrollback::block::RenderBlock::AgentMessage(b) => b.text().to_owned(),
            other => panic!("expected agent message card, got {other:?}"),
        };
        assert!(
            text.contains(crate::views::plan_approval_view::PLAN_CARD_HEADER),
            "card header missing: {text:?}"
        );
        assert!(
            text.contains("# Card Plan") && text.contains("Do it"),
            "card must embed plan preview: {text:?}"
        );
        assert!(
            text.contains(crate::views::plan_approval_view::PLAN_CARD_CTAS),
            "card must list CTAs: {text:?}"
        );
        assert_eq!(
            agent.plan_card_committed_id.as_deref(),
            Some("call-approve-flush")
        );
    }

    /// Option C: soft-park CTAs (empty prompt, no viewer) approve without modal.
    #[test]
    fn soft_park_cta_a_approves_without_line_viewer() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft CTAs");
        assert!(agent.line_viewer.is_none());
        agent.set_active_pane(ActivePane::Prompt, true);
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park a must approve; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
    }

    /// Soft-park CTA keys do not steal input when the prompt has draft text.
    #[test]
    fn soft_park_cta_does_not_steal_when_prompt_has_draft() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Draft guard");
        let before = "still drafting a note";
        agent.prompt.set_text(before);
        // Pin cursor at end so the typed char has a deterministic insert site
        // (set_text does not move the caret).
        agent.prompt.set_cursor(before.len());
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&a);
        assert!(
            agent.plan_approval_view.is_some(),
            "must not approve while draft is non-empty"
        );
        // Exact equality — not a false-green contains on seed text that
        // already has 'a' / "drafting".
        assert_eq!(
            agent.prompt.text(),
            format!("{before}a"),
            "typed char must append to draft; soft CTA must not swallow key"
        );
    }

    /// Soft-park non-approve CTA: `q` abandons without opening the line viewer.
    #[test]
    fn soft_park_cta_q_abandons_without_line_viewer() {
        let mut agent = make_agent();
        let mut rx = install_plan_approval(&mut agent, "# Soft quit");
        assert!(agent.line_viewer.is_none());
        agent.set_active_pane(ActivePane::Prompt, true);
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&q);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park q must abandon; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        let resp = rx
            .try_recv()
            .expect("should receive exit_plan_mode response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "abandoned");
    }

    /// Soft-park non-approve CTA: `s` focuses revise prompt without line viewer.
    #[test]
    fn soft_park_cta_s_focuses_revise_without_line_viewer() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft revise");
        assert!(agent.line_viewer.is_none());
        agent.set_active_pane(ActivePane::Prompt, true);
        let s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&s);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park s must focus revise; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_none());
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(pav.prompt_intent, PlanPromptIntent::Revise);
    }

    /// P3: revise with a screenshot attached drains the image and returns
    /// Interject so multimodal content rides the same turn as the ACP feedback.
    #[test]
    fn send_plan_feedback_with_screenshot_returns_interject_images() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nDo the thing");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("see screenshot");
        agent
            .prompt
            .insert_image(test_fixtures::test_pasted_image())
            .expect("insert screenshot");

        let freeform = Some(agent.prompt.text().to_string());
        let outcome = agent.send_plan_feedback(freeform);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "cancelled");
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("see screenshot"),
            "text feedback still goes over ACP; got {:?}",
            parsed["feedback"]
        );
        match outcome {
            InputOutcome::Action(Action::Interject { images, text }) => {
                assert_eq!(images.len(), 1, "screenshot must ride Interject");
                assert!(
                    text.contains("Screenshot"),
                    "interject caption should mark the attachment; got {text:?}"
                );
            }
            other => panic!("expected Interject with screenshot, got {other:?}"),
        }
        assert!(
            agent.prompt.images.is_empty(),
            "composer images must be drained on submit"
        );
        // Stashed chat restored (not the revise draft).
        assert_eq!(agent.prompt.text(), "original chat");
    }

    /// P3: clarify with screenshot also drains images onto Interject.
    #[test]
    fn send_plan_questions_with_screenshot_returns_interject_images() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nUse Redis");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Questions;
        }
        agent.prompt.set_text("why this shape?");
        agent
            .prompt
            .insert_image(test_fixtures::test_pasted_image())
            .expect("insert screenshot");

        let freeform = Some(agent.prompt.text().to_string());
        let outcome = agent.send_plan_questions(freeform);

        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "questions");
        match outcome {
            InputOutcome::Action(Action::Interject { images, .. }) => {
                assert_eq!(images.len(), 1);
            }
            other => panic!("expected Interject with screenshot, got {other:?}"),
        }
    }

    /// P3: approve with notes + screenshot carries images on the notes Interject.
    #[test]
    fn approve_plan_with_screenshot_carries_images_on_interject() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nShip it");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::ApproveNotes;
        }
        agent.prompt.set_text("watch the race");
        agent
            .prompt
            .insert_image(test_fixtures::test_pasted_image())
            .expect("insert screenshot");

        let outcome = agent.approve_plan();

        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, images }) => {
                assert!(
                    text.contains("watch the race"),
                    "notes must still ride; got {text:?}"
                );
                assert_eq!(images.len(), 1, "screenshot must ride approve Interject");
            }
            other => panic!("expected Interject with notes+screenshot, got {other:?}"),
        }
        assert!(agent.prompt.images.is_empty());
    }

    /// P3: images-only approve (no freeform, no comments) uses the dedicated
    /// caption branch so visual context still rides the implement turn.
    #[test]
    fn approve_plan_images_only_uses_screenshot_caption() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nShip it");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::ApproveNotes;
        }
        agent.prompt.set_text("");
        agent
            .prompt
            .insert_image(test_fixtures::test_pasted_image())
            .expect("insert screenshot");

        let outcome = agent.approve_plan();

        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, images }) => {
                assert_eq!(
                    text, "Screenshot(s) attached with plan approval.",
                    "images-only approve must use the dedicated caption; got {text:?}"
                );
                assert_eq!(images.len(), 1);
            }
            other => panic!("expected Interject with images-only caption, got {other:?}"),
        }
        assert!(agent.prompt.images.is_empty());
    }

    /// P3: empty Enter on the Prompt with an image chip under Revise must not
    /// plain-approve — it submits revise and drains the chip onto Interject.
    #[test]
    fn empty_enter_with_image_chip_under_revise_does_not_plain_approve() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nDo the thing");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("");
        agent
            .prompt
            .insert_image(test_fixtures::test_pasted_image())
            .expect("insert screenshot");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "cancelled",
            "empty Enter + image under Revise must revise, not plain-approve; got {parsed:?}"
        );
        match outcome {
            InputOutcome::Action(Action::Interject { images, .. }) => {
                assert_eq!(images.len(), 1, "image chip must drain onto Interject");
            }
            other => panic!("expected Interject with drained image, got {other:?}"),
        }
        assert!(agent.prompt.images.is_empty());
        assert!(agent.plan_approval_view.is_none());
    }

    /// `?` / `A` / `s` focus Prompt with the matching freeform intent.
    #[test]
    fn focus_plan_prompt_sets_intent() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan");
        let _ = agent.focus_plan_prompt(PlanPromptIntent::Questions);
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().focus,
            PlanApprovalFocus::Prompt
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().prompt_intent,
            PlanPromptIntent::Questions
        );
        let _ = agent.focus_plan_prompt(PlanPromptIntent::Revise);
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().prompt_intent,
            PlanPromptIntent::Revise
        );
        let _ = agent.focus_plan_prompt(PlanPromptIntent::ApproveNotes);
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().focus,
            PlanApprovalFocus::Prompt
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().prompt_intent,
            PlanPromptIntent::ApproveNotes
        );
    }

    /// Empty Prompt Enter still approves even when intent was Questions.
    #[test]
    fn empty_enter_still_approves_under_questions_intent() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nempty questions path");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Questions;
        }
        agent.prompt.set_text("");

        // Mirror handle_plan_feedback_key empty+prompt path.
        let outcome = agent.approve_plan();
        assert!(matches!(outcome, InputOutcome::Changed));
        assert_outcome_approved(rx);
    }

    /// `A` path: ApproveNotes intent + non-empty freeform → approved + notes Interject.
    #[test]
    fn approve_notes_intent_submits_approved_with_notes() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nShip it");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::ApproveNotes;
        }
        agent.prompt.set_text("watch the race in auth");

        // Mirror handle_plan_feedback_key non-empty + ApproveNotes.
        let outcome = agent.approve_plan();

        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("watch the race in auth"),
                    "approve w/ comment must attach freeform notes; got {text:?}"
                );
                assert!(
                    text.contains("approved the plan with the following review comments"),
                    "must use approve-with-comments framing; got {text:?}"
                );
            }
            other => panic!("expected Interject with notes, got {other:?}"),
        }
        assert_eq!(agent.prompt.text(), "original chat");
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
