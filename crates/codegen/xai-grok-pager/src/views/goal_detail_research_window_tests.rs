//! Goal card, subagent failure row, and header for a research-role window miss.

use std::time::Duration;

use ratatui::layout::{Position, Rect};

use crate::app::agent::{GoalDisplayPhase, GoalDisplayState, GoalDisplayStatus};
use crate::appearance::AppearanceConfig;
use crate::scrollback::block::BlockContent;
use crate::scrollback::blocks::SubagentBlock;
use crate::scrollback::types::{BlockContext, DisplayMode};
use crate::theme::Theme;
use crate::views::agent_status::goal_status_line;

const OPERATOR_SENTENCE: &str =
    "now run just deploy-dry-run and fix all the errors in a complete way";

fn research_window_pause_message() -> String {
    const HUMAN: &str = "Context is over this session's sampling window (230127/200000 tokens). Not retrying the same oversized model request. Compact once or start a new session.";
    format!(
        "The research role stopped with a runtime error: Session error: Internal error: {{\"message\":{human:?},\"promptUsage\":{{\"costUsdTicks\":13939020000}}}}",
        human = HUMAN
    )
}

fn assert_names_research_window_without_raw_json(text: &str) {
    assert!(
        text.contains("research role"),
        "the visible sentence must name the research role, got:\n{text}"
    );
    assert!(
        text.contains("200000"),
        "the visible sentence must name the research role's 200000 window, got:\n{text}"
    );
    assert!(
        !text.contains("promptUsage"),
        "promptUsage must not render, got:\n{text}"
    );
    assert!(
        !text.contains("costUsdTicks"),
        "costUsdTicks must not render, got:\n{text}"
    );
    assert!(
        !text.contains("Session error: Internal error:"),
        "raw session error text must not render, got:\n{text}"
    );
    assert!(
        !text.contains("Compact once"),
        "the card must not tell the operator to compact the main session, got:\n{text}"
    );
}

fn render_card(goal: &GoalDisplayState) -> String {
    let screen = Rect::new(0, 0, 100, 40);
    let mut buf = ratatui::buffer::Buffer::empty(screen);
    let area = super::goal_detail_area(screen, goal, &[]);
    super::render_goal_detail(
        &mut buf,
        area,
        goal,
        &[],
        0,
        None,
        0,
        super::GoalDetailHovers::default(),
    );
    let mut out = String::new();
    let area = buf.area;
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell(Position::new(x, y)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

fn paused_goal() -> GoalDisplayState {
    let mut goal = GoalDisplayState::test_stub();
    goal.status = GoalDisplayStatus::InfraPaused;
    goal.objective = OPERATOR_SENTENCE.into();
    goal.pause_message = Some(research_window_pause_message());
    goal
}

fn subagent_failure_line(raw: &str) -> String {
    let block = SubagentBlock::failed(
        "goal research specialist",
        "research-session",
        Duration::from_secs(2),
        Some(raw.to_string()),
    );
    let ctx = BlockContext {
        mode: DisplayMode::Collapsed,
        is_running: false,
        width: 240,
        raw: false,
        max_lines: None,
        appearance: AppearanceConfig::default(),
        is_selected: false,
        cwd: None,
    };
    let output = BlockContent::output(&block, &ctx);
    output
        .lines
        .iter()
        .flat_map(|line| line.content.spans.iter().map(|span| span.content.as_ref()))
        .collect()
}

#[test]
fn goal_card_and_subagent_row_name_the_research_role_window_without_raw_session_json() {
    let raw = research_window_pause_message();
    assert!(
        raw.contains("promptUsage") && raw.contains("costUsdTicks"),
        "the fixture keeps the raw session object"
    );
    assert!(raw.contains("Session error: Internal error:"));
    let card = render_card(&paused_goal());
    assert_names_research_window_without_raw_json(&card);
    assert!(
        card.contains(OPERATOR_SENTENCE),
        "the card keeps the operator sentence, got:\n{card}"
    );
    let row = subagent_failure_line(&raw);
    assert_names_research_window_without_raw_json(&row);
}

#[test]
fn goal_command_row_stays_on_the_card_for_a_research_window_miss() {
    let text = render_card(&paused_goal());
    assert!(
        text.contains("/goal resume | pause | status | clear"),
        "goal command row must stay on the card, got:\n{text}"
    );
    assert_names_research_window_without_raw_json(&text);
}

#[test]
fn header_does_not_say_goal_idle_while_a_research_replacement_is_in_progress() {
    let mut goal = GoalDisplayState::test_stub();
    goal.status = GoalDisplayStatus::Active;
    goal.phase = GoalDisplayPhase::Idle;
    goal.planning = false;
    goal.verifying_completion = false;
    goal.current_subagent_role = Some("goal research specialist".into());
    let line = goal_status_line(&goal, &Theme::current(), false, 0, None, 0);
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert!(
        !text.contains("Goal: Idle"),
        "header must not say Goal: Idle while a research replacement is in progress, got: {text}"
    );
    assert!(
        text.contains("Research"),
        "header names the research role while the replacement is in progress, got: {text}"
    );
}
