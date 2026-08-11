//! Fearless global work pause: hold every in-process session's work, then
//! resume only truly incomplete units.
//!
//! Design notes:
//! - **Spacebar chord (default Ctrl+Shift+Space):** modifier chord so bare
//!   Space still focuses the prompt / types spaces. Open for remap via the
//!   action registry when key rebinding lands; chord is documented as the
//!   product default.
//! - **Global:** one process-level gate covers every agent session in this
//!   pager process (not only the focused agent).
//! - **Waiting vs finished:** incomplete work is [`WorkLifecycle::Waiting`];
//!   finished work stays finished and is never re-spawned on resume.
//! - **Resume once:** a mid-turn interrupt stashes at most one resume prompt
//!   per session; resume re-queues that unit once, then clears it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::agent::AgentId;

/// Lifecycle of one unit of agent work (a turn or a queued item).
///
/// The state machine never confuses finished with waiting: a finished unit
/// is terminal for this pause cycle; only waiting/interrupted work restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkLifecycle {
    /// Nothing incomplete for this unit.
    Idle,
    /// Actively running when pause engaged.
    Running,
    /// Truly incomplete: queued, or interrupted mid-turn and pending one resume.
    Waiting,
    /// Terminal for this unit: completed or cancelled without a resume stash.
    Finished,
}

impl WorkLifecycle {
    /// True only for incomplete work that may restart on resume.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Running | Self::Waiting)
    }

    /// Finished work must never be treated as pending.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished)
    }
}

/// Per-session snapshot taken when global pause engages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PausedSessionSnapshot {
    pub agent_id: AgentId,
    /// Wire session id when known (display / logs only).
    pub session_id: Option<String>,
    /// How many local drip-feed prompts were waiting (not finished).
    pub pending_queue_len: usize,
    /// True if a turn was running and we interrupted it for pause.
    pub interrupted_running_turn: bool,
    /// At most one prompt to re-drive once on resume (mid-turn interrupt).
    pub resume_prompt_once: Option<String>,
    /// True after resume has already re-queued `resume_prompt_once`.
    pub resume_consumed: bool,
    /// Lifecycle of the interrupted/queued work for this session.
    pub lifecycle: WorkLifecycle,
}

impl PausedSessionSnapshot {
    /// Build a snapshot from live session facts at pause time.
    pub fn capture(
        agent_id: AgentId,
        session_id: Option<String>,
        turn_running: bool,
        pending_queue_len: usize,
        in_flight_prompt_text: Option<String>,
    ) -> Self {
        let interrupted_running_turn = turn_running;
        let resume_prompt_once = if turn_running {
            // Prefer the stashed in-flight text; empty string still means
            // "something was running" only if we had text to re-drive.
            in_flight_prompt_text.filter(|t| !t.is_empty())
        } else {
            None
        };
        let lifecycle = if turn_running || pending_queue_len > 0 {
            if turn_running {
                WorkLifecycle::Running
            } else {
                WorkLifecycle::Waiting
            }
        } else {
            // Idle session: record as finished for this pause cycle so resume
            // does not invent work.
            WorkLifecycle::Finished
        };
        Self {
            agent_id,
            session_id,
            pending_queue_len,
            interrupted_running_turn,
            resume_prompt_once,
            resume_consumed: false,
            lifecycle,
        }
    }

    /// Whether resume should re-queue a mid-turn prompt once.
    pub fn needs_resume_requeue(&self) -> bool {
        !self.resume_consumed && self.resume_prompt_once.is_some()
    }

    /// Mark the one-shot resume as consumed. Idempotent.
    pub fn mark_resume_consumed(&mut self) {
        self.resume_consumed = true;
        if self.lifecycle == WorkLifecycle::Running {
            // The interrupted turn is no longer "running" in the snapshot
            // sense: remaining queue items stay Waiting; with no queue left
            // the unit is terminal for this pause cycle (Finished), so a
            // second resume path cannot re-spawn finished work.
            self.lifecycle = if self.pending_queue_len > 0 {
                WorkLifecycle::Waiting
            } else {
                WorkLifecycle::Finished
            };
        }
    }

