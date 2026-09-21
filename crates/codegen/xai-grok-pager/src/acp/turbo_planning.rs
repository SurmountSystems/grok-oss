//! Turbo planning: live exclusive / Isolated Preview plan turns use xhigh effort.
//!
//! This diverges from SpaceXAI: upstream keeps the stored session effort during
//! `/plan`. Surmount `turbo_planning` (default on) raises *effective* effort to
//! xhigh for the live plan turn only. Stored `session.models.reasoning_effort`
//! is not mutated. Chrome shows the same `{model} ({effort})` token; there is
//! no TURBO badge, banner, or toast.

use xai_grok_shell::sampling::types::ReasoningEffort;

/// Effective sampling / chrome effort for this turn.
///
/// Returns `Some(ReasoningEffort::Xhigh)` when turbo planning is on and this
/// is a live exclusive `/plan` or Isolated Preview `/plan --soft` turn.
/// Otherwise returns `stored` unchanged.
pub fn effective_reasoning_effort(
    stored: Option<ReasoningEffort>,
    turbo_on: bool,
    live_plan_turn: bool,
) -> Option<ReasoningEffort> {
    if turbo_on && live_plan_turn {
        Some(ReasoningEffort::Xhigh)
    } else {
        stored
    }
}

/// Live plan turn for turbo planning.
///
/// `plan_mode_pending.unwrap_or(plan_mode_active) || isolated_preview_shows_secondary_plan`
///
/// Isolated Preview `/plan --soft` sets `isolated_preview_shows_secondary_plan`
/// and does **not** set `plan_mode_active`.
pub fn live_plan_turn(
    plan_mode_pending: Option<bool>,
    plan_mode_active: bool,
    isolated_preview_shows_secondary_plan: bool,
) -> bool {
    plan_mode_pending.unwrap_or(plan_mode_active) || isolated_preview_shows_secondary_plan
}

/// Lower-right chrome effort token. Magenta model id stays the model id.
pub fn effort_chrome_token(effort: Option<ReasoningEffort>) -> String {
    match effort {
        Some(e) => e.as_str().to_string(),
        None => String::new(),
    }
}

/// `{model} ({effort})` lower-right line. Must not contain TURBO.
pub fn model_effort_chrome_line(model: &str, effort: Option<ReasoningEffort>) -> String {
    match effort {
        Some(e) => format!("{model} ({})", e.as_str()),
        None => model.to_string(),
    }
}

