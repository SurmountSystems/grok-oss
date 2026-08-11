//! Soft stop: finish the current top-level turn, then hold the queue.
//!
//! Distinct from fearless global pause (`global_work_pause`):
//! - **Soft stop** does **not** cancel mid-flight. It arms a drain gate that
//!   takes effect after the current L1 main-thread turn finishes (success or
//!   terminal fail). Queued follow-ups stay queued until the operator clears
//!   soft stop.
//! - **Global pause** freezes everything immediately (cancels running turns).
//!
//! Default chord: `Ctrl+Shift+S` (does not steal `Ctrl+Shift+Space` pause).

/// Soft-stop phase for this pager process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoftStopPhase {
    /// Not armed; queue drains normally.
    #[default]
    Off,
    /// Armed: when the current top-level turn finishes, hold the queue.
    Armed,
    /// Taken effect: current turn finished; further automatic queue drain is blocked.
    Holding,
}

/// Process-level soft-stop gate (one arming state for this pager process).
#[derive(Debug, Clone, Default)]
pub struct SoftStop {
    phase: SoftStopPhase,
}

impl SoftStop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phase(&self) -> SoftStopPhase {
        self.phase
    }

    pub fn is_off(&self) -> bool {
        self.phase == SoftStopPhase::Off
    }

    pub fn is_armed(&self) -> bool {
        self.phase == SoftStopPhase::Armed
    }

    pub fn is_holding(&self) -> bool {
        self.phase == SoftStopPhase::Holding
    }

    /// True when automatic queue drain must not start the next item.
    pub fn blocks_drain(&self) -> bool {
        self.phase == SoftStopPhase::Holding
    }

    /// Toggle arming.
    ///
    /// - Off → Armed
    /// - Armed → Off (cancel arming before the turn ends)
    /// - Holding → Off (release the queue hold)
    ///
    /// Returns the new phase and a toast string.
    pub fn toggle(&mut self) -> (SoftStopPhase, String) {
        match self.phase {
            SoftStopPhase::Off => {
                self.phase = SoftStopPhase::Armed;
                (self.phase, Self::armed_toast())
            }
            SoftStopPhase::Armed => {
                self.phase = SoftStopPhase::Off;
                (self.phase, Self::disarm_toast())
            }
            SoftStopPhase::Holding => {
                self.phase = SoftStopPhase::Off;
                (self.phase, Self::release_toast())
            }
        }
    }

    /// Call when a top-level L1 turn reaches a terminal outcome (success or fail).
    ///
    /// If armed, transitions to Holding and returns the take-effect toast.
    /// If already Holding or Off, returns `None` (no second toast).
    pub fn on_top_level_turn_finished(&mut self) -> Option<String> {
        if self.phase == SoftStopPhase::Armed {
            self.phase = SoftStopPhase::Holding;
            Some(Self::taken_effect_toast())
        } else {
            None
        }
    }

    /// Status chrome line while armed or holding.
    pub fn status_label(&self) -> Option<&'static str> {
        match self.phase {
            SoftStopPhase::Off => None,
            SoftStopPhase::Armed => Some("Soft stop armed (Ctrl+Shift+S)"),
            SoftStopPhase::Holding => Some("Soft stop: queue held (Ctrl+Shift+S to resume)"),
        }
    }

    pub fn armed_toast() -> String {
        "Soft stop armed: will hold the queue after this turn finishes".to_string()
    }

    pub fn disarm_toast() -> String {
        "Soft stop disarmed".to_string()
    }

    pub fn release_toast() -> String {
        "Soft stop cleared: queue may continue".to_string()
    }

    pub fn taken_effect_toast() -> String {
        "Soft stop: finished current turn; queue held.".to_string()
    }
}

/// Whether soft stop should block starting the next queued item after a turn ends.
///
/// Pure helper for tests and call sites that already know the phase.
pub fn should_block_queue_drain(phase: SoftStopPhase) -> bool {
    phase == SoftStopPhase::Holding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_off_and_does_not_block() {
        let s = SoftStop::new();
        assert_eq!(s.phase(), SoftStopPhase::Off);
        assert!(!s.blocks_drain());
        assert!(!should_block_queue_drain(SoftStopPhase::Off));
        assert!(s.status_label().is_none());
    }

    #[test]
    fn arm_then_turn_finish_holds_queue() {
        let mut s = SoftStop::new();
        let (phase, toast) = s.toggle();
        assert_eq!(phase, SoftStopPhase::Armed);
        assert!(toast.contains("armed"));
        assert!(!s.blocks_drain(), "armed must not cancel mid-turn drain");
        assert!(s.status_label().unwrap().contains("armed"));

        let take = s.on_top_level_turn_finished().expect("take effect");
        assert_eq!(take, SoftStop::taken_effect_toast());
        assert!(s.is_holding());
        assert!(s.blocks_drain());
        assert!(should_block_queue_drain(SoftStopPhase::Holding));
        assert!(s.status_label().unwrap().contains("held"));
    }

    #[test]
    fn unarmed_turn_finish_does_not_hold() {
        let mut s = SoftStop::new();
        assert!(s.on_top_level_turn_finished().is_none());
        assert!(!s.blocks_drain());
        assert_eq!(s.phase(), SoftStopPhase::Off);
    }

    #[test]
    fn disarm_before_turn_ends_cancels_arm() {
        let mut s = SoftStop::new();
        s.toggle();
        assert!(s.is_armed());
        let (phase, toast) = s.toggle();
        assert_eq!(phase, SoftStopPhase::Off);
        assert!(toast.contains("disarmed"));
        assert!(s.on_top_level_turn_finished().is_none());
        assert!(!s.blocks_drain());
    }

    #[test]
    fn release_holding_allows_drain() {
        let mut s = SoftStop::new();
        s.toggle();
        let _ = s.on_top_level_turn_finished();
        assert!(s.blocks_drain());
        let (phase, toast) = s.toggle();
        assert_eq!(phase, SoftStopPhase::Off);
        assert!(toast.contains("cleared") || toast.contains("continue"));
        assert!(!s.blocks_drain());
    }

    #[test]
    fn second_turn_finish_while_holding_no_extra_toast() {
        let mut s = SoftStop::new();
        s.toggle();
        assert!(s.on_top_level_turn_finished().is_some());
        assert!(s.on_top_level_turn_finished().is_none());
        assert!(s.is_holding());
    }

    #[test]
    fn distinct_from_mid_turn_cancel_semantics() {
        // Soft stop while armed never claims to cancel; only blocks after finish.
        let mut s = SoftStop::new();
        s.toggle();
        assert!(!s.blocks_drain());
        // Simulate "still mid-turn": drain allowed until finish.
        assert!(!should_block_queue_drain(s.phase()));
        let _ = s.on_top_level_turn_finished();
        assert!(should_block_queue_drain(s.phase()));
    }
}