    /// True when this session had incomplete work at pause time.
    pub fn had_incomplete_work(&self) -> bool {
        self.interrupted_running_turn
            || self.pending_queue_len > 0
            || self.resume_prompt_once.is_some()
    }
}

/// Process-wide pause gate for all in-process agent sessions.
#[derive(Debug, Clone, Default)]
pub struct GlobalWorkPause {
    active: bool,
    paused_at: Option<Instant>,
    /// Sessions that had incomplete work when pause engaged.
    sessions: HashMap<AgentId, PausedSessionSnapshot>,
    /// How many sessions had incomplete work at pause engage (stable for UI).
    sessions_held_count: usize,
}

impl GlobalWorkPause {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Wall time spent paused so far, or `None` when not paused.
    pub fn paused_duration(&self, now: Instant) -> Option<Duration> {
        self.paused_at.map(|t| now.saturating_duration_since(t))
    }

    /// Sessions held with incomplete work at the last engage (UI count).
    pub fn sessions_held_count(&self) -> usize {
        self.sessions_held_count
    }

    pub fn snapshots(&self) -> &HashMap<AgentId, PausedSessionSnapshot> {
        &self.sessions
    }

    pub fn snapshot_mut(&mut self, id: AgentId) -> Option<&mut PausedSessionSnapshot> {
        self.sessions.get_mut(&id)
    }

    /// Engage pause with the given session snapshots. Replaces any prior hold.
    ///
    /// Only sessions with incomplete work count toward `sessions_held_count`.
    pub fn engage(&mut self, now: Instant, snapshots: Vec<PausedSessionSnapshot>) {
        let mut map = HashMap::new();
        let mut held = 0usize;
        for snap in snapshots {
            if snap.had_incomplete_work() {
                held += 1;
            }
            map.insert(snap.agent_id, snap);
        }
        self.active = true;
        self.paused_at = Some(now);
        self.sessions = map;
        self.sessions_held_count = held;
    }

    /// Clear the pause gate. Returns the snapshots so callers can re-queue
    /// resume-once prompts. Does **not** invent new pending work.
    pub fn disengage(&mut self) -> Vec<PausedSessionSnapshot> {
        self.active = false;
        self.paused_at = None;
        self.sessions_held_count = 0;
        std::mem::take(&mut self.sessions).into_values().collect()
    }

    /// Toggle: engage when inactive, disengage when active.
    ///
    /// Returns `true` when the new state is paused.
    pub fn toggle(
        &mut self,
        now: Instant,
        engage_snapshots: impl FnOnce() -> Vec<PausedSessionSnapshot>,
    ) -> bool {
        if self.active {
            let _ = self.disengage();
            false
        } else {
            self.engage(now, engage_snapshots());
            true
        }
    }

    /// Short status line for chrome/toast while paused.
    pub fn status_label(&self, now: Instant) -> Option<String> {
        if !self.active {
            return None;
        }
        let secs = self.paused_duration(now).map(|d| d.as_secs()).unwrap_or(0);
        let n = self.sessions_held_count;
        let session_word = if n == 1 { "session" } else { "sessions" };
        Some(format!(
            "Paused {secs}s · {n} {session_word} held (Ctrl+Shift+Space to resume)"
        ))
    }

    /// Toast when pause engages.
    pub fn engage_toast(&self) -> String {
        let n = self.sessions_held_count;
        let session_word = if n == 1 { "session" } else { "sessions" };
        if n == 0 {
            "Paused all work (nothing was running)".to_string()
        } else {
            format!("Paused all work · {n} {session_word} held")
        }
    }

    /// Toast when pause clears.
    pub fn disengage_toast(resumed_count: usize, had_pending: bool) -> String {
        if resumed_count > 0 {
            let unit = if resumed_count == 1 {
                "interrupted turn"
            } else {
                "interrupted turns"
            };
            format!("Resumed · continuing {resumed_count} {unit}")
        } else if had_pending {
            "Resumed · queued work will continue".to_string()
        } else {
            "Resumed · nothing pending".to_string()
        }
    }
}

