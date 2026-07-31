use agent_client_protocol as acp;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use xai_acp_lib::AcpResult;

pub use xai_grok_tools::implementations::grok_build::exit_plan_mode::{
    ExitPlanModeExtRequest, ExitPlanModeExtResponse,
};

use crate::theme::Theme;
use crate::views::prompt_widget::StashedPrompt;

/// Placeholder body for the plan-approval preview when `exit_plan_mode` parks
/// with no plan content (missing/empty `plan.md`, or a whitespace-only body).
///
/// Must be non-empty after trim so `LineViewerState::open_markdown_content`
/// accepts it — empty bodies are rejected there.
pub const EMPTY_PLAN_PLACEHOLDER: &str = "\
# No plan written yet

The agent exited plan mode without writing a plan.

Use the footer buttons below to approve, revise, or quit.
";

/// Toast shown when `exit_plan_mode` parks approval and auto-opens the
/// non-capturing side panel (default soft park).
///
/// Fullscreen modal is still opt-in via `[ui] plan_approval_park = "modal"`.
/// `/view-plan` / status click / `ShowPlan` reopen the panel if dismissed.
pub const PLAN_PARKED_TOAST: &str =
    "Plan ready. Review the side panel, or click Approve/Quit below";

/// Header line for the inline transcript plan card (option C).
pub const PLAN_CARD_HEADER: &str = "Plan ready for review";

/// Empty-plan header for the inline transcript card.
pub const PLAN_CARD_HEADER_EMPTY: &str = "No plan written yet";

/// Plain footer pointer on the soft-park transcript card (not a button row).
///
/// Real clickable CTAs live only on soft-park footer chrome (`paint_soft_park_cta_buttons`).
/// This string must not look like dead clickable buttons or an AI-Dungeon option list.
pub const PLAN_CARD_CTAS: &str = "Use the footer buttons below to approve, revise, or quit.";

/// Max body lines embedded in the soft-park transcript card before ellipsis.
pub const PLAN_CARD_PREVIEW_LINES: usize = 12;

/// Build the scrollback body for a soft-parked plan (option C).
///
/// Header + truncated plan preview + plain review pointer. Real clickable CTAs
/// live on soft-park footer chrome only; the card body must not fake a button row.
pub fn format_parked_plan_card(plan_content: Option<&str>) -> String {
    let has_plan = plan_content.is_some_and(|s| !s.trim().is_empty());
    let header = if has_plan {
        PLAN_CARD_HEADER
    } else {
        PLAN_CARD_HEADER_EMPTY
    };
    let mut out = String::new();
    out.push_str(header);
    out.push('\n');
    out.push('\n');
    if let Some(body) = plan_content.map(str::trim).filter(|s| !s.is_empty()) {
        let lines: Vec<&str> = body.lines().collect();
        let take = lines.len().min(PLAN_CARD_PREVIEW_LINES);
        for line in &lines[..take] {
            out.push_str(line);
            out.push('\n');
        }
        if lines.len() > PLAN_CARD_PREVIEW_LINES {
            out.push_str("…\n");
        }
        out.push('\n');
    } else {
        out.push_str("The agent exited plan mode without writing a plan.\n\n");
    }
    out.push_str(PLAN_CARD_CTAS);
    out
}

/// Hit rects for soft-park footer CTA buttons (mouse primary).
#[derive(Debug, Clone, Copy, Default)]
pub struct SoftParkCtaAreas {
    pub approve: Option<Rect>,
    pub notes: Option<Rect>,
    pub clarify: Option<Rect>,
    pub revise: Option<Rect>,
    pub quit: Option<Rect>,
}

/// Hover flags for soft-park footer CTA paint (mirrors panel footer).
#[derive(Debug, Clone, Copy, Default)]
pub struct SoftParkCtaHovers {
    pub approve: bool,
    pub notes: bool,
    pub clarify: bool,
    pub revise: bool,
    pub quit: bool,
}

