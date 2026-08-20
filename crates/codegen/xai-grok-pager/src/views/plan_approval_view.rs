use agent_client_protocol as acp;
use xai_acp_lib::AcpResult;

pub use xai_grok_tools::implementations::grok_build::exit_plan_mode::{
    ExitPlanModeExtRequest, ExitPlanModeExtResponse,
};

use crate::views::prompt_widget::StashedPrompt;

/// Placeholder body for the plan-approval preview when `exit_plan_mode` parks
/// with no plan content (missing/empty `plan.md`, or a whitespace-only body).
///
/// Must be non-empty after trim so `LineViewerState::open_markdown_content`
/// accepts it — empty bodies are rejected there.
pub const EMPTY_PLAN_PLACEHOLDER: &str = "\
# No plan written yet

The agent exited plan mode without writing a plan.

- **Approve** - leave plan mode and start implementing
- **Comment** - type a note, then Approve, Clarify, or Revise
- **Clarify** - after Comment, ask a question; do not rewrite the plan
- **Revise** - focus the box and wait for notes, then send the agent back to rewrite the plan
- **Exit** - abandon and turn plan mode off
";

/// Status-line label while plan mode is active without a live reverse-request
/// (idle / freeform dead end). Never return this while Revise/Clarify rewrite
/// is in flight (see [`PLAN_REVISING_STATUS`] / [`PLAN_WAITING_UPDATED_STATUS`]).
/// Never return this while the side panel is shut (see [`PLAN_READY_STATUS`]).
pub const PLAN_IDLE_REVIEW_STATUS: &str = "Plan written. Click or /view-plan";

/// Status while a plan is parked and the side panel is shut. Not exclusive
/// keyboard capture and not the click-or-/view-plan ceremony.
pub const PLAN_READY_STATUS: &str = "Plan ready";

/// Toast when freeform Enter cannot attach to a live plan-feedback channel
/// (Revise/Clarify already unparked) and the message will queue as a normal
/// follow-up instead. Never pretend the second note was live Revise/Clarify.
pub const PLAN_FEEDBACK_QUEUE_TOAST: &str =
    "No live plan feedback channel. Message will queue as a normal follow-up.";

/// Human scrollback line when decisive Revise unparks with no freeform notes.
/// Keeps the transcript from looking barren while the agent rewrites.
pub const PLAN_REVISE_HUMAN_LINE: &str = "Revise the plan";

/// Synthetic tool_call_id for local idle decision park (no shell reverse-request).
pub const IDLE_PLAN_DECISION_TOOL_CALL_ID: &str = "local-idle-plan-decision";

/// Model-facing text after a real plan-panel Approve with no live waiter.
/// Mid-turn Approve uses the same sentence in the shell tool result.
pub const PLAN_APPROVED_IMPLEMENT_MESSAGE: &str =
    "The user approved the plan. Implement the plan in plan.md.";

/// Status while Revise unparked and the agent is rewriting `plan.md`
/// (waiting for a new `exit_plan_mode` present). Not idle click ceremony.
pub const PLAN_REVISING_STATUS: &str = "Revising plan...";

/// Status while Clarify unparked and the agent is answering without a new
/// present yet. Same no-idle-chrome contract as revise-in-flight.
pub const PLAN_WAITING_UPDATED_STATUS: &str = "Waiting for updated plan...";

/// What decisive plan feedback is waiting on before decision chrome re-arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFeedbackInFlight {
    /// Revise (ACP cancelled / Interject rewrite).
    Revising,
    /// Clarify (ACP questions / Interject answer-only).
    Clarifying,
}

impl PlanFeedbackInFlight {
    /// Status-line label while this feedback is in flight (no live park).
    pub fn status_label(self) -> &'static str {
        match self {
            Self::Revising => PLAN_REVISING_STATUS,
            Self::Clarifying => PLAN_WAITING_UPDATED_STATUS,
        }
    }
}

