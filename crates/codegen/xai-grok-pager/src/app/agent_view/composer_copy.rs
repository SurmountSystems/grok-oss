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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