fn soft_park_button_spans<'a>(
    key: char,
    rest: &str,
    hovered: bool,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let bg = if hovered {
        theme.bg_highlight
    } else {
        theme.bg_base
    };
    let key_style = Style::default()
        .fg(theme.text_primary)
        .bg(bg)
        .add_modifier(Modifier::BOLD);
    if rest.is_empty() {
        return vec![Span::styled(key.to_string(), key_style)];
    }
    let label_style = Style::default().fg(theme.gray).bg(bg);
    vec![
        Span::styled(key.to_string(), key_style),
        Span::styled(format!(" {rest}"), label_style),
    ]
}

/// Paint soft-park plan-approval CTA buttons into `area` (usually the
/// shortcuts row). Returns hit rects for mouse dispatch.
///
/// Same five actions as the side-panel footer. Tries full labels → compact
/// → key-only, denser separators, then multi-row wrap when `area.height >= 2`,
/// so **Revise** (and every other button) stays hit-testable on narrow rows.
///
/// Separator width uses **display** columns (`UnicodeWidthStr`), never
/// `str::len()` bytes — middle-dot `" · "` is 3 cols / 4 UTF-8 bytes, and
/// byte width used to over-count packing and drop later buttons.
pub fn paint_soft_park_cta_buttons(
    buf: &mut Buffer,
    area: Rect,
    theme: &Theme,
    hovers: SoftParkCtaHovers,
) -> SoftParkCtaAreas {
    if area.width == 0 || area.height == 0 {
        return SoftParkCtaAreas::default();
    }
    use unicode_width::UnicodeWidthStr;

    let sep_style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
    let keys = ['a', 'A', '?', 's', 'q'];
    let hovers_arr = [
        hovers.approve,
        hovers.notes,
        hovers.clarify,
        hovers.revise,
        hovers.quit,
    ];
    // Prefer readable separators, fall back to denser packing so all five fit.
    let separators = [" · ", " ", ""];
    let label_modes: [[&str; 5]; 3] = [
        ["approve", "approve w/ comment", "clarify", "revise", "quit"],
        ["approve", "notes", "clarify", "revise", "quit"],
        ["", "", "", "", ""],
    ];

    for labels in &label_modes {
        let span_sets: Vec<Vec<Span>> = keys
            .iter()
            .zip(labels.iter())
            .zip(hovers_arr.iter())
            .map(|((&k, &lab), &hov)| soft_park_button_spans(k, lab, hov, theme))
            .collect();
        let widths: Vec<u16> = span_sets
            .iter()
            .map(|s| s.iter().map(|sp| sp.width() as u16).sum())
            .collect();

        for separator in separators {
            let sep_w = UnicodeWidthStr::width(separator) as u16;
            let mut total_w = widths.iter().copied().sum::<u16>();
            total_w = total_w.saturating_add(sep_w.saturating_mul(4));
            if total_w > area.width {
                continue;
            }
            let y = area.y;
            let mut x = area.x + (area.width - total_w) / 2;
            let mut areas = [None; 5];
            for i in 0..5 {
                if i > 0 {
                    if sep_w > 0 {
                        buf.set_string(x, y, separator, sep_style);
                    }
                    x = x.saturating_add(sep_w);
                }
                let start = x;
                let bw = widths[i].max(1);
                for span in &span_sets[i] {
                    let w = span.width() as u16;
                    buf.set_span(x, y, span, w);
                    x = x.saturating_add(w);
                }
                areas[i] = Some(Rect::new(start, y, bw, 1));
            }
            return SoftParkCtaAreas {
                approve: areas[0],
                notes: areas[1],
                clarify: areas[2],
                revise: areas[3],
                quit: areas[4],
            };
        }
    }

    // Multi-row wrap (when footer grants height > 1): left-align key-only
    // buttons so every CTA remains hit-testable.
    if area.height >= 2 {
        let mut areas = [None; 5];
        let mut x = area.x;
        let mut y = area.y;
        let row_end = area.x.saturating_add(area.width);
        let y_max = area.y.saturating_add(area.height.saturating_sub(1));
        let sep = " ";
        let sep_w = 1u16;
        for (i, &k) in keys.iter().enumerate() {
            let spans = soft_park_button_spans(k, "", hovers_arr[i], theme);
            let w: u16 = spans.iter().map(|s| s.width() as u16).sum::<u16>().max(1);
            let need = if x == area.x {
                w
            } else {
                sep_w.saturating_add(w)
            };
            if x.saturating_add(need) > row_end {
                if y >= y_max {
                    // Last row full: still place remaining keys by restarting
                    // the row (prefer clipped hits over None).
                    y = y_max;
                } else {
                    y = y.saturating_add(1);
                }
                x = area.x;
            } else if x != area.x {
                buf.set_string(x, y, sep, sep_style);
                x = x.saturating_add(sep_w);
            }
            // If even a single key won't fit, force it at row start.
            if x.saturating_add(w) > row_end {
                x = area.x;
            }
            let start = x;
            for span in &spans {
                let sw = span.width() as u16;
                buf.set_span(x, y, span, sw);
                x = x.saturating_add(sw);
            }
            areas[i] = Some(Rect::new(start, y, w, 1));
        }
        return SoftParkCtaAreas {
            approve: areas[0],
            notes: areas[1],
            clarify: areas[2],
            revise: areas[3],
            quit: areas[4],
        };
    }

    // Height-1 extreme narrow: partition the row into five non-empty slots so
    // **Revise** (index 3) is never dropped. Paint key-only glyphs when a slot
    // has room; hit rects cover each slot (no zero-width / no full drop).
    let mut areas = [None; 5];
    let n = 5u16;
    let base = (area.width / n).max(1);
    let mut rem = area
        .width
        .saturating_sub(base.saturating_mul(n.min(area.width)));
    // When width < 5, stack remaining keys on the last column (still Some).
    let mut x = area.x;
    for (i, &k) in keys.iter().enumerate() {
        let extra = if rem > 0 {
            rem -= 1;
            1
        } else {
            0
        };
        let slot_w = if area.width >= 5 {
            base.saturating_add(extra).max(1)
        } else {
            1
        };
        let start = if area.width >= 5 {
            x
        } else {
            // Overlap last columns rather than leave None.
            area.x
                .saturating_add((i as u16).min(area.width.saturating_sub(1)))
        };
        let spans = soft_park_button_spans(k, "", hovers_arr[i], theme);
        for span in &spans {
            let sw = (span.width() as u16).min(slot_w).max(1);
            buf.set_span(start, area.y, span, sw);
        }
        areas[i] = Some(Rect::new(start, area.y, slot_w, 1));
        if area.width >= 5 {
            x = x.saturating_add(slot_w);
        }
    }
    SoftParkCtaAreas {
        approve: areas[0],
        notes: areas[1],
        clarify: areas[2],
        revise: areas[3],
        quit: areas[4],
    }
}

