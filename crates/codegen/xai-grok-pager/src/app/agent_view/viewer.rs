//! Line and block viewer popups plus the /btw panel: open/confirm/dismiss and their key/mouse handlers.

use super::{AgentPane, AgentView, BlockViewerResume, render_char_buttons};
use crate::app::app_view::InputOutcome;
use crate::key;
use crate::scrollback::selection::SelectionBox;
use crate::scrollback::types::DisplayMode;
use crate::theme::Theme;
use crate::views::block_viewer::{BlockViewerPane, format_blockquote};
use crate::views::btw_overlay::BTW_OVERLAY_ENTRY_IDX;
use crate::views::file_search::line_viewer::{LineViewerState, PlanViewerItem, SelectedPlanCta};
use crate::views::list_pane::ListItem;
use crate::views::plan_approval_view::PlanApprovalFocus;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use xai_grok_telemetry::events::{BlockViewerOpened, BlockViewerQuoted};
use xai_grok_telemetry::session_ctx::log_event;

pub(crate) enum IdleEnterQuote {
    NotHandled,
    ConsumedEmpty,
    Quoted(String),
}

#[cfg(test)]
#[path = "isolated_preview_revise_tests.rs"]
mod isolated_preview_revise_tests;

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

/// Isolated Preview search owns keys while the search bar is open, and
/// n/N after the query is accepted. Composer `/` stays slash.
pub(super) fn isolated_preview_search_owns_key(agent: &AgentView, key: &KeyEvent) -> bool {
    let Some(viewer) = agent.line_viewer.as_ref() else {
        return false;
    };
    if viewer.list_state.input_mode().is_some() {
        return true;
    }
    viewer.list_state.matcher().is_some() && (key!('n').matches(key) || key!('N').matches(key))
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
            // The file couldn't be read, so cancel the undo group
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

    pub(crate) fn selected_plan_cta(&self) -> Option<SelectedPlanCta> {
        self.line_viewer
            .as_ref()
            .and_then(|v| v.plan_ref())
            .and_then(|p| p.selected_cta)
    }

    fn plan_cta_has_comment_payload(&self) -> bool {
        self.composer_has_operator_notes()
            || self
                .plan_approval_view
                .as_ref()
                .is_some_and(|pav| !pav.comments.is_empty())
    }

    pub(super) fn composer_has_operator_notes(&self) -> bool {
        !self.prompt.text().trim().is_empty()
            || !self.prompt.images.is_empty()
            || self
                .prompt
                .textarea
                .elements()
                .iter()
                .any(|e| e.kind == crate::views::prompt_widget::KIND_PASTE)
    }

    /// Leftover slash-palette `/` (or `/view-plan`) is not review notes.
    pub(super) fn composer_is_leftover_slash_palette_only(&self) -> bool {
        matches!(
            self.prompt.text().trim(),
            "/" | "/view-plan" | "/show-plan" | "/plan-view"
        )
    }

    pub(super) fn composer_is_recognized_slash_command(&self) -> bool {
        // A `[Pasted: N lines]` chip is review notes even if the folded
        // body starts with `/implement`. Typed `/implement` without a
        // paste chip is still a slash / continue-nested-work path.
        // `/implement` is a skill, not a pager builtin, so registry
        // membership alone would Approve-with-comment that typed slash.
        if self
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == crate::views::prompt_widget::KIND_PASTE)
        {
            return false;
        }
        let trimmed = self.prompt.text().trim();
        if crate::app::auto_implement::is_implement_command_sentence(trimmed) {
            return true;
        }
        let Some(invocation) = crate::slash::parse_invocation(trimmed) else {
            return false;
        };
        let reg = self.prompt.slash_controller.registry();
        reg.get_for_dispatch(invocation.token).is_some() || reg.is_builtin(invocation.token)
    }

    /// Isolated Preview idle after present: a paste chip plus Enter
    /// Approves with those notes. A typed sentence while the plan viewer
    /// is open is not Approve. A `[Pasted: 13 lines]` chip whose body
    /// starts with `/implement` is still Approve-with-comment, not Plan
    /// Exit. Empty Enter never Approves. Keep-draft from before present
    /// still SendPrompt. Typed slash commands without a paste chip still
    /// send. Line-comment overlay still saves. Vanished Isolated Preview
    /// (pane shut, live waiter, Preview focus) still Approves with those
    /// notes. A typed human sentence in an open Isolated Preview is not
    /// that setup. Leftover slash-palette `/` is not notes.
    pub(crate) fn isolated_preview_idle_enter_approves_with_notes(&self) -> bool {
        if self.plan_decision_resolved {
            return false;
        }
        if self.plan_feedback_in_flight.is_some() {
            return false;
        }
        let Some(pav) = self.plan_approval_view.as_ref() else {
            return false;
        };
        if pav.focus == PlanApprovalFocus::Commenting {
            return false;
        }
        if !self.is_plan_viewer() && pav.focus != PlanApprovalFocus::Preview {
            return false;
        }
        if !self.composer_has_operator_notes() {
            return false;
        }
        if self.composer_is_leftover_slash_palette_only() {
            return false;
        }
        if self.composer_is_keep_draft_from_before_present() {
            return false;
        }
        if self.composer_is_recognized_slash_command() {
            return false;
        }
        // A typed sentence whose keystroke snapshot matches the composer is
        // a human turn, not Approve. A paste chip still Approves. Leftover
        // slash plus notes has no snapshot, so Enter still Approves. A
        // marked Comment CTA Enter sends and must not be re-approved.
        let paste_chip = self
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == crate::views::prompt_widget::KIND_PASTE);
        if self.selected_plan_cta() == Some(SelectedPlanCta::Comment) && !paste_chip {
            return false;
        }
        if self.isolated_preview_typed_open_enter_is_human_turn() {
            let typed_snapshot = pav
                .feedback_draft
                .as_deref()
                .is_some_and(|draft| draft.trim() == self.prompt.text().trim());
            if typed_snapshot {
                return false;
            }
        }
        if pav.focus == PlanApprovalFocus::Prompt {
            return matches!(
                pav.prompt_intent,
                PlanPromptIntent::Comment | PlanPromptIntent::ApproveNotes
            );
        }
        true
    }

    /// Typed sentence in an open Isolated Preview. Not a paste chip, not a
    /// vanished pane, not keep-draft, not a typed slash, not a line comment.
    /// Session Multiline Enter still inserts a newline.
    pub(crate) fn isolated_preview_typed_open_enter_is_human_turn(&self) -> bool {
        if !self.is_plan_viewer() {
            return false;
        }
        if self.plan_decision_resolved || self.plan_feedback_in_flight.is_some() {
            return false;
        }
        let Some(pav) = self.plan_approval_view.as_ref() else {
            return false;
        };
        if pav.focus == PlanApprovalFocus::Commenting {
            return false;
        }
        // Prompt focus is plan feedback (Revise / Comment), not a Preview
        // human turn. Prompt+Revise must reach send_plan_feedback.
        if pav.focus == PlanApprovalFocus::Prompt {
            return false;
        }
        if self
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == crate::views::prompt_widget::KIND_PASTE)
        {
            return false;
        }
        if self.prompt.text().trim().is_empty() {
            return false;
        }
        if self.composer_is_leftover_slash_palette_only()
            || self.composer_is_keep_draft_from_before_present()
            || self.composer_is_recognized_slash_command()
        {
            return false;
        }
        if self.multiline_mode && crate::appearance::cache::load_composer_multiline() {
            return false;
        }
        true
    }

    /// Flush the sentence to the prompt write-ahead log before any send.
    /// Leave the composer and the parked plan so click Approve still has
    /// the sentence as notes. Do not Approve. Do not Interject. Do not
    /// SendPrompt. Do not set `plan_decision_resolved`.
    pub(crate) fn record_open_preview_typed_enter_human_turn(&mut self) -> InputOutcome {
        let text = self.prompt.text().to_string();
        let images = self.prompt.images.clone();
        self.append_prompt_wal(
            xai_grok_shell::session::prompt_wal::PromptWalKind::Send,
            &text,
            &images,
        );
        InputOutcome::Changed
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
        // Operator: "I can't even revise plans now... exit won't work too.
        // it's fucking stuck!!" Rewrite-wait must not swallow Exit. Revise
        // still waits for a live park (`send_plan_feedback` takes
        // `plan_approval_view`). Empty Enter never Approves.
        if self.plan_feedback_in_flight.is_some() && choice != SelectedPlanCta::Exit {
            return InputOutcome::Changed;
        }
        self.mark_selected_plan_cta(choice);
        match choice {
            SelectedPlanCta::Approve => self.approve_plan(),
            SelectedPlanCta::Comment => {
                // Isolated Preview Comment CTA still arms a line range.
                // Empty-prompt `c` types in the Human box; it is not this path.
                // Restore the stashed Human box so Comment-then-Approve stays
                // Prompt + Comment intent, not a wiped line-note overlay.
                let _ = self.enter_plan_commenting();
                if let Some(pav) = self.plan_approval_view.as_mut() {
                    if let Some(stashed) = pav.stashed_feedback_prompt.take() {
                        self.prompt.restore(stashed);
                    }
                }
                self.focus_plan_prompt(PlanPromptIntent::Comment)
            }
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

    /// Keep-draft from before live present: Isolated Preview Enter still
    /// SendPrompt. Isolated Preview Preview Human text is also SendPrompt.
    fn composer_is_keep_draft_from_before_present(&self) -> bool {
        let Some(pav) = self.plan_approval_view.as_ref() else {
            return false;
        };
        let notes = self.prompt.text();
        pav.stashed_prompt.text.trim() == notes.trim()
            && pav.stashed_prompt.images.len() == self.prompt.images.len()
            && (!notes.trim().is_empty() || !self.prompt.images.is_empty())
    }

    /// Isolated Preview composer is a Human box unless Comment was clicked.
    /// Operator: soft planning is broken; lost that prompt; nothing happened;
    /// cannot submit the prompt now. Human text while Isolated Preview is
    /// open is a Human turn, not only plan comment 1. Comment CTA is the
    /// comment path. Ride-Approve chrome must not block a later non-empty
    /// Enter. Empty Enter never Approves. Keep-draft from before live
    /// present still SendPrompt. Line-comment overlay Enter still saves.
    /// Session Multiline Enter still inserts a newline.
    pub(super) fn hold_parked_plan_review_comments_from_enter(&mut self) -> bool {
        let text = self.prompt.text().to_string();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        // Recognized slash commands are not plan review comments.
        // `/plan queue` must still hold on the prompt queue; `--soft` is
        // not the queue hold token. Typed `/implement` without a paste
        // chip is still that slash, not a parked Isolated Preview comment.
        if self.composer_is_recognized_slash_command() {
            return false;
        }
        let allow_newlines = crate::appearance::cache::load_composer_multiline();
        if self.multiline_mode && allow_newlines {
            return false;
        }
        let Some(pav) = self.plan_approval_view.as_ref() else {
            return false;
        };
        if pav.focus == PlanApprovalFocus::Commenting {
            return false;
        }
        if self.composer_is_keep_draft_from_before_present() {
            return false;
        }
        let already_stashed = pav.comment_held_from_enter
            && pav
                .feedback_draft
                .as_deref()
                .is_some_and(|draft| draft.trim() == trimmed);
        // Isolated Preview Preview is a Human send. Comment CTA stashes
        // once. A matching ride-Approve draft must not recapture Enter.
        // Keystroke snapshots of `feedback_draft` are not that Enter.
        let hold = match pav.prompt_intent {
            PlanPromptIntent::Comment => !already_stashed,
            PlanPromptIntent::Revise
            | PlanPromptIntent::Questions
            | PlanPromptIntent::ApproveNotes => false,
        };
        if !hold {
            return false;
        }
        if let Some(pav) = self.plan_approval_view.as_mut() {
            pav.feedback_draft = Some(text);
            pav.comment_held_from_enter = true;
        }
        self.persist_unsent_composer_draft_now();
        self.show_toast("This comment will ride Approve. Click Approve, Clarify, or Revise.");
        true
    }

    /// Handle a key event while the line viewer is open.
    pub(super) fn handle_line_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let in_plan_approval = self.plan_approval_view.is_some();
        let plan_present = in_plan_approval || self.is_plan_viewer();

        let input_bar_active = self
            .line_viewer
            .as_ref()
            .is_some_and(|v| v.list_state.input_mode().is_some());

        // When the search/filter/goto input bar is active, let ListPane handle everything
        // Comment mode is special: the list state does not consume Enter/Esc (it returns false), so save/cancel are handled here
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

        if in_plan_approval && crate::input::key::RowWalk::from_key(key).is_some() {
            return self.handle_plan_feedback_key(key);
        }

        // In plan approval, `Esc` doesn't close the viewer (use `q` / `Ctrl+\`)
        // It still clears a transient visual selection or an accepted search matcher first
        // Backing out of the dashboard overlay declines to fire while a matcher is active, so without this clearing Esc would be a dead key
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
            // Esc closes ride-Approve capture. It does not Approve, and it
            // does not wipe a mid-compose Human draft.
            if let Some(pav) = self.plan_approval_view.as_mut() {
                pav.feedback_draft = None;
                pav.comment_held_from_enter = false;
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

        if in_plan_approval && key!('c').matches(key) {
            return self.enter_plan_commenting();
        }

        // Casual mode: same `c` / `s` shortcuts as plan approval so the footer hints actually work
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

        if in_plan_approval && key!('a').matches(key) {
            return self.approve_plan();
        }

        // s: switch to prompt so the user can type an overall revision message before submitting
        // Enter from Prompt does the actual send
        if in_plan_approval && key!('s').matches(key) {
            if let Some(ref mut pav) = self.plan_approval_view {
                pav.focus = PlanApprovalFocus::Prompt;
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
                let focus = self.plan_approval_view.as_ref().map(|p| p.focus);
                if focus == Some(PlanApprovalFocus::Prompt) {
                    return self.handle_plan_feedback_key(key);
                }
                if let Some(outcome) = self.send_marked_comment_cta_enter() {
                    return outcome;
                }
                if self.isolated_preview_idle_enter_approves_with_notes() {
                    self.snapshot_or_clear_plan_feedback_draft();
                    self.prompt.slash_close();
                    return self.approve_plan_from_enter();
                }
                if self.isolated_preview_typed_open_enter_is_human_turn() {
                    return self.record_open_preview_typed_enter_human_turn();
                }
                if self.hold_parked_plan_review_comments_from_enter() {
                    return InputOutcome::Changed;
                }
                // Preview: empty Enter never Approves. A non-empty draft
                // already stashed above so it can ride Approve.
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
            if in_plan_approval || self.is_plan_viewer() {
                return self.delete_plan_comment_at_cursor();
            }
            self.confirm_line_viewer(false);
            return InputOutcome::Changed;
        }
        if key!('y').matches(key) {
            if self.is_plan_viewer() {
                let commenting = self
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|pav| pav.focus == PlanApprovalFocus::Commenting)
                    || self.is_casual_commenting();
                let empty =
                    self.prompt.text().trim().is_empty() && !self.prompt.file_search_visible();
                // Empty Preview `y` copies. A focused plan comment composer
                // inserts `y`, including an empty draft. A live Prompt or
                // Preview draft still types `y`.
                if commenting || !empty {
                    return self.handle_plan_feedback_key(key);
                }
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
        // Two-stage Ctrl+C on every Isolated Preview prompt: first press
        // with a draft clears; empty second press Exits / abandons. Do not
        // map first Ctrl+C to leftover close.
        if key!('c', CONTROL).matches(key) {
            return self.handle_plan_feedback_key(key);
        }
        if key!(Esc).matches(key) || key!('q').matches(key) {
            if in_plan_approval {
                return InputOutcome::Changed;
            }
            // In the plan viewer, Esc first clears visual selection / search before closing
            // q and Ctrl-C always close immediately
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
        self.persist_session_plan_dock_open(false);
        if let Some(sid) = self.session.session_id.as_ref() {
            crate::slash::commands::plan::persist_isolated_preview_open(
                &self.session.cwd.to_string_lossy(),
                sid.0.as_ref(),
                false,
            );
        }
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
        // The modal can close mid-comment via [✗], click-outside, or any path that skips `cancel_casual_plan_commenting`
        // Restore the pre-comment prompt text so the user's original text isn't lost behind the comment draft
        // Mirrors `cancel_casual_plan_commenting`.
        if let Some(stashed) = self.casual_stashed_prompt.take() {
            self.prompt.restore(stashed);
        }
        self.casual_commenting_range = None;
        self.casual_editing_comment_id = None;
        self.clear_prompt_double_click_pairing();
    }

    /// Dismiss the /btw panel. If Done, flush response to scrollback first.
    pub(super) fn dismiss_btw_panel(&mut self) -> InputOutcome {
        self.flush_open_btw_to_scrollback();
        self.btw_state = None;
        self.minimal_btw_lifecycle = None;
        self.btw_focused = false;
        self.clear_btw_drag_state();
        // Panel gone: drop the held highlight. Scroll only cancels an in-flight drag.
        self.clear_btw_owned_selection();
        InputOutcome::Changed
    }

    /// Cancel an in-flight `/btw` text drag. Does not drop a finished highlight.
    /// Selection coordinates are content-relative, so scroll does not make them stale.
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

        // Header directory click stays live while Isolated Preview is docked.
        // The line viewer must not swallow it as an outside-modal dismiss.
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.hit_cwd.contains(mouse.column, mouse.row)
        {
            let cwd = self.session.cwd.clone();
            self.open_path(&cwd);
            return InputOutcome::Changed;
        }

        let Some(ref mut viewer) = self.line_viewer else {
            return InputOutcome::Changed;
        };
        // `discard_in_progress_comment` needs `&mut self`. Defer it until
        // after this `viewer` borrow ends (E0499).
        let mut restore_stashed_on_leave_commenting = false;

        // `popup_area` is the list-rendered area, excluding the divider and footer rows in plan modes
        // Mouse events dispatch into `ListPaneState` against it
        // The click-outside check uses `modal_area` so clicks on the divider or the space between footer buttons don't close the modal
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
        let close_hit = viewer.comment_close_button_at(mouse.column, mouse.row);
        // Cached `is_plan_viewer()` so we don't need to call self while the line_viewer is mutably borrowed below
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
                // A click on the close button cancels
                if close_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    self.cancel_line_viewer();
                    return InputOutcome::Changed;
                }
                // A click on the fullscreen button toggles fullscreen
                if fs_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if let Some(ref mut v) = self.line_viewer {
                        v.fullscreen = !v.fullscreen;
                    }
                    return InputOutcome::Changed;
                }
                // A click on the `[✗]` close button must not fall through to click-to-comment edit mode
                if let Some(comment_id) = close_hit {
                    return self.delete_plan_comment_by_id(comment_id);
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
                        // Casual mode: the only action button shown is `s send` (when there are comments to send)
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
                    // The comment button is only set on plan viewers, so the two arms above are exhaustive in practice
                    // Return here to make the dead fall-through explicit and to match the abandon/approve hit patterns just above
                    return InputOutcome::Changed;
                }
                if search_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if let Some(ref mut viewer) = self.line_viewer {
                        viewer.list_state.open_search(&viewer.lines);
                    }
                    return InputOutcome::Changed;
                }
                if copy_btn_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    return self.copy_plan_full();
                }
                if send_area.is_some_and(|a| a.contains((mouse.column, mouse.row).into())) {
                    if self.plan_approval_view.is_some() {
                        return self.click_plan_cta(SelectedPlanCta::Revise);
                    }
                    // Isolated Preview Revise is not a casual line-comment send.
                    // Empty notes still revise via PLAN_REVISE_HUMAN_LINE inside
                    // send_plan_feedback. Comment stays the hub for notes.
                    if self.isolated_preview_shows_secondary_plan {
                        self.park_local_idle_plan_decision_if_needed();
                        self.park_isolated_preview_revise_decision();
                        let notes = self.isolated_preview_revise_notes_from_comments();
                        let feedback = if notes.trim().is_empty() {
                            None
                        } else {
                            Some(notes)
                        };
                        return self.send_plan_feedback(feedback);
                    }
                    return self.send_casual_plan_comments();
                }
                // Mermaid buttons are checked before click-to-comment
                // The early return ends the `viewer` borrow so `handle_inline_media_click` can take `&mut self`
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
                    if was_commenting {
                        pav.commenting_range = None;
                        pav.editing_comment_id = None;
                    }
                }
                if was_commenting {
                    let stashed = self
                        .plan_approval_view
                        .as_mut()
                        .and_then(|pav| pav.stashed_feedback_prompt.take());
                    if let Some(stashed) = stashed {
                        self.prompt.restore(stashed);
                    } else {
                        self.prompt.set_text("");
                    }
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
                if is_plan_preview {
                    let hovered_comment = popup_area
                        .filter(|area| area.contains((mouse.column, mouse.row).into()))
                        .and_then(|area| viewer.comment_id_at_screen_row(mouse.row, area));
                    let prev_hovered = viewer.plan_ref().and_then(|p| p.hovered_comment_id);
                    if hovered_comment != prev_hovered {
                        viewer.plan_mut().hovered_comment_id = hovered_comment;
                        changed = true;
                    }
                    let close_hover = close_hit.is_some();
                    let prev_close = viewer.plan_ref().is_some_and(|p| p.close_button_hovered);
                    if close_hover != prev_close {
                        viewer.plan_mut().close_button_hovered = close_hover;
                        changed = true;
                    }
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
                // Drag-to-extend works in both plan-approval and casual plan-preview modes (anywhere the PlanPreview viewer is showing)
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
                            // Stash only on the first entry into commenting, same as enter_plan_commenting
                            // A second gutter drag while Commenting must not replace the stashed prompt text
                            if pav.stashed_feedback_prompt.is_none() {
                                pav.stashed_feedback_prompt = Some(self.prompt.stash());
                            }
                            pav.commenting_range = Some(range);
                            pav.editing_comment_id = None;
                            pav.focus = PlanApprovalFocus::Commenting;
                            self.prompt.set_text("");
                        } else {
                            // Stash only on the first entry; see enter_casual_plan_commenting
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
                // Drag selection works in both modes whenever the plan preview is showing
                // It only works on source rows: the drag anchor needs a 1-based line number
                if is_plan_preview && let Some(ln) = clicked_line {
                    viewer.plan_mut().gutter_drag_start = Some(ln);
                    viewer.plan_mut().gutter_drag_end = Some(ln);
                }

                viewer.plan_mut().last_click_at = Some(std::time::Instant::now());

                // A single click on any list row (a source line or an existing comment row) enters commenting or comment editing for that row
                // It is the same shortcut as selecting the row and pressing `c` / Enter, in both plan-approval and casual plan-preview modes
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
                // Skip the click-to-comment trigger if the user is already composing a comment
                // Without this guard, any click on a list row would re-enter commenting and re-stash the prompt, now holding the comment draft
                // That clobbers the user's pre-comment text and makes any mouse click commit to a fresh comment instead of just moving the cursor
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
    /// **Corner row** (expanded or ungrouped): buttons on the `╭...╮` row.
    /// **Inline** (collapsed and grouped): buttons on the selected entry's row, overlaying content at the right edge.
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
        // Inline: entry is collapsed and part of a group (group_range > 1)
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

    pub(crate) fn dismiss_block_viewer(&mut self) {
        if let Some(viewer) = self.block_viewer.take() {
            self.block_viewer_resume = Some(BlockViewerResume {
                entry_id: viewer.entry_id,
                kind: viewer.kind,
                selected_id: viewer.resume_selected_id(),
                scroll_offset: viewer.list_state.scroll_offset(),
                follow_mode: viewer.list_state.follow_mode,
            });
        }
    }

    pub(crate) fn clear_block_viewer(&mut self) {
        self.block_viewer = None;
        self.block_viewer_resume = None;
    }

    pub(crate) fn show_bg_task_viewer(&mut self, task_id: &str) -> bool {
        let Some(task) = self.session.bg_tasks.get(task_id) else {
            return false;
        };
        // A task can lack a scrollback anchor: the completed-early race never pushes a block,
        // and a scrollback swap can drop it. The viewer renders from the task's own stdout,
        // so open it on the sentinel anchor instead of dead-clicking the [↗] button.
        let entry_id = task
            .scrollback_entry_id
            .unwrap_or_else(|| crate::scrollback::entry::EntryId::new(0));
        let is_running = task.status == crate::app::agent::BgTaskStatus::Running;
        let pane = crate::views::block_viewer::BlockViewerPane::for_bg_task(
            entry_id,
            task_id,
            &task.stdout,
            is_running,
        );
        self.install_block_viewer(pane);
        self.set_active_pane(AgentPane::Scrollback, true);
        true
    }

    pub(crate) fn install_block_viewer(&mut self, mut pane: BlockViewerPane) {
        if let Some(resume) = self.block_viewer_resume
            && resume.entry_id == pane.entry_id
            && resume.kind == pane.kind
            && !(resume.follow_mode && pane.list_state.follow_mode)
        {
            if resume.follow_mode {
                pane.pin_to_tail();
            } else {
                pane.list_state.follow_mode = false;
                if let Some(id) = resume
                    .selected_id
                    .filter(|id| pane.contains_item_id(*id) || *id > u64::MAX / 2)
                {
                    pane.list_state.select_by_id(id);
                }
                pane.list_state.set_scroll_offset(resume.scroll_offset);
                pane.request_reveal_selection();
            }
        }
        self.show_block_viewer(pane);
    }

    pub(crate) fn show_block_viewer(&mut self, pane: BlockViewerPane) {
        log_event(BlockViewerOpened {
            kind: pane.kind.telemetry_kind(),
        });
        self.block_viewer = Some(pane);
    }

    pub(crate) fn try_take_idle_enter_quote(&mut self, key: &KeyEvent) -> IdleEnterQuote {
        let Some(viewer) = self.block_viewer.as_ref() else {
            return IdleEnterQuote::NotHandled;
        };
        if viewer.list_state.input_mode().is_some()
            || key.code != KeyCode::Enter
            || key.modifiers != KeyModifiers::NONE
            || key.kind != KeyEventKind::Press
        {
            return IdleEnterQuote::NotHandled;
        }
        let quoted = format_blockquote(&viewer.selected_plain_text());
        if quoted.is_empty() {
            return IdleEnterQuote::ConsumedEmpty;
        }
        log_event(BlockViewerQuoted {
            kind: viewer.kind.telemetry_kind(),
        });
        self.dismiss_block_viewer();
        IdleEnterQuote::Quoted(quoted)
    }

    pub(crate) fn insert_quoted_reply(&mut self, quoted: &str) {
        self.prompt_input_mode = super::PromptInputMode::Normal;
        let delim_at = self
            .prompt
            .textarea
            .selection_range()
            .map(|range| range.start)
            .unwrap_or_else(|| self.prompt.cursor());
        let at_line_start = delim_at == 0
            || self
                .prompt
                .text()
                .as_bytes()
                .get(delim_at - 1)
                .is_some_and(|b| *b == b'\n');
        self.prompt.textarea.begin_undo_group();
        if !at_line_start {
            self.prompt.insert_replacing_selection("\n");
        } else if self.prompt.textarea.selection_range().is_some() {
            self.prompt.insert_replacing_selection("");
        }
        self.prompt.handle_paste(quoted);
        self.prompt.insert_replacing_selection("\n\n");
        self.prompt.textarea.end_undo_group();
        self.prompt.refresh_slash(&self.session.models);
        if let Some(eff) = self.notify_suggestion_text_changed() {
            self.pending_effects.push(eff);
        }
        if let Some(eff) = self.notify_plugin_cta_text_changed() {
            self.pending_effects.push(eff);
        }
        self.set_active_pane(AgentPane::Prompt, true);
    }

    /// Handle a key event when the block viewer is open.
    ///
    /// Returns `Changed` if consumed, `Unchanged` if the key should bubble up.
    pub(super) fn handle_block_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let Some(viewer) = self.block_viewer.as_ref() else {
            return InputOutcome::Unchanged;
        };

        if viewer.is_close_key(key) {
            self.dismiss_block_viewer();
            return InputOutcome::Changed;
        }

        match self.try_take_idle_enter_quote(key) {
            IdleEnterQuote::NotHandled => {}
            IdleEnterQuote::ConsumedEmpty => return InputOutcome::Changed,
            IdleEnterQuote::Quoted(quoted) => {
                self.insert_quoted_reply(&quoted);
                return InputOutcome::Changed;
            }
        }

        let Some(ref mut viewer) = self.block_viewer else {
            return InputOutcome::Unchanged;
        };

        if !viewer.handle_key(key) {
            return InputOutcome::Unchanged;
        }

        // Handle raw toggle: capture old source map, toggle, rebuild with stability
        if viewer.raw_toggle_pending {
            viewer.raw_toggle_pending = false;
            // Record the scroll anchor before the toggle so the selected line stays at the same screen position after the rebuild
            viewer.list_state.set_scroll_anchor();
            // Capture the source map before the toggle for cursor mapping
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

        // Route to the modal window controls first (close button, click-outside)
        let modal_outcome =
            handle_modal_mouse(&mut viewer.modal, mouse.kind, mouse.column, mouse.row);
        match modal_outcome {
            ModalWindowOutcome::CloseRequested => {
                self.dismiss_block_viewer();
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

        // Collect any pending copy text: drag-release auto-copy (like scrollback finish_text_drag) or Y/y key handler copy
        let drag_text = viewer.drag_copy_text.take();
        let entry_id = viewer.entry_id;
        let key_text = if drag_text.is_none() {
            self.scrollback
                .get_by_id(entry_id)
                .and_then(|entry| viewer.process_pending_copy(entry))
        } else {
            None
        };
        // The viewer borrow ends here, so clipboard and toast can use &mut self
        if let Some(text) = drag_text.or(key_text) {
            self.copy_to_clipboard(&text);
        }

        InputOutcome::Changed
    }

    /// Dynamic fold label for the shortcuts bar hint.
    ///
    /// Returns "expand" if the selected entry is collapsed/truncated, "collapse" if expanded, or `None` if the selected entry isn't foldable.
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
