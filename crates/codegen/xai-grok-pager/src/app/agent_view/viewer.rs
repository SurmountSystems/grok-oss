//! Line and block viewer popups plus the /btw panel: open/confirm/dismiss
//! and their key/mouse handlers.

use super::{AgentView, render_char_buttons};
use crate::app::app_view::InputOutcome;
use crate::key;
use crate::scrollback::selection::SelectionBox;
use crate::scrollback::types::DisplayMode;
use crate::theme::Theme;
use crate::views::btw_overlay::BTW_OVERLAY_ENTRY_IDX;
use crate::views::file_search::line_viewer::{LineViewerState, PlanViewerItem, SelectedPlanCta};
use crate::views::list_pane::ListItem;
use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Bare typing while plan.md is open: letters and delete keys go to the
/// composer. Ctrl+Backspace / Alt+Backspace / Ctrl+Delete are word-edit
/// on that composer. Left/Right, Ctrl/Alt word-move, and Ctrl-A/E stay
/// on that composer too. Ctrl/Cmd+Z (undo) and Shift+Z (redo) stay on
/// that composer so a wiped Human box can come back while Preview is
/// focused. Enter, Shift+Enter, and Alt+Enter stay on that composer so
/// Preview matches the main Human box (newline or send). Other
/// Ctrl/Alt/Super chords stay with the viewer (fullscreen, quit,
/// worktree).
pub(super) fn plan_preview_key_is_composer_text(key: &KeyEvent) -> bool {
    if matches!(key.code, KeyCode::Char('z' | 'Z'))
        && key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
    {
        return true;
    }
    // Shift+Enter / Alt+Enter (and Apple Terminal rescued Enter) must
    // reach the Human box. Without this, Preview forwards them to the
    // plan list, so the overlay can steal copy / clarify / row walk.
    if crate::input::is_mod_enter(key) {
        return true;
    }
    if key.code == KeyCode::Enter && key.modifiers.is_empty() {
        return true;
    }
    if key.modifiers.contains(KeyModifiers::SUPER) {
        return false;
    }
    let word_edit = matches!(key.code, KeyCode::Backspace | KeyCode::Delete)
        && key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    if word_edit {
        return true;
    }
    let composer_cursor = match key.code {
        KeyCode::Left | KeyCode::Right => {
            key.modifiers.is_empty()
                || key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        }
        KeyCode::Char('a' | 'e') => key.modifiers == KeyModifiers::CONTROL,
        _ => false,
    };
    if composer_cursor {
        return true;
    }
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return false;
    }
    matches!(
        key.code,
        KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
    )
}

impl AgentView {
    // ── Line viewer methods ────────────────────────────────────────────

    /// Open the line viewer for a file path with optional initial line range.
    pub(in crate::app) fn open_line_viewer(
        &mut self,
        path: &std::path::Path,
        initial_range: Option<std::ops::Range<usize>>,
    ) {
        // Resolve path relative to cwd.
        let full_path = if path.is_relative() {
            self.session.cwd.join(path)
        } else {
            path.to_path_buf()
        };

        // Get the element ID of the last file ref element (just created).
        let element_id = self
            .prompt
            .textarea
            .elements()
            .iter()
            .rev()
            .find(|e| e.kind == crate::views::prompt_widget::KIND_FILE_REF)
            .map(|e| e.id);

        if let Some(mut viewer) = LineViewerState::open(&full_path, element_id) {
            // If we have an initial line range, scroll to it and select.
            if let Some(range) = initial_range {
                viewer.set_initial_selection(range);
            }
            self.line_viewer = Some(viewer);
        } else {
            // File couldn't be read — cancel the undo group.
            self.prompt.textarea.cancel_undo_group();
        }
    }

    pub(super) fn copy_plan_full(&mut self) -> InputOutcome {
        let text = self
            .line_viewer
            .as_ref()
            .and_then(|v| v.markdown_content_for_feedback())
            .filter(|s| !s.is_empty())
            .or_else(|| self.plan_body_for_preview());
        if let Some(text) = text {
            self.copy_to_clipboard(&text);
        }
        InputOutcome::Changed
    }

    fn mark_selected_plan_cta(&mut self, choice: SelectedPlanCta) {
        if let Some(viewer) = self.line_viewer.as_mut() {
            viewer.plan_mut().selected_cta = Some(choice);
        }
    }

    fn selected_plan_cta(&self) -> Option<SelectedPlanCta> {
        self.line_viewer
            .as_ref()
            .and_then(|v| v.plan_ref())
            .and_then(|p| p.selected_cta)
    }

