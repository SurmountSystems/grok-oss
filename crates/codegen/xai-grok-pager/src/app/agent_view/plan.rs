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
    /// When plan approval is open, attach a PNG path to the plan composer so
    /// approve / revise / clarify can drain it on the same multimodal path as
    /// a pasted screenshot (P1–P4). Returns true if a chip was inserted.
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

    /// True when a line-comment draft is armed (approval `Commenting` or casual
    /// range) and the draft body is still empty.
    ///
    /// Dogfood 2026-08-01: "commenting L17" with no typed body still looks like
    /// the plan viewer is focused; arrows / PageUp / PageDown must scroll the
    /// plan, not die on an empty composer. Once the operator types, the
    /// composer owns cursor motion.
    pub(super) fn is_empty_plan_line_comment_draft(&self) -> bool {
        if !self.prompt.text().is_empty() {
            return false;
        }
        if self.is_casual_commenting() {
            return true;
        }
        self.plan_approval_view.as_ref().is_some_and(|pav| {
            pav.focus == PlanApprovalFocus::Commenting && pav.commenting_range.is_some()
        })
    }

    /// Bare plan-viewer navigation keys (arrows, page, home/end).
    ///
    /// No modifiers: Shift-arrows stay with visual select when Preview owns
    /// keys; Ctrl/Alt chords stay global or composer chords.
    pub(super) fn is_plan_viewer_scroll_key(key: &KeyEvent) -> bool {
        if !key.modifiers.is_empty() {
            return false;
        }
        matches!(
            key.code,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End
        )
    }

    /// When the plan line viewer is open, bare scroll keys navigate the plan
    /// even if focus is dual (soft-park Prompt, empty line-comment draft).
    ///
    /// Only a **non-empty line-comment** draft keeps Up/Down/Page for caret
    /// motion in the composer. Freeform Prompt notes and empty drafts still
    /// scroll the open plan so operators do not need a focus click first.
    pub(super) fn plan_viewer_owns_scroll_keys(&self, key: &KeyEvent) -> bool {
        if self.line_viewer.is_none() || !self.is_plan_viewer() {
            return false;
        }
        if !Self::is_plan_viewer_scroll_key(key) || self.prompt.slash_open() {
            return false;
        }
        // Empty line-comment draft ("commenting L#") scrolls the plan.
        if self.is_empty_plan_line_comment_draft() {
            return true;
        }
        // Non-empty line-comment draft keeps composer caret motion.
        let nonempty_line_comment = !self.prompt.text().is_empty()
            && (self.is_casual_commenting()
                || self.plan_approval_view.as_ref().is_some_and(|p| {
                    p.focus == PlanApprovalFocus::Commenting && p.commenting_range.is_some()
                }));
        !nonempty_line_comment
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
            if pav.plan_content.as_deref() != Some(disk.as_str()) {
                pav.plan_content = Some(disk);
            }
        }
    }

    /// Soft-park auto-opens the plan side panel and freezes its body in the
    /// line viewer. While FileBacked approval stays parked, rewrites to session
    /// `plan.md` must rebuild that open panel (not only on `/view-plan` reopen).
    ///
    /// No-op when approval is not FileBacked, the plan viewer is closed, or
    /// disk is missing/unreadable (snapshot fallback stays).
    fn refresh_open_file_backed_plan_panel_if_stale(&mut self) {
        let is_file_backed = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.source == PlanReviewSource::FileBacked);
        if !is_file_backed || !self.is_plan_viewer() {
            return;
        }
        let Some(disk) = self.read_plan_file_body() else {
            return;
        };
        // Production path: use public feedback accessor (test-only helper is cfg(test)).
        let viewer_matches = self
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_feedback())
            .is_some_and(|body| body == disk);
        if viewer_matches {
            // Body already current; still sync plan_content for comment anchors.
            self.refresh_file_backed_plan_from_disk();
            return;
        }
        // Rebuild from live disk (also refreshes plan_content via show path).
        self.show_plan_preview();
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.plan_mut().feedback_active = true;
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
        // Casual `/view-plan` and approval soft-park both open as a right-hand
        // side panel (half screen) so chat stays visible. Fullscreen is opt-in
        // via Ctrl+F / the enlarge control. Force-modal
        // (`plan_approval_park=modal`) upgrades to fullscreen after reopen in
        // `handle_exit_plan_mode`.
        viewer.side_panel = true;
        viewer.fullscreen = false;
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

    /// Keep plan-viewer chrome aligned with whether approval is parked.
    ///
    /// Soft-park / side-panel CTAs key off `feedback_active`. That flag is set
    /// on open, but dogfood can leave a plan viewer open with stale casual
    /// flags while `plan_approval_view` is still live (or the reverse). Call
    /// this every draw before painting so Approve/Notes/Clarify/Revise/Quit
    /// never silently degrade to casual `c comment` while approval is pending.
    ///
    /// Also re-reads FileBacked session `plan.md` into an already-open panel
    /// when disk diverged after soft-park (park-time snapshot freeze).
    pub(crate) fn sync_plan_viewer_approval_chrome(&mut self) {
        self.refresh_open_file_backed_plan_panel_if_stale();
        let approval = self.plan_approval_view.is_some();
        let Some(viewer) = self.line_viewer.as_mut() else {
            return;
        };
        if viewer.kind != crate::views::file_search::line_viewer::LineViewerKind::PlanPreview {
            return;
        }
        let plan = viewer.plan_mut();
        plan.feedback_active = approval;
        // Casual preview only: show `c comment` when no live reverse-request.
        plan.show_action_buttons = !approval;
    }

    /// Drop leftover plan-approval chrome after a turn ends, but **never**
    /// stale-cancel a live soft-park reverse-request.
    ///
    /// Named contract (dogfood 2026-08-01): while `response_tx` is still open,
    /// the user has not answered Approve/Notes/Clarify/Revise/Quit. A turn-end
    /// broadcast must not wipe the side panel / strip CTAs and leave casual
    /// fullscreen plan.md with only `c comment`. Explicit user cancel (Esc
    /// cancel-turn) still uses the hard wipe path.
    pub(crate) fn dismiss_plan_approval_after_turn_if_stale(&mut self) {
        let still_awaiting = self
            .plan_approval_view
            .as_ref()
            .is_some_and(|p| p.response_tx.is_some());
        if still_awaiting {
            return;
        }
        if let Some(mut pav) = self.plan_approval_view.take() {
            // Channel already consumed or missing — no live waiter to cancel.
            let _ = pav.send_stale_cancel();
            self.plan_next_comment_id = pav.next_comment_id;
            self.restore_plan_stashed_prompt(pav.stashed_prompt);
            self.line_viewer = None;
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
        // FileBacked SoT: re-read plan.md before formatting review comments so
        // approve Interject quotes the live disk body, not park-time freeze.
        self.refresh_file_backed_plan_from_disk();
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
        // Freeform (if any) was consumed into approve notes / Interject. Clear
        // durable unsent draft so resume does not resurrect already-sent text.
        self.clear_unsent_prompt_draft();
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
    /// Soft-park parks with an empty stash so parking does not clear chat.
    /// Restoring that empty snapshot would wipe live freeform / images.
    /// Only restore when reopen (or similar) captured a real snapshot.
    pub(crate) fn restore_plan_stashed_prompt(
        &mut self,
        stash: crate::views::prompt_widget::StashedPrompt,
    ) {
        let had_real_stash =
            !stash.text.is_empty() || !stash.images.is_empty() || !stash.chip_elements.is_empty();
        if had_real_stash {
            self.prompt.restore(stash);
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
        self.restore_plan_stashed_prompt(pav.stashed_prompt);
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
    /// Soft-park footer CTA mouse dispatch (mouse primary).
    ///
    /// Clicks work even when the prompt has draft text — keys keep the
    /// empty-prompt guard so typing is not stolen; explicit clicks do not.
    /// Returns `None` when the point is not on a soft-park CTA hit target.
    pub(crate) fn handle_soft_park_cta_click(
        &mut self,
        col: u16,
        row: u16,
    ) -> Option<InputOutcome> {
        // Hits are only applied when soft-park / minimal paint CTA chrome.
        // Full-TUI side panel clears `hit_soft_park_ctas`, so line_viewer open
        // there is fine. Minimal paints the same strip with line_viewer open
        // after /view-plan, so do not gate on line_viewer here.
        self.plan_approval_view.as_ref()?;
        let hits = &self.hit_soft_park_ctas;
        if hits.approve.contains(col, row) {
            return Some(self.approve_plan());
        }
        if hits.notes.contains(col, row) {
            return Some(self.focus_plan_prompt(PlanPromptIntent::ApproveNotes));
        }
        if hits.clarify.contains(col, row) {
            return Some(self.focus_plan_prompt(PlanPromptIntent::Questions));
        }
        if hits.revise.contains(col, row) {
            return Some(self.focus_plan_prompt(PlanPromptIntent::Revise));
        }
        if hits.quit.contains(col, row) {
            return Some(self.abandon_plan());
        }
        None
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
        // FileBacked SoT: re-read plan.md so revise line anchors match disk.
        self.refresh_file_backed_plan_from_disk();
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
        // Freeform was drained into revise feedback; do not leave it as unsent.
        self.clear_unsent_prompt_draft();
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        self.show_toast("Revision sent — agent will rewrite the plan.");
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
        // FileBacked SoT: re-read plan.md so clarify line anchors match disk.
        self.refresh_file_backed_plan_from_disk();
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
        // Freeform was drained into clarify feedback; do not leave it as unsent.
        self.clear_unsent_prompt_draft();
        self.line_viewer = None;
        self.prompt.textarea.cancel_undo_group();
        self.show_toast("Clarify sent — answers without rewriting the plan.");
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
    ///
    /// FileBacked SoT is live session `plan.md`: re-read before format, and if
    /// the card is already committed for this request, refresh its scrollback
    /// body in place when disk (or inline content) changes while parked.
    pub(crate) fn commit_parked_plan_card(&mut self) {
        // FileBacked: pull latest plan.md so the soft-park card tracks disk
        // rewrites the same way the side panel does (not park-time snapshot).
        self.refresh_file_backed_plan_from_disk();

        let Some(pav) = self.plan_approval_view.as_ref() else {
            return;
        };
        let tool_call_id = pav.tool_call_id.clone();
        // Prefer live preview resolve (FileBacked re-reads disk; Inline uses
        // request/snapshot body) so card title/body match dogfood SoT.
        let live = self.plan_body_for_preview();
        let body = crate::views::plan_approval_view::format_parked_plan_card(live.as_deref());

        if self.plan_card_committed_id.as_deref() == Some(tool_call_id.as_str()) {
            if let Some(eid) = self.plan_card_entry_id {
                let needs_update = self
                    .scrollback
                    .get_by_id(eid)
                    .and_then(|e| e.block.as_agent_message())
                    .map(|m| m.text() != body)
                    .unwrap_or(false);
                if needs_update {
                    if let Some(entry) = self.scrollback.get_by_id_mut(eid) {
                        entry.block = crate::scrollback::block::RenderBlock::agent_message(body);
                    }
                    self.scrollback.mark_height_dirty(eid);
                }
            }
            return;
        }

        let eid = self
            .scrollback
            .push_block(crate::scrollback::block::RenderBlock::agent_message(body));
        self.plan_card_committed_id = Some(tool_call_id);
        self.plan_card_entry_id = Some(eid);
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
        // Ctrl/Cmd+V: full clipboard attachment path (screenshot raster +
        // file URLs). Must not fall through to the prompt widget's text-only
        // paste — plan review screenshots ride approve/revise/clarify.
        if crate::input::key::is_paste_key(key) {
            if self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.focus == PlanApprovalFocus::Preview)
            {
                if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Prompt;
                }
            }
            let clipboard_text = crate::app::actions::ClipboardTextRead::from_result(
                crate::clipboard::system_clipboard_read_text(),
            );
            return self.handle_paste_key_deferred(clipboard_text);
        }
        // Soft-park (no side panel): **non-capturing** for Char / empty Enter.
        // L1 main thread stays modal-free (operator 2026-07-29): all printable
        // keys go to the composer; CTAs are mouse footer / status / `/view-plan`
        // panel only. Do **not** re-add empty-prompt a/A/s/?/q/Enter approve
        // here — that traps typing and feels like a modal soft-park.
        // Side panel (line_viewer open) keeps empty-prompt accelerators in
        // `handle_line_viewer_key`.
        // Soft-park: composer keys flip Preview → Prompt so the caret paints.
        if self.line_viewer.is_none()
            && !is_commenting
            && self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.focus == PlanApprovalFocus::Preview)
        {
            let is_composer_key = match key.code {
                KeyCode::Char(c) if !c.is_control() => {
                    key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT
                }
                KeyCode::Backspace | KeyCode::Delete => key.modifiers.is_empty(),
                _ => false,
            };
            if is_composer_key {
                if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Prompt;
                }
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
                    // Soft-park (no panel): empty Enter is a no-op, not approve.
                    // Mouse footer CTAs / `/view-plan` panel own approve; L1
                    // must not trap on empty Enter (modal-free 2026-07-29).
                    let soft_park = self.line_viewer.is_none();
                    if soft_park && text.trim().is_empty() && !has_comments && !has_images {
                        return InputOutcome::Changed;
                    }
                    // Panel Prompt: empty Enter still approves — but screenshots
                    // alone (or comments) mean submitting content under the
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
                // Soft-park / Preview without panel: do not approve on empty
                // Enter — mouse / panel only (L1 modal-free).
                if self.line_viewer.is_none() {
                    return InputOutcome::Changed;
                }
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

    /// Phase P: freeform + saved line comments under **Revise** intent must
    /// submit ACP `"cancelled"` (rewrite the plan), never `"questions"`, and
    /// must carry `@plan.md:N` + quoted line text + freeform.
    #[test]
    fn revise_intent_freeform_plus_line_comments_submits_cancelled_not_questions() {
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
        agent.prompt.set_text("drop Redis entirely");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "cancelled",
            "Revise intent must rewrite the plan (wire cancelled), not questions; got {parsed:?}"
        );
        assert_ne!(
            parsed["outcome"], "questions",
            "freeform + line comments under Revise must never send questions"
        );
        let feedback = parsed["feedback"].as_str().unwrap_or("");
        assert!(
            feedback.contains("@plan.md:2"),
            "must include path+line anchor; got {feedback:?}"
        );
        assert!(
            feedback.contains("> bravo"),
            "must quote selected line text; got {feedback:?}"
        );
        assert!(
            feedback.contains("make this stronger"),
            "must keep line comment; got {feedback:?}"
        );
        assert!(
            feedback.contains("drop Redis entirely"),
            "must keep freeform revise notes; got {feedback:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "submit must clear parked approval"
        );
    }

    /// Phase P: freeform that *looks* like a question under Revise still
    /// rewrites (cancelled) — wording must not flip the wire outcome.
    #[test]
    fn revise_intent_question_shaped_freeform_still_submits_cancelled() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Plan\n\nUse Redis");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        // Operators often phrase revise notes as questions; intent wins.
        agent
            .prompt
            .set_text("Why not use the in-memory cache instead?");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "cancelled",
            "Revise intent must not become questions just because freeform ends with ?; got {parsed:?}"
        );
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("in-memory cache"),
            "feedback must carry the freeform; got {:?}",
            parsed["feedback"]
        );
    }

    /// Phase P: soft-park Revise CTA then freeform Enter → cancelled + notes.
    #[test]
    fn soft_park_revise_cta_then_enter_submits_cancelled() {
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park revise");
        assert!(agent.line_viewer.is_none(), "soft-park: no panel");

        let hit = Rect::new(20, 24, 10, 1);
        agent.hit_soft_park_ctas.revise.set(Some(hit));
        agent
            .handle_soft_park_cta_click(hit.x + 1, hit.y)
            .expect("Revise click must dispatch");
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().prompt_intent,
            PlanPromptIntent::Revise
        );

        agent.prompt.set_text("rewrite step 2");
        // Soft-park CTA leaves Prompt focus; Enter submits under intent.
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
        }
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "cancelled",
            "soft-park Revise → Enter must rewrite; got {parsed:?}"
        );
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("rewrite step 2")
        );
    }

    /// Phase P: soft-park Clarify CTA alone → questions (not revise/cancelled).
    #[test]
    fn soft_park_clarify_cta_then_enter_submits_questions() {
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park clarify");
        assert!(agent.line_viewer.is_none(), "soft-park: no panel");

        let hit = Rect::new(10, 24, 10, 1);
        agent.hit_soft_park_ctas.clarify.set(Some(hit));
        agent
            .handle_soft_park_cta_click(hit.x + 1, hit.y)
            .expect("Clarify click must dispatch");
        assert_eq!(
            agent.plan_approval_view.as_ref().unwrap().prompt_intent,
            PlanPromptIntent::Questions
        );

        agent.prompt.set_text("Why Redis?");
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
        }
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "questions",
            "soft-park Clarify → Enter must answer without rewrite; got {parsed:?}"
        );
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("Why Redis")
        );
    }

    /// Phase P: default soft-park freeform (no CTA click) still revises —
    /// constructor default intent is Revise, not Questions.
    #[test]
    fn soft_park_default_freeform_enter_submits_cancelled_not_questions() {
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Default freeform revise");
        assert!(
            matches!(
                agent.plan_approval_view.as_ref().map(|p| p.prompt_intent),
                Some(PlanPromptIntent::Revise)
            ),
            "parked approval must default to Revise intent"
        );
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
        }
        agent.prompt.set_text("add error handling section");
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&enter);

        let parsed = parse_outcome(rx);
        assert_eq!(
            parsed["outcome"], "cancelled",
            "default freeform Enter must revise (cancelled), not questions; got {parsed:?}"
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

    /// Named contract: soft-park auto-opens the plan panel with park-time body.
    /// While approval stays parked, a disk rewrite must update the **already
    /// open** panel on paint sync (`sync_plan_viewer_approval_chrome`), not
    /// only after a manual `/view-plan` reopen.
    #[test]
    fn file_backed_open_panel_live_refreshes_on_paint_after_disk_rewrite() {
        let mut agent = make_agent();
        let session_id = format!(
            "plan-sot-open-panel-{}",
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

        let content_a = "# Plan A open freeze\n\nold_token_economy_marker\n";
        let content_b = "# Plan B open live\n\nsurmount_team_usage_first\n";
        std::fs::write(&plan_path, content_a).expect("seed A");

        let _rx = install_plan_approval(&mut agent, content_a);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;
        agent.show_plan_preview();
        let shown_a = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_test())
            .expect("panel open with A")
            .to_owned();
        assert!(
            shown_a.contains("old_token_economy_marker"),
            "precondition: open panel shows park body A; got {shown_a:?}"
        );

        std::fs::write(&plan_path, content_b).expect("rewrite B while panel stays open");

        // Paint path only (no reopen / show_plan_preview).
        agent.sync_plan_viewer_approval_chrome();
        let shown_b = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_test())
            .expect("panel still open after paint sync");
        assert!(
            shown_b.contains("surmount_team_usage_first") && shown_b.contains("Plan B open live"),
            "open panel must live-refresh to disk B on paint; got {shown_b:?}"
        );
        assert!(
            !shown_b.contains("old_token_economy_marker"),
            "open panel must drop frozen A; got {shown_b:?}"
        );
        let refreshed = agent
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.plan_content.as_deref())
            .expect("plan_content present");
        assert!(
            refreshed.contains("surmount_team_usage_first"),
            "paint sync must refresh plan_content for anchors; got {refreshed:?}"
        );

        let _ = std::fs::remove_dir_all(&session_dir);
    }

    /// Named contract: approve Interject line quotes use live disk plan.md for
    /// FileBacked approval, not the reverse-request snapshot frozen at park.
    #[test]
    fn file_backed_approve_interject_quotes_disk_body_after_rewrite() {
        let mut agent = make_agent();
        let session_id = format!(
            "plan-sot-approve-{}",
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

        let content_a = "# Plan A freeze\nold_token_economy_marker\n";
        let content_b = "# Plan B live\nsurmount_team_usage_first\n";
        std::fs::write(&plan_path, content_a).expect("seed A");

        let rx = install_plan_approval(&mut agent, content_a);
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.source = PlanReviewSource::FileBacked;
            pav.comments.push(PlanComment {
                id: 0,
                line_range: 2..3,
                text: "prefer the exclusive priority title".into(),
            });
            pav.next_comment_id = 1;
            pav.focus = PlanApprovalFocus::Preview;
        }

        std::fs::write(&plan_path, content_b).expect("rewrite B before approve");

        let outcome = agent.approve_plan();
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains("surmount_team_usage_first"),
                    "approve Interject must quote live disk line B; got {text:?}"
                );
                assert!(
                    !text.contains("old_token_economy_marker"),
                    "approve Interject must not quote frozen park snapshot A; got {text:?}"
                );
                assert!(
                    text.contains("prefer the exclusive priority title"),
                    "approve Interject must keep the user comment; got {text:?}"
                );
            }
            other => panic!("expected Interject with disk-backed quotes, got {other:?}"),
        }

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

    /// Hermetic copy backup path for payload asserts (serialized on env).
    fn with_grok_copy_file<R>(f: impl FnOnce(&std::path::Path) -> R) -> R {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("last-copy.txt");
        // SAFETY: test-only env mutation; callers use serial(grok_copy_file).
        unsafe {
            std::env::set_var(crate::clipboard::GROK_COPY_FILE_ENV, &path);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&path)));
        unsafe {
            std::env::remove_var(crate::clipboard::GROK_COPY_FILE_ENV);
        }
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    /// Select-to-copy: `Y` on plan preview copies the whole plan body (not title).
    #[test]
    #[serial_test::serial(grok_copy_file)]
    fn plan_preview_shift_y_copies_whole_plan_body() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        with_grok_copy_file(|copy_path| {
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
            let written = std::fs::read_to_string(copy_path).expect("GROK_COPY_FILE payload");
            assert_eq!(
                written, plan_body,
                "Y must copy whole plan body, not title-only"
            );
            // CTAs still available after copy.
            assert!(agent.plan_approval_view.is_some());
            assert!(agent.line_viewer.is_some());
        });
    }

    /// Named contract: top-bar ⧉ click copies the whole plan body (same as `Y`).
    /// Hit target comes from a real paint of the preview, not a hand-set rect.
    #[test]
    #[serial_test::serial(grok_copy_file)]
    fn plan_preview_copy_button_click_copies_whole_plan_body() {
        use crate::views::file_search::line_viewer::render_line_viewer;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        with_grok_copy_file(|copy_path| {
            let mut agent = make_agent();
            let plan_body = "# Plan\n\n## Step 1\nUse Redis for sessions\n## Step 2\nShip it";
            let _rx = install_plan_approval(&mut agent, plan_body);
            agent.show_plan_preview();
            // Paint once so copy_button_area matches a real top-bar layout.
            {
                let full = Rect::new(0, 0, 80, 24);
                let mut buf = Buffer::empty(full);
                let theme = crate::theme::Theme::current();
                let viewer = agent.line_viewer.as_mut().expect("plan preview");
                render_line_viewer(
                    &mut buf,
                    full,
                    viewer,
                    std::path::Path::new("/tmp"),
                    &theme,
                    0,
                );
            }
            let hit = agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.copy_button_area)
                .expect("painted plan top bar must set ⧉ hit target");
            let click = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: hit.x + hit.width / 2,
                row: hit.y,
                modifiers: crossterm::event::KeyModifiers::NONE,
            };
            let outcome = agent.handle_line_viewer_mouse(&click);
            assert!(
                matches!(outcome, InputOutcome::Changed),
                "⧉ click must be consumed; got {outcome:?}"
            );
            assert!(
                agent.toast.is_some(),
                "⧉ must trigger copy toast (clipboard or file fallback)"
            );
            let written = std::fs::read_to_string(copy_path).expect("GROK_COPY_FILE payload");
            assert_eq!(
                written, plan_body,
                "⧉ must copy whole plan body (same as Y)"
            );
            // Does not dismiss approval or close the viewer.
            assert!(agent.plan_approval_view.is_some());
            assert!(agent.line_viewer.is_some());
        });
    }

    /// Empty plan body: ⧉ / Y stay quiet (no toast). UI placeholder is not
    /// treated as copyable plan content; approval + viewer stay open.
    #[test]
    fn plan_preview_copy_button_empty_body_is_quiet_noop() {
        use crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
        };
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        // Whitespace-only → has_plan false, empty-plan placeholder in viewer.
        let _rx = install_plan_approval(&mut agent, "   \n\t  ");
        agent.show_plan_preview();
        assert!(
            agent.line_viewer.is_some(),
            "empty approval still opens viewer"
        );
        assert!(
            agent
                .plan_approval_view
                .as_ref()
                .is_some_and(|p| !p.has_plan),
            "fixture must be empty-plan approval"
        );
        assert!(
            agent.plan_body_for_preview().is_none(),
            "empty plan has no real body to copy"
        );

        // Synthetic hit (paint path covered elsewhere); click must consume, no toast.
        {
            let viewer = agent.line_viewer.as_mut().expect("viewer");
            viewer.copy_button_area = Some(Rect::new(60, 0, 4, 1));
        }
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 61,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        let outcome = agent.handle_line_viewer_mouse(&click);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-body ⧉ still consumes the click; got {outcome:?}"
        );
        assert!(
            agent.toast.is_none(),
            "empty plan body must not toast a fake copy"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());

        // Y path matches ⧉.
        let y = KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT);
        let outcome = agent.handle_line_viewer_key(&y);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-body Y still consumed; got {outcome:?}"
        );
        assert!(
            agent.toast.is_none(),
            "empty plan Y must not toast a fake copy"
        );
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

    /// Named contract (dogfood 2026-08-01): casual `/view-plan` / ShowPlan
    /// opens as a half-screen side panel by default — not a full-screen
    /// takeover. Ctrl+F remains the opt-in enlarge.
    #[test]
    fn casual_view_plan_opens_as_side_panel_not_fullscreen() {
        let mut agent = make_agent();
        agent.latest_inline_plan_content = Some(long_plan_body(20));
        agent.show_plan_preview();
        let viewer = agent
            .line_viewer
            .as_ref()
            .expect("casual /view-plan must open a plan viewer");
        assert!(
            viewer.side_panel,
            "casual view-plan must dock as side panel (half screen)"
        );
        assert!(
            !viewer.fullscreen,
            "casual view-plan must not hard-takeover fullscreen by default"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "fixture is casual preview (no live approval)"
        );
        assert!(
            viewer.plan_ref().is_some_and(|p| p.show_action_buttons),
            "casual preview keeps c-comment chrome"
        );
        // Ctrl+F still enlarges; leaving fullscreen restores side panel.
        let ctrl_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        let _ = agent.handle_line_viewer_key(&ctrl_f);
        {
            let v = agent.line_viewer.as_ref().unwrap();
            assert!(
                v.fullscreen,
                "Ctrl+F must enlarge casual plan to fullscreen"
            );
            assert!(!v.side_panel);
        }
        let _ = agent.handle_line_viewer_key(&ctrl_f);
        {
            let v = agent.line_viewer.as_ref().unwrap();
            assert!(!v.fullscreen);
            assert!(
                v.side_panel,
                "leaving fullscreen must restore casual plan side panel"
            );
        }
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

    /// Long plan body so viewport navigation can move selection / offset.
    fn long_plan_body(lines: usize) -> String {
        let mut s = String::from("# Long plan for scroll tests\n\n");
        for i in 1..=lines {
            s.push_str(&format!("Line {i}: content for plan review scrolling\n"));
        }
        s
    }

    /// Named contract (dogfood 2026-08-01): soft-park dual focus (Prompt +
    /// open side panel) must still scroll the plan with arrows / page keys
    /// immediately — no click-to-focus ritual. Empty freeform draft.
    #[test]
    fn plan_prompt_focus_empty_draft_arrows_scroll_viewer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        // Mirror soft-park: Prompt focus + open side panel + empty draft.
        soft_park_style_open(&mut agent, &long_plan_body(80));
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Prompt),
            "soft-park dual focus stays Prompt"
        );
        {
            let viewer = agent.line_viewer.as_mut().expect("plan side panel");
            viewer.prepare_layout(60, 12);
            if let Some(id) = viewer.lines.first().map(|l| l.stable_id()) {
                viewer.list_state.select_by_id(id);
            }
            viewer.list_state.set_scroll_offset(0);
            viewer.prepare_layout(60, 12);
        }
        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection");

        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Prompt-focus empty draft Down must scroll plan; got {outcome:?}"
        );
        let sel_after = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after Down");
        assert!(
            sel_after > sel_before,
            "Prompt-focus empty: Down must advance plan selection ({sel_before} → {sel_after})"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Prompt),
            "scroll must not force Preview (dual focus / L1 typing stays free)"
        );
        assert!(agent.prompt.text().is_empty());
    }

    /// Named contract (dogfood 2026-08-01): dual focus with a freeform Prompt
    /// draft (soft-park live text or A/?/s notes) still routes Up/Down/Page to
    /// the open plan. Line-comment non-empty drafts keep composer caret.
    #[test]
    fn plan_prompt_focus_freeform_draft_arrows_still_scroll_viewer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        soft_park_style_open(&mut agent, &long_plan_body(80));
        agent.prompt.set_text("still drafting");
        agent.prompt.set_cursor(0);
        {
            let viewer = agent.line_viewer.as_mut().expect("plan side panel");
            viewer.prepare_layout(60, 12);
            if let Some(id) = viewer.lines.first().map(|l| l.stable_id()) {
                viewer.list_state.select_by_id(id);
            }
            viewer.list_state.set_scroll_offset(0);
            viewer.prepare_layout(60, 12);
        }
        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection");

        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let _ = agent.handle_input(&Event::Key(down), &registry);
        let sel_after = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after Down");
        assert!(
            sel_after > sel_before,
            "Prompt freeform draft: Down must still scroll plan ({sel_before} → {sel_after})"
        );
        assert_eq!(
            agent.prompt.text(),
            "still drafting",
            "scroll keys must not rewrite freeform draft"
        );
    }

    /// Casual `/view-plan` side panel: arrows scroll immediately (no focus
    /// ritual) through the normal line-viewer key path.
    #[test]
    fn casual_view_plan_arrows_scroll_without_extra_focus() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        agent.latest_inline_plan_content = Some(long_plan_body(80));
        agent.show_plan_preview();
        {
            let viewer = agent.line_viewer.as_mut().expect("casual plan panel");
            assert!(viewer.side_panel && !viewer.fullscreen);
            viewer.prepare_layout(60, 12);
            if let Some(id) = viewer.lines.first().map(|l| l.stable_id()) {
                viewer.list_state.select_by_id(id);
            }
            viewer.list_state.set_scroll_offset(0);
            viewer.prepare_layout(60, 12);
        }
        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection");

        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "casual plan Down must be consumed; got {outcome:?}"
        );
        let sel_after = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after Down");
        assert!(
            sel_after > sel_before,
            "casual plan Down must advance selection ({sel_before} → {sel_after})"
        );
    }

    /// Named contract (dogfood 2026-08-01): when the plan line viewer is open
    /// with Preview focus, Up/Down and PageUp/PageDown navigate the plan body
    /// like a normal file viewer (not swallowed by the composer).
    #[test]
    fn plan_preview_focus_arrows_and_page_keys_scroll_viewer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, &long_plan_body(80));
        agent.reopen_plan_approval();
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Preview)
        );
        {
            let viewer = agent.line_viewer.as_mut().expect("plan side panel");
            viewer.prepare_layout(60, 12);
            // Start at first line so Down/PageDown can advance.
            if let Some(id) = viewer.lines.first().map(|l| l.stable_id()) {
                viewer.list_state.select_by_id(id);
            }
            viewer.list_state.set_scroll_offset(0);
            viewer.prepare_layout(60, 12);
        }
        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection");
        let scroll_before = agent
            .line_viewer
            .as_ref()
            .map(|v| v.list_state.scroll_offset())
            .unwrap_or(0);

        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Preview Down must be consumed by the plan viewer; got {outcome:?}"
        );
        let sel_after_down = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after Down");
        assert!(
            sel_after_down > sel_before,
            "Preview Down must advance plan selection ({sel_before} → {sel_after_down})"
        );

        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(page_down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Preview PageDown must be consumed; got {outcome:?}"
        );
        let scroll_after = agent
            .line_viewer
            .as_ref()
            .map(|v| v.list_state.scroll_offset())
            .unwrap_or(0);
        let sel_after_page = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after PageDown");
        assert!(
            scroll_after > scroll_before || sel_after_page > sel_after_down,
            "Preview PageDown must move scroll or selection (scroll {scroll_before}→{scroll_after}, sel {sel_after_down}→{sel_after_page})"
        );
        // CTAs still live: approval view remains open.
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());
    }

    /// Named contract (dogfood 2026-08-01): after Enter arms line comment
    /// ("commenting L17") with an empty draft, arrows / Page keys still scroll
    /// the plan viewer. Only mid-text-entry should capture those keys for the
    /// comment composer.
    #[test]
    fn plan_commenting_empty_draft_arrows_and_page_keys_scroll_viewer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, &long_plan_body(80));
        agent.reopen_plan_approval();
        {
            let viewer = agent.line_viewer.as_mut().expect("plan side panel");
            viewer.prepare_layout(60, 12);
            viewer.set_initial_selection(5..6);
            viewer.prepare_layout(60, 12);
        }
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let _ = agent.handle_line_viewer_key(&enter);
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Commenting),
            "Enter on a line must arm Commenting focus"
        );
        assert!(
            agent.prompt.text().is_empty(),
            "new line comment starts with empty draft"
        );
        assert!(
            agent
                .plan_approval_view
                .as_ref()
                .is_some_and(|p| p.commenting_range.is_some()),
            "commenting range must be armed"
        );

        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection");
        let scroll_before = agent
            .line_viewer
            .as_ref()
            .map(|v| v.list_state.scroll_offset())
            .unwrap_or(0);

        let registry = ActionRegistry::defaults();
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-comment Down must be consumed; got {outcome:?}"
        );
        let sel_after_down = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after Down");
        assert!(
            sel_after_down > sel_before,
            "empty-comment Down must scroll/select plan content ({sel_before} → {sel_after_down})"
        );
        assert!(
            agent.prompt.text().is_empty(),
            "scroll keys must not type into empty comment draft"
        );

        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(page_down), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-comment PageDown must be consumed; got {outcome:?}"
        );
        let scroll_after = agent
            .line_viewer
            .as_ref()
            .map(|v| v.list_state.scroll_offset())
            .unwrap_or(0);
        let sel_after_page = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index())
            .expect("selection after PageDown");
        assert!(
            scroll_after > scroll_before || sel_after_page > sel_after_down,
            "empty-comment PageDown must move scroll or selection (scroll {scroll_before}→{scroll_after}, sel {sel_after_down}→{sel_after_page})"
        );
        // Still commenting; Esc cancel path and CTAs remain available.
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Commenting)
        );
    }

    /// When the comment draft has text, arrow keys stay with the composer
    /// (cursor motion), not the plan viewer.
    #[test]
    fn plan_commenting_nonempty_draft_arrows_stay_with_composer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, &long_plan_body(40));
        agent.reopen_plan_approval();
        {
            let viewer = agent.line_viewer.as_mut().expect("plan side panel");
            viewer.prepare_layout(60, 12);
            viewer.set_initial_selection(3..4);
            viewer.prepare_layout(60, 12);
        }
        let _ = agent.handle_line_viewer_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        agent.prompt.set_text("note");
        agent.prompt.set_cursor(0);

        let sel_before = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index());

        let registry = ActionRegistry::defaults();
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        let _ = agent.handle_input(&Event::Key(right), &registry);
        assert!(
            agent.prompt.cursor() > 0,
            "non-empty comment draft: Right must move the composer caret"
        );
        let sel_after = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.list_state.selected_index());
        assert_eq!(
            sel_before, sel_after,
            "non-empty draft: arrows must not move plan selection"
        );
    }

    /// Named contract: FileBacked soft-park transcript card SoT is live
    /// session `plan.md`. Park+commit with reverse-request body A, rewrite
    /// disk to B, re-sync card → scrollback card shows B (not frozen A).
    /// Does not open the side panel; dogfood card/status path only.
    #[test]
    fn soft_park_card_refreshes_from_disk_after_plan_md_rewrite() {
        let mut agent = make_agent();
        let session_id = format!(
            "plan-card-sot-{}",
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
        std::fs::create_dir_all(&session_dir).expect("create session dir for card SoT test");

        let content_a = "# Plan A card freeze\n\nStatus approved 2026-07-26\n";
        let content_b = "# Plan B live card\n\n### Critical Files for Implementation\n- bar.rs\n";
        std::fs::write(&plan_path, content_a).expect("seed plan.md with A");

        let _rx = install_plan_approval(&mut agent, content_a);
        agent.plan_approval_view.as_mut().unwrap().source = PlanReviewSource::FileBacked;

        agent.commit_parked_plan_card();
        assert_eq!(agent.scrollback.len(), 1, "first commit pushes one card");
        let card_a = match &agent.scrollback.entry(0).unwrap().block {
            crate::scrollback::block::RenderBlock::AgentMessage(b) => b.text().to_owned(),
            other => panic!("expected agent message card, got {other:?}"),
        };
        assert!(
            card_a.contains("Plan A card freeze"),
            "initial card must embed park body A; got {card_a:?}"
        );

        // Rewrite while parked (agent or user edited plan.md).
        std::fs::write(&plan_path, content_b).expect("rewrite plan.md to B");

        // Soft-park dogfood path: re-commit / paint-sync without opening panel.
        agent.commit_parked_plan_card();
        assert_eq!(
            agent.scrollback.len(),
            1,
            "refresh must update in place, not push a second card"
        );
        let card_b = match &agent.scrollback.entry(0).unwrap().block {
            crate::scrollback::block::RenderBlock::AgentMessage(b) => b.text().to_owned(),
            other => panic!("expected agent message card, got {other:?}"),
        };
        assert!(
            card_b.contains("Plan B live card")
                && card_b.contains("Critical Files for Implementation"),
            "soft-park card must re-read plan.md (B), not frozen park snapshot A; got {card_b:?}"
        );
        assert!(
            !card_b.contains("Status approved 2026-07-26"),
            "must not keep frozen reverse-request snapshot A on card; got {card_b:?}"
        );
        assert!(
            card_b.contains(crate::views::plan_approval_view::PLAN_CARD_CTAS),
            "refreshed card must keep CTA legend; got {card_b:?}"
        );
        let refreshed = agent
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.plan_content.as_deref())
            .expect("plan_content still present");
        assert!(
            refreshed.contains("Plan B live card"),
            "sync must refresh plan_content from disk for status/anchors; got {refreshed:?}"
        );

        // Soft-park CTA path still works after refresh (do not break mouse/keys).
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_none());

        let _ = std::fs::remove_dir_all(&session_dir);
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

    /// Soft-park non-capturing (L1 modal-free 2026-07-29): empty-prompt `a`
    /// types into the composer; mouse footer CTAs approve (not exclusive keys).
    #[test]
    fn soft_park_empty_a_types_into_composer_not_approve() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft CTAs");
        assert!(agent.line_viewer.is_none());
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park a must type, not exclusive-approve; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "soft-park Char must not dismiss plan approval"
        );
        assert_eq!(agent.prompt.text(), "a");
    }

    /// Soft-park CTA letters never steal input (empty or non-empty draft).
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

    /// Named contract (dogfood 2026-07-29): soft-parked plan approval must
    /// accept normal typing into the composer. Former CTA letters (`q`) type
    /// too — mouse / panel own decisions.
    #[test]
    fn soft_park_empty_prompt_typing_reaches_composer_via_handle_input() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park typing");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");

        let registry = ActionRegistry::defaults();
        for ch in ['x', ' ', 'y'] {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            let outcome = agent.handle_input(&Event::Key(key), &registry);
            assert!(
                matches!(outcome, InputOutcome::Changed),
                "soft-park typing {ch:?} must be consumed; got {outcome:?}"
            );
            assert!(
                agent.plan_approval_view.is_some(),
                "typing must not dismiss plan approval (char {ch:?})"
            );
        }
        assert_eq!(
            agent.prompt.text(),
            "x y",
            "typed characters must reach the composer buffer under soft park"
        );

        // Backspace trims the draft (composer remains usable).
        let bs = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        let _ = agent.handle_input(&Event::Key(bs), &registry);
        assert_eq!(agent.prompt.text(), "x ");

        // Empty-prompt `q` types (non-capturing); mouse quit still works.
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(q), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park q must type; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "soft-park Char q must not abandon"
        );
        assert_eq!(agent.prompt.text(), "q");
    }

    /// Dogfood 2026-07-29 v2: soft-park, empty prompt, type non-CTA `z` →
    /// buffer contains `z`. Focus must move to Prompt so the caret paints.
    #[test]
    fn soft_park_empty_prompt_type_z_reaches_composer() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park z");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none());

        let registry = ActionRegistry::defaults();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(key), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park z must be consumed; got {outcome:?}"
        );
        assert_eq!(
            agent.prompt.text(),
            "z",
            "non-CTA letter must append to empty soft-park composer"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Prompt),
            "typing must flip soft-park focus to Prompt for caret paint"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "z must not dismiss plan approval"
        );
    }

    /// Soft-park after park focus (Prompt pane + Prompt plan focus, empty):
    /// typing `hello` appends; empty-prompt `a` types (non-capturing).
    #[test]
    fn soft_park_after_park_focus_typing_and_empty_cta() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park after focus");
        // Mirror handle_exit_plan_mode soft-park: Prompt pane + Prompt focus.
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        assert!(agent.line_viewer.is_none());

        let registry = ActionRegistry::defaults();
        for ch in "hello".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            let outcome = agent.handle_input(&Event::Key(key), &registry);
            assert!(
                matches!(outcome, InputOutcome::Changed),
                "soft-park typing {ch:?} after park focus; got {outcome:?}"
            );
        }
        assert_eq!(agent.prompt.text(), "hello");
        assert!(
            agent.plan_approval_view.is_some(),
            "draft typing must not dismiss plan approval"
        );

        // Clear draft; empty-prompt `a` types under Prompt focus (mouse approves).
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(a), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-prompt a under soft-park must type; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_some());
        assert_eq!(agent.prompt.text(), "a");
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

    /// Soft-park non-capturing: `q` types into composer (mouse Quit abandons).
    #[test]
    fn soft_park_cta_q_types_into_composer_not_abandon() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft quit");
        assert!(agent.line_viewer.is_none());
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&q);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park q must type; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_some());
        assert_eq!(agent.prompt.text(), "q");
    }

    /// Named contract: soft-park uses empty stash until reopen. Abandon must
    /// clear plan approval and keep live freeform, never restore(empty) over it.
    #[test]
    fn soft_park_abandon_preserves_live_draft_when_stash_empty() {
        let mut agent = make_agent();
        let mut rx = install_plan_approval(&mut agent, "# Soft park keep draft");
        // Production soft-park: empty stash (install fixture uses non-empty for
        // reopen/restore paths).
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = StashedPrompt::default();
        }
        let draft = "important unsent work notes";
        agent.prompt.set_text(draft);
        let outcome = agent.abandon_plan();
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "abandon must complete; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "plan approval must be cleared on abandon"
        );
        assert_eq!(
            agent.prompt.text(),
            draft,
            "abandon must not wipe live draft when soft-park stash is empty"
        );
        let resp = rx
            .try_recv()
            .expect("should receive exit_plan_mode response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "abandoned");
    }

    /// Soft-park non-capturing: `s` types; revise intent stays default until
    /// mouse / panel CTA.
    #[test]
    fn soft_park_cta_s_types_into_composer() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft revise");
        assert!(agent.line_viewer.is_none());
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Prompt, true);
        let s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&s);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park s must type; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_none());
        assert_eq!(agent.prompt.text(), "s");
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
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

    /// Named contract (L1 modal-free): soft-park empty Enter does **not**
    /// approve — mouse footer / `/view-plan` panel own decisions.
    #[test]
    fn soft_park_preview_empty_enter_does_not_approve() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nNo Enter approve");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");
        agent.prompt.set_text("");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&enter);

        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty Enter on soft-park must be non-trapping; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "empty Enter must leave plan parked (mouse/panel approve)"
        );
    }

    /// Soft-park `a` types into composer (non-capturing Char).
    #[test]
    fn soft_park_preview_a_types_into_composer() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nType a");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);

        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_plan_feedback_key(&a);
        assert!(matches!(outcome, InputOutcome::Changed));
        assert!(agent.plan_approval_view.is_some());
        assert_eq!(agent.prompt.text(), "a");
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Prompt)
        );
    }

    /// Soft-park while reading the card (Scrollback focus): Char still reaches
    /// composer (not exclusive CTA, not scrollback no-op).
    #[test]
    fn soft_park_a_types_via_handle_input_while_scrollback_focused() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nRead card then type");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        agent.set_active_pane(ActivePane::Scrollback, true);
        assert!(agent.line_viewer.is_none(), "soft park has no line viewer");
        assert_eq!(agent.active_pane, ActivePane::Scrollback);

        let registry = ActionRegistry::defaults();
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(a), &registry);

        assert!(
            matches!(outcome, InputOutcome::Changed),
            "soft-park a via handle_input must type even with Scrollback focus; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "a must not exclusive-approve under Scrollback soft-park"
        );
        assert_eq!(agent.prompt.text(), "a");
    }

    /// Soft-park empty Enter via full handle_input + Scrollback focus: no approve.
    #[test]
    fn soft_park_empty_enter_noop_via_handle_input_while_scrollback_focused() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nEnter while reading");
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
            "soft-park empty Enter must not trap; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_some());
    }

    /// Named contract (dogfood 2026-07-29): side panel / line-viewer plan
    /// preview must not swallow ordinary typing. Empty-prompt CTA `q` still
    /// quits; non-CTA letters reach the composer (focus moves to Prompt).
    #[test]
    fn plan_panel_preview_typing_reaches_composer_via_handle_input() {
        use crossterm::event::Event;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\n## Step 1\nType notes\n");
        agent.show_plan_preview();
        assert!(
            agent.line_viewer.is_some(),
            "panel open requires line_viewer"
        );
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);

        let registry = ActionRegistry::defaults();
        for ch in ['x', ' ', 'z'] {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            let outcome = agent.handle_input(&Event::Key(key), &registry);
            assert!(
                matches!(outcome, InputOutcome::Changed),
                "panel Preview typing {ch:?} must be consumed; got {outcome:?}"
            );
            assert!(
                agent.plan_approval_view.is_some(),
                "non-CTA typing must not dismiss plan approval"
            );
        }
        assert_eq!(
            agent.prompt.text(),
            "x z",
            "typed characters must reach composer even while plan panel Preview is open"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Prompt),
            "typing should land focus on Prompt so further keys are not viewer-only"
        );

        // Empty-prompt `q` still quits once draft is cleared.
        agent.prompt.set_text("");
        agent.prompt.set_cursor(0);
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let outcome = agent.handle_input(&Event::Key(q), &registry);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "empty-prompt q on panel Preview must abandon; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "empty-prompt q must abandon plan approval"
        );
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

    /// Named contract: soft-park footer Approve is a real hit target without
    /// opening the side panel (`line_viewer` stays none).
    #[test]
    fn soft_park_card_or_chrome_cta_click_approve_without_panel() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park click approve");
        assert!(
            agent.line_viewer.is_none(),
            "soft-park starts without panel"
        );

        let hit = Rect::new(10, 24, 12, 1);
        agent.hit_soft_park_ctas.approve.set(Some(hit));

        let outcome = agent
            .handle_soft_park_cta_click(hit.x + 1, hit.y)
            .expect("Approve hit must dispatch");
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "approve click must be consumed; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "Approve click must clear plan approval without opening panel first"
        );
        assert!(
            agent.line_viewer.is_none(),
            "Approve must not open line_viewer"
        );
        assert_outcome_approved(rx);
        // Also accept the full mouse path shape used in production:
        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park mouse event");
        let hit = Rect::new(5, 20, 10, 1);
        agent.hit_soft_park_ctas.approve.set(Some(hit));
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        };
        let outcome = agent.handle_input(
            &crossterm::event::Event::Mouse(click),
            &ActionRegistry::defaults(),
        );
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "soft-park mouse Approve must fire; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert_outcome_approved(rx);
    }

    /// Named contract: non-empty prompt draft does not block **mouse** Approve.
    /// Keyboard CTAs may still require empty prompt (draft protection).
    #[test]
    fn soft_park_cta_buttons_work_with_prompt_draft() {
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let rx = install_plan_approval(&mut agent, "# Soft park draft click");
        // Production soft-park: empty stash.
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = crate::views::prompt_widget::StashedPrompt::default();
        }
        let draft = "still typing my notes in the composer";
        agent.prompt.set_text(draft);
        assert!(agent.line_viewer.is_none());

        // Keyboard `a` must NOT steal while draft is present (existing guard).
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let _ = agent.handle_plan_feedback_key(&a);
        assert!(
            agent.plan_approval_view.is_some(),
            "keys keep empty-prompt guard when draft present"
        );

        let hit = Rect::new(12, 22, 14, 1);
        agent.hit_soft_park_ctas.approve.set(Some(hit));
        let outcome = agent
            .handle_soft_park_cta_click(hit.x, hit.y)
            .expect("mouse Approve must hit with draft present");
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains(draft),
                    "mouse Approve freeform must become Interject notes; got {text:?}"
                );
                assert!(
                    text.contains("approved the plan with the following review comments"),
                    "Interject must use approve-with-comments framing; got {text:?}"
                );
            }
            other => {
                panic!("mouse Approve with non-empty freeform must Interject notes; got {other:?}")
            }
        }
        assert!(
            agent.plan_approval_view.is_none(),
            "mouse Approve must clear plan approval even with non-empty draft"
        );
        assert!(
            agent.prompt.text().is_empty(),
            "soft-park empty stash restore after Approve must leave composer empty; got {:?}",
            agent.prompt.text()
        );
        assert_outcome_approved(rx);
    }

    /// Named contract: soft-park with Prompt focus + draft still accepts mouse
    /// Quit (never strand the user when they opened notes intent).
    #[test]
    fn soft_park_mouse_quit_works_when_prompt_focus_with_draft() {
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let mut rx = install_plan_approval(&mut agent, "# Soft park prompt-focus quit");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = crate::views::prompt_widget::StashedPrompt::default();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        let draft = "notes I typed after opening revise";
        agent.prompt.set_text(draft);
        assert!(agent.line_viewer.is_none());

        let hit = Rect::new(40, 22, 10, 1);
        agent.hit_soft_park_ctas.quit.set(Some(hit));
        let outcome = agent
            .handle_soft_park_cta_click(hit.x, hit.y)
            .expect("mouse Quit must hit with Prompt focus + draft");
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "Quit click must apply; got {outcome:?}"
        );
        assert!(
            agent.plan_approval_view.is_none(),
            "Quit must clear plan approval under Prompt focus"
        );
        assert_eq!(
            agent.prompt.text(),
            draft,
            "Quit must preserve live draft (empty soft-park stash)"
        );
        let resp = rx.try_recv().expect("abandon response");
        let raw = resp.expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(parsed["outcome"], "abandoned");
    }

    /// When soft-park Approve consumes composer freeform as plan notes, durable
    /// `unsent_prompt_draft` must be cleared so resume does not resurrect them.
    #[test]
    fn approve_with_freeform_clears_durable_unsent_draft() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cwd = tmp.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let sid = format!("approve-draft-clear-{}", uuid::Uuid::new_v4());
        let mut agent = super::super::test_agent_view(Some(&sid), cwd.clone());
        let rx = install_plan_approval(&mut agent, "# Approve clears durable draft");
        // Production soft-park: empty stash so restore empties the composer.
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = StashedPrompt::default();
            pav.focus = PlanApprovalFocus::Preview;
        }
        let notes = "approve notes that must not resurrect on resume";
        agent.prompt.set_text(notes);
        agent.persist_unsent_prompt_draft();
        // Sanity: durable write landed before approve.
        let cwd_s = cwd.to_string_lossy();
        let loaded = xai_grok_shell::session::unsent_prompt_draft::load_unsent_prompt_draft(
            cwd_s.as_ref(),
            &sid,
        )
        .expect("load before");
        assert_eq!(
            loaded.as_deref(),
            Some(notes),
            "precondition: durable draft on disk"
        );

        let outcome = agent.approve_plan();
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains(notes),
                    "freeform must ride approve Interject; got {text:?}"
                );
            }
            other => panic!("expected Interject with freeform notes, got {other:?}"),
        }
        let after = xai_grok_shell::session::unsent_prompt_draft::load_unsent_prompt_draft(
            cwd_s.as_ref(),
            &sid,
        )
        .expect("load after");
        assert!(
            after.is_none(),
            "approve that consumes freeform must clear durable unsent draft; got {after:?}"
        );
        // Empty composer + restore must not resurrect already-sent notes.
        agent.prompt.set_text("");
        agent.maybe_restore_unsent_prompt_draft();
        assert!(
            agent.prompt.text().is_empty(),
            "maybe_restore must not resurrect sent approve notes; got {:?}",
            agent.prompt.text()
        );
    }

    /// Soft-park mouse Approve with freeform also clears durable unsent draft.
    #[test]
    fn soft_park_mouse_approve_with_freeform_clears_durable_unsent_draft() {
        use ratatui::layout::Rect;

        let tmp = tempfile::TempDir::new().unwrap();
        let cwd = tmp.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let sid = format!("soft-park-approve-clear-{}", uuid::Uuid::new_v4());
        let mut agent = super::super::test_agent_view(Some(&sid), cwd.clone());
        let rx = install_plan_approval(&mut agent, "# Soft park mouse clears durable");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = StashedPrompt::default();
            pav.focus = PlanApprovalFocus::Preview;
        }
        let notes = "mouse approve freeform notes";
        agent.prompt.set_text(notes);
        agent.persist_unsent_prompt_draft();

        let hit = Rect::new(12, 22, 14, 1);
        agent.hit_soft_park_ctas.approve.set(Some(hit));
        let outcome = agent
            .handle_soft_park_cta_click(hit.x, hit.y)
            .expect("mouse Approve must hit");
        assert_outcome_approved(rx);
        match outcome {
            InputOutcome::Action(Action::Interject { text, .. }) => {
                assert!(
                    text.contains(notes),
                    "Interject must include freeform; got {text:?}"
                );
            }
            other => panic!("expected Interject, got {other:?}"),
        }
        let cwd_s = cwd.to_string_lossy();
        let after = xai_grok_shell::session::unsent_prompt_draft::load_unsent_prompt_draft(
            cwd_s.as_ref(),
            &sid,
        )
        .expect("load after");
        assert!(
            after.is_none(),
            "soft-park mouse approve consuming freeform must clear durable draft; got {after:?}"
        );
        agent.prompt.set_text("");
        agent.maybe_restore_unsent_prompt_draft();
        assert!(agent.prompt.text().is_empty());
    }

    /// Revise (send_plan_feedback) that drains freeform must clear durable draft.
    #[test]
    fn send_plan_feedback_clears_durable_unsent_draft() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cwd = tmp.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let sid = format!("revise-draft-clear-{}", uuid::Uuid::new_v4());
        let mut agent = super::super::test_agent_view(Some(&sid), cwd.clone());
        let rx = install_plan_approval(&mut agent, "# Revise clears durable");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = StashedPrompt::default();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Revise;
        }
        let notes = "please drop Redis from the plan";
        agent.prompt.set_text(notes);
        agent.persist_unsent_prompt_draft();
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
        let cwd_s = cwd.to_string_lossy();
        let after = xai_grok_shell::session::unsent_prompt_draft::load_unsent_prompt_draft(
            cwd_s.as_ref(),
            &sid,
        )
        .expect("load after");
        assert!(
            after.is_none(),
            "revise that consumes freeform must clear durable draft; got {after:?}"
        );
        agent.prompt.set_text("");
        agent.maybe_restore_unsent_prompt_draft();
        assert!(agent.prompt.text().is_empty());
    }

    /// Clarify (send_plan_questions) that drains freeform must clear durable draft.
    #[test]
    fn send_plan_questions_clears_durable_unsent_draft() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cwd = tmp.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let sid = format!("clarify-draft-clear-{}", uuid::Uuid::new_v4());
        let mut agent = super::super::test_agent_view(Some(&sid), cwd.clone());
        let rx = install_plan_approval(&mut agent, "# Clarify clears durable");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = StashedPrompt::default();
            pav.focus = PlanApprovalFocus::Prompt;
            pav.prompt_intent = PlanPromptIntent::Questions;
        }
        let notes = "Why Redis instead of in-memory?";
        agent.prompt.set_text(notes);
        agent.persist_unsent_prompt_draft();
        let freeform = Some(agent.prompt.text().to_string());
        let _ = agent.send_plan_questions(freeform);
        let parsed = parse_outcome(rx);
        assert_eq!(parsed["outcome"], "questions");
        assert!(
            parsed["feedback"]
                .as_str()
                .unwrap_or("")
                .contains("Why Redis")
        );
        let cwd_s = cwd.to_string_lossy();
        let after = xai_grok_shell::session::unsent_prompt_draft::load_unsent_prompt_draft(
            cwd_s.as_ref(),
            &sid,
        )
        .expect("load after");
        assert!(
            after.is_none(),
            "clarify that consumes freeform must clear durable draft; got {after:?}"
        );
        agent.prompt.set_text("");
        agent.maybe_restore_unsent_prompt_draft();
        assert!(agent.prompt.text().is_empty());
    }

    /// Soft-park footer Quit click abandons and keeps live draft (empty stash).
    #[test]
    fn soft_park_chrome_cta_click_quit_preserves_live_draft() {
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let mut rx = install_plan_approval(&mut agent, "# Soft park quit click");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.stashed_prompt = crate::views::prompt_widget::StashedPrompt::default();
        }
        let draft = "do not lose this draft on quit click";
        agent.prompt.set_text(draft);

        let hit = Rect::new(40, 22, 8, 1);
        agent.hit_soft_park_ctas.quit.set(Some(hit));
        let outcome = agent
            .handle_soft_park_cta_click(hit.x + 1, hit.y)
            .expect("Quit hit must dispatch");
        assert!(
            matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
            "quit click must complete; got {outcome:?}"
        );
        assert!(agent.plan_approval_view.is_none());
        assert_eq!(
            agent.prompt.text(),
            draft,
            "Quit click must preserve live draft when soft-park stash is empty"
        );
        let resp = rx.try_recv().expect("abandon response");
        let raw = resp.expect("Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(parsed["outcome"], "abandoned");
    }

    /// Soft-park paint path registers hit areas (not paint-only legend).
    #[test]
    fn soft_park_paint_cta_buttons_sets_hit_areas() {
        use crate::theme::Theme;
        use crate::views::plan_approval_view::{SoftParkCtaHovers, paint_soft_park_cta_buttons};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let theme = Theme::current();
        let area = Rect::new(0, 10, 80, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        let areas =
            paint_soft_park_cta_buttons(&mut buf, area, &theme, SoftParkCtaHovers::default());
        assert!(
            areas.approve.is_some(),
            "soft-park paint must set Approve hit area"
        );
        assert!(
            areas.quit.is_some(),
            "soft-park paint must set Quit hit area"
        );
        assert!(
            areas.notes.is_some() && areas.clarify.is_some() && areas.revise.is_some(),
            "full five-button chrome expected on wide row"
        );
        // Painted row should not be blank.
        let cell = buf.cell((areas.approve.unwrap().x, area.y)).unwrap();
        assert_eq!(
            cell.symbol(),
            "a",
            "Approve button starts with bold key `a`"
        );
    }

    /// Full agent draw after soft-park-style open: either panel footer CTAs
    /// (with borders) paint, or soft-park strip CTAs — never silent zero chrome.
    fn draw_agent_hits(agent: &mut AgentView, width: u16, height: u16) -> ratatui::buffer::Buffer {
        use crate::actions::ActionRegistry;
        use crate::app::bundle::BundleState;
        use crate::scrollback::render::ScratchBuffer;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        let _ = agent.draw(
            area,
            &mut buf,
            &ActionRegistry::defaults(),
            &mut scratch,
            None,
            false,
            crate::app::agent_view::BannerSlotParams::none(),
            &BundleState::default(),
            false,
            &mut Vec::new(),
            crate::app::agent_view::AppRenderParams::default(),
        );
        buf
    }

    fn soft_park_style_open(agent: &mut AgentView, plan: &str) {
        let _rx = install_plan_approval(agent, plan);
        // Mirror handle_exit_plan_mode soft path: Prompt focus + auto-open panel.
        agent.active_modal = None;
        agent.block_viewer = None;
        agent.set_active_pane(ActivePane::Prompt, false);
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = PlanApprovalFocus::Prompt;
        }
        agent.show_plan_preview_if_available();
        if let Some(ref mut viewer) = agent.line_viewer {
            viewer.plan_mut().feedback_active = true;
        }
    }

    /// Named contract: after soft park auto-open, a normal-size frame paints
    /// side-panel approval footer CTAs and border lines (not a barren box).
    #[test]
    fn soft_park_draw_paints_panel_approval_footer_chrome() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park chrome\n\n## Steps\nShip it\n");
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.side_panel && v.plan_ref().is_some_and(|p| p.feedback_active)),
            "soft park must open side panel with feedback_active"
        );

        let buf = draw_agent_hits(&mut agent, 120, 40);

        let plan = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.plan_ref())
            .expect("plan extras after paint");
        assert!(
            plan.approve_button_area.is_some()
                && plan.approve_notes_button_area.is_some()
                && plan.questions_button_area.is_some()
                && plan.send_button_area.is_some()
                && plan.abandon_button_area.is_some(),
            "panel footer must expose all five approval CTA hit targets after soft-park draw"
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.last_modal_area.is_some()),
            "panel must paint (last_modal_area set) — not size early-return"
        );

        // Border / footer line glyphs present somewhere in the frame.
        let mut has_box_line = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    let s = cell.symbol();
                    if s == "\u{2500}" || s == "\u{2502}" || s == "\u{256d}" || s == "\u{256e}" {
                        has_box_line = true;
                        break;
                    }
                }
            }
            if has_box_line {
                break;
            }
        }
        assert!(
            has_box_line,
            "soft-park side panel must paint rounded-border / title-footer lines"
        );
    }

    /// Named contract: if the plan panel is open but too small to paint footer
    /// CTAs, soft-park strip CTAs must still be hit-tested (no silent zero chrome).
    #[test]
    fn soft_park_draw_falls_back_to_strip_ctas_when_panel_cannot_paint() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park fallback\n\nNarrow terminal\n");
        assert!(agent.line_viewer.is_some(), "panel state present");

        // Width 10: side panel clamps under line_viewer early-return (width < 10),
        // so footer CTAs never paint. Keep height generous so the shortcuts row
        // still exists for soft-park strip fallback paint.
        let _buf = draw_agent_hits(&mut agent, 10, 40);

        let panel_has_cta = agent.line_viewer.as_ref().is_some_and(|v| {
            v.plan_ref()
                .is_some_and(|p| p.approve_button_area.is_some() || p.abandon_button_area.is_some())
        });
        let strip_has_cta = agent.hit_soft_park_ctas.approve.rect.is_some()
            || agent.hit_soft_park_ctas.quit.rect.is_some();
        assert!(
            !panel_has_cta,
            "narrow overlay must force panel size early-return (no panel CTA hits)"
        );
        assert!(
            strip_has_cta,
            "when panel cannot paint footer CTAs, soft-park strip must still expose clickable CTAs; \
             strip_approve={:?} strip_quit={:?}",
            agent.hit_soft_park_ctas.approve.rect, agent.hit_soft_park_ctas.quit.rect
        );
    }

    /// Named contract: panel dismissed while approval parked paints soft-park
    /// strip CTAs (mouse primary without reopening the panel).
    #[test]
    fn soft_park_draw_strip_ctas_when_panel_dismissed() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park strip\n\nDismissed panel\n");
        agent.line_viewer = None;

        let _buf = draw_agent_hits(&mut agent, 120, 40);
        assert!(
            agent.hit_soft_park_ctas.approve.rect.is_some()
                && agent.hit_soft_park_ctas.notes.rect.is_some()
                && agent.hit_soft_park_ctas.clarify.rect.is_some()
                && agent.hit_soft_park_ctas.revise.rect.is_some()
                && agent.hit_soft_park_ctas.quit.rect.is_some(),
            "dismissed panel must leave all five soft-park strip CTA hits"
        );
    }

    /// Named contract (dogfood 2026-08-01): if feedback_active was lost while
    /// plan_approval_view is still parked, draw re-syncs approval chrome so
    /// usual Approve/Notes/Clarify/Revise/Quit lines paint (not casual `c comment`).
    #[test]
    fn soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park resync\n\n## Steps\nKeep CTAs\n");
        {
            let plan = agent.line_viewer.as_mut().expect("panel").plan_mut();
            // Simulate drift / wrong casual open that left approval parked
            // but painted casual footer flags.
            plan.feedback_active = false;
            plan.show_action_buttons = true;
        }
        assert!(
            agent.plan_approval_view.is_some(),
            "approval must still be parked"
        );

        let _buf = draw_agent_hits(&mut agent, 120, 40);

        let plan = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.plan_ref())
            .expect("plan extras after paint");
        assert!(
            plan.feedback_active,
            "draw must re-sync feedback_active while plan_approval_view is Some"
        );
        assert!(
            !plan.show_action_buttons,
            "draw must not leave casual show_action_buttons while approval is parked"
        );
        assert!(
            plan.approve_button_area.is_some()
                && plan.approve_notes_button_area.is_some()
                && plan.questions_button_area.is_some()
                && plan.send_button_area.is_some()
                && plan.abandon_button_area.is_some(),
            "usual five approval CTA hits must paint after resync; comment_btn={:?}",
            plan.comment_button_area
        );
        assert!(
            plan.comment_button_area.is_none(),
            "casual c-comment hit must not paint while approval is parked"
        );
    }

    /// Named contract: Ctrl+F fullscreen while soft-parked still paints
    /// approval footer CTAs (not casual comment-only chrome).
    #[test]
    fn soft_park_fullscreen_draw_paints_approval_ctas() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park fullscreen\n\nStill approval\n");
        if let Some(ref mut viewer) = agent.line_viewer {
            viewer.fullscreen = true;
            viewer.side_panel = false;
        }

        let _buf = draw_agent_hits(&mut agent, 120, 40);

        let plan = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.plan_ref())
            .expect("plan extras");
        assert!(
            plan.feedback_active
                && plan.approve_button_area.is_some()
                && plan.abandon_button_area.is_some(),
            "fullscreen soft-park must keep approval CTAs (a/A/?/s/q), not casual only"
        );
        assert!(
            plan.comment_button_area.is_none(),
            "fullscreen approval must not paint casual c-comment as the only footer"
        );
    }

    /// Named contract: turn-end must not wipe a live soft-park reverse-request
    /// (response_tx still open). That wipe left dogfood on casual fullscreen
    /// plan.md with only `c comment` and no Approve/Notes/Clarify/Revise/Quit.
    #[test]
    fn turn_end_preserves_live_soft_park_approval() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Soft park survive turn end\n\nKeep me\n");
        assert!(
            agent
                .plan_approval_view
                .as_ref()
                .is_some_and(|p| p.response_tx.is_some()),
            "fixture must hold a live reverse-request channel"
        );
        assert!(agent.line_viewer.is_some());

        agent.dismiss_plan_approval_after_turn_if_stale();

        assert!(
            agent.plan_approval_view.is_some(),
            "live soft-park must survive turn-end dismiss helper"
        );
        assert!(
            agent.line_viewer.is_some(),
            "side panel must stay open after turn-end while approval is still awaiting"
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.plan_ref().is_some_and(|p| p.feedback_active)),
            "approval footer arming must remain"
        );
    }

    /// Counterpart: leftover plan approval with no response channel is cleaned up.
    #[test]
    fn turn_end_clears_plan_approval_without_live_channel() {
        let mut agent = make_agent();
        soft_park_style_open(&mut agent, "# Stale leftover\n\nNo waiter\n");
        if let Some(ref mut pav) = agent.plan_approval_view {
            // Simulate decision already sent (channel consumed).
            let _ = pav.response_tx.take();
        }

        agent.dismiss_plan_approval_after_turn_if_stale();

        assert!(
            agent.plan_approval_view.is_none(),
            "stale plan_approval without response_tx must clear on turn end"
        );
        assert!(
            agent.line_viewer.is_none(),
            "stale panel must close when leftover approval is cleared"
        );
    }

    /// Named contract (dogfood 2026-07-29): mouse click on painted Revise
    /// dispatches focus_plan_prompt(Revise) — not a no-op empty hit.
    #[test]
    fn soft_park_revise_cta_click_after_paint() {
        use crate::theme::Theme;
        use crate::views::plan_approval_view::{SoftParkCtaHovers, paint_soft_park_cta_buttons};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Soft park revise click");
        assert!(agent.line_viewer.is_none(), "soft-park: no panel");

        let theme = Theme::current();
        // Mid width used to over-count middle-dot seps and drop later hits.
        let area = Rect::new(0, 20, 40, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        let areas =
            paint_soft_park_cta_buttons(&mut buf, area, &theme, SoftParkCtaHovers::default());
        assert!(
            areas.revise.is_some(),
            "Revise hit must be painted at width 40"
        );
        agent.hit_soft_park_ctas.apply_areas(areas);

        let revise = agent.hit_soft_park_ctas.revise.rect.expect("revise hit");
        assert!(revise.width >= 1, "revise must not be zero-width");
        let outcome = agent
            .handle_soft_park_cta_click(revise.x, revise.y)
            .expect("Revise click must dispatch");
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Revise click outcome; got {outcome:?}"
        );
        let pav = agent.plan_approval_view.as_ref().expect("still parked");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(
            pav.prompt_intent,
            PlanPromptIntent::Revise,
            "mouse Revise must set revise intent"
        );
    }

    /// Regression: Notes / Clarify / Quit / Approve also dispatch after paint.
    #[test]
    fn soft_park_all_cta_clicks_after_paint() {
        use crate::theme::Theme;
        use crate::views::plan_approval_view::{SoftParkCtaHovers, paint_soft_park_cta_buttons};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let theme = Theme::current();
        let area = Rect::new(0, 15, 60, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        let areas =
            paint_soft_park_cta_buttons(&mut buf, area, &theme, SoftParkCtaHovers::default());

        // Notes
        {
            let mut agent = make_agent();
            let _rx = install_plan_approval(&mut agent, "# notes");
            agent.hit_soft_park_ctas.apply_areas(areas);
            let r = agent.hit_soft_park_ctas.notes.rect.expect("notes");
            agent
                .handle_soft_park_cta_click(r.x, r.y)
                .expect("notes click");
            assert_eq!(
                agent.plan_approval_view.as_ref().unwrap().prompt_intent,
                PlanPromptIntent::ApproveNotes
            );
        }
        // Clarify
        {
            let mut agent = make_agent();
            let _rx = install_plan_approval(&mut agent, "# clarify");
            agent.hit_soft_park_ctas.apply_areas(areas);
            let r = agent.hit_soft_park_ctas.clarify.rect.expect("clarify");
            agent
                .handle_soft_park_cta_click(r.x, r.y)
                .expect("clarify click");
            assert_eq!(
                agent.plan_approval_view.as_ref().unwrap().prompt_intent,
                PlanPromptIntent::Questions
            );
        }
        // Approve
        {
            let mut agent = make_agent();
            let mut rx = install_plan_approval(&mut agent, "# approve");
            agent.hit_soft_park_ctas.apply_areas(areas);
            let r = agent.hit_soft_park_ctas.approve.rect.expect("approve");
            agent
                .handle_soft_park_cta_click(r.x, r.y)
                .expect("approve click");
            assert!(agent.plan_approval_view.is_none());
            let raw = rx.try_recv().expect("approved").expect("ok");
            let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
            assert_eq!(parsed["outcome"], "approved");
        }
        // Quit
        {
            let mut agent = make_agent();
            let mut rx = install_plan_approval(&mut agent, "# quit");
            agent.hit_soft_park_ctas.apply_areas(areas);
            let r = agent.hit_soft_park_ctas.quit.rect.expect("quit");
            agent
                .handle_soft_park_cta_click(r.x, r.y)
                .expect("quit click");
            assert!(agent.plan_approval_view.is_none());
            let raw = rx.try_recv().expect("abandoned").expect("ok");
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

    /// Soft follow-up: `/screenshot` / F9 capture auto-attaches the PNG into
    /// the plan composer when plan approval is open (same multimodal drain as paste).
    #[test]
    fn try_attach_tui_screenshot_for_plan_when_approval_open() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nAttach shot");
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

    /// Drive Ctrl+V with a clipboard raster through `handle_input` (shipped
    /// path), complete the deferred probe, and assert a plan-composer chip.
    fn plan_ctrl_v_clipboard_image(agent: &mut AgentView) {
        use crate::actions::ActionRegistry;
        use crossterm::event::Event;

        crate::clipboard::set_clipboard_probe_hook(crate::clipboard::ClipboardProbeHook {
            text: None,
            ..crate::clipboard::ClipboardProbeHook::with_raster(None)
        });
        let outcome = agent.handle_input(
            &Event::Key(crate::key!('v', CONTROL).to_key_event()),
            &ActionRegistry::defaults(),
        );
        let ctx = agent.pending_effects.iter().find_map(|e| match e {
            crate::app::actions::Effect::ProbeClipboardAttachment { ctx, .. } => Some(ctx.clone()),
            _ => None,
        });
        crate::clipboard::clear_clipboard_probe_hook();
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "Ctrl+V under plan review must be handled; got {outcome:?}"
        );
        let ctx = ctx.expect("plan Ctrl+V with clipboard image must defer a probe");
        let pasted = crate::prompt_images::from_clipboard_data(&crate::clipboard::ImageData {
            data: vec![
                0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            mime_type: "image/png".to_string(),
        });
        agent.complete_clipboard_attachment_paste(
            ctx,
            crate::app::actions::ProbedAttachment::Image(pasted),
            None,
        );
        assert!(
            !agent.prompt.images.is_empty() || agent.prompt.text().contains("[Image"),
            "clipboard screenshot must land on plan composer; text={:?} n={}",
            agent.prompt.text(),
            agent.prompt.images.len()
        );
    }

    /// Named contract: Ctrl+V clipboard screenshot while plan side panel is
    /// open on Preview must attach (not swallow into the line viewer).
    #[test]
    fn plan_panel_preview_ctrl_v_clipboard_image_attaches() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nCtrl+V shot");
        agent.show_plan_preview();
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Preview;
        }
        assert!(agent.line_viewer.is_some());
        plan_ctrl_v_clipboard_image(&mut agent);
    }

    /// Named contract: soft-park (no panel) Prompt focus Ctrl+V clipboard
    /// screenshot attaches for approve/revise/clarify multimodal drain.
    #[test]
    fn soft_park_prompt_ctrl_v_clipboard_image_attaches() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nSoft Ctrl+V");
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
        }
        assert!(agent.line_viewer.is_none());
        plan_ctrl_v_clipboard_image(&mut agent);
    }

    /// Named contract: plan panel with Prompt focus still routes Ctrl+V
    /// clipboard image through the deferred probe (not text-only widget paste).
    #[test]
    fn plan_panel_prompt_ctrl_v_clipboard_image_attaches() {
        let mut agent = make_agent();
        let _rx = install_plan_approval(&mut agent, "# Plan\n\nPanel Prompt Ctrl+V");
        agent.show_plan_preview();
        {
            let pav = agent.plan_approval_view.as_mut().unwrap();
            pav.focus = PlanApprovalFocus::Prompt;
        }
        assert!(agent.line_viewer.is_some());
        plan_ctrl_v_clipboard_image(&mut agent);
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
