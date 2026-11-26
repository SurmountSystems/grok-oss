//! The planning box shows the text the L1 prompt implementation produced.
//!
//! A second insert (its own string push, or a missing revision-box helper)
//! fails this test. So does a painted string, or `feedback_draft`, that
//! differs from that implementation. With the planning box closed, the L1
//! prompt box still matches that same implementation. The letter a does
//! not approve.
//!
//! This is a crate test. `test_fixtures::make_agent` is `#[cfg(test)]` and
//! `pub(crate)`, and `AgentSession` has `pub(crate)` fields, so an
//! integration test under `tests/` cannot construct an `AgentView`.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::actions::ActionRegistry;
use crate::app::app_view::InputOutcome;
use crate::views::prompt_widget::PromptWidget;

use super::AgentView;

fn char_key(ch: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), mods)
}

fn left_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
}

fn feed_widget(widget: &mut PromptWidget, key: &KeyEvent) {
    let _ = widget.handle_key(key);
}

fn feed_agent(agent: &mut AgentView, key: &KeyEvent) -> InputOutcome {
    agent.handle_input(&Event::Key(*key), &ActionRegistry::defaults())
}

#[test]
fn planning_box_shows_l1_prompt_text_and_closed_l1_still_inserts() {
    let registry_keys = [
        char_key('a', KeyModifiers::NONE),
        char_key('b', KeyModifiers::NONE),
        left_key(),
        char_key('c', KeyModifiers::NONE),
    ];

    let mut reference = PromptWidget::new();
    for key in &registry_keys {
        feed_widget(&mut reference, key);
    }
    let expected = reference.text().to_string();
    assert_eq!(
        expected, "acb",
        "the L1 prompt implementation inserts c between a and b, once, in order"
    );
    assert_eq!(
        expected.matches('a').count(),
        1,
        "the letter a stays one letter in the L1 implementation"
    );

    let mut closed = super::test_fixtures::make_agent();
    assert!(
        closed.planning_box_shown_text().is_empty(),
        "a closed planning box paints no revision text before typing"
    );
    for key in &registry_keys {
        let outcome = feed_agent(&mut closed, key);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "closed L1 typing, including the letter a, must not approve; got {outcome:?}"
        );
    }
    assert_eq!(
        closed.prompt.text(),
        expected,
        "with the planning box closed, the L1 prompt box must still match the L1 prompt implementation"
    );
    assert!(
        closed.planning_box_shown_text().is_empty(),
        "typing while the planning box is closed must not paint a planning box"
    );

    let mut planning = super::test_fixtures::make_agent();
    planning.open_file_backed_revise_box();
    assert_eq!(
        planning.prompt.text(),
        "",
        "opening the planning box must not insert text into the L1 prompt"
    );
    for key in &registry_keys {
        let outcome = feed_agent(&mut planning, key);
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "the letter in the planning box must insert once and must not approve; got {outcome:?}"
        );
    }
    let prompt_text = planning.prompt.text().to_string();
    let shown = planning.planning_box_shown_text();
    let draft = planning.planning_feedback_draft_text();
    assert_eq!(
        prompt_text, expected,
        "planning keys must produce the L1 prompt implementation text, not a second insert"
    );
    assert_eq!(
        shown, prompt_text,
        "the painted planning box must show the L1 prompt text, not a different buffer"
    );
    assert_eq!(
        draft, prompt_text,
        "feedback_draft must hold the same text as the L1 prompt, not a different second buffer"
    );
    assert!(
        prompt_text.contains('a') && !shown.contains("Type a revision"),
        "the letter a is text in the planning box, and the placeholder is not part of that text; got {shown:?}"
    );
}