/// Status-line label while plan approval is parked.
///
/// A new `exit_plan_mode` present is review park, not operator Approve.
/// Empty plans still name a review path so the status line never looks stuck.
pub fn plan_approval_status_label(has_plan: bool) -> &'static str {
    if has_plan {
        "Plan ready. Side panel open"
    } else {
        "No plan written. Side panel open"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanApprovalFocus {
    Preview,
    Prompt,
    Commenting,
}

/// What freeform Enter on the plan-approval prompt means.
///
/// - **Comment**: comment composer hub. Enter does not decide. Click
///   Approve, Clarify, or Revise to attach the typed comment.
/// - **Revise**: ACP `"cancelled"` after typed notes (or comments) on Enter.
/// - **Questions** (`?` clarify): ACP `"questions"`; answer read-only; do not rewrite.
/// - **ApproveNotes**: ACP `"approved"` + notes via the Approve button or Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanPromptIntent {
    #[default]
    Revise,
    Questions,
    ApproveNotes,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanReviewSource {
    Inline,
    FileBacked,
}

#[derive(Debug, Clone)]
pub struct PlanComment {
    pub id: u64,
    pub line_range: std::ops::Range<usize>,
    pub text: String,
}

pub struct PlanApprovalViewState {
    pub tool_call_id: String,
    pub has_plan: bool,
    pub plan_content: Option<String>,
    pub source: PlanReviewSource,
    pub stashed_prompt: StashedPrompt,
    pub response_tx: Option<tokio::sync::oneshot::Sender<AcpResult<acp::ExtResponse>>>,

    pub focus: PlanApprovalFocus,
    /// Semantic for Prompt Enter with non-empty text (or comments).
    pub prompt_intent: PlanPromptIntent,
    pub comments: Vec<PlanComment>,
    pub next_comment_id: u64,
    pub editing_comment_id: Option<u64>,
    pub commenting_range: Option<std::ops::Range<usize>>,

    pub stashed_feedback_prompt: Option<StashedPrompt>,
    /// Local idle decision park: no live `exit_plan_mode` reverse-request.
    /// Approve / Revise / Quit still work; Revise Interjects a rewrite.
    pub is_local_idle_decision: bool,
}

impl PlanApprovalViewState {
    pub fn new(
        request: ExitPlanModeExtRequest,
        stashed_prompt: StashedPrompt,
        response_tx: tokio::sync::oneshot::Sender<AcpResult<acp::ExtResponse>>,
    ) -> Self {
        Self::with_source(
            request,
            PlanReviewSource::Inline,
            stashed_prompt,
            response_tx,
        )
    }

    pub fn with_source(
        request: ExitPlanModeExtRequest,
        source: PlanReviewSource,
        stashed_prompt: StashedPrompt,
        response_tx: tokio::sync::oneshot::Sender<AcpResult<acp::ExtResponse>>,
    ) -> Self {
        let plan_content = request.plan_content.filter(|s| !s.trim().is_empty());
        let has_plan = plan_content.is_some();
        Self {
            tool_call_id: request.tool_call_id,
            has_plan,
            plan_content,
            source,
            stashed_prompt,
            response_tx: Some(response_tx),
            focus: PlanApprovalFocus::Preview,
            prompt_intent: PlanPromptIntent::Revise,
            comments: Vec::new(),
            next_comment_id: 0,
            editing_comment_id: None,
            commenting_range: None,
            stashed_feedback_prompt: None,
            is_local_idle_decision: false,
        }
    }

    /// Local decision park when plan mode is idle with a plan body but no
    /// live `exit_plan_mode` reverse-request. Same side-panel CTAs; decisions
    /// leave plan mode / Interject rather than ACP outcomes. Starts on
    /// Preview so first paint is idle Comment, not comment-flow Clarify.
    pub fn for_idle_decision(plan_content: Option<String>) -> Self {
        let plan_content = plan_content.filter(|s| !s.trim().is_empty());
        let has_plan = plan_content.is_some();
        Self {
            tool_call_id: IDLE_PLAN_DECISION_TOOL_CALL_ID.to_owned(),
            has_plan,
            plan_content,
            source: PlanReviewSource::FileBacked,
            stashed_prompt: StashedPrompt::default(),
            response_tx: None,
            focus: PlanApprovalFocus::Preview,
            prompt_intent: PlanPromptIntent::Revise,
            comments: Vec::new(),
            next_comment_id: 0,
            editing_comment_id: None,
            commenting_range: None,
            stashed_feedback_prompt: None,
            is_local_idle_decision: true,
        }
    }

    pub fn format_feedback(&self, freeform: Option<&str>) -> String {
        let mut parts: Vec<String> = self
            .comments
            .iter()
            .map(|comment| match self.source {
                PlanReviewSource::Inline => {
                    let label = if comment.line_range.len() == 1 {
                        format!("Proposed plan line {}:", comment.line_range.start)
                    } else {
                        format!(
                            "Proposed plan lines {}-{}:",
                            comment.line_range.start,
                            comment.line_range.end - 1
                        )
                    };
                    let snippets =
                        inline_plan_snippets(self.plan_content.as_deref(), &comment.line_range);
                    format!("{label}\n{snippets}\n\nComment:\n{}", comment.text)
                }
                PlanReviewSource::FileBacked => format_file_backed_plan_comment(comment),
            })
            .collect();

        if let Some(text) = freeform
            && !text.trim().is_empty()
        {
            let text = match (self.source, self.comments.is_empty()) {
                (PlanReviewSource::Inline, false) => format!("Additional feedback:\n{text}"),
                _ => text.to_owned(),
            };
            parts.push(text);
        }

        parts.join("\n\n")
    }
}

pub fn send_exit_plan_response(
    tx: tokio::sync::oneshot::Sender<AcpResult<acp::ExtResponse>>,
    outcome: &str,
    feedback: Option<String>,
) {
    let feedback = feedback.filter(|f| !f.trim().is_empty());
    let resp = ExitPlanModeExtResponse {
        outcome: outcome.into(),
        feedback,
    };
    let raw = serde_json::value::to_raw_value(&resp)
        .expect("ExitPlanModeExtResponse serialization should not fail");
    tx.send(Ok(acp::ExtResponse::new(raw.into()))).ok();
}

fn send_ext_response(
    tx: &mut Option<tokio::sync::oneshot::Sender<AcpResult<acp::ExtResponse>>>,
    outcome: &str,
    feedback: Option<String>,
) -> bool {
    let Some(tx) = tx.take() else {
        return false;
    };
    send_exit_plan_response(tx, outcome, feedback);
    true
}

impl PlanApprovalViewState {
    pub fn send_approved(&mut self) -> bool {
        send_ext_response(&mut self.response_tx, "approved", None)
    }

    pub fn send_abandoned(&mut self) -> bool {
        send_ext_response(&mut self.response_tx, "abandoned", None)
    }

    pub fn send_cancelled(&mut self, feedback: Option<String>) -> bool {
        send_ext_response(&mut self.response_tx, "cancelled", feedback)
    }

    /// Clarifying questions — plan mode stays Active; shell injects answer-only turn.
    pub fn send_questions(&mut self, feedback: Option<String>) -> bool {
        send_ext_response(&mut self.response_tx, "questions", feedback)
    }

    pub fn send_stale_cancel(&mut self) -> bool {
        self.send_cancelled(None)
    }
}

fn format_file_backed_plan_comment(comment: &PlanComment) -> String {
    let range = if comment.line_range.len() == 1 {
        format!("@plan.md:{}", comment.line_range.start)
    } else {
        format!(
            "@plan.md:{}-{}",
            comment.line_range.start,
            comment.line_range.end - 1
        )
    };
    format!("{range}\n{}", comment.text)
}

pub(crate) fn inline_plan_snippets(
    plan_content: Option<&str>,
    range: &std::ops::Range<usize>,
) -> String {
    let Some(plan_content) = plan_content else {
        return "> [plan content unavailable]".to_owned();
    };
    let lines: Vec<&str> = plan_content.lines().collect();
    if range.start == 0 || range.start >= range.end || range.start > lines.len() {
        return "> [selected lines unavailable]".to_owned();
    }

    let end = range.end.saturating_sub(1).min(lines.len());
    if end < range.start {
        return "> [selected lines unavailable]".to_owned();
    }

    lines[range.start - 1..end]
        .iter()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn format_plan_comments(comments: &[PlanComment], plan_content: Option<&str>) -> String {
    comments
        .iter()
        .map(|comment| {
            let label = if comment.line_range.len() == 1 {
                format!("Proposed plan line {}:", comment.line_range.start)
            } else {
                format!(
                    "Proposed plan lines {}-{}:",
                    comment.line_range.start,
                    comment.line_range.end - 1
                )
            };
            let snippets = inline_plan_snippets(plan_content, &comment.line_range);
            format!("{label}\n{snippets}\n\nComment:\n{}", comment.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_state() -> (
        PlanApprovalViewState,
        tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>>,
    ) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call_123".into(),
            plan_content: Some("# Plan\n\n## Step 1\nDo something".into()),
        };
        let state = PlanApprovalViewState::new(
            request,
            StashedPrompt {
                text: "stashed text".into(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        );
        (state, rx)
    }

    #[test]
    fn test_send_approved() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_approved());
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "approved");
        assert!(parsed.get("feedback").is_none());
    }

    #[test]
    fn test_send_cancelled_with_feedback() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_cancelled(Some("fix auth flow".into())));
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "cancelled");
        assert_eq!(parsed["feedback"], "fix auth flow");
    }

    #[test]
    fn test_send_cancelled_without_feedback() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_cancelled(None));
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "cancelled");
        assert!(parsed.get("feedback").is_none());
    }

    #[test]
    fn test_send_questions_with_feedback() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_questions(Some("Why Redis?".into())));
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "questions");
        assert_eq!(parsed["feedback"], "Why Redis?");
    }

    #[test]
    fn test_send_cancelled_empty_feedback_is_none() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_cancelled(Some("   ".into())));
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "cancelled");
        assert!(parsed.get("feedback").is_none());
    }

    #[test]
    fn test_send_stale_cancel() {
        let (mut state, mut rx) = make_test_state();
        assert!(state.send_stale_cancel());
        let resp = rx.try_recv().expect("should receive response");
        let raw = resp.expect("should be Ok");
        let parsed: serde_json::Value =
            serde_json::from_str(raw.0.get()).expect("should be valid JSON");
        assert_eq!(parsed["outcome"], "cancelled");
        assert!(parsed.get("feedback").is_none());
    }

    #[test]
    fn test_double_send_returns_false() {
        let (mut state, _rx) = make_test_state();
        assert!(state.send_approved());
        assert!(!state.send_approved());
        assert!(!state.send_cancelled(None));
    }

    #[test]
    fn test_constructor_defaults() {
        let (state, _rx) = make_test_state();
        assert_eq!(state.tool_call_id, "call_123");
        assert!(state.has_plan);
        assert_eq!(
            state.plan_content.as_deref(),
            Some("# Plan\n\n## Step 1\nDo something")
        );
        assert_eq!(state.source, PlanReviewSource::Inline);
        assert_eq!(state.stashed_prompt.text, "stashed text");
        assert!(state.response_tx.is_some());
        assert_eq!(state.focus, PlanApprovalFocus::Preview);
        assert!(state.comments.is_empty());
        assert_eq!(state.next_comment_id, 0);
        assert!(state.editing_comment_id.is_none());
        assert!(state.commenting_range.is_none());
        assert!(state.stashed_feedback_prompt.is_none());
    }

    fn make_empty_plan_state() -> (
        PlanApprovalViewState,
        tokio::sync::oneshot::Receiver<AcpResult<acp::ExtResponse>>,
    ) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call_456".into(),
            plan_content: None,
        };
        let state = PlanApprovalViewState::new(
            request,
            StashedPrompt {
                text: "stashed".into(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        );
        (state, rx)
    }

    #[test]
    fn test_empty_plan_has_plan_false() {
        let (state, _rx) = make_empty_plan_state();
        assert!(!state.has_plan);
        assert!(state.plan_content.is_none());
    }

    #[test]
    fn plan_approval_status_label_distinguishes_empty() {
        assert_eq!(
            plan_approval_status_label(true),
            "Plan ready. Side panel open"
        );
        assert_eq!(
            plan_approval_status_label(false),
            "No plan written. Side panel open"
        );
        // Placeholder must be non-empty so the line viewer accepts it.
        assert!(!EMPTY_PLAN_PLACEHOLDER.trim().is_empty());
        assert!(
            PLAN_FEEDBACK_QUEUE_TOAST.to_lowercase().contains("queue"),
            "queue toast must mention queue; got {PLAN_FEEDBACK_QUEUE_TOAST:?}"
        );
        assert_eq!(PLAN_REVISE_HUMAN_LINE, "Revise the plan");
    }

    #[test]
    fn test_empty_plan_whitespace_only() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let request = ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: "call_789".into(),
            plan_content: Some("   \n\n  ".into()),
        };
        let state = PlanApprovalViewState::new(
            request,
            StashedPrompt {
                text: "stashed".into(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        );
        assert!(!state.has_plan);
        assert!(state.plan_content.is_none());
    }

    #[test]
    fn inline_plan_feedback_quotes_selected_line_snippets() {
        let (mut state, _rx) = make_test_state();
        state.plan_content = Some("alpha\nbravo\ncharlie\ndelta".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 2..3,
            text: "rewrite this".into(),
        });
        state.comments.push(PlanComment {
            id: 1,
            line_range: 3..5,
            text: "combine these".into(),
        });

        let feedback = state.format_feedback(Some("overall note"));

        assert_eq!(
            feedback,
            "Proposed plan line 2:\n> bravo\n\nComment:\nrewrite this\n\nProposed plan lines 3-4:\n> charlie\n> delta\n\nComment:\ncombine these\n\nAdditional feedback:\noverall note"
        );
    }

    #[test]
    fn inline_plan_feedback_handles_out_of_range_lines() {
        let (mut state, _rx) = make_test_state();
        state.plan_content = Some("alpha".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 9..10,
            text: "where is this".into(),
        });

        assert_eq!(
            state.format_feedback(None),
            "Proposed plan line 9:\n> [selected lines unavailable]\n\nComment:\nwhere is this"
        );
    }

    #[test]
    fn file_backed_plan_feedback_keeps_plan_md_references() {
        let (mut state, _rx) = make_test_state();
        state.source = PlanReviewSource::FileBacked;
        state.plan_content = Some("alpha\nbravo".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 1..3,
            text: "keep file ref".into(),
        });

        assert_eq!(
            state.format_feedback(Some("freeform")),
            "@plan.md:1-2\nkeep file ref\n\nfreeform"
        );
    }
}