/// Status-line label while plan approval is parked (soft or modal).
///
/// Default soft park auto-opens the side panel; this chip stays until the
/// user decides. Empty plans still name a review path so the status line
/// never looks stuck with no way forward.
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
/// - **Revise** (`s`): ACP `"cancelled"` — rewrite the plan.
/// - **Questions** (`?` clarify): ACP `"questions"` — answer read-only; do not rewrite.
/// - **ApproveNotes** (`A`): ACP `"approved"` + notes via approve Interject.
///
/// Wire outcomes keep their historical strings; user-facing labels use
/// clarify / revise / approve w/ comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanPromptIntent {
    #[default]
    Revise,
    Questions,
    ApproveNotes,
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
        }
    }

    pub fn format_feedback(&self, freeform: Option<&str>) -> String {
        self.format_feedback_with_selection(freeform, None)
    }

    /// Build revise/clarify feedback for the agent.
    ///
    /// Every selected plan line the user commented on (or, when there are no
    /// saved comments, the current viewer `selection`) is rendered as:
    /// path (`@plan.md:N`), quoted line text, then the user's words.
    /// That trio is required so the model does not have to guess which line
    /// "this line" refers to.
    pub fn format_feedback_with_selection(
        &self,
        freeform: Option<&str>,
        selection: Option<&std::ops::Range<usize>>,
    ) -> String {
        let mut parts: Vec<String> = self
            .comments
            .iter()
            .map(|comment| format_plan_line_comment(self.plan_content.as_deref(), comment))
            .collect();

        // Freeform-only path: attach the live viewer selection so revise /
        // clarify about "this line" still carries path + line + text.
        if self.comments.is_empty()
            && let Some(range) = selection
            && range.start > 0
            && range.end > range.start
        {
            parts.push(format_selected_plan_lines(
                self.plan_content.as_deref(),
                range,
            ));
        }

        if let Some(text) = freeform
            && !text.trim().is_empty()
        {
            let text = if self.comments.is_empty() {
                text.to_owned()
            } else {
                format!("Additional feedback:\n{text}")
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

/// Session plan file basename used in agent-facing selection anchors.
/// Matches the on-disk name under the session directory (`…/plan.md`).
pub const PLAN_FEEDBACK_PATH: &str = "plan.md";

/// `@plan.md:N` or `@plan.md:N-M` for a 1-based half-open line range.
pub(crate) fn format_plan_line_loc(range: &std::ops::Range<usize>) -> String {
    if range.len() == 1 {
        format!("@{PLAN_FEEDBACK_PATH}:{}", range.start)
    } else {
        format!("@{PLAN_FEEDBACK_PATH}:{}-{}", range.start, range.end - 1)
    }
}

/// Path + line number(s) + quoted line text — the selection payload the agent
/// needs when the user refers to "this line".
pub(crate) fn format_selected_plan_lines(
    plan_content: Option<&str>,
    range: &std::ops::Range<usize>,
) -> String {
    let loc = format_plan_line_loc(range);
    let snippets = inline_plan_snippets(plan_content, range);
    format!("{loc}\n{snippets}")
}

fn format_plan_line_comment(plan_content: Option<&str>, comment: &PlanComment) -> String {
    let header = format_selected_plan_lines(plan_content, &comment.line_range);
    format!("{header}\n\nComment:\n{}", comment.text)
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
        .map(|comment| format_plan_line_comment(plan_content, comment))
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
    fn test_constructor_defaults_prompt_intent_revise() {
        let (state, _rx) = make_test_state();
        assert_eq!(state.prompt_intent, PlanPromptIntent::Revise);
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
        assert!(
            PLAN_PARKED_TOAST.contains("Plan ready") && PLAN_PARKED_TOAST.contains("side panel"),
            "soft-park toast must name auto-open side panel; got {PLAN_PARKED_TOAST:?}"
        );
        assert!(
            !PLAN_PARKED_TOAST.contains("/view-plan"),
            "toast must not nudge /view-plan when the panel auto-opens"
        );
        // Placeholder must be non-empty so the line viewer accepts it.
        assert!(!EMPTY_PLAN_PLACEHOLDER.trim().is_empty());
        assert!(
            EMPTY_PLAN_PLACEHOLDER.contains("footer"),
            "empty-plan copy must point at real review paths; got {EMPTY_PLAN_PLACEHOLDER:?}"
        );
        assert!(
            !EMPTY_PLAN_PLACEHOLDER.contains("(`a`)")
                && !EMPTY_PLAN_PLACEHOLDER.contains("- **Approve**")
                && !EMPTY_PLAN_PLACEHOLDER.contains("Approve w/ comment"),
            "empty-plan body must not fake a key/option menu; got {EMPTY_PLAN_PLACEHOLDER:?}"
        );
    }

    /// Named contract: scrollback plan card is preview + plain pointer only.
    /// No dead "Approve · Notes · …" button row or "keys when prompt empty" theater.
    #[test]
    fn parked_plan_card_has_no_fake_button_chrome() {
        let card = format_parked_plan_card(Some("# Title\n\nLine two\nLine three"));
        assert!(card.starts_with(PLAN_CARD_HEADER));
        assert!(card.contains("# Title") && card.contains("Line two"));
        assert!(
            card.contains(PLAN_CARD_CTAS),
            "card must keep plain review pointer; got {card:?}"
        );
        assert!(
            !card.contains("Approve ·")
                && !card.contains("when prompt empty")
                && !card.contains("a/A/?/s/q")
                && !card.contains("Click footer: Approve"),
            "card must not look like dead clickable CTAs; got {card:?}"
        );
        let empty = format_parked_plan_card(None);
        assert!(empty.starts_with(PLAN_CARD_HEADER_EMPTY));
        assert!(empty.contains(PLAN_CARD_CTAS));
        assert!(
            !empty.contains("Approve ·") && !empty.contains("a/A/?/s/q"),
            "empty card must not fake button chrome; got {empty:?}"
        );
    }

    #[test]
    fn format_parked_plan_card_embeds_preview_and_ctas() {
        let card = format_parked_plan_card(Some("# Title\n\nLine two\nLine three"));
        assert!(card.starts_with(PLAN_CARD_HEADER));
        assert!(card.contains("# Title") && card.contains("Line two"));
        assert!(card.contains(PLAN_CARD_CTAS));
        let empty = format_parked_plan_card(None);
        assert!(empty.starts_with(PLAN_CARD_HEADER_EMPTY));
        assert!(empty.contains(PLAN_CARD_CTAS));
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

        // Path + line number(s) + line text must all reach the agent.
        assert_eq!(
            feedback,
            "@plan.md:2\n> bravo\n\nComment:\nrewrite this\n\n@plan.md:3-4\n> charlie\n> delta\n\nComment:\ncombine these\n\nAdditional feedback:\noverall note"
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
            "@plan.md:9\n> [selected lines unavailable]\n\nComment:\nwhere is this"
        );
    }

    #[test]
    fn file_backed_plan_feedback_includes_path_line_and_text() {
        let (mut state, _rx) = make_test_state();
        state.source = PlanReviewSource::FileBacked;
        state.plan_content = Some("alpha\nbravo".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 1..3,
            text: "keep file ref".into(),
        });

        let feedback = state.format_feedback(Some("freeform"));
        // P1: agent must receive plan path, line range, and quoted line text
        // (not just @plan.md:N + comment without body).
        assert_eq!(
            feedback,
            "@plan.md:1-2\n> alpha\n> bravo\n\nComment:\nkeep file ref\n\nAdditional feedback:\nfreeform"
        );
    }

    /// Freeform revise/clarify with a viewer selection (no saved line comments)
    /// must still deliver path + line number + line text so the agent is not
    /// left guessing which "this line" the user means.
    #[test]
    fn freeform_with_selection_includes_path_line_and_text() {
        let (mut state, _rx) = make_test_state();
        state.source = PlanReviewSource::FileBacked;
        state.plan_content = Some("alpha\nbravo\ncharlie".into());

        let feedback = state.format_feedback_with_selection(Some("fix this line"), Some(&(2..3)));

        assert_eq!(feedback, "@plan.md:2\n> bravo\n\nfix this line");
    }

    /// P2: multi-line highlight freeform must deliver the full range loc
    /// (`@plan.md:N-M`) and quoted text for every selected line.
    #[test]
    fn freeform_with_multiline_selection_includes_range_and_all_line_text() {
        let (mut state, _rx) = make_test_state();
        state.source = PlanReviewSource::FileBacked;
        state.plan_content = Some("alpha\nbravo\ncharlie\ndelta".into());

        let feedback =
            state.format_feedback_with_selection(Some("rewrite this block"), Some(&(2..4)));

        assert_eq!(
            feedback,
            "@plan.md:2-3\n> bravo\n> charlie\n\nrewrite this block"
        );
    }

    /// P2: multi-line saved comment also uses start–end loc + all quoted lines.
    #[test]
    fn multiline_comment_includes_range_and_all_line_text() {
        let (mut state, _rx) = make_test_state();
        state.plan_content = Some("alpha\nbravo\ncharlie\ndelta".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 2..4,
            text: "tighten these two".into(),
        });

        assert_eq!(
            state.format_feedback(None),
            "@plan.md:2-3\n> bravo\n> charlie\n\nComment:\ntighten these two"
        );
    }

    /// Without a selection, freeform alone is unchanged (no invented anchors).
    #[test]
    fn freeform_without_selection_is_plain() {
        let (mut state, _rx) = make_test_state();
        state.source = PlanReviewSource::FileBacked;
        state.plan_content = Some("alpha\nbravo".into());

        assert_eq!(
            state.format_feedback_with_selection(Some("overall rewrite"), None),
            "overall rewrite"
        );
    }

    /// Saved comments already carry ranges — do not double-prefix the cursor
    /// selection on top of them.
    #[test]
    fn selection_ignored_when_comments_already_present() {
        let (mut state, _rx) = make_test_state();
        state.plan_content = Some("alpha\nbravo\ncharlie".into());
        state.comments.push(PlanComment {
            id: 0,
            line_range: 1..2,
            text: "first".into(),
        });

        let feedback = state.format_feedback_with_selection(Some("more"), Some(&(3..4)));

        assert_eq!(
            feedback,
            "@plan.md:1\n> alpha\n\nComment:\nfirst\n\nAdditional feedback:\nmore"
        );
        assert!(
            !feedback.contains("@plan.md:3"),
            "cursor selection must not double-attach when comments exist: {feedback}"
        );
    }

    /// Named contract (dogfood 2026-07-29): all five soft-park footer CTAs get
    /// non-empty hit rects after paint — including Revise — on typical and
    /// narrow rows. Hit rects must not zero-width and must not overlap.
    #[test]
    fn soft_park_paint_all_five_cta_hit_areas_wide_and_narrow() {
        let theme = Theme::current();
        for width in [80u16, 52, 40, 24, 17, 12, 9, 5] {
            let area = Rect::new(0, 3, width, 1);
            let mut buf = Buffer::empty(Rect::new(0, 0, width.max(1), 6));
            let areas =
                paint_soft_park_cta_buttons(&mut buf, area, &theme, SoftParkCtaHovers::default());
            let rects = [
                ("approve", areas.approve),
                ("notes", areas.notes),
                ("clarify", areas.clarify),
                ("revise", areas.revise),
                ("quit", areas.quit),
            ];
            for (name, r) in &rects {
                let r = r.unwrap_or_else(|| panic!("{name} hit missing at width={width}"));
                assert!(
                    r.width >= 1 && r.height >= 1,
                    "{name} zero-sized hit at width={width}: {r:?}"
                );
            }
            // No pairwise overlap on a single-row paint (width >= 5 partitions).
            if width >= 5 {
                let filled: Vec<Rect> = rects.iter().filter_map(|(_, r)| *r).collect();
                for i in 0..filled.len() {
                    for j in (i + 1)..filled.len() {
                        let a = filled[i];
                        let b = filled[j];
                        let x_overlap =
                            a.x < b.x.saturating_add(b.width) && b.x < a.x.saturating_add(a.width);
                        let y_overlap = a.y < b.y.saturating_add(b.height)
                            && b.y < a.y.saturating_add(a.height);
                        assert!(
                            !(x_overlap && y_overlap),
                            "CTA hits overlap at width={width}: {a:?} vs {b:?}"
                        );
                    }
                }
            }
            // Revise specifically: non-empty and inside the paint area.
            let revise = areas.revise.expect("revise");
            assert!(
                revise.y >= area.y && revise.y < area.y.saturating_add(area.height),
                "revise y out of area at width={width}"
            );
            assert!(
                revise.x >= area.x && revise.x < area.x.saturating_add(area.width.max(1)),
                "revise x out of area at width={width}: {revise:?}"
            );
        }
    }

    /// Multi-row wrap still exposes all five when height allows.
    #[test]
    fn soft_park_paint_wraps_when_height_allows() {
        let theme = Theme::current();
        // Width too narrow for one key-only row with middle-dot seps in old code;
        // height 2 must still yield five hits.
        let area = Rect::new(2, 1, 10, 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 6));
        let areas =
            paint_soft_park_cta_buttons(&mut buf, area, &theme, SoftParkCtaHovers::default());
        assert!(areas.approve.is_some());
        assert!(areas.notes.is_some());
        assert!(areas.clarify.is_some());
        assert!(
            areas.revise.is_some(),
            "wrap path must keep Revise hit-testable"
        );
        assert!(areas.quit.is_some());
    }
}