/// Classify a live session for pause bookkeeping without inventing work.
pub fn classify_session_work(
    turn_running: bool,
    pending_queue_len: usize,
    already_finished: bool,
) -> WorkLifecycle {
    if already_finished {
        return WorkLifecycle::Finished;
    }
    if turn_running {
        WorkLifecycle::Running
    } else if pending_queue_len > 0 {
        WorkLifecycle::Waiting
    } else {
        WorkLifecycle::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: usize) -> AgentId {
        AgentId(n)
    }

    #[test]
    fn lifecycle_never_confuses_finished_with_waiting() {
        assert!(!WorkLifecycle::Finished.is_pending());
        assert!(WorkLifecycle::Finished.is_finished());
        assert!(WorkLifecycle::Waiting.is_pending());
        assert!(!WorkLifecycle::Waiting.is_finished());
        assert!(WorkLifecycle::Running.is_pending());
        assert!(!WorkLifecycle::Idle.is_pending());
        assert!(!WorkLifecycle::Idle.is_finished());
    }

    #[test]
    fn classify_idle_is_not_waiting() {
        assert_eq!(classify_session_work(false, 0, false), WorkLifecycle::Idle);
        assert_eq!(
            classify_session_work(false, 0, true),
            WorkLifecycle::Finished
        );
        assert_eq!(
            classify_session_work(true, 0, false),
            WorkLifecycle::Running
        );
        assert_eq!(
            classify_session_work(false, 2, false),
            WorkLifecycle::Waiting
        );
    }

    #[test]
    fn capture_running_stashes_resume_once() {
        let snap = PausedSessionSnapshot::capture(
            id(0),
            Some("s1".into()),
            true,
            0,
            Some("keep going".into()),
        );
        assert!(snap.interrupted_running_turn);
        assert_eq!(snap.resume_prompt_once.as_deref(), Some("keep going"));
        assert!(snap.needs_resume_requeue());
        assert_eq!(snap.lifecycle, WorkLifecycle::Running);
        assert!(snap.had_incomplete_work());
    }

    #[test]
    fn capture_running_without_text_still_marks_interrupted() {
        let snap = PausedSessionSnapshot::capture(id(0), None, true, 0, None);
        assert!(snap.interrupted_running_turn);
        assert!(snap.resume_prompt_once.is_none());
        assert!(!snap.needs_resume_requeue());
        assert!(snap.had_incomplete_work());
    }

    #[test]
    fn capture_queued_only_is_waiting_not_resume_once() {
        let snap = PausedSessionSnapshot::capture(id(1), Some("s2".into()), false, 3, None);
        assert!(!snap.interrupted_running_turn);
        assert!(snap.resume_prompt_once.is_none());
        assert_eq!(snap.pending_queue_len, 3);
        assert_eq!(snap.lifecycle, WorkLifecycle::Waiting);
        assert!(snap.had_incomplete_work());
    }

    #[test]
    fn capture_idle_finished_not_incomplete() {
        let snap = PausedSessionSnapshot::capture(id(2), Some("s3".into()), false, 0, None);
        assert!(!snap.had_incomplete_work());
        assert_eq!(snap.lifecycle, WorkLifecycle::Finished);
    }

    #[test]
    fn engage_counts_only_incomplete_sessions() {
        let mut g = GlobalWorkPause::new();
        let now = Instant::now();
        g.engage(
            now,
            vec![
                PausedSessionSnapshot::capture(id(0), None, true, 0, Some("a".into())),
                PausedSessionSnapshot::capture(id(1), None, false, 2, None),
                PausedSessionSnapshot::capture(id(2), None, false, 0, None), // idle
            ],
        );
        assert!(g.is_active());
        assert_eq!(g.sessions_held_count(), 2);
        assert_eq!(g.snapshots().len(), 3);
        assert!(g.paused_duration(now + Duration::from_secs(5)).unwrap() >= Duration::from_secs(5));
        let label = g.status_label(now + Duration::from_secs(5)).unwrap();
        assert!(label.contains("Paused 5s"), "{label}");
        assert!(label.contains("2 sessions"), "{label}");
    }

    #[test]
    fn resume_with_nothing_pending_does_nothing() {
        let mut g = GlobalWorkPause::new();
        g.engage(
            Instant::now(),
            vec![PausedSessionSnapshot::capture(id(0), None, false, 0, None)],
        );
        assert_eq!(g.sessions_held_count(), 0);
        let snaps = g.disengage();
        assert!(!g.is_active());
        let mut resumed = 0usize;
        let mut had_pending = false;
        for s in snaps {
            if s.needs_resume_requeue() {
                resumed += 1;
            }
            if s.had_incomplete_work() {
                had_pending = true;
            }
        }
        assert_eq!(resumed, 0);
        assert!(!had_pending);
        assert_eq!(
            GlobalWorkPause::disengage_toast(resumed, had_pending),
            "Resumed · nothing pending"
        );
    }

    #[test]
    fn mid_turn_resume_continues_once() {
        let mut g = GlobalWorkPause::new();
        g.engage(
            Instant::now(),
            vec![PausedSessionSnapshot::capture(
                id(0),
                Some("sess".into()),
                true,
                1,
                Some("continue me".into()),
            )],
        );
        let mut snaps = g.disengage();
        assert_eq!(snaps.len(), 1);
        let s = &mut snaps[0];
        assert!(s.needs_resume_requeue());
        let text = s.resume_prompt_once.clone().unwrap();
        assert_eq!(text, "continue me");
        s.mark_resume_consumed();
        assert!(!s.needs_resume_requeue(), "second resume must not re-queue");
        // Queue still had items at pause time → Waiting after consume.
        assert_eq!(s.lifecycle, WorkLifecycle::Waiting);
        assert_eq!(
            GlobalWorkPause::disengage_toast(1, true),
            "Resumed · continuing 1 interrupted turn"
        );
    }

    #[test]
    fn mark_resume_consumed_finished_when_queue_empty() {
        let mut snap = PausedSessionSnapshot::capture(
            id(0),
            Some("solo".into()),
            true,
            0,
            Some("only this".into()),
        );
        assert_eq!(snap.lifecycle, WorkLifecycle::Running);
        snap.mark_resume_consumed();
        assert!(!snap.needs_resume_requeue());
        assert_eq!(snap.lifecycle, WorkLifecycle::Finished);
        assert!(snap.lifecycle.is_finished());
        assert!(!snap.lifecycle.is_pending());
    }

    #[test]
    fn finished_session_not_re_spawned_on_resume() {
        let mut g = GlobalWorkPause::new();
        g.engage(
            Instant::now(),
            vec![
                // Finished / idle at pause: no resume prompt, no queue.
                PausedSessionSnapshot::capture(id(0), Some("done".into()), false, 0, None),
                // Waiting queue only.
                PausedSessionSnapshot::capture(id(1), Some("wait".into()), false, 1, None),
            ],
        );
        let snaps = g.disengage();
        for s in &snaps {
            if s.agent_id == id(0) {
                assert!(!s.needs_resume_requeue());
                assert!(!s.had_incomplete_work());
                assert!(s.lifecycle.is_finished());
            }
            if s.agent_id == id(1) {
                assert!(!s.needs_resume_requeue());
                assert!(s.had_incomplete_work());
                assert_eq!(s.lifecycle, WorkLifecycle::Waiting);
            }
        }
    }

    #[test]
    fn toggle_pause_resume_cycle() {
        let mut g = GlobalWorkPause::new();
        let now = Instant::now();
        assert!(g.toggle(now, || {
            vec![PausedSessionSnapshot::capture(
                id(0),
                None,
                true,
                0,
                Some("x".into()),
            )]
        }));
        assert!(g.is_active());
        assert_eq!(g.sessions_held_count(), 1);
        assert!(!g.toggle(now, || panic!("must not engage while active")));
        assert!(!g.is_active());
        assert_eq!(g.sessions_held_count(), 0);
    }

    #[test]
    fn empty_in_flight_text_not_stashed() {
        let snap = PausedSessionSnapshot::capture(id(0), None, true, 0, Some(String::new()));
        assert!(snap.resume_prompt_once.is_none());
        assert!(snap.interrupted_running_turn);
    }
}
