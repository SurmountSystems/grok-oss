/// Composer copy for text typed in the wrong prompt.
///
/// The copy button copies the typed text. It does not submit. It does not
/// clear. It posts a toast for how many characters were copied. All copy
/// functions should state how many characters were copied.
///
/// Unicode scalar values, not bytes.
pub fn copied_character_count(text: &str) -> usize {
    text.chars().count()
}

/// Same character-count toast for the composer Copy button and for `y:copy`.
pub fn copied_characters_toast(text: &str) -> String {
    let n = copied_character_count(text);
    format!("Copied {n} characters.")
}

/// Result of copying the composer. `submit` stays false. `cleared` stays false.
/// `text` is the typed text that was copied, unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerCopy {
    pub text: String,
    pub toast: String,
    pub submit: bool,
    pub cleared: bool,
}

pub fn copy_composer_text(typed: &str) -> ComposerCopy {
    ComposerCopy {
        text: typed.to_string(),
        toast: copied_characters_toast(typed),
        submit: false,
        cleared: false,
    }
}

/// Painted label for the composer Copy button. Six columns.
pub(crate) const COMPOSER_COPY_LABEL: &str = "[Copy]";

impl super::AgentView {
    /// Copy the text typed in the composer. Does not submit. Does not clear.
    /// [`Self::copy_to_clipboard`] already toasts the character count, so this
    /// does not show a second toast.
    pub fn copy_typed_composer(&mut self) -> crate::clipboard::CopyDelivery {
        let copied = copy_composer_text(self.prompt.text());
        debug_assert!(!copied.submit, "the copy button does not submit");
        debug_assert!(!copied.cleared, "the copy button does not clear");
        self.copy_to_clipboard(&copied.text)
    }

    /// Remember the composer `[Copy]` hit rectangle for this frame.
    pub(crate) fn set_composer_copy_button(&mut self, rect: Option<ratatui::layout::Rect>) {
        self.composer_copy_button = rect;
    }

    /// Click on the composer `[Copy]` button copies the typed text.
    /// Returns true when the click landed on that button.
    pub(crate) fn click_composer_copy_button(&mut self, col: u16, row: u16) -> bool {
        let Some(rect) = self.composer_copy_button else {
            return false;
        };
        if !rect.contains((col, row).into()) {
            return false;
        }
        self.copy_typed_composer();
        true
    }