    fn plan_cta_has_comment_payload(&self) -> bool {
        !self.prompt.text().trim().is_empty()
            || self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| !pav.comments.is_empty())
    }

    /// Click marks the CTA and runs it. Enter also submits the marked CTA.
    /// First click on Approve still Approves. Letter keys type; they do
    /// not steal Approve. A second click on an already-marked CTA still
    /// runs that action.
    fn click_plan_cta(&mut self, choice: SelectedPlanCta) -> InputOutcome {
        self.activate_selected_plan_cta(choice)
    }

    /// Enter (and a second click) run the marked idle CTA.
    fn activate_selected_plan_cta(&mut self, choice: SelectedPlanCta) -> InputOutcome {
        self.mark_selected_plan_cta(choice);
        match choice {
            SelectedPlanCta::Approve => self.approve_plan(),
            SelectedPlanCta::Comment => self.focus_plan_prompt(PlanPromptIntent::Comment),
            SelectedPlanCta::Clarify => {
                if self.plan_cta_has_comment_payload() {
                    let text = self.prompt.text().to_string();
                    let freeform = if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    };
                    self.send_plan_questions(freeform)
                } else {
                    self.focus_plan_prompt(PlanPromptIntent::Questions)
                }
            }
            SelectedPlanCta::Revise => {
                if self.plan_cta_has_comment_payload() {
                    let text = self.prompt.text().to_string();
                    let freeform = if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    };
                    self.send_plan_feedback(freeform)
                } else {
                    self.focus_plan_prompt(PlanPromptIntent::Revise)
                }
            }
            SelectedPlanCta::Exit => self.abandon_plan(),
        }
    }

    /// Empty Preview Enter submits the marked CTA. Empty Enter never Approves.
    fn submit_marked_idle_plan_cta(&mut self) -> InputOutcome {
        match self.selected_plan_cta() {
            None | Some(SelectedPlanCta::Approve) => self.send_composer_as_normal_prompt(),
            Some(choice) => self.activate_selected_plan_cta(choice),
        }
    }

    /// Handle a key event while the line viewer is open.
    pub(super) fn handle_line_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let in_plan_approval = self.plan_approval_view.is_some();
        let plan_present = in_plan_approval || self.is_plan_viewer();

        // Idle or cancelling plan present: `x`/`e`/`j`/`k` type in the Human
        // box. They must not become list capture (delete / edit / row walk).
        if plan_present
            && matches!(key.code, KeyCode::Char('x' | 'e' | 'j' | 'k'))
            && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
        {
            return self.handle_plan_feedback_key(key);
        }

        let input_bar_active = self
            .line_viewer
            .as_ref()
            .is_some_and(|v| v.list_state.input_mode().is_some());

        // When the search/filter/goto input bar is active, let ListPane
        // handle everything. Comment mode is special: Enter/Esc are not
        // consumed by the list state (it returns false), so we handle
        // save/cancel here.
        if input_bar_active {
            let is_comment_mode = self.line_viewer.as_ref().is_some_and(|v| {
                v.list_state.input_mode() == Some(crate::views::list_pane::InputBarMode::Comment)
            });
            if is_comment_mode {
                if key!(Enter).matches(key) {
                    return self.save_casual_plan_comment();
                }
                if key!(Esc).matches(key) {
                    return self.cancel_casual_plan_commenting();
                }
            }
            if let Some(ref mut viewer) = self.line_viewer {
                viewer.list_state.handle_key_event(key, &viewer.lines);
            }
            return InputOutcome::Changed;
        }

        // Isolated plan.md / side panel is visual. Printable keys and
        // Backspace stay on the composer so present never steals typing.
        // Letter CTA keys type. Empty Preview `?` still arms Clarify. A
        // live draft or Prompt focus inserts `?`. Empty Preview `y` copies
        // the plan (footer `y:copy`). A live draft inserts `y`. Empty-prompt
        // `c` is the line-comment gesture (clicking a row is not). A live
        // draft types `c` so Preview does not stash-and-wipe the Human box.
        if in_plan_approval && key!('c').matches(key) {
            let already_commenting = self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
            if !already_commenting && self.prompt.text().trim().is_empty() {
                return self.enter_plan_commenting();
            }
        }
        if in_plan_approval && key!('y').matches(key) {
            let commenting = self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
            let empty = self.prompt.text().trim().is_empty() && !self.prompt.file_search_visible();
            // Empty Preview `y` copies (footer `y:copy`). Comment overlay
            // `y` copies even with a line-comment draft. A live Prompt or
            // Preview draft still types `y`.
            if commenting || empty {
                return self.copy_plan_full();
            }
        }
        if in_plan_approval && key!(Enter).matches(key) && !crate::input::is_mod_enter(key) {
            let focus = self.plan_approval_view.as_ref().map(|p| p.focus);
            if focus == Some(PlanApprovalFocus::Commenting)
                || focus == Some(PlanApprovalFocus::Prompt)
            {
                return self.handle_plan_feedback_key(key);
            }
            if !self.prompt.text().trim().is_empty() || !self.prompt.images.is_empty() {
                return self.handle_plan_feedback_key(key);
            }
            if self.selected_plan_cta().is_some() {
                return self.submit_marked_idle_plan_cta();
            }
        }
        if in_plan_approval && plan_preview_key_is_composer_text(key) {
            return self.handle_plan_feedback_key(key);
        }

        if in_plan_approval && crate::input::key::RowWalk::from_key(key).is_some() {
            return self.handle_plan_feedback_key(key);
        }

        // Plan-approval Esc dismisses the pane after visual/search clear.
        // That is not Approve and not Exit / abandon. Empty Ctrl+C abandons.
        if in_plan_approval && key!(Esc).matches(key) {
            if let Some(ref mut viewer) = self.line_viewer {
                if viewer.list_state.visual_mode {
                    viewer.list_state.exit_visual_mode();
                    return InputOutcome::Changed;
                }
                if viewer.list_state.matcher().is_some() {
                    viewer.list_state.handle_key_event(key, &viewer.lines);
                    return InputOutcome::Changed;
                }
            }
            self.cancel_line_viewer();
            return InputOutcome::Changed;
        }

        // Ctrl+F: toggle fullscreen.
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(ref mut viewer) = self.line_viewer {
                viewer.fullscreen = !viewer.fullscreen;
            }
            return InputOutcome::Changed;
        }

        // Casual mode: same `c` / `s` shortcuts as plan approval so the
        // footer hints actually work.
        if !in_plan_approval && self.is_plan_viewer() && key!('c').matches(key) {
            return self.enter_casual_plan_commenting();
        }
        if !in_plan_approval
            && self.is_plan_viewer()
            && key!('s').matches(key)
            && !self.plan_comments.is_empty()
        {
            return self.send_casual_plan_comments();
        }

        if in_plan_approval
            && key.code == KeyCode::Char('?')
            && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
            && self.prompt.text().trim().is_empty()
        {
            return self
                .focus_plan_prompt(crate::views::plan_approval_view::PlanPromptIntent::Questions);
        }

        if !in_plan_approval
            && self.is_plan_viewer()
            && !self.plan_comments.is_empty()
            && key.code == KeyCode::Enter
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return self.send_casual_plan_comments();
        }

        if key!(Enter).matches(key) {
            if in_plan_approval {
                let focus = self.plan_approval_view.as_ref().map(|p| p.focus);
                if focus == Some(PlanApprovalFocus::Prompt) {
                    return self.handle_plan_feedback_key(key);
                }
                // Preview: empty Enter never Approves. A draft submits as
                // a normal prompt so present cannot steal the composer.
                return self.send_composer_as_normal_prompt();
            }
            if self.is_plan_viewer() {
                return self.enter_casual_plan_commenting();
            }
            let has_visual = self
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.list_state.visual_mode);
            self.confirm_line_viewer(has_visual);
            return InputOutcome::Changed;
        }
        if key!('x').matches(key) {
            if in_plan_approval {
                return self.delete_plan_comment_at_cursor();
            }
            if self.is_plan_viewer() {
                return self.delete_casual_plan_comment_at_cursor();
            }
            self.confirm_line_viewer(false);
            return InputOutcome::Changed;
        }
        if key!('y').matches(key) {
            if self.is_plan_viewer() {
                return self.copy_plan_full();
            }
            if let Some(ref viewer) = self.line_viewer {
                let text = if viewer.list_state.visual_mode {
                    if let Some(ref range) = viewer.list_state.multi_range() {
                        let lines: Vec<String> = (range.start..range.end)
                            .filter_map(|vi| {
                                let pi = viewer.list_state.to_physical(vi);
                                viewer.lines.get(pi)
                            })
                            .map(|item| item.copy_text())
                            .collect();
                        Some(lines.join("\n"))
                    } else {
                        None
                    }
                } else {
                    viewer
                        .list_state
                        .selected_index()
                        .and_then(|vi| {
                            let pi = viewer.list_state.to_physical(vi);
                            viewer.lines.get(pi)
                        })
                        .map(|item| item.copy_text())
                };
                if let Some(text) = text
                    && !text.is_empty()
                {
                    self.copy_to_clipboard(&text);
                }
            }
            return InputOutcome::Changed;
        }
        if key!('Y').matches(key) {
            if self.is_plan_viewer() {
                return InputOutcome::Changed;
            }
            if let Some(ref viewer) = self.line_viewer {
                let name = viewer
                    .title_override
                    .as_deref()
                    .unwrap_or_else(|| {
                        viewer
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                    })
                    .to_owned();
                self.copy_to_clipboard(&name);
            }
            return InputOutcome::Changed;
        }
        if key!(Esc).matches(key) || key!('q').matches(key) || key!('c', CONTROL).matches(key) {
            if in_plan_approval {
                // Ctrl+C must reach the plan feedback path: empty composer
                // abandons (like panel `q`); non-empty clears the draft.
                // Do not return Changed and swallow the chord.
                if key!('c', CONTROL).matches(key) {
                    return self.handle_plan_feedback_key(key);
                }
                return InputOutcome::Changed;
            }
            // In the plan viewer, Esc first clears visual selection / search
            // before closing. q and Ctrl-C always close immediately.
            if key!(Esc).matches(key)
                && let Some(ref mut viewer) = self.line_viewer
            {
                if viewer.list_state.visual_mode {
                    viewer.list_state.exit_visual_mode();
                    return InputOutcome::Changed;
                }
                if viewer.list_state.matcher().is_some() {
                    viewer.list_state.handle_key_event(key, &viewer.lines);
                    return InputOutcome::Changed;
                }
            }
            self.cancel_line_viewer();
            return InputOutcome::Changed;
        }
        // All other keys (including Ctrl-D/U for page nav): forward to ListPaneState.
        if let Some(ref mut viewer) = self.line_viewer {
            viewer.list_state.handle_key_event(key, &viewer.lines);
        }
        InputOutcome::Changed
    }

    /// Confirm line viewer: update the element, optionally with a line range.
    ///
    /// `include_range`: if true and visual mode is active, appends `:N-M`.
    /// If false, confirms with just the file path (strips any existing range).
    fn confirm_line_viewer(&mut self, include_range: bool) {
        if let Some(viewer) = self.line_viewer.take() {
            if let Some(elem_id) = viewer.element_id {
                let rel_path = viewer
                    .path
                    .strip_prefix(&self.session.cwd)
                    .unwrap_or(&viewer.path);

                let suffix = if include_range {
                    viewer.line_range_suffix().unwrap_or_default()
                } else {
                    String::new()
                };

                let path_display = format!("{}{suffix}", rel_path.display());
                let new_text = format!("@{path_display}");
                let display = crate::views::prompt_widget::file_ref_display(&path_display);

                if let Some(elem) = self
                    .prompt
                    .textarea
                    .elements()
                    .iter()
                    .find(|e| e.id == elem_id)
                {
                    let range = elem.range.clone();
                    self.prompt.textarea.replace_range_with_element(
                        range,
                        &new_text,
                        crate::views::prompt_widget::KIND_FILE_REF,
                        Some(display),
                    );
                }
            }
            // Close the undo group.
            self.prompt.textarea.insert_str(" ");
            self.prompt.textarea.end_undo_group();
        }
    }

    /// Cancel line viewer: revert all changes.
    pub(crate) fn cancel_line_viewer(&mut self) {
        self.line_viewer = None;
        self.view_plan_requested = false;
        if self.plan_approval_view.is_some() {
            // Keep Revise / Comment box text. `cancel_undo_group` reverts the
            // open group and drops Undo, which is why revision notes vanished.
            self.snapshot_or_clear_plan_feedback_draft();
            self.prompt.textarea.end_undo_group();
        } else {
            self.prompt.textarea.cancel_undo_group();
        }
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }
        self.restore_plan_feedback_draft_if_composer_lost();
        // If a casual plan comment was in progress when the modal
        // closed (via [✗], click-outside, or any other path that
        // doesn't route through `cancel_casual_plan_commenting`),
        // restore the pre-comment prompt text so the user's original
        // text isn't lost behind the in-progress comment draft.
        // Mirrors `cancel_casual_plan_commenting`.
        if let Some(stashed) = self.casual_stashed_prompt.take() {
            self.prompt.restore(stashed);
        }
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
    }

    /// Dismiss the /btw panel. If Done, flush response to scrollback first.
    pub(super) fn dismiss_btw_panel(&mut self) -> InputOutcome {
        use crate::scrollback::block::RenderBlock;
        use crate::scrollback::blocks::BtwBlock;
        use crate::views::btw_overlay::BtwOverlayState;
        if let Some(BtwOverlayState::Done {
            question, content, ..
        }) = self.btw_state.take()
        {
            self.scrollback
                .push_block(RenderBlock::Btw(BtwBlock::new(question, content.text())));
        } else {
            self.btw_state = None;
        }
        self.minimal_btw_lifecycle = None;
        self.btw_focused = false;
        self.clear_btw_drag_state();
        InputOutcome::Changed
    }

    pub(super) fn clear_btw_drag_state(&mut self) {
        let is_btw = self
            .pending_text_drag
            .is_some_and(|p| p.anchor.entry_idx == BTW_OVERLAY_ENTRY_IDX)
            || self
                .drag_selection
                .as_ref()
                .is_some_and(|d| d.anchor.entry_idx == BTW_OVERLAY_ENTRY_IDX);
        if is_btw {
            self.pending_text_drag = None;
            self.drag_selection = None;
            self.drag_autoscroll = None;
            self.last_drag_mouse = None;
        }
    }

    /// Handle mouse events while the line viewer is open.
    pub(super) fn handle_line_viewer_mouse(
        &mut self,
        mouse: &crossterm::event::MouseEvent,
    ) -> InputOutcome {
        use crossterm::event::{MouseButton, MouseEventKind};

        let Some(ref mut viewer) = self.line_viewer else {
            return InputOutcome::Changed;
        };
        // `discard_in_progress_comment` needs `&mut self`. Defer it until
        // after this `viewer` borrow ends (E0499).
        let mut restore_stashed_on_leave_commenting = false;

        // `popup_area` is the list-rendered area (excludes the divider
        // + footer rows in plan modes); used for dispatching mouse
        // events into `ListPaneState`. `modal_area` is the full inner
        // rect of the modal frame (includes the footer); used by the
        // click-outside-modal check so that clicks on the divider or
        // the empty space between footer buttons don't accidentally
        // close the modal.
        let popup_area = viewer.last_popup_area;
        let modal_area = viewer.last_modal_area;

        let close_area = viewer.close_button_area;
        let fs_area = viewer.fullscreen_button_area;
        let send_area = viewer.plan_ref().and_then(|p| p.send_button_area);
        let abandon_area = viewer.plan_ref().and_then(|p| p.abandon_button_area);
        let approve_area = viewer.plan_ref().and_then(|p| p.approve_button_area);
        let approve_notes_area = viewer.plan_ref().and_then(|p| p.approve_notes_button_area);
        let questions_area = viewer.plan_ref().and_then(|p| p.questions_button_area);
        let comment_btn_area = viewer.plan_ref().and_then(|p| p.comment_button_area);
        let copy_btn_area = viewer.plan_ref().and_then(|p| p.copy_button_area);
        // Cached `is_plan_viewer()` so we don't need to call self while
        // the line_viewer is mutably borrowed below.
        let is_plan_preview =
            viewer.kind == crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;

        let scrollbar_owns_gesture = match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                viewer.list_state.scrollbar_hit(mouse.column, mouse.row)
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                viewer.list_state.is_scrollbar_dragging()
            }
            _ => false,
        };
        if scrollbar_owns_gesture {
            viewer.list_state.handle_mouse_event(
                mouse.kind,
                mouse.column,
                mouse.row,
                popup_area.unwrap_or_default(),
                &viewer.lines,
            );
            if is_plan_preview {
                viewer.plan_mut().gutter_drag_start = None;
                viewer.plan_mut().gutter_drag_end = None;
            }
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let was_commenting = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
                if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Preview;
                }
                if was_commenting {
                    self.discard_in_progress_comment();
                }
            }
            return InputOutcome::Changed;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Click on close button -> cancel.
                if close_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    self.cancel_line_viewer();
                    return InputOutcome::Changed;
                }
                // Click on fullscreen button -> toggle fullscreen.
                if fs_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if let Some(ref mut v) = self.line_viewer {
                        v.fullscreen = !v.fullscreen;
                    }
                    return InputOutcome::Changed;
                }
                if abandon_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    return self.click_plan_cta(SelectedPlanCta::Exit);
                }
                if approve_notes_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()))
                {
                    return self.focus_plan_prompt(PlanPromptIntent::ApproveNotes);
                }
                if questions_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    return self.click_plan_cta(SelectedPlanCta::Clarify);
                }
                if approve_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.click_plan_cta(SelectedPlanCta::Approve);
                    } else if is_plan_preview && !self.plan_comments.is_empty() {
                        // Casual mode: the only action button shown is
                        // `s send` (when there are comments to send).
                        return self.send_casual_plan_comments();
                    }
                    return InputOutcome::Changed;
                }
                if comment_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.click_plan_cta(SelectedPlanCta::Comment);
                    }
                    if is_plan_preview {
                        return self.enter_casual_plan_commenting();
                    }
                    // The comment button is only set on plan viewers,
                    // so the two arms above are exhaustive in practice.
                    // Return here to make the dead fall-through
                    // explicit and to match the abandon/approve hit
                    // patterns just above.
                    return InputOutcome::Changed;
                }
                if copy_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    return self.copy_plan_full();
                }
                if send_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.click_plan_cta(SelectedPlanCta::Revise);
                    }
                    return self.send_casual_plan_comments();
                }
                // Mermaid buttons before click-to-comment (early return ends
                // the `viewer` borrow so `handle_inline_media_click` can take
                // `&mut self`).
                let mermaid_hit = self
                    .inline_media_hits
                    .mermaid_buttons
                    .iter()
                    .any(|(rect, _, _)| rect.contains((mouse.column, mouse.row).into()));
                if mermaid_hit {
                    return self
                        .handle_inline_media_click(mouse.column, mouse.row)
                        .unwrap_or(InputOutcome::Changed);
                }
                if modal_area.is_none_or(|a| !a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some()
                        && self
                            .pane_areas
                            .prompt
                            .contains((mouse.column, mouse.row).into())
                    {
                        if let Some(ref mut pav) = self.plan_approval_view {
                            pav.focus = PlanApprovalFocus::Prompt;
                            if pav.prompt_intent
                                == crate::views::plan_approval_view::PlanPromptIntent::Revise
                            {
                                pav.prompt_intent =
                                    crate::views::plan_approval_view::PlanPromptIntent::Comment;
                            }
                        }
                        return InputOutcome::Changed;
                    }
                    if self.plan_approval_view.is_some() {
                        return InputOutcome::Changed;
                    }
                    self.cancel_line_viewer();
                    return InputOutcome::Changed;
                }
                let was_commenting = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
                if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Preview;
                }
                if was_commenting {
                    restore_stashed_on_leave_commenting = true;
                }
                // Forward below.
            }
            MouseEventKind::Moved => {
                let mut changed = false;
                // Redraw only when mermaid button hover would change.
                if self.last_mouse_pos != (mouse.column, mouse.row) {
                    let old = self.last_mouse_pos;
                    self.last_mouse_pos = (mouse.column, mouse.row);
                    let hits = |col: u16, row: u16| {
                        self.inline_media_hits
                            .mermaid_buttons
                            .iter()
                            .any(|(rect, _, _)| rect.contains((col, row).into()))
                    };
                    if hits(old.0, old.1) || hits(mouse.column, mouse.row) {
                        changed = true;
                    }
                }
                let close_hover =
                    close_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                if close_hover != viewer.close_hovered {
                    viewer.close_hovered = close_hover;
                    changed = true;
                }
                let fs_hover =
                    fs_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                if fs_hover != viewer.fullscreen_hovered {
                    viewer.fullscreen_hovered = fs_hover;
                    changed = true;
                }
                let send_hover =
                    send_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_send = viewer.plan_ref().is_some_and(|p| p.send_hovered);
                if send_hover != prev_send {
                    viewer.plan_mut().send_hovered = send_hover;
                    changed = true;
                }
                let abandon_hover =
                    abandon_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_abandon = viewer.plan_ref().is_some_and(|p| p.abandon_hovered);
                if abandon_hover != prev_abandon {
                    viewer.plan_mut().abandon_hovered = abandon_hover;
                    changed = true;
                }
                let approve_hover =
                    approve_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_approve = viewer.plan_ref().is_some_and(|p| p.approve_hovered);
                if approve_hover != prev_approve {
                    viewer.plan_mut().approve_hovered = approve_hover;
                    changed = true;
                }
                let notes_hover = approve_notes_area
                    .is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_notes = viewer.plan_ref().is_some_and(|p| p.approve_notes_hovered);
                if notes_hover != prev_notes {
                    viewer.plan_mut().approve_notes_hovered = notes_hover;
                    changed = true;
                }
                let questions_hover =
                    questions_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_questions = viewer.plan_ref().is_some_and(|p| p.questions_hovered);
                if questions_hover != prev_questions {
                    viewer.plan_mut().questions_hovered = questions_hover;
                    changed = true;
                }
                let comment_btn_hover =
                    comment_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_comment_btn = viewer.plan_ref().is_some_and(|p| p.comment_hovered);
                if comment_btn_hover != prev_comment_btn {
                    viewer.plan_mut().comment_hovered = comment_btn_hover;
                    changed = true;
                }
                let copy_btn_hover =
                    copy_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_copy_btn = viewer.plan_ref().is_some_and(|p| p.copy_hovered);
                if copy_btn_hover != prev_copy_btn {
                    viewer.plan_mut().copy_hovered = copy_btn_hover;
                    changed = true;
                }
                if self.plan_approval_view.is_some()
                    && let Some(area) = popup_area
                    && area.contains((mouse.column, mouse.row).into())
                    && mouse.row >= area.y
                {
                    let ry = (mouse.row - area.y) as usize;
                    let vy = viewer.list_state.scroll_offset() + ry;
                    if viewer.list_state.layout().item_at_y(vy).is_some()
                        && viewer.list_state.select_at_y(vy, &viewer.lines)
                    {
                        changed = true;
                    }
                }
                return if changed {
                    InputOutcome::Changed
                } else {
                    InputOutcome::Unchanged
                };
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Drag-to-extend works in both plan-approval and casual
                // plan-preview modes (anywhere the PlanPreview viewer is
                // showing).
                if is_plan_preview
                    && let Some(area) = popup_area
                    && let Some(ln) = viewer.source_line_at_screen_row(mouse.row, area)
                {
                    let has_start = viewer
                        .plan_ref()
                        .is_some_and(|p| p.gutter_drag_start.is_some());
                    if has_start {
                        viewer.plan_mut().gutter_drag_end = Some(ln);
                        return InputOutcome::Changed;
                    }
                }
                if let Some(area) = popup_area
                    && area.contains((mouse.column, mouse.row).into())
                {
                    viewer.list_state.handle_mouse_event(
                        mouse.kind,
                        mouse.column,
                        mouse.row,
                        area,
                        &viewer.lines,
                    );
                }
                return InputOutcome::Changed;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if is_plan_preview {
                    let drag_start = viewer.plan_ref().and_then(|p| p.gutter_drag_start);
                    let drag_end = viewer.plan_ref().and_then(|p| p.gutter_drag_end);
                    viewer.plan_mut().gutter_drag_start = None;
                    viewer.plan_mut().gutter_drag_end = None;
                    if let (Some(start), Some(end)) = (drag_start, drag_end)
                        && start != end
                    {
                        let lo = start.min(end);
                        let hi = start.max(end);
                        let range = lo..hi + 1;
                        if let Some(ref mut pav) = self.plan_approval_view {
                            pav.stashed_feedback_prompt = Some(self.prompt.stash());
                            pav.commenting_range = Some(range);
                            pav.editing_comment_id = None;
                            pav.focus = PlanApprovalFocus::Commenting;
                            self.prompt.set_text("");
                        } else {
                            // First-entry-only stash; see
                            // `enter_casual_plan_commenting` for the
                            // same guard rationale.
                            if self.casual_stashed_prompt.is_none() {
                                self.casual_stashed_prompt = Some(self.prompt.stash());
                            }
                            self.casual_commenting_range = Some(range);
                            self.casual_editing_comment_id = None;
                            self.prompt.set_text("");
                        }
                        return InputOutcome::Changed;
                    }
                }
                if let Some(area) = popup_area
                    && area.contains((mouse.column, mouse.row).into())
                {
                    viewer.list_state.handle_mouse_event(
                        mouse.kind,
                        mouse.column,
                        mouse.row,
                        area,
                        &viewer.lines,
                    );
                }
                return InputOutcome::Changed;
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {}
            _ => return InputOutcome::Changed,
        }

        // Forward to ListPaneState if inside the popup area.
        let mut should_enter_commenting = false;
        if let Some(area) = popup_area
            && area.contains((mouse.column, mouse.row).into())
        {
            viewer.list_state.handle_mouse_event(
                mouse.kind,
                mouse.column,
                mouse.row,
                area,
                &viewer.lines,
            );

            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let clicked_line = viewer.source_line_at_screen_row(mouse.row, area);
                // Drag selection works in both modes whenever the
                // plan preview is showing — but only on source rows
                // (we need a 1-based line number as the drag anchor).
                if is_plan_preview && let Some(ln) = clicked_line {
                    viewer.plan_mut().gutter_drag_start = Some(ln);
                    viewer.plan_mut().gutter_drag_end = Some(ln);
                }

                viewer.plan_mut().last_click_at = Some(std::time::Instant::now());

                // A single click on a plan-approval row focuses or
                // scrolls. It does not enter Commenting (that steals
                // the composer). Casual preview still uses click-to-
                // comment. Explicit `c` still comments.
                // Skip Mermaid affordance rows (button hits handled above).
                let on_list_row = mouse.row >= area.y && {
                    let ry = (mouse.row - area.y) as usize;
                    let vy = viewer.list_state.scroll_offset() + ry;
                    viewer
                        .list_state
                        .layout()
                        .item_at_y(vy)
                        .map(|vi| {
                            let pi = viewer.list_state.to_physical(vi);
                            viewer.lines.get(pi).is_some_and(|item| {
                                !matches!(item, PlanViewerItem::MermaidAffordance(_))
                            })
                        })
                        .unwrap_or(false)
                };
                // Skip the click-to-comment trigger if the user is
                // already composing a comment. Without this guard, any
                // click on a list row would re-enter commenting and
                // re-stash the (now-comment) prompt, clobbering the
                // user's pre-comment text and preventing the mouse from
                // being used to reposition the cursor without
                // committing to a fresh comment.
                let in_pav_commenting = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting);
                let in_casual_commenting =
                    self.plan_approval_view.is_none() && self.casual_commenting_range.is_some();
                if on_list_row
                    && is_plan_preview
                    && viewer.list_state.input_mode().is_none()
                    && !in_pav_commenting
                    && !in_casual_commenting
                    && self.plan_approval_view.is_none()
                {
                    should_enter_commenting = true;
                }
            }
        }
        if should_enter_commenting {
            return self.enter_casual_plan_commenting();
        }
        if restore_stashed_on_leave_commenting {
            self.discard_in_progress_comment();
        }
        InputOutcome::Changed
    }

    // -- Scrollback selection box buttons -------------------------------------

    /// Render ⧉ (copy) and ↗ (view) buttons on the scrollback selection box.
    ///
    /// Two modes:
    /// - **Corner row** (expanded or ungrouped): buttons on the `╭...╮` row.
    /// - **Inline** (collapsed + grouped): buttons on the selected entry's row,
    ///   overlaying content at the right edge.
    pub(super) fn render_selection_buttons(
        &mut self,
        buf: &mut Buffer,
        selection_box: &SelectionBox,
        selected_entry_area: Option<Rect>,
        theme: &Theme,
    ) {
        // Gated by appearance config (opt-in while testing).
        if !self
            .scrollback
            .appearance()
            .scrollback
            .display
            .selection_buttons
        {
            self.hit_sb_copy.clear();
            self.hit_sb_view.clear();
            return;
        }

        let Some(selected_idx) = self.scrollback.selected() else {
            self.hit_sb_copy.clear();
            self.hit_sb_view.clear();
            return;
        };
        let Some(entry) = self.scrollback.entry(selected_idx) else {
            self.hit_sb_copy.clear();
            self.hit_sb_view.clear();
            return;
        };

        let header_selected = self.scrollback.entry_content_hidden_by_group(selected_idx);
        let bubble_copy = self
            .scrollback
            .appearance()
            .scrollback
            .display
            .bubble_copy_buttons;
        let has_copy = entry.block.supports_copy() && !header_selected && !bubble_copy;
        let has_view = entry.block.supports_fullscreen() && !header_selected;
        if !has_copy && !has_view {
            self.hit_sb_copy.clear();
            self.hit_sb_view.clear();
            return;
        }

        // Determine inline vs corner mode.
        // Inline: entry is collapsed AND part of a group (group_range > 1).
        let split_mode = self
            .scrollback
            .appearance()
            .scrollback
            .display
            .group_selection_split;
        let group_range = self.scrollback.group_range_of(selected_idx, split_mode);
        let is_grouped = group_range.len() > 1;
        let is_collapsed = entry.display_mode == DisplayMode::Collapsed;
        let inline = is_collapsed && is_grouped;

        let sel = &selection_box.inner_area;
        let right_x = sel.x + sel.width.saturating_sub(1);

        let btn_base = Style::default().fg(theme.selection_border);
        let btn_hover = Style::default().fg(theme.text_primary);

        // Build button array based on capabilities.
        if has_copy && has_view {
            let (btn_right_x, y) = if inline {
                // Inline: buttons on the selected entry's content row.
                let entry_y = selected_entry_area.map(|r| r.y).unwrap_or(sel.y);
                // Place inside the right border (right_x has │).
                (right_x.saturating_sub(2), entry_y)
            } else {
                // Corner row: buttons to the left of ╮.
                let corner_y = sel.y.saturating_sub(1);
                (right_x.saturating_sub(2), corner_y)
            };
            if !selection_box.top_clipped || inline {
                let areas = render_char_buttons(
                    buf,
                    btn_right_x,
                    y,
                    [
                        (crate::glyphs::copy_icon(), self.hit_sb_copy.hovered),
                        (crate::glyphs::enlarge(), self.hit_sb_view.hovered),
                    ],
                    btn_base,
                    btn_hover,
                    1,
                );
                self.hit_sb_copy.set(Some(areas[0]));
                self.hit_sb_view.set(Some(areas[1]));
            } else {
                self.hit_sb_copy.clear();
                self.hit_sb_view.clear();
            }
        } else if has_copy {
            let (btn_right_x, y) = if inline {
                let entry_y = selected_entry_area.map(|r| r.y).unwrap_or(sel.y);
                (right_x.saturating_sub(2), entry_y)
            } else {
                let corner_y = sel.y.saturating_sub(1);
                (right_x.saturating_sub(2), corner_y)
            };
            if !selection_box.top_clipped || inline {
                let areas = render_char_buttons(
                    buf,
                    btn_right_x,
                    y,
                    [(crate::glyphs::copy_icon(), self.hit_sb_copy.hovered)],
                    btn_base,
                    btn_hover,
                    0,
                );
                self.hit_sb_copy.set(Some(areas[0]));
            } else {
                self.hit_sb_copy.clear();
            }
            self.hit_sb_view.clear();
        } else {
            // has_view only
            let (btn_right_x, y) = if inline {
                let entry_y = selected_entry_area.map(|r| r.y).unwrap_or(sel.y);
                (right_x.saturating_sub(2), entry_y)
            } else {
                let corner_y = sel.y.saturating_sub(1);
                (right_x.saturating_sub(2), corner_y)
            };
            if !selection_box.top_clipped || inline {
                let areas = render_char_buttons(
                    buf,
                    btn_right_x,
                    y,
                    [(crate::glyphs::enlarge(), self.hit_sb_view.hovered)],
                    btn_base,
                    btn_hover,
                    0,
                );
                self.hit_sb_view.set(Some(areas[0]));
            } else {
                self.hit_sb_view.clear();
            }
            self.hit_sb_copy.clear();
        }
    }

    // -- Block viewer input handling ------------------------------------------

    /// Handle a key event when the block viewer is open.
    ///
    /// Returns `Changed` if consumed, `Unchanged` if the key should bubble up.
    pub(super) fn handle_block_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let Some(ref mut viewer) = self.block_viewer else {
            return InputOutcome::Unchanged;
        };

        // Check for close signals first (Esc/q/Ctrl-F)
        if viewer.is_close_key(key) {
            self.block_viewer = None;
            return InputOutcome::Changed;
        }

        // Route to viewer — returns whether the key was consumed
        if !viewer.handle_key(key) {
            return InputOutcome::Unchanged;
        }

        // Handle raw toggle: capture old source map, toggle, rebuild with stability
        if viewer.raw_toggle_pending {
            viewer.raw_toggle_pending = false;
            // Record scroll anchor BEFORE toggle so the selected line stays
            // at the same screen position after the rebuild.
            viewer.list_state.set_scroll_anchor();
            // Capture source map BEFORE toggle for cursor mapping
            let old_source_line = self
                .scrollback
                .get_by_id(viewer.entry_id)
                .and_then(|entry| {
                    viewer.list_state.selected_id().and_then(|id| {
                        crate::views::block_viewer::BlockViewerPane::source_line_for_id(
                            &entry.block,
                            id,
                        )
                    })
                });
            // Toggle raw mode on the entry
            if let Some(entry) = self.scrollback.get_by_id_mut(viewer.entry_id) {
                entry.toggle_raw();
            }
            // Re-borrow immutably to rebuild items (avoids clone)
            if let Some(entry) = self.scrollback.get_by_id(viewer.entry_id) {
                viewer.rebuild_items(entry);
                viewer.jump_to_source_line(entry, old_source_line);
            }
        }

        // Process pending copy actions (logic lives in BlockViewerPane)
        let entry_id = viewer.entry_id;
        if let Some(entry) = self.scrollback.get_by_id(entry_id)
            && let Some(text) = viewer.process_pending_copy(entry)
        {
            self.copy_to_clipboard(&text);
        }

        InputOutcome::Changed
    }

    /// Handle a mouse event when the block viewer modal is open.
    pub(in crate::app) fn handle_block_viewer_mouse(
        &mut self,
        mouse: &crossterm::event::MouseEvent,
    ) -> InputOutcome {
        use crate::views::modal_window::{ModalWindowOutcome, handle_modal_mouse};
        use crossterm::event::{MouseButton, MouseEventKind};

        let Some(ref mut viewer) = self.block_viewer else {
            return InputOutcome::Changed;
        };

        // Route to modal chrome first (close button, click-outside).
        let modal_outcome =
            handle_modal_mouse(&mut viewer.modal, mouse.kind, mouse.column, mouse.row);
        match modal_outcome {
            ModalWindowOutcome::CloseRequested => {
                self.block_viewer = None;
                return InputOutcome::Changed;
            }
            ModalWindowOutcome::Handled => return InputOutcome::Changed,
            _ => {}
        }

        // Content interaction (scroll, click, drag).
        match mouse.kind {
            MouseEventKind::ScrollDown => viewer.handle_scroll(3),
            MouseEventKind::ScrollUp => viewer.handle_scroll(-3),
            MouseEventKind::Down(MouseButton::Left)
            | MouseEventKind::Drag(MouseButton::Left)
            | MouseEventKind::Up(MouseButton::Left) => {
                viewer.handle_mouse(mouse.kind, mouse.column, mouse.row);
            }
            MouseEventKind::Moved => {
                // Update hover state for content area.
                viewer.handle_mouse(mouse.kind, mouse.column, mouse.row);
            }
            _ => {}
        }

        // Collect any pending copy text: drag-release auto-copy (like
        // scrollback finish_text_drag) or Y/y key handler copy.
        let drag_text = viewer.drag_copy_text.take();
        let entry_id = viewer.entry_id;
        let key_text = if drag_text.is_none() {
            self.scrollback
                .get_by_id(entry_id)
                .and_then(|entry| viewer.process_pending_copy(entry))
        } else {
            None
        };
        // viewer borrow ends here — clipboard + toast can use &mut self.
        if let Some(text) = drag_text.or(key_text) {
            self.copy_to_clipboard(&text);
        }

        InputOutcome::Changed
    }

    /// Dynamic fold label for the shortcuts bar hint.
    ///
    /// Returns "expand" if the selected entry is collapsed/truncated,
    /// "collapse" if expanded, or `None` if the selected entry isn't foldable.
    pub(super) fn selected_fold_label(&self) -> Option<&'static str> {
        let idx = self.scrollback.selected()?;
        let entry = self.scrollback.get(idx)?;
        if !entry.is_foldable() {
            return None;
        }
        Some(match entry.display_mode() {
            DisplayMode::Expanded => "collapse",
            _ => "expand",
        })
    }
}

#[cfg(test)]
#[path = "viewer_tests.rs"]
mod tests;
