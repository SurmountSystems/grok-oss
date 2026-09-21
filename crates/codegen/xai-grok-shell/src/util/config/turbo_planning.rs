//! Turbo planning: live exclusive / Isolated Preview plan turns use xhigh.
//!
//! This diverges from SpaceXAI: upstream keeps the stored session effort
//! during `/plan`. Surmount `[ui].turbo_planning` (default on) raises
//! *effective* effort to xhigh for the live plan turn only. Stored
//! `handle.reasoning_effort` is not mutated. Off is the upstream-like path.
//!
//! Duplicate of the pager chrome helper: this crate cannot import pager
//! `acp/turbo_planning.rs`. Keep the apply rules in sync.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use xai_grok_sampling_types::ReasoningEffort;

use crate::config::load_effective_config;

/// Default when `[ui].turbo_planning` is unset.
pub const TURBO_PLANNING_DEFAULT: bool = true;

thread_local! {
    static TURBO_PLANNING_LIVE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Cross-thread: pager send and the agent worker do not share a thread-local.
static LIVE_PLAN_TURN: AtomicBool = AtomicBool::new(false);

/// Immediate override after a settings toggle (disk persist is async).
pub fn set_turbo_planning_live(enabled: bool) {
    TURBO_PLANNING_LIVE.with(|c| c.set(Some(enabled)));
}

/// Live cache, else disk, else default on.
pub fn turbo_planning_enabled() -> bool {
    if let Some(live) = TURBO_PLANNING_LIVE.with(|c| c.get()) {
        return live;
    }
    turbo_planning_from_disk()
}

/// Read `[ui].turbo_planning` from disk-merged config. Default on when unset.
pub fn turbo_planning_from_disk() -> bool {
    let root = match load_effective_config() {
        Ok(v) => v,
        Err(_) => return TURBO_PLANNING_DEFAULT,
    };
    match root
        .get("ui")
        .and_then(|u| u.get("turbo_planning"))
        .and_then(|v| v.as_bool())
    {
        Some(b) => b,
        None => TURBO_PLANNING_DEFAULT,
    }
}

/// Effective sampling effort for this turn. Does not mutate `stored`.
pub fn effective_reasoning_effort_for_turn(
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

/// Pager send stamps this before the agent worker samples.
pub fn set_live_plan_turn(live: bool) {
    LIVE_PLAN_TURN.store(live, Ordering::SeqCst);
}

/// Live exclusive `/plan` or Isolated Preview `/plan --soft` for this send.
pub fn live_plan_turn_flag() -> bool {
    LIVE_PLAN_TURN.load(Ordering::SeqCst)
}

/// Apply turbo planning to stored effort. Does not mutate `stored`.
pub fn apply_live_plan_turn_effort(stored: Option<ReasoningEffort>) -> Option<ReasoningEffort> {
    effective_reasoning_effort_for_turn(stored, turbo_planning_enabled(), live_plan_turn_flag())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diverges from SpaceXAI: upstream would keep medium through exclusive `/plan`.
    #[test]
    fn exclusive_plan_turn_uses_xhigh_when_session_is_medium_and_turbo_planning_is_on() {
        let stored = Some(ReasoningEffort::Medium);
        let effective = effective_reasoning_effort_for_turn(stored, true, true);
        assert_eq!(effective, Some(ReasoningEffort::Xhigh));
        assert_eq!(stored, Some(ReasoningEffort::Medium));
    }

    #[test]
    fn turbo_planning_off_keeps_plan_turns_at_session_medium() {
        let stored = Some(ReasoningEffort::Medium);
        assert_eq!(
            effective_reasoning_effort_for_turn(stored, false, true),
            stored
        );
    }

    #[test]
    fn exit_plan_returns_to_stored_medium() {
        let stored = Some(ReasoningEffort::Medium);
        assert_eq!(
            effective_reasoning_effort_for_turn(stored, true, false),
            stored
        );
    }

    #[test]
    fn set_live_plan_turn_drives_apply_without_mutating_stored() {
        let stored = Some(ReasoningEffort::Medium);
        set_turbo_planning_live(true);
        set_live_plan_turn(true);
        assert_eq!(
            apply_live_plan_turn_effort(stored),
            Some(ReasoningEffort::Xhigh)
        );
        assert_eq!(stored, Some(ReasoningEffort::Medium));
        set_live_plan_turn(false);
        assert_eq!(apply_live_plan_turn_effort(stored), stored);
        set_turbo_planning_live(false);
        set_live_plan_turn(true);
        assert_eq!(apply_live_plan_turn_effort(stored), stored);
        set_live_plan_turn(false);
    }
}