    /// A left click inside the stored composer copy rect copies the typed prompt.
    /// Returns true when this click was that button, so the textarea must not see it.
    pub(crate) fn left_click_on_composer_copy_button(
        &mut self,
        mouse: &crossterm::event::MouseEvent,
    ) -> bool {
        if !matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            return false;
        }
        self.click_composer_copy_button(mouse.column, mouse.row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    /// Operator: put a copy button on the composer for text typed in the wrong
    /// prompt. The copy button copies the typed text. It does not submit. It
    /// does not clear. It posts a toast for how many characters were copied.
    #[test]
    fn composer_copy_button_copies_typed_text_does_not_submit_or_clear() {
        let copied = copy_composer_text("wrong prompt");
        assert_eq!(
            copied.text, "wrong prompt",
            "the copy button copies the typed text"
        );
        assert!(!copied.submit, "it does not submit");
        assert!(!copied.cleared, "it does not clear");
        assert_eq!(copied.toast, "Copied 12 characters.");
        assert_eq!(
            copied.text, "wrong prompt",
            "composer text stays after copy"
        );
    }

    /// Operator: it posts a toast for how many characters were copied.
    /// All copy functions should state how many characters were copied.
    /// é is one character, not two bytes.
    #[test]
    fn copy_toast_states_unicode_character_count() {
        assert_eq!(copied_characters_toast("café"), "Copied 4 characters.");
        assert_eq!(copied_character_count("café"), 4);
        assert_eq!(copied_characters_toast(""), "Copied 0 characters.");
        assert_eq!(copy_composer_text("").toast, "Copied 0 characters.");
        assert!(!copy_composer_text("").submit);
        assert!(!copy_composer_text("").cleared);
    }

    /// Operator: `y:copy` may stay. It must use the same character-count toast.
    #[test]
    fn y_copy_uses_the_same_character_count_toast() {
        let typed = "wrong prompt";
        assert_eq!(
            copied_characters_toast(typed),
            copy_composer_text(typed).toast
        );
    }

    const TYPED: &str = "wrong prompt";

    fn agent_with_stored_copy_rect() -> (super::super::AgentView, u16, u16) {
        let mut agent = super::super::test_fixtures::make_agent();
        agent.prompt.set_text(TYPED);
        agent.pane_areas.prompt = Rect::new(0, 10, 80, 8);
        agent.set_composer_copy_button(Some(Rect::new(70, 10, 3, 1)));
        (agent, 71, 10)
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn assert_click_copied_the_typed_prompt(agent: &super::super::AgentView) {
        let toast = agent.toast.as_ref().map(|(msg, _)| msg.as_str());
        assert!(
            toast.is_some_and(|msg| msg.starts_with("Copied 12 characters.")),
            "a left click on the stored composer copy rect copies the typed prompt and toasts how many characters were copied, got {toast:?}"
        );
        assert_eq!(
            agent.prompt.text(),
            TYPED,
            "the copy click does not clear the typed prompt"
        );
        assert!(
            agent.session.pending_prompts.is_empty(),
            "the copy click does not submit the typed prompt"
        );
    }

    /// A left click on the stored composer copy rect copies the typed prompt.
    /// The textarea must not swallow that click.
    #[test]
    fn left_click_on_the_stored_composer_copy_rect_copies_the_typed_prompt() {
        let (mut agent, column, row) = agent_with_stored_copy_rect();
        let _ = agent.handle_mouse(&left_click(1, 11));
        assert!(
            agent.toast.is_none(),
            "a click inside the prompt but outside the stored copy rect does not copy"
        );
        assert_eq!(agent.prompt.text(), TYPED);
        let _ = agent.handle_mouse(&left_click(column, row));
        assert_click_copied_the_typed_prompt(&agent);
    }

    /// While a plan line viewer is open, the same stored rect still copies.
    /// That click is routed through the plan prompt before the textarea.
    #[test]
    fn left_click_on_the_stored_composer_copy_rect_copies_while_a_plan_line_viewer_is_open() {
        let (mut agent, column, row) = agent_with_stored_copy_rect();
        let viewer =
            crate::views::file_search::line_viewer::LineViewerState::open_markdown_content(
                "plan.md",
                "# Plan\n\nStep\n".to_owned(),
                None,
            )
            .expect("plan markdown opens a line viewer");
        agent.line_viewer = Some(viewer);
        agent.plan_approval_view =
            Some(super::super::test_fixtures::make_plan_approval_view_state());
        let registry = crate::actions::ActionRegistry::defaults();
        let _ = agent.handle_input(
            &crossterm::event::Event::Mouse(left_click(column, row)),
            &registry,
        );
        assert_click_copied_the_typed_prompt(&agent);
    }

    /// Question feedback paints yellow `[Copy]` into the same stored rect.
    /// That click copies through the same function and does not leave input mode.
    #[test]
    fn left_click_on_the_question_feedback_copy_rect_copies_the_typed_prompt() {
        let (mut agent, column, row) = agent_with_stored_copy_rect();
        agent.set_active_pane(super::super::AgentPane::Prompt, true);
        let mut question =
            super::super::paste::paste_key_tests::make_question_view_state_in_input_mode();
        question.tool_call_id = "feedback".into();
        agent.question_view = Some(question);
        assert_eq!(
            agent.focused_card(),
            Some(super::super::BlockingCard::Question),
            "question feedback owns this click, so the main mouse path must not be the one that copies"
        );
        let registry = crate::actions::ActionRegistry::defaults();
        let _ = agent.handle_input(
            &crossterm::event::Event::Mouse(left_click(column, row)),
            &registry,
        );
        assert_click_copied_the_typed_prompt(&agent);
        assert!(
            agent
                .question_view
                .as_ref()
                .is_some_and(|qv| qv.focus == crate::views::question_view::QuestionFocus::InputMode),
            "the copy click stays in question input mode"
        );
    }
}
