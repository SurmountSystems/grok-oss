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
    /// Read non-empty session `plan.md` when the path resolves and is readable.
    fn read_plan_file_body(&self) -> Option<String> {
        self.plan_file_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .filter(|s| !s.trim().is_empty())
    }
    /// Resolve the plan body for the line-viewer preview.
    ///
    /// **File-backed** approval (`exit_plan_mode`): session `plan.md` is SoT —
    /// re-read on every resolve so rewrites while parked appear in the panel.
    /// The reverse-request snapshot on `plan_approval_view` is fallback when
    /// the file is missing or unreadable.
    ///
    /// **Inline** CreatePlan: request body / `latest_inline_plan_content` first,
    /// then disk.
    pub(super) fn plan_body_for_preview(&self) -> Option<String> {
        let source = self.plan_approval_view.as_ref().map(|p| p.source);
        if source == Some(PlanReviewSource::FileBacked) {
            if let Some(disk) = self.read_plan_file_body() {
                return Some(disk);
            }
            if let Some(content) = self
                .plan_approval_view
                .as_ref()
                .and_then(|p| p.plan_content.as_deref())
                .filter(|s| !s.trim().is_empty())
            {
                return Some(content.to_owned());
            }
            return None;
        }
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
        self.read_plan_file_body()
    }
    /// Refresh FileBacked `plan_content` from disk so comment/feedback line
    /// anchors match the body shown after a while-parked rewrite.
    fn refresh_file_backed_plan_from_disk(&mut self) {
        let is_file_backed = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.source == PlanReviewSource::FileBacked);
        if !is_file_backed {
            return;
        }
        let Some(disk) = self.read_plan_file_body() else {
            return;
        };
        if let Some(pav) = self.plan_approval_view.as_mut() {
            pav.has_plan = true;
            pav.plan_content = Some(disk);
        }
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
        // File-backed SoT: pull latest plan.md before painting so the panel
        // and comment anchors track disk rewrites while approval is parked.
        self.refresh_file_backed_plan_from_disk();
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
    /// When plan approval owns the prompt (soft park or panel), a fully typed
    /// registered slash command must route through the normal slash pipeline
    /// (`SendPrompt` → registry → e.g. `ShowPlan` for `/view-plan`), not be
    /// treated as freeform revise/approve notes.
    fn try_submit_registered_slash_from_plan_prompt(&mut self) -> Option<InputOutcome> {
        let raw = self.prompt.text().to_string();
        let trimmed = raw.trim();
        if !trimmed.starts_with('/') {
            return None;
        }
        let invocation = crate::slash::parse_invocation(trimmed)?;
        let reg = self.prompt.slash_controller.registry();
        reg.get_for_dispatch(invocation.token)?;
        if !crate::slash::is_command_complete(trimmed, reg) {
            return None;
        }
        let to_send = trimmed.to_string();
        self.prompt.slash_commit_preview();
        self.prompt.slash_close();
        self.prompt.set_text("");
        Some(InputOutcome::Action(Action::SendPrompt(to_send)))
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
            // Empty Enter = approve (same as `a` / empty Prompt Enter). Soft
            // park previously swallowed bare Enter as a no-op while CTAs
            // a/A/?/s/q worked — dogfood "I can't press Enter".
            if key.code == KeyCode::Enter && key.modifiers.is_empty() {
                return self.approve_plan();
            }
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
        // Slash completion while plan approval owns the prompt (soft park or
        // panel). Soft-park used to skip `refresh_slash` and treat fully typed
        // `/view-plan` as freeform feedback — toast says use `/view-plan`.
        if self.prompt.slash_open() && !self.prompt.file_search_visible() {
            match key.code {
                KeyCode::Up => {
                    self.prompt.slash_move_selection(-1);
                    self.prompt.slash_preview_current_selection();
                    return InputOutcome::Changed;
                }
                KeyCode::Down => {
                    self.prompt.slash_move_selection(1);
                    self.prompt.slash_preview_current_selection();
                    return InputOutcome::Changed;
                }
                KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                    self.prompt.slash_move_selection(-1);
                    self.prompt.slash_preview_current_selection();
                    return InputOutcome::Changed;
                }
                KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                    self.prompt.slash_move_selection(1);
                    self.prompt.slash_preview_current_selection();
                    return InputOutcome::Changed;
                }
                KeyCode::Tab => {
                    self.prompt.slash_commit_preview();
                    self.prompt.accept_slash_completion(&self.session.models);
                    return InputOutcome::Changed;
                }
                KeyCode::Esc => {
                    self.prompt.slash_cancel_preview();
                    self.prompt.slash_close();
                    return InputOutcome::Changed;
                }
                KeyCode::Enter if key.modifiers.is_empty() => {
                    let snap = self.prompt.slash_snapshot();
                    let exact_command = snap.cursor_in_command
                        && crate::slash::parse_invocation(self.prompt.text()).is_some_and(
                            |invocation| {
                                invocation.args.is_empty()
                                    && self
                                        .prompt
                                        .slash_controller
                                        .registry()
                                        .get_for_dispatch(invocation.token)
                                        .is_some()
                                    && crate::slash::is_command_complete(
                                        self.prompt.text(),
                                        self.prompt.slash_controller.registry(),
                                    )
                            },
                        );
                    if exact_command {
                        self.prompt.slash_commit_preview();
                        self.prompt.slash_close();
                        if let Some(outcome) = self.try_submit_registered_slash_from_plan_prompt() {
                            return outcome;
                        }
                    } else {
                        let chains = snap
                            .selection()
                            .is_some_and(|row| row.insert_text.ends_with(' '));
                        self.prompt.slash_commit_preview();
                        self.prompt.accept_slash_completion(&self.session.models);
                        if chains {
                            return InputOutcome::Changed;
                        }
                        self.prompt.slash_close();
                        if let Some(outcome) = self.try_submit_registered_slash_from_plan_prompt() {
                            return outcome;
                        }
                    }
                }
                _ => {}
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
                // Registered slash (e.g. `/view-plan`) before freeform feedback.
                if let Some(outcome) = self.try_submit_registered_slash_from_plan_prompt() {
                    return outcome;
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
                // Soft-park / Preview without panel: empty Enter already handled
                // in soft_preview above. If we still reach here with empty
                // freeform (e.g. focus drifted), approve rather than swallow.
                if text.trim().is_empty() && !has_comments && !has_images {
                    return self.approve_plan();
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
                // Soft-park / plan-approval path owns keys; must refresh slash
                // so `/view-plan` appears in autocomplete (normal prompt path
                // does this in `handle_prompt_key`).
                self.prompt.refresh_slash(&self.session.models);
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

    /// Named contract: FileBacked plan approval SoT is live session `plan.md`.
    /// Park with reverse-request body A, rewrite disk to B, then preview /
    /// `plan_body_for_preview` / reopen must show B — not the frozen snapshot.
    #[test]
    fn file_backed_plan_preview_rereads_disk_after_park_rewrite() {
        let mut agent = make_agent();
        let session_id = format!(
            "plan-sot-reread-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let cwd = "/tmp";
        agent.session.session_id = Some(agent_client_protocol::SessionId::new(session_id.clone()));
        agent.session.cwd = std::path::PathBuf::from(cwd);

        let plan_path = xai_grok_shell::util::grok_home::grok_home()
            .join("sessions")
            .join(urlencoding::encode(cwd).as_ref())
            .join(&session_id)
            .join("plan.md");
        let session_dir = plan_path
            .parent()
            .expect("plan.md has a parent")
            .to_path_buf();
        std::fs::create_dir_all(&session_dir).expect("create session dir for plan SoT test");

        let content_a = "# Plan A\n\nStatus approved 2026-07-26\nMode implementing\n";
        let content_b =
            "# Plan B — failover + keyring\n\n### Critical Files for Implementation\n- foo.rs\n";
        std::fs::write(&plan_path, content_a).expect("seed plan.md with A");

        let _rx = install_plan_approval(&mut agent, content_a);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;

        // Agent rewrites (or partially rewrites) plan.md while approval is parked.
        std::fs::write(&plan_path, content_b).expect("rewrite plan.md to B");

        let body = agent
            .plan_body_for_preview()
            .expect("FileBacked preview must resolve a body");
        assert!(
            body.contains("Plan B") && body.contains("Critical Files for Implementation"),
            "FileBacked preview must re-read plan.md (B), not frozen park snapshot A; got {body:?}"
        );
        assert!(
            !body.contains("Status approved 2026-07-26"),
            "must not show frozen reverse-request snapshot A; got {body:?}"
        );

        agent.reopen_plan_approval();
        let shown = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_test())
            .expect("reopen must open plan side panel");
        assert!(
            shown.contains("Plan B") && shown.contains("Critical Files for Implementation"),
            "reopen/show path must paint disk B; got {shown:?}"
        );
        assert!(
            !shown.contains("Status approved 2026-07-26"),
            "reopen must not paint frozen A; got {shown:?}"
        );
        let refreshed = agent
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.plan_content.as_deref())
            .expect("plan_content still present");
        assert!(
            refreshed.contains("Plan B"),
            "open must refresh plan_content from disk for comment anchors; got {refreshed:?}"
        );

        let _ = std::fs::remove_dir_all(&session_dir);
    }

    /// FileBacked fallback: when session plan.md is missing/unreadable, keep
    /// reverse-request snapshot so soft-park engage still works.
    #[test]
    fn file_backed_plan_preview_falls_back_to_snapshot_when_disk_missing() {
        let mut agent = make_agent();
        agent.session.session_id = Some(agent_client_protocol::SessionId::new(
            "plan-sot-missing-disk".to_string(),
        ));
        agent.session.cwd = std::path::PathBuf::from("/tmp");
        // Intentionally do not write plan.md under this session.
        let snapshot = "# File Plan from reverse-request\n";
        let _rx = install_plan_approval(&mut agent, snapshot);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;

        assert_eq!(
            agent.plan_body_for_preview().as_deref(),
            Some(snapshot),
            "missing disk must fall back to park snapshot"
        );
        agent.reopen_plan_approval();
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some(snapshot)
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

    /// Named contract (dogfood): soft-park routes keys through
    /// `handle_plan_feedback_key`. Typing `/view` must refresh slash
    /// autocomplete so `/view-plan` is offered (not missing from the menu).
    #[test]
    fn soft_park_view_plan_slash_autocomplete_offers_view_plan() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park slash menu");
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");
        agent.set_active_pane(ActivePane::Prompt, true);
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);

        for ch in "/view".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            let _ = agent.handle_plan_feedback_key(&key);
        }

        let snap = agent.prompt.slash_snapshot();
        assert!(
            snap.active,
            "slash state must be active after typing /view under soft park"
        );
        let names: Vec<&str> = snap.matches.iter().map(|r| r.display.as_str()).collect();
        assert!(
            names.iter().any(|n| *n == "/view-plan"),
            "/view-plan must appear in soft-park slash autocomplete, got {names:?}"
        );
    }

    /// Named contract: fully typed `/view-plan` under soft park must route as
    /// a registered slash command (`SendPrompt`), not freeform plan feedback
    /// or a swallowed Enter.
    #[test]
    fn soft_park_view_plan_enter_sends_slash_not_feedback() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park view-plan dispatch");
        assert!(agent.line_viewer.is_none());
        agent.set_active_pane(ActivePane::Prompt, true);
        // Preview focus (default soft park) with draft `/view-plan`.
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        agent.prompt.set_text("/view-plan");
        agent.prompt.set_cursor("/view-plan".len());
        agent.prompt.refresh_slash(&agent.session.models);

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&enter);

        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert_eq!(
                    text.trim(),
                    "/view-plan",
                    "must dispatch the view-plan slash token, not freeform"
                );
            }
            other => panic!(
                "expected SendPrompt(/view-plan) so slash pipeline opens the plan panel; got {other:?}"
            ),
        }
        assert!(
            agent.plan_approval_view.is_some(),
            "must not dismiss approval as revise/abandon before slash runs"
        );
        assert!(
            agent.prompt.text().is_empty(),
            "slash submit clears the composer"
        );
    }

    /// Aliases documented in the user guide (`/show-plan`, `/plan-view`) also
    /// dispatch under soft park.
    #[test]
    fn soft_park_view_plan_aliases_send_slash() {
        for alias in ["/show-plan", "/plan-view"] {
            let mut agent = make_agent();
            let _rx = install_plan_approval(&mut agent, "# alias");
            agent.prompt.set_text(alias);
            agent.prompt.set_cursor(alias.len());
            let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
            let outcome = agent.handle_plan_feedback_key(&enter);
            match outcome {
                InputOutcome::Action(Action::SendPrompt(text)) => {
                    assert_eq!(text.trim(), alias, "alias {alias} must send as slash");
                }
                other => panic!("alias {alias}: expected SendPrompt, got {other:?}"),
            }
        }
    }

    /// Integration: `SendPrompt("/view-plan")` while soft-parked hits the
    /// slash registry → `ShowPlan` → reopens the plan panel.
    #[test]
    fn soft_park_view_plan_slash_pipeline_returns_show_plan() {
        use crate::app::agent::AgentId;
        use crate::app::app_view::tests::test_app_with_agent;
        use crate::app::dispatch::dispatch;

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            let _rx = install_plan_approval(agent, "# Soft park plan\n\nBody");
            agent.latest_inline_plan_content = Some("# Soft park plan\n\nBody".into());
        }

        let effects = dispatch(Action::SendPrompt("/view-plan".into()), &mut app);
        assert!(
            effects.is_empty(),
            "ShowPlan is sync; expected no async effects, got {effects:?}"
        );
        let agent = &app.agents[&id];
        assert!(
            agent.plan_approval_view.is_some(),
            "approval must remain after /view-plan"
        );
        assert!(
            agent.line_viewer.is_some(),
            "/view-plan must open the plan panel (line viewer)"
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

    /// Named contract: soft-park plan approval (Preview focus, no side panel,
    /// empty prompt) — bare Enter must approve, same as `a` and empty Prompt
    /// Enter. Must not be a silent no-op (dogfood: "I can't press Enter").
    #[test]
    fn soft_park_preview_empty_enter_approves() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nApprove via Enter");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            // Soft park leaves focus on Preview and does not open the panel.
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");
        agent.prompt.set_text("");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&enter);

        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty Enter on soft-park Preview must approve (Changed); got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "empty Enter must clear plan_approval_view (approve), not leave it parked"
        );
        assert_outcome_approved(rx);
    }

    /// Soft-park `a` still approves (regression guard next to Enter contract).
    #[test]
    fn soft_park_preview_a_still_approves() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nApprove via a");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");

        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
    }

    /// Named contract (dogfood 2026-07-27): soft-park CTAs must survive full
    /// `handle_input` when Scrollback is focused (user clicked the parked plan
    /// card to read it). Previously `active_pane != Scrollback` gated the route
    /// and keys fell through as scrollback no-ops while the legend still showed.
    #[test]
    fn soft_park_cta_a_approves_via_handle_input_while_scrollback_focused() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nRead card then approve");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.set_active_pane(ActivePane::Scrollback, true);
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");
        assert_eq!(agent.active_pane, ActivePane::Scrollback);

        let registry = ActionRegistry::defaults();
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(a), &registry);

        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park a via handle_input must approve even with Scrollback focus; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "a must clear plan_approval_view (approve), not leave it parked under Scrollback"
        );
        assert_outcome_approved(rx);
    }

    /// Soft-park empty Enter via full handle_input + Scrollback focus.
    #[test]
    fn soft_park_empty_enter_approves_via_handle_input_while_scrollback_focused() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nEnter while reading");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.set_active_pane(ActivePane::Scrollback, true);

        let registry = ActionRegistry::defaults();
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(enter), &registry);

        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park empty Enter via handle_input must approve under Scrollback; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
    }

    /// Panel Preview: Enter on a selected plan line still opens line-comment
    /// (secondary notes path; primary approve remains `a` / empty Prompt Enter).
    #[test]
    fn plan_panel_preview_enter_opens_line_commenting() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\n## Step 1\nDo something\n");
        agent.show_plan_preview();
        {
            let viewer = agent
                .line_viewer
                .as_mut()
                .expect("show_plan_preview must open panel");
            viewer.prepare_layout(80, 20);
            // Select a real source line so Enter can attach line notes.
            if viewer.selected_line_range().is_none() {
                viewer.set_initial_selection(1..2);
                viewer.prepare_layout(80, 20);
            }
            assert!(
                viewer.selected_line_range().is_some(),
                "fixture must have a selected plan line"
            );
        }
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_line_viewer_key(&enter);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Enter on panel Preview must be consumed; got {outcome:?}"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Commenting),
            "Enter on selected plan line must enter Commenting (line notes)"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "line-note Enter must not approve/dismiss plan approval"
        );
    }

    /// FileBacked panel SoT after park rewrite: reopen must not paint a
    /// dual/merged body (old snapshot A mixed with new disk B).
    #[test]
    fn file_backed_reopen_panel_body_is_single_disk_plan_not_dual_merge() {
        let mut agent = make_agent();
        let session_id = format!(
            "plan-sot-dual-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let cwd = "/tmp";
        agent.session.session_id = Some(agent_client_protocol::SessionId::new(session_id.clone()));
        agent.session.cwd = std::path::PathBuf::from(cwd);

        let plan_path = xai_grok_shell::util::grok_home::grok_home()
            .join("sessions")
            .join(urlencoding::encode(cwd).as_ref())
            .join(&session_id)
            .join("plan.md");
        let session_dir = plan_path
            .parent()
            .expect("plan.md has a parent")
            .to_path_buf();
        std::fs::create_dir_all(&session_dir).expect("create session dir");

        let content_a = "# Plan A failover\n\nStatus approved 2026-07-26\nMode implementing\n";
        let content_b =
            "# Plan B usage-display\n\n### Critical Files for Implementation\n- usage_detail.rs\n";
        std::fs::write(&plan_path, content_a).expect("seed A");

        let _rx = install_plan_approval(&mut agent, content_a);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;

        // Agent rewrote plan.md while approval stayed parked.
        std::fs::write(&plan_path, content_b).expect("rewrite B");

        agent.reopen_plan_approval();
        let shown = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_test())
            .expect("reopen must open panel");

        assert!(
            shown.contains("Plan B usage-display") && shown.contains("usage_detail.rs"),
            "panel must show current disk plan B; got {shown:?}"
        );
        assert!(
            !shown.contains("Plan A failover")
                && !shown.contains("Status approved 2026-07-26")
                && !shown.contains("Mode implementing"),
            "panel must not dual-merge frozen A with B; got {shown:?}"
        );

        let _ = std::fs::remove_dir_all(&session_dir);
    }

    /// Named contract: plan-panel footer CTAs are real clickable buttons.
    /// Click on `approve_button_area` must approve (same as `a`).
    #[test]
    fn plan_panel_click_approve_button_approves() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nClick approve");
        agent.show_plan_preview();
        assert!(agent.line_viewer.is_some(), "panel must open");

        // Simulate a rendered footer hit target (render paints these; tests set
        // them without a full TUI frame).
        let hit = Rect::new(10, 20, 12, 1);
        {
            let viewer = agent.line_viewer.as_mut().unwrap();
            viewer.plan_mut().feedback_active = true;
            viewer.plan_mut().approve_button_area = Some(hit);
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 30));
        }

        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x + 1,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "approve button click must be consumed; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "approve button click must clear plan approval"
        );
        assert_outcome_approved(rx);
    }

    /// Click Approve-with-notes button focuses Prompt with ApproveNotes intent.
    #[test]
    fn plan_panel_click_approve_notes_button_focuses_prompt() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nNotes path");
        agent.show_plan_preview();
        let hit = Rect::new(30, 20, 18, 1);
        {
            let viewer = agent.line_viewer.as_mut().unwrap();
            viewer.plan_mut().feedback_active = true;
            viewer.plan_mut().approve_notes_button_area = Some(hit);
            viewer.last_modal_area = Some(Rect::new(0, 0, 80, 30));
        }
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        };
        let _ = agent.handle_line_viewer_mouse(&click);
        let pav = agent.plan_approval_view.as_ref().expect("still parked");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(pav.prompt_intent, PlanPromptIntent::ApproveNotes);
    }

    /// Click clarify / revise / quit buttons dispatch the matching actions.
    #[test]
    fn plan_panel_click_clarify_revise_quit_buttons() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        // Clarify
        {
            let mut agent = make_agent();
            let _rx = install_plan_approval(&mut agent, "# Plan");
            agent.show_plan_preview();
            let hit = Rect::new(5, 22, 10, 1);
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .feedback_active = true;
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .questions_button_area = Some(hit);
            agent.line_viewer.as_mut().unwrap().last_modal_area = Some(Rect::new(0, 0, 80, 30));
            let click = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: hit.x,
                row: hit.y,
                modifiers: KeyModifiers::NONE,
            };
            let _ = agent.handle_line_viewer_mouse(&click);
            let pav = agent.plan_approval_view.as_ref().unwrap();
            assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
            assert_eq!(pav.prompt_intent, PlanPromptIntent::Questions);
        }
        // Revise
        {
            let mut agent = make_agent();
            let _rx = install_plan_approval(&mut agent, "# Plan");
            agent.show_plan_preview();
            let hit = Rect::new(5, 22, 10, 1);
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .feedback_active = true;
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .send_button_area = Some(hit);
            agent.line_viewer.as_mut().unwrap().last_modal_area = Some(Rect::new(0, 0, 80, 30));
            let click = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: hit.x,
                row: hit.y,
                modifiers: KeyModifiers::NONE,
            };
            let _ = agent.handle_line_viewer_mouse(&click);
            let pav = agent.plan_approval_view.as_ref().unwrap();
            assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
            assert_eq!(pav.prompt_intent, PlanPromptIntent::Revise);
        }
        // Quit
        {
            let mut agent = make_agent();
            let mut rx = install_plan_approval(&mut agent, "# Plan");
            agent.show_plan_preview();
            let hit = Rect::new(5, 22, 8, 1);
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .feedback_active = true;
            agent
                .line_viewer
                .as_mut()
                .unwrap()
                .plan_mut()
                .abandon_button_area = Some(hit);
            agent.line_viewer.as_mut().unwrap().last_modal_area = Some(Rect::new(0, 0, 80, 30));
            let click = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: hit.x,
                row: hit.y,
                modifiers: KeyModifiers::NONE,
            };
            let _ = agent.handle_line_viewer_mouse(&click);
            assert!(agent.plan_approval_view.is_none());
            let resp = rx.try_recv().expect("abandon response");
            let raw = resp.expect("Ok");
            let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
            assert_eq!(parsed["outcome"], "abandoned");
        }
    }

    /// Valid 8×8 PNG bytes for drop-classifier image paste tests.
    fn test_png_bytes() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(8, 8, Rgba([128, 64, 32, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("encode png");
        buf
    }

    /// Named contract: paste while plan approval Preview is active (panel open)
    /// must attach images/paths to the plan composer — not swallow into the
    /// list-pane search bar.
    #[test]
    fn plan_panel_preview_paste_image_path_attaches_to_prompt() {
        use crate::actions::ActionRegistry;
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nPaste screenshot");
        agent.show_plan_preview();
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.line_viewer.is_some());
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().focus,
            PlanApprovalFocus::Preview
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("shot.png");
        std::fs::write(&png_path, test_png_bytes()).unwrap();

        let paste_text = format!("file://{}", png_path.display());
        let outcome = agent.handle_input(&Event::Paste(paste_text), &ActionRegistry::defaults());
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "plan panel Preview paste must be handled; got {outcome:?}"
        );
        assert!(
            !agent.prompt.images.is_empty() || agent.prompt.text().contains("[Image"),
            "paste must attach image chip into plan composer; text={:?} images={}",
            agent.prompt.text(),
            agent.prompt.images.len()
        );
    }

    /// Soft-park Preview (no panel): paste must not be Unchanged/swallowed —
    /// screenshots attach to the plan composer for approve/revise/clarify.
    #[test]
    fn soft_park_preview_paste_attaches_image_path() {
        use crate::actions::ActionRegistry;
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nSoft paste");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.line_viewer.is_none());

        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("shot.png");
        std::fs::write(&png_path, test_png_bytes()).unwrap();

        let outcome = agent.handle_input(
            &Event::Paste(format!("file://{}", png_path.display())),
            &ActionRegistry::defaults(),
        );
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park Preview paste must not be Unchanged; got {outcome:?}"
        );
        assert!(
            !agent.prompt.images.is_empty() || agent.prompt.text().contains("[Image"),
            "soft-park paste must attach image; text={:?} n={}",
            agent.prompt.text(),
            agent.prompt.images.len()
        );
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
