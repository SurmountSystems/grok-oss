//! Session-scoped subagent spawn limits, enforced by the coordinator.

use super::types::SubagentRequest;
use crate::util::env::parse_positive_env;

pub const DEFAULT_MAX_CONCURRENT: usize = 32;

/// What happens to a spawn that arrives at the concurrent limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LimitBehavior {
    #[default]
    Queue,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubagentLimits {
    /// Subagents one session may run at once. Zero does not disable the
    /// limit: [`Admission::new`] clamps it to 1.
    pub max_concurrent: usize,
    pub behavior: LimitBehavior,
}

impl Default for SubagentLimits {
    fn default() -> Self {
        Self {
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            behavior: LimitBehavior::Queue,
        }
    }
}

impl SubagentLimits {
    /// Read once at the composition root; inject everywhere else.
    pub fn from_env() -> Self {
        Self::from_lookup(|var| std::env::var(var).ok())
    }

    /// A limit can be adjusted but never disabled.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let default = Self::default();
        let max_concurrent = parse_positive_env(
            "GROK_MAX_CONCURRENT_SUBAGENTS",
            lookup("GROK_MAX_CONCURRENT_SUBAGENTS"),
        )
        .unwrap_or(default.max_concurrent);
        let behavior = match lookup("GROK_SUBAGENT_LIMIT_BEHAVIOR") {
            None => LimitBehavior::Queue,
            Some(value) if value.eq_ignore_ascii_case("fail") => LimitBehavior::Fail,
            Some(value) if value.eq_ignore_ascii_case("queue") => LimitBehavior::Queue,
            Some(value) => {
                tracing::warn!(
                    %value,
                    "GROK_SUBAGENT_LIMIT_BEHAVIOR is neither `queue` nor `fail`; keeping `queue`"
                );
                LimitBehavior::Queue
            }
        };
        Self {
            max_concurrent,
            behavior,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AdmissionDecision {
    Start,
    Enqueue,
    Reject(AdmissionError),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AdmissionError {
    ConcurrentLimitReached { limit: usize },
}

/// Whether a Review-row description may join the live Subagent list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplementLoopReviewAdmit {
    Admit,
    Reject,
}

/// Token Economy implement-loop effort is thoroughness, not reviewer count.
/// One reviewer unless the operator explicitly asked for more.
pub fn review_row_count_for_implement_effort(
    effort: u8,
    operator_asked_for_more_reviewers: bool,
) -> u8 {
    if operator_asked_for_more_reviewers {
        effort.max(1)
    } else {
        1
    }
}

/// Review-row spawn text (not an implementer or explore job).
pub fn is_implement_loop_review_description(description: &str) -> bool {
    let d = description.trim();
    if d.is_empty() {
        return false;
    }
    let lower = d.to_ascii_lowercase();
    lower.starts_with("[reviewer]")
        || lower.starts_with("review ")
        || lower.starts_with("review:")
        || lower.contains("review implementation")
}

/// Admit or reject a candidate Review description for this implement-loop
/// effort. `operator_asked == false` caps live Review rows at one even when
/// the new text is distinct. Effort 2 with an operator ask may still be 2.
pub fn admit_implement_loop_review_description(
    effort: u8,
    operator_asked: bool,
    live_review_descriptions: impl IntoIterator<Item = impl AsRef<str>>,
    candidate: &str,
) -> ImplementLoopReviewAdmit {
    if !is_implement_loop_review_description(candidate) {
        return ImplementLoopReviewAdmit::Admit;
    }
    let allowed = usize::from(review_row_count_for_implement_effort(
        effort,
        operator_asked,
    ));
    let candidate_trim = candidate.trim();
    let mut distinct: Vec<String> = live_review_descriptions
        .into_iter()
        .map(|s| s.as_ref().trim().to_string())
        .filter(|s| is_implement_loop_review_description(s) && s.as_str() != candidate_trim)
        .collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() >= allowed {
        ImplementLoopReviewAdmit::Reject
    } else {
        ImplementLoopReviewAdmit::Admit
    }
}

impl AdmissionError {
    /// Model-facing failure text; reachable only when an operator opts
    /// into `GROK_SUBAGENT_LIMIT_BEHAVIOR=fail`.
    pub(super) fn message(&self) -> String {
        match self {
            Self::ConcurrentLimitReached { limit } => format!(
                "Concurrent subagent limit reached: {limit} subagents are already running for \
                 this session. Do not retry; spawning succeeds again when a running subagent \
                 finishes."
            ),
        }
    }
}

/// Admission policy; the coordinator owns the state it judges against.
pub(super) struct Admission {
    limits: SubagentLimits,
}

impl Admission {
    /// `max_concurrent: 0` is clamped to 1 (the env path already filters
    /// zero): a limit can be adjusted but never disabled, and the actor's
    /// "queued implies running" exit invariant needs at least one slot.
    pub(super) fn new(mut limits: SubagentLimits) -> Self {
        limits.max_concurrent = limits.max_concurrent.max(1);
        Self { limits }
    }

    /// `running` is the session's live non-workflow child count.
    pub(super) fn admit(&self, request: &SubagentRequest, running: usize) -> AdmissionDecision {
        if request.owner.is_workflow() {
            // Workflow agents follow the run's own pool.
            return AdmissionDecision::Start;
        }
        if self.has_capacity(running) {
            AdmissionDecision::Start
        } else {
            match self.limits.behavior {
                LimitBehavior::Queue => AdmissionDecision::Enqueue,
                LimitBehavior::Fail => {
                    AdmissionDecision::Reject(AdmissionError::ConcurrentLimitReached {
                        limit: self.limits.max_concurrent,
                    })
                }
            }
        }
    }

    pub(super) fn has_capacity(&self, running: usize) -> bool {
        running < self.limits.max_concurrent
    }

    /// The one configured value; the coordinator must not re-read its config.
    pub(super) fn max_concurrent(&self) -> usize {
        self.limits.max_concurrent
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
