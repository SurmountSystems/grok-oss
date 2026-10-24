//! Mention is not invoke. A sentence that contains `/plan` must send as
//! ordinary Operator text. Lone `/plan` still enters plan mode.

use super::*;
use pretty_assertions::assert_eq;

/// Operator 2026-10-21: sentences where you type things like this: /plan
/// like if you type a skill in the prompt.
const MENTION: &str =
    "sentences where you type things like this: /plan like if you type a skill in the prompt";

fn toast_text(app: &AppView, id: AgentId) -> Option<&str> {
    test_agent(app, id)
        .toast
        .as_ref()
        .map(|(message, _)| message.as_str())
}

/// Operator 2026-10-21: mentioning `/plan` in a prompt must not toast and
/// block submit. Submit anyway sends the raw text, not plan mode, not the skill.
#[test]
fn mid_text_plan_mention_sends_as_ordinary_text_not_plan_mode() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    test_agent_mut(&mut app, id).prompt.set_text(MENTION);

    let effects = dispatch(Action::SendPrompt(MENTION.to_owned()), &mut app);

    assert!(
        toast_text(&app, id).is_none(),
        "a /plan mention must not toast and trap submit, got {:?}",
        toast_text(&app, id)
    );
    assert!(
        test_agent(&app, id).plan_mode_pending.is_none(),
        "mention is not invoke: /plan in a sentence must not enter plan mode"
    );
    assert!(
        !test_agent(&app, id).plan_mode_active,
        "mention is not invoke: /plan in a sentence must not enter plan mode"
    );
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::SendPrompt { text, .. }] if text == MENTION
        ),
        "submit anyway must send the raw prompt, got {effects:?}"
    );
}

/// Leading `/plan` with no surrounding sentence still invokes plan mode.
#[test]
fn leading_plan_command_still_invokes() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    let effects = dispatch(Action::SendPrompt("/plan".into()), &mut app);

    assert!(
        matches!(
            effects.first(),
            Some(Effect::SetSessionMode { mode_id, .. }) if &*mode_id.0 == "plan"
        ),
        "lone /plan must still invoke, got {effects:?}"
    );
    assert_eq!(test_agent(&app, id).plan_mode_pending, Some(true));
}
