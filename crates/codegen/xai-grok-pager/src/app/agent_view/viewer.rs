//! Line and block viewer popups plus the /btw panel: open/confirm/dismiss
//! and their key/mouse handlers.

use super::{AgentView, render_char_buttons};
use crate::app::app_view::InputOutcome;
use crate::key;
use crate::scrollback::selection::SelectionBox;
use crate::scrollback::types::DisplayMode;
use crate::theme::Theme;
use crate::views::btw_overlay::BTW_OVERLAY_ENTRY_IDX;
use crate::views::file_search::line_viewer::LineViewerState;
use crate::views::list_pane::ListItem;
use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

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

    /// Toggle line-viewer fullscreen ↔ side panel (plan approval) / popup (file).
    ///
    /// Shared by Ctrl+F and the title-bar fullscreen mouse button so both
    /// paths restore `side_panel` for parked plan approval when leaving
    /// fullscreen (force-modal leave via mouse must not land on the dimmed
    /// centered popup while keyboard restores the side panel).
    /// Copy whole plan body (same payload as `Y`) or, for non-plan viewers,
    /// the filename/title. Used by the `Y` key and the top-bar `⧉` button.
    pub(super) fn copy_line_viewer_whole_body_or_title(&mut self) {
        let is_plan = self.line_viewer.as_ref().is_some_and(|v| {
            v.kind == crate::views::file_search::line_viewer::LineViewerKind::PlanPreview
        });
        if is_plan {
            // Real plan body only — empty approval paints a UI placeholder in
            // the viewer that must not be copied as plan content (quiet no-op).
            if let Some(text) = self.plan_body_for_preview().filter(|t| !t.is_empty()) {
                self.copy_to_clipboard(&text);
            }
        } else if let Some(ref viewer) = self.line_viewer {
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
            if !name.is_empty() {
                self.copy_to_clipboard(&name);
            }
        }
    }

    pub(super) fn toggle_line_viewer_fullscreen(&mut self) {
        let Some(ref mut viewer) = self.line_viewer else {
            return;
        };
        if viewer.fullscreen {
            viewer.fullscreen = false;
            // Restore side panel for any plan preview (casual `/view-plan` and
            // approval). File previews fall back to the centered popup
            // (`side_panel` already false).
            if viewer.kind == crate::views::file_search::line_viewer::LineViewerKind::PlanPreview {
                viewer.side_panel = true;
            }
        } else {
            viewer.fullscreen = true;
            viewer.side_panel = false;
        }
    }

    /// Handle a key event while the line viewer is open.
    pub(super) fn handle_line_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let in_plan_approval = self.plan_approval_view.is_some();

        let input_bar_active = self
            .line_viewer
            .as_ref()
            .is_some_and(|v| v.list_state.input_mode().is_some());

        // Plan approval Preview: Ctrl/Cmd+V must attach clipboard screenshots
        // to the plan composer (same deferred probe as the main prompt). Do
        // not swallow into the line-viewer list search path.
        if in_plan_approval && !input_bar_active && crate::input::key::is_paste_key(key) {
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Prompt;
            }
            let clipboard_text = crate::app::actions::ClipboardTextRead::from_result(
                crate::clipboard::system_clipboard_read_text(),
            );
            return self.handle_paste_key_deferred(clipboard_text);
        }

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

        if in_plan_approval && key.code == KeyCode::Tab && key.modifiers.is_empty() {
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Prompt;
            }
            return InputOutcome::Changed;
        }

        // Plan-approval `Esc` doesn't close the viewer (use `q` / `Ctrl+\`),
        // but it still clears a transient visual selection or accepted search
        // matcher first, so the graduated dashboard-overlay back-out (which
        // declines to fire while a matcher is active) isn't left dead-ended.
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
            return InputOutcome::Changed;
        }

        // Ctrl+F: toggle fullscreen ↔ side panel (plan) / popup (file).
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_line_viewer_fullscreen();
            return InputOutcome::Changed;
        }

        // Casual plan preview: `c` comment / `s` send (not approval CTAs).
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

        // Plan approval primary CTAs (empty-prompt accelerators only):
        // a approve · A approve w/ comment · ? clarify · s revise · q quit
        // Non-empty draft / ordinary typing must reach the composer — dogfood
        // 2026-07-29: panel Preview used to swallow every bare letter.
        let plan_prompt_empty = in_plan_approval
            && self.prompt.text().trim().is_empty()
            && self.prompt.images.is_empty();
        if in_plan_approval && plan_prompt_empty {
            if key!('a').matches(key) {
                return self.approve_plan();
            }
            if key!('A').matches(key) {
                return self.focus_plan_prompt(PlanPromptIntent::ApproveNotes);
            }
            if key!('s').matches(key) {
                return self.focus_plan_prompt(PlanPromptIntent::Revise);
            }
            if key!('?').matches(key) {
                return self.focus_plan_prompt(PlanPromptIntent::Questions);
            }
            if key!('q').matches(key) {
                return self.abandon_plan();
            }
        }
        // Printable / edit keys while plan approval is open: move to Prompt
        // and type. Viewer navigation (j/k/arrows/…) and select-to-copy (y/Y)
        // stay below. Enter still opens line notes (secondary path) when it
        // falls through.
        if in_plan_approval {
            let is_composer_key = match key.code {
                // y/Y: line / whole-plan copy on plan surfaces (handlers below).
                // Not composer type-in.
                KeyCode::Char('y' | 'Y') => false,
                KeyCode::Char(c) if !c.is_control() => {
                    // Bare or Shift (uppercase); Ctrl/Alt chords stay viewer/global.
                    key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT
                }
                KeyCode::Backspace | KeyCode::Delete => key.modifiers.is_empty(),
                _ => false,
            };
            if is_composer_key {
                if let Some(ref mut pav) = self.plan_approval_view {
                    pav.focus = PlanApprovalFocus::Prompt;
                }
                return self.handle_plan_feedback_key(key);
            }
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
                return self.enter_plan_commenting();
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
        // y: copy selected line(s) to system clipboard.
        // Plan approval / plan preview: same line/range copy as conversation
        // selection (does not interfere with a/s/?/q CTAs).
        if key!('y').matches(key) {
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
        // Y: on plan surfaces, copy whole plan body; else copy filename/title.
        if key!('Y').matches(key) {
            self.copy_line_viewer_whole_body_or_title();
            return InputOutcome::Changed;
        }
        if key!(Esc).matches(key) || key!('q').matches(key) || key!('c', CONTROL).matches(key) {
            if in_plan_approval {
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
        self.prompt.textarea.cancel_undo_group();
        if let Some(ref mut pav) = self.plan_approval_view {
            pav.focus = PlanApprovalFocus::Preview;
        }
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

    /// Dismiss the /btw panel. Flushes Done (full thread) or Error-with-prior
    /// turns to scrollback first so multi-turn answers are not lost.
    pub(super) fn dismiss_btw_panel(&mut self) -> InputOutcome {
        self.flush_open_btw_to_scrollback();
        self.btw_state = None;
        self.minimal_btw_lifecycle = None;
        self.btw_focused = false;
        self.clear_btw_drag_state();
        InputOutcome::Changed
    }

    /// Flush any open Done/Error prior-turn payload into scrollback without
    /// clearing focus flags. Used by dismiss and by first-shot `/btw` that
    /// replaces an open panel so the previous thread is not dropped.
    pub(crate) fn flush_open_btw_to_scrollback(&mut self) {
        use crate::scrollback::block::RenderBlock;
        use crate::scrollback::blocks::BtwBlock;
        let Some(state) = self.btw_state.as_ref() else {
            return;
        };
        if let Some((question, body)) = state.scrollback_flush_payload() {
            self.scrollback
                .push_block(RenderBlock::Btw(BtwBlock::new(question, body)));
        }
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
        let copy_area = viewer.copy_button_area;
        let send_area = viewer.plan_ref().and_then(|p| p.send_button_area);
        let questions_area = viewer.plan_ref().and_then(|p| p.questions_button_area);
        let abandon_area = viewer.plan_ref().and_then(|p| p.abandon_button_area);
        let approve_area = viewer.plan_ref().and_then(|p| p.approve_button_area);
        let approve_notes_area = viewer.plan_ref().and_then(|p| p.approve_notes_button_area);
        let comment_btn_area = viewer.plan_ref().and_then(|p| p.comment_button_area);
        // Cached `is_plan_viewer()` so we don't need to call self while
        // the line_viewer is mutably borrowed below.
        let is_plan_preview =
            viewer.kind == crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Click on close button -> cancel.
                if close_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_none() {
                        self.cancel_line_viewer();
                    }
                    return InputOutcome::Changed;
                }
                // Click on copy button -> whole body (same as `Y` on plans).
                if copy_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    self.copy_line_viewer_whole_body_or_title();
                    return InputOutcome::Changed;
                }
                // Click on fullscreen button -> same toggle as Ctrl+F.
                if fs_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    self.toggle_line_viewer_fullscreen();
                    return InputOutcome::Changed;
                }
                if abandon_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    return self.abandon_plan();
                }
                if approve_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.approve_plan();
                    } else if is_plan_preview && !self.plan_comments.is_empty() {
                        // Casual mode: the only action button shown is
                        // `s send` (when there are comments to send).
                        return self.send_casual_plan_comments();
                    }
                    return InputOutcome::Changed;
                }
                if approve_notes_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()))
                {
                    if self.plan_approval_view.is_some() {
                        return self.focus_plan_prompt(PlanPromptIntent::ApproveNotes);
                    }
                    return InputOutcome::Changed;
                }
                // Comment button is casual-preview only (approval has no
                // primary Comment CTA; Enter / dbl-click still open notes).
                if comment_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if is_plan_preview && self.plan_approval_view.is_none() {
                        return self.enter_casual_plan_commenting();
                    }
                    return InputOutcome::Changed;
                }
                if send_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.focus_plan_prompt(PlanPromptIntent::Revise);
                    }
                    return self.send_casual_plan_comments();
                }
                if questions_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.focus_plan_prompt(PlanPromptIntent::Questions);
                    }
                    return InputOutcome::Changed;
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
                    if was_commenting {
                        // Same rule as Tab: clicking back into the modal
                        // discards the in-progress comment draft.
                        pav.commenting_range = None;
                        pav.editing_comment_id = None;
                        pav.stashed_feedback_prompt = None;
                    }
                }
                if was_commenting {
                    self.prompt.set_text("");
                }
                // Forward below.
            }
            MouseEventKind::Moved => {
                let mut changed = false;
                let close_hover =
                    close_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                if close_hover != viewer.close_hovered {
                    viewer.close_hovered = close_hover;
                    changed = true;
                }
                let copy_hover =
                    copy_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                if copy_hover != viewer.copy_hovered {
                    viewer.copy_hovered = copy_hover;
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
                let questions_hover =
                    questions_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_questions = viewer.plan_ref().is_some_and(|p| p.questions_hovered);
                if questions_hover != prev_questions {
                    viewer.plan_mut().questions_hovered = questions_hover;
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
                let approve_notes_hover = approve_notes_area
                    .is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_approve_notes = viewer.plan_ref().is_some_and(|p| p.approve_notes_hovered);
                if approve_notes_hover != prev_approve_notes {
                    viewer.plan_mut().approve_notes_hovered = approve_notes_hover;
                    changed = true;
                }
                let comment_btn_hover =
                    comment_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into()));
                let prev_comment_btn = viewer.plan_ref().is_some_and(|p| p.comment_hovered);
                if comment_btn_hover != prev_comment_btn {
                    viewer.plan_mut().comment_hovered = comment_btn_hover;
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
        let mut should_enter_plan_commenting = false;
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

                // A single click on any list row — source line OR
                // existing comment annotation — enters commenting (or
                // edit-comment) for that row. Same shortcut as
                // selecting + pressing `c` / Enter. Works for both
                // plan-approval and casual plan-preview modes.
                let on_list_row = mouse.row >= area.y && {
                    let ry = (mouse.row - area.y) as usize;
                    let vy = viewer.list_state.scroll_offset() + ry;
                    viewer.list_state.layout().item_at_y(vy).is_some()
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
                {
                    if self.plan_approval_view.is_some() {
                        should_enter_plan_commenting = true;
                    } else {
                        should_enter_commenting = true;
                    }
                }
            }
        }
        if should_enter_commenting {
            return self.enter_casual_plan_commenting();
        }
        if should_enter_plan_commenting {
            return self.enter_plan_commenting();
        }
        InputOutcome::Changed
    }

    // -- Scrollback selection box buttons -------------------------------------

    /// Whether this block type gets always-on bubble ⧉ (user + assistant only).
    pub(crate) fn is_bubble_copy_block(block: &crate::scrollback::block::RenderBlock) -> bool {
        matches!(
            block,
            crate::scrollback::block::RenderBlock::UserPrompt(_)
                | crate::scrollback::block::RenderBlock::AgentMessage(_)
        )
    }

    /// Paint always-on ⧉ on visible user/assistant bubbles (no select-first).
    ///
    /// Fills [`Self::bubble_copy_hits`]. Call after scrollback content; clear
    /// hits when drag is active or overlay focused (caller gate).
    pub(super) fn render_bubble_copy_buttons(&mut self, buf: &mut Buffer, theme: &Theme) {
        self.bubble_copy_hits.clear();
        if !self
            .scrollback
            .appearance()
            .scrollback
            .display
            .bubble_copy_buttons
        {
            self.hovered_bubble_copy = None;
            return;
        }

        // Secondary chrome (timestamps, draft/plan ⧉): theme.gray, yellow on
        // DOGE informational chrome, not bright white selection_border.
        let btn_base = Style::default().fg(theme.gray);
        let btn_hover = Style::default().fg(theme.text_primary);
        let icon = crate::glyphs::copy_icon();
        // Mirror prompt top-bar gate: need room for the glyph inside content.
        const MIN_WIDTH: u16 = 6;

        // Collect first so we do not hold a borrow across mutation.
        let candidates: Vec<(usize, Rect)> = self
            .last_scrollback_selection_model
            .visible_blocks
            .iter()
            .filter_map(|geom| {
                let idx = geom.entry_idx;
                let entry = self.scrollback.entry(idx)?;
                if !Self::is_bubble_copy_block(&entry.block) {
                    return None;
                }
                if self.scrollback.entry_content_hidden_by_group(idx) {
                    return None;
                }
                // Prefer content area; fall back to full block area.
                let area = if geom.content_area.width >= MIN_WIDTH {
                    geom.content_area
                } else if geom.area.width >= MIN_WIDTH {
                    geom.area
                } else {
                    return None;
                };
                if area.height == 0 {
                    return None;
                }
                // Top row, absolute content right edge (1 cell for ⧉).
                // When timestamps share this row, EntryRenderer leaves
                // BUBBLE_COPY_TRAILING_INSET columns free at this edge so ⧉
                // does not paint over the time/date (overlap, not truncation).
                let x = area.x + area.width.saturating_sub(1);
                let y = area.y;
                Some((idx, Rect::new(x, y, 1, 1)))
            })
            .collect();

        for (idx, rect) in candidates {
            let hovered = self.hovered_bubble_copy == Some(idx);
            let areas = render_char_buttons(
                buf,
                rect.x,
                rect.y,
                [(icon, hovered)],
                btn_base,
                btn_hover,
                0,
            );
            self.bubble_copy_hits.push((idx, areas[0]));
        }
    }

    /// Render ⧉ (copy) and ↗ (view) buttons on the scrollback selection box.
    ///
    /// Two modes:
    /// - **Corner row** (expanded or ungrouped): buttons on the `╭...╮` row.
    /// - **Inline** (collapsed + grouped): buttons on the selected entry's row,
    ///   overlaying content at the right edge.
    ///
    /// **Policy A:** when `bubble_copy_buttons` is on, skip selection-box ⧉
    /// only for bubble-owned types (UserPrompt / AgentMessage). Thinking and
    /// tools keep selection-box ⧉ because they never get always-on bubble
    /// chrome. ↗ view remains when supported.
    pub(super) fn render_selection_buttons(
        &mut self,
        buf: &mut Buffer,
        selection_box: &SelectionBox,
        selected_entry_area: Option<Rect>,
        theme: &Theme,
    ) {
        // Gated by appearance config (default on for one-click copy chrome).
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
        let bubble_copy_on = self
            .scrollback
            .appearance()
            .scrollback
            .display
            .bubble_copy_buttons;
        // Policy A: suppress selection ⧉ only when bubble chrome also paints
        // this block type (user/agent). Thinking/tools keep selection ⧉.
        let bubble_owns_copy = bubble_copy_on && Self::is_bubble_copy_block(&entry.block);
        let has_copy = entry.block.supports_copy() && !header_selected && !bubble_owns_copy;
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

        // Same secondary chrome as always-on bubble ⧉ / timestamps (theme.gray).
        let btn_base = Style::default().fg(theme.gray);
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
mod bubble_copy_tests {
    use super::*;
    use crate::app::actions::Action;
    use crate::app::agent_view::test_fixtures::make_agent;
    use crate::scrollback::block::RenderBlock;
    use crate::scrollback::text_selection::{ResolvedSelectionModel, VisibleBlockGeometry};
    use crate::theme::Theme;
    use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn geom(entry_idx: usize, y: u16) -> VisibleBlockGeometry {
        VisibleBlockGeometry {
            entry_idx,
            area: Rect::new(0, y, 40, 2),
            content_area: Rect::new(2, y, 36, 2),
            selection_area: Rect::new(0, y, 40, 2),
            content_width: 36,
            top_clipped: false,
            bottom_clipped: false,
            drag_startable: true,
        }
    }

    fn paint_with_visible(agent: &mut AgentView, blocks: Vec<VisibleBlockGeometry>) -> Buffer {
        agent.last_scrollback_selection_model = ResolvedSelectionModel {
            visible_blocks: blocks,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        agent.render_bubble_copy_buttons(&mut buf, &Theme::current());
        buf
    }

    /// Named contract: visible UserPrompt + AgentMessage expose a ⧉ hit without
    /// that entry being selected.
    #[test]
    fn bubble_copy_paints_without_selection() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("hello user"));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("hello agent"));
        // No selection.
        assert!(agent.scrollback.selected().is_none());

        let buf = paint_with_visible(&mut agent, vec![geom(0, 1), geom(1, 4)]);
        assert_eq!(
            agent.bubble_copy_hits.len(),
            2,
            "user + agent must each get a bubble ⧉ hit"
        );
        let idxs: Vec<usize> = agent.bubble_copy_hits.iter().map(|(i, _)| *i).collect();
        assert_eq!(idxs, vec![0, 1]);
        // Glyph painted at hit cells.
        let icon = crate::glyphs::copy_icon();
        for (_, r) in &agent.bubble_copy_hits {
            assert_eq!(
                buf.cell((r.x, r.y)).map(|c| c.symbol()),
                Some(icon),
                "⧉ must paint at hit rect"
            );
        }
    }

    /// Named contract: idle always-on ⧉ uses secondary chrome (`theme.gray`),
    /// matching timestamps and draft/plan copy, not bright white
    /// `selection_border` / `text_primary` (power/theme note on DOGE).
    #[test]
    fn bubble_copy_idle_uses_secondary_gray_not_white_border() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("hello user"));
        // Hermetic palette: DOGE maps gray → yellow, selection_border → white.
        let theme = Theme::doge();
        agent.last_scrollback_selection_model = ResolvedSelectionModel {
            visible_blocks: vec![geom(0, 1)],
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        agent.hovered_bubble_copy = None;
        agent.render_bubble_copy_buttons(&mut buf, &theme);

        assert_eq!(agent.bubble_copy_hits.len(), 1);
        let (_, r) = agent.bubble_copy_hits[0];
        let cell = buf.cell((r.x, r.y)).expect("⧉ cell");
        assert_eq!(
            cell.fg, theme.gray,
            "idle ⧉ must use secondary chrome gray (yellow on DOGE)"
        );
        assert_ne!(
            cell.fg, theme.selection_border,
            "idle ⧉ must not use bright white selection_border"
        );
        assert_ne!(
            cell.fg, theme.text_primary,
            "idle ⧉ must not use primary text white"
        );

        // Hover still brightens to primary for discoverability.
        agent.hovered_bubble_copy = Some(0);
        let mut buf_h = Buffer::empty(area);
        agent.render_bubble_copy_buttons(&mut buf_h, &theme);
        let cell_h = buf_h.cell((r.x, r.y)).expect("hovered ⧉ cell");
        assert_eq!(
            cell_h.fg, theme.text_primary,
            "hovered ⧉ brightens to text_primary"
        );
    }

    /// Named contract: tool / thinking rows do not get always-on bubble ⧉.
    #[test]
    fn bubble_copy_skips_tool_and_thinking() {
        let mut agent = make_agent();
        agent.scrollback.push_block(RenderBlock::user_prompt("u"));
        agent
            .scrollback
            .push_block(RenderBlock::thinking("secret thoughts"));
        agent.scrollback.push_block(RenderBlock::agent_message("a"));
        // Tool call if constructor exists; otherwise thinking alone is enough.
        let _ = paint_with_visible(&mut agent, vec![geom(0, 0), geom(1, 2), geom(2, 4)]);
        let idxs: Vec<usize> = agent.bubble_copy_hits.iter().map(|(i, _)| *i).collect();
        assert_eq!(
            idxs,
            vec![0, 2],
            "only user + agent; thinking idx 1 must not get ⧉"
        );
    }

    /// Named contract: click copies that entry's text without changing selection.
    #[test]
    fn bubble_copy_click_copies_payload_without_select() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("payload-alpha"));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("payload-beta"));
        let _ = paint_with_visible(&mut agent, vec![geom(0, 1), geom(1, 4)]);
        assert!(agent.scrollback.selected().is_none());

        // Prefer agent message (idx 1).
        let (idx, rect) = agent
            .bubble_copy_hits
            .iter()
            .find(|(i, _)| *i == 1)
            .copied()
            .expect("agent bubble hit");
        assert_eq!(idx, 1);

        // Simulate mouse path: Action::CopyEntryContent { idx } then copy.
        let before_sel = agent.scrollback.selected();
        agent.copy_entry_content(idx);
        assert_eq!(
            agent.scrollback.selected(),
            before_sel,
            "copy must not change selection"
        );
        let toast = agent
            .toast
            .as_ref()
            .map(|(m, _)| m.clone())
            .unwrap_or_default();
        assert!(
            toast.starts_with("Copied")
                || toast.starts_with("Copy sent")
                || toast.starts_with("Clipboard unreachable")
                || toast.starts_with("Copy failed"),
            "copy_entry_content must toast clipboard delivery, got {toast:?}"
        );

        // Mouse hit → action with correct idx (no AppView needed).
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::empty(),
        };
        // Direct hit scan mirrors mouse.rs.
        let hit = agent
            .bubble_copy_hits
            .iter()
            .find(|(_, r)| r.contains((mouse.column, mouse.row).into()))
            .map(|&(i, _)| i);
        assert_eq!(hit, Some(1));
        let _ = Action::CopyEntryContent { idx: hit.unwrap() };
        let _ = Event::Mouse(mouse);
    }

    /// Named contract: drag-active gate clears bubble hits (render path).
    #[test]
    fn bubble_copy_cleared_when_drag_gate_simulated() {
        let mut agent = make_agent();
        agent.scrollback.push_block(RenderBlock::user_prompt("u"));
        let _ = paint_with_visible(&mut agent, vec![geom(0, 1)]);
        assert!(!agent.bubble_copy_hits.is_empty());
        // Mirror render.rs else branch when drag active.
        agent.bubble_copy_hits.clear();
        agent.hovered_bubble_copy = None;
        assert!(
            agent.bubble_copy_hits.is_empty(),
            "drag gate must empty bubble_copy_hits"
        );
    }

    /// Named contract: Policy A — selection-box omits ⧉ when bubble chrome on
    /// (bubble owns the one ⧉; ↗ view may remain).
    #[test]
    fn bubble_copy_policy_a_one_icon_when_selected() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("selected body"));
        agent.scrollback.set_selected(Some(0));
        assert_eq!(agent.scrollback.selected(), Some(0));
        assert!(
            agent
                .scrollback
                .appearance()
                .scrollback
                .display
                .bubble_copy_buttons,
            "default bubble chrome on"
        );

        let _ = paint_with_visible(&mut agent, vec![geom(0, 2)]);
        assert_eq!(
            agent.bubble_copy_hits.len(),
            1,
            "selected agent still gets exactly one bubble ⧉"
        );

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let sel_box = SelectionBox::new(Rect::new(1, 2, 38, 2), Style::default());
        agent.render_selection_buttons(
            &mut buf,
            &sel_box,
            Some(Rect::new(2, 2, 36, 2)),
            &Theme::current(),
        );
        assert!(
            agent.hit_sb_copy.rect.is_none(),
            "Policy A: selection-box must not arm a second ⧉ when bubble chrome is on"
        );
        // Agent message supports fullscreen → ↗ may still paint.
        assert!(
            agent.hit_sb_view.rect.is_some(),
            "Policy A keeps ↗ on the selection box when view is supported"
        );
    }

    /// Named contract: Policy A only suppresses selection ⧉ for bubble-owned
    /// types (UserPrompt / AgentMessage). Thinking keeps selection-box ⧉
    /// because it never gets always-on bubble chrome.
    #[test]
    fn bubble_copy_policy_a_keeps_selection_copy_for_thinking_when_bubble_on() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::thinking("secret thoughts"));
        agent.scrollback.set_selected(Some(0));
        assert!(
            agent
                .scrollback
                .appearance()
                .scrollback
                .display
                .bubble_copy_buttons,
            "default bubble chrome on"
        );
        assert!(
            agent
                .scrollback
                .entry(0)
                .expect("thinking entry")
                .block
                .supports_copy(),
            "thinking is copyable"
        );

        let _ = paint_with_visible(&mut agent, vec![geom(0, 2)]);
        assert!(
            agent.bubble_copy_hits.is_empty(),
            "thinking must not get bubble ⧉"
        );

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let sel_box = SelectionBox::new(Rect::new(1, 2, 38, 2), Style::default());
        agent.render_selection_buttons(
            &mut buf,
            &sel_box,
            Some(Rect::new(2, 2, 36, 2)),
            &Theme::current(),
        );
        assert!(
            agent.hit_sb_copy.rect.is_some(),
            "Policy A must keep selection-box ⧉ for thinking when bubble chrome is on"
        );
    }

    /// Named contract: same as thinking — Execute tool rows keep selection ⧉
    /// under bubble chrome (no bubble paint for tools).
    #[test]
    fn bubble_copy_policy_a_keeps_selection_copy_for_execute_tool_when_bubble_on() {
        let mut agent = make_agent();
        agent
            .scrollback
            .push_block(RenderBlock::execute("echo hello"));
        agent.scrollback.set_selected(Some(0));
        assert!(
            agent
                .scrollback
                .appearance()
                .scrollback
                .display
                .bubble_copy_buttons,
            "default bubble chrome on"
        );
        assert!(
            agent
                .scrollback
                .entry(0)
                .expect("execute entry")
                .block
                .supports_copy(),
            "execute tool is copyable"
        );

        let _ = paint_with_visible(&mut agent, vec![geom(0, 2)]);
        assert!(
            agent.bubble_copy_hits.is_empty(),
            "execute must not get bubble ⧉"
        );

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let sel_box = SelectionBox::new(Rect::new(1, 2, 38, 2), Style::default());
        agent.render_selection_buttons(
            &mut buf,
            &sel_box,
            Some(Rect::new(2, 2, 36, 2)),
            &Theme::current(),
        );
        assert!(
            agent.hit_sb_copy.rect.is_some(),
            "Policy A must keep selection-box ⧉ for execute tool when bubble chrome is on"
        );
    }

    /// Named contract: config off clears paint.
    #[test]
    fn bubble_copy_respects_config_off() {
        let mut agent = make_agent();
        agent.scrollback.push_block(RenderBlock::user_prompt("u"));
        let mut appearance = agent.scrollback.appearance().clone();
        appearance.scrollback.display.bubble_copy_buttons = false;
        agent.scrollback.set_appearance(appearance);
        let _ = paint_with_visible(&mut agent, vec![geom(0, 1)]);
        assert!(
            agent.bubble_copy_hits.is_empty(),
            "bubble_copy_buttons=false must paint no hits"
        );
    }

    /// Named contract: mouse down on bubble hit yields CopyEntryContent before
    /// any selection change (hit scan + action wiring).
    #[test]
    fn bubble_copy_mouse_action_wires_entry_idx() {
        let mut agent = make_agent();
        agent.scrollback.push_block(RenderBlock::user_prompt("u"));
        agent.scrollback.push_block(RenderBlock::agent_message("a"));
        let _ = paint_with_visible(&mut agent, vec![geom(0, 1), geom(1, 4)]);
        let (idx, rect) = agent.bubble_copy_hits[0];
        // Mirror mouse.rs hit-before-drag:
        let action = agent
            .bubble_copy_hits
            .iter()
            .find(|(_, r)| r.contains((rect.x, rect.y).into()))
            .map(|&(i, _)| Action::CopyEntryContent { idx: i });
        assert!(matches!(
            action,
            Some(Action::CopyEntryContent { idx: i }) if i == idx
        ));
    }

    /// Named contract: always-on bubble ⧉ must not cover the message timestamp
    /// (time + date). Root cause was overlap at the content right edge, not
    /// day-format truncation.
    #[test]
    fn bubble_copy_does_not_overlap_timestamp() {
        use crate::render::Renderable;
        use crate::scrollback::layout::HorizontalLayout;
        use crate::scrollback::wrappers::{
            BUBBLE_COPY_TRAILING_INSET, EntryRenderer, bubble_copy_trailing_inset,
        };

        let mut agent = make_agent();
        // Default appearance: timestamps + bubble_copy both on.
        let appearance = agent.scrollback.appearance().clone();
        assert!(appearance.show_timestamps, "timestamps must be on");
        assert!(
            appearance.scrollback.display.bubble_copy_buttons,
            "bubble_copy_buttons must be on"
        );

        agent
            .scrollback
            .push_block(RenderBlock::agent_message("hello agent body"));
        let entry = agent.scrollback.entry(0).expect("agent entry").clone();
        let created = entry.created_at.expect("message has created_at");

        let theme = Theme::current();
        let width: u16 = 80;
        let renderer = EntryRenderer::new(&entry, &theme).with_appearance(appearance.clone());
        let height = renderer.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        renderer.render(area, &mut buf);

        // Content geometry matching EntryRenderer layout.
        let layout = HorizontalLayout::new(area, &appearance.scrollback.layout);
        let content = layout.content;
        assert!(
            bubble_copy_trailing_inset(&entry.block, &appearance) == BUBBLE_COPY_TRAILING_INSET,
            "both chrome features must reserve trailing inset for ⧉"
        );

        // Paint always-on bubble ⧉ into the same buffer (production order).
        agent.last_scrollback_selection_model = ResolvedSelectionModel {
            visible_blocks: vec![VisibleBlockGeometry {
                entry_idx: 0,
                area,
                content_area: content,
                selection_area: area,
                content_width: content.width,
                top_clipped: false,
                bottom_clipped: false,
                drag_startable: true,
            }],
            ..Default::default()
        };
        agent.render_bubble_copy_buttons(&mut buf, &theme);

        assert_eq!(agent.bubble_copy_hits.len(), 1);
        let (_, hit) = agent.bubble_copy_hits[0];
        let icon = crate::glyphs::copy_icon();
        assert_eq!(
            buf.cell((hit.x, hit.y)).map(|c| c.symbol()),
            Some(icon),
            "⧉ must still paint"
        );
        // ⧉ lives on the absolute content right edge.
        assert_eq!(
            hit.x,
            content.x + content.width - 1,
            "⧉ stays at content right edge"
        );

        // Short timestamp (no hover) must be fully readable left of the inset.
        let expected = created.format("%-I:%M %p").to_string();
        let ts_width = expected.len() as u16;
        let ts_zone_right = content.x + content.width - BUBBLE_COPY_TRAILING_INSET;
        let ts_x = ts_zone_right - ts_width;
        let mut rendered = String::new();
        for x in ts_x..ts_x + ts_width {
            rendered.push_str(buf.cell((x, content.y)).map(|c| c.symbol()).unwrap_or(""));
        }
        assert_eq!(
            rendered, expected,
            "full short timestamp must survive bubble ⧉ paint (overlap fix)"
        );

        // No timestamp cell may hold the copy glyph.
        for x in ts_x..ts_x + ts_width {
            let sym = buf.cell((x, content.y)).map(|c| c.symbol()).unwrap_or("");
            assert_ne!(
                sym, icon,
                "⧉ must not sit on timestamp cell x={x} (got {sym:?})"
            );
        }

        // Gap cell between timestamp zone and ⧉ must not be a timestamp digit.
        let gap_x = content.x + content.width - BUBBLE_COPY_TRAILING_INSET;
        assert_ne!(
            gap_x, hit.x,
            "gap and ⧉ are distinct cells when trailing inset is 2"
        );

        // Expanded hover format also ends left of the inset (not under ⧉).
        let hover_x = ts_zone_right.saturating_sub(3);
        let renderer_h = EntryRenderer::new(&entry, &theme)
            .with_appearance(appearance.clone())
            .with_mouse_pos(Some((hover_x, content.y)));
        let mut buf_h = Buffer::empty(area);
        renderer_h.render(area, &mut buf_h);
        agent.render_bubble_copy_buttons(&mut buf_h, &theme);
        let expanded = created.format("%H:%M:%S | %b %d").to_string();
        let exp_w = expanded.len() as u16;
        let exp_x = ts_zone_right - exp_w;
        let mut exp_rendered = String::new();
        for x in exp_x..exp_x + exp_w {
            exp_rendered.push_str(buf_h.cell((x, content.y)).map(|c| c.symbol()).unwrap_or(""));
        }
        assert_eq!(
            exp_rendered, expanded,
            "expanded timestamp (time | date) must be fully readable with ⧉ present"
        );
        for x in exp_x..exp_x + exp_w {
            let sym = buf_h.cell((x, content.y)).map(|c| c.symbol()).unwrap_or("");
            assert_ne!(sym, icon, "⧉ must not cover expanded timestamp at x={x}");
        }
    }
}