/// Stamp this send's request effort without mutating stored session `/effort`.
///
/// Sets the shell live-plan-turn flag so `reconstruct_full_config` applies
/// the same effective value. Stored `session.models.reasoning_effort` stays.
pub fn stamp_request_effort(
    stored: Option<ReasoningEffort>,
    turbo_on: bool,
    live_plan_turn: bool,
) -> Option<ReasoningEffort> {
    xai_grok_shell::util::config::set_live_plan_turn(live_plan_turn);
    effective_reasoning_effort(stored, turbo_on, live_plan_turn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium() -> Option<ReasoningEffort> {
        Some(ReasoningEffort::Medium)
    }

    /// Diverges from SpaceXAI: upstream would keep medium through exclusive `/plan`.
    #[test]
    fn exclusive_plan_turn_uses_xhigh_when_session_is_medium_and_turbo_planning_is_on() {
        let stored = medium();
        let live = live_plan_turn(None, true, false);
        let effective = effective_reasoning_effort(stored, true, live);
        assert_eq!(effective, Some(ReasoningEffort::Xhigh));
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
        let chrome = model_effort_chrome_line("grok-4", effective);
        assert!(
            chrome.contains("xhigh"),
            "chrome must show xhigh, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
    }

    /// Diverges from SpaceXAI: Isolated Preview `/plan --soft` is a live plan turn.
    #[test]
    fn isolated_preview_plan_soft_live_turn_uses_xhigh_when_turbo_planning_is_on() {
        let stored = medium();
        let live = live_plan_turn(None, false, true);
        assert!(
            live,
            "/plan --soft sets isolated_preview_shows_secondary_plan without plan_mode_active"
        );
        let effective = effective_reasoning_effort(stored, true, live);
        assert_eq!(effective, Some(ReasoningEffort::Xhigh));
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
        let chrome = model_effort_chrome_line("grok-4", effective);
        assert!(
            chrome.contains("xhigh"),
            "chrome must show xhigh, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
    }

    /// After Exit/Approve the live plan turn is over; chrome and requests return to stored.
    #[test]
    fn exclusive_plan_exit_returns_chrome_and_requests_to_session_medium() {
        let stored = medium();
        let during = effective_reasoning_effort(stored, true, live_plan_turn(None, true, false));
        assert_eq!(during, Some(ReasoningEffort::Xhigh));
        let after = effective_reasoning_effort(stored, true, live_plan_turn(None, false, false));
        assert_eq!(after, medium());
        let chrome = model_effort_chrome_line("grok-4", after);
        assert!(
            chrome.contains("medium"),
            "after Exit/Approve chrome must show medium, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
    }

    /// Settings off = session effort (SpaceXAI-like / upstream-like).
    #[test]
    fn turbo_planning_off_keeps_plan_turns_at_session_medium() {
        let stored = medium();
        let exclusive =
            effective_reasoning_effort(stored, false, live_plan_turn(None, true, false));
        let isolated = effective_reasoning_effort(stored, false, live_plan_turn(None, false, true));
        assert_eq!(exclusive, medium());
        assert_eq!(isolated, medium());
        let chrome = model_effort_chrome_line("grok-4", exclusive);
        assert!(
            chrome.contains("medium"),
            "turbo off must keep session medium on the chrome line, got {chrome}"
        );
        assert!(
            !chrome.contains("xhigh"),
            "turbo off must not paint xhigh, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
    }

    #[test]
    fn pending_exclusive_plan_is_a_live_turn() {
        assert!(live_plan_turn(Some(true), false, false));
        assert!(!live_plan_turn(Some(false), true, false));
    }

    /// Operator: while exclusive `/plan` or Isolated Preview `/plan --soft`
    /// is the live plan turn, reasoning effort is xhigh even if the session
    /// is medium. Only the lower-right yellow model/effort line shows xhigh.
    /// Magenta model id stays the model id. No TURBO badge.
    ///
    /// Diverges from SpaceXAI: upstream would keep medium through `/plan`.
    #[test]
    fn session_medium_enter_plan_request_uses_xhigh_and_lower_right_shows_xhigh() {
        let stored = medium();
        let exclusive = live_plan_turn(None, true, false);
        let isolated = live_plan_turn(None, false, true);
        let exclusive_effort = stamp_request_effort(stored, true, exclusive);
        let isolated_effort = stamp_request_effort(stored, true, isolated);
        assert_eq!(exclusive_effort, Some(ReasoningEffort::Xhigh));
        assert_eq!(isolated_effort, Some(ReasoningEffort::Xhigh));
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
        let chrome = model_effort_chrome_line("grok-4", exclusive_effort);
        assert!(
            chrome.contains("xhigh"),
            "lower-right chrome must show xhigh, got {chrome}"
        );
        assert!(
            chrome.contains("grok-4"),
            "magenta model id stays the model id, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
    }

    /// Operator: after Exit / Approve, return to the Operator session setting.
    #[test]
    fn exit_or_approve_plan_returns_session_medium_effort() {
        let stored = medium();
        let during = stamp_request_effort(stored, true, live_plan_turn(None, true, false));
        assert_eq!(during, Some(ReasoningEffort::Xhigh));
        let after_exit = stamp_request_effort(stored, true, live_plan_turn(None, false, false));
        let after_approve =
            stamp_request_effort(stored, true, live_plan_turn(Some(false), true, false));
        assert_eq!(after_exit, medium());
        assert_eq!(after_approve, medium());
        let chrome = model_effort_chrome_line("grok-4", after_exit);
        assert!(
            chrome.contains("medium"),
            "after Exit/Approve chrome must show medium, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
    }

    /// Operator: settings off = upstream-like (plan at session effort).
    ///
    /// Diverges from SpaceXAI only when turbo is on. Off matches upstream.
    #[test]
    fn turbo_planning_settings_off_plan_turn_stays_session_medium() {
        let stored = medium();
        let exclusive = stamp_request_effort(stored, false, live_plan_turn(None, true, false));
        let isolated = stamp_request_effort(stored, false, live_plan_turn(None, false, true));
        assert_eq!(exclusive, medium());
        assert_eq!(isolated, medium());
        let chrome = model_effort_chrome_line("grok-4", exclusive);
        assert!(
            chrome.contains("medium"),
            "turbo off must keep session medium on the chrome line, got {chrome}"
        );
        assert!(
            !chrome.contains("xhigh"),
            "turbo off must not paint xhigh, got {chrome}"
        );
        assert!(
            !chrome.to_ascii_uppercase().contains("TURBO"),
            "chrome must not contain TURBO, got {chrome}"
        );
        assert_eq!(stored, medium(), "stored session effort must stay Medium");
    }
}
