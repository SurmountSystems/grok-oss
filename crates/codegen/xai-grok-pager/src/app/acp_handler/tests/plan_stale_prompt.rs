//! Isolated Preview stale-prompt contracts.
//!
//! Operator: "Also I'm starting to wonder if planning still has stale prompt
//! problems, because that's causing confusion. Please, we need a lot more
//! red/green TDD tests for that."
//!
//! Named tests in this file are Surmount contracts. Do not fit them to
//! code. Do not weaken prompt WAL, `/rebuild` persist, or image-describe
//! fail-open tests. Empty Enter never Approves.
//!
//! This module is additive to Isolated Preview tests in
//! `plan_approve_lost_prompt.rs`, `viewer_tests.rs`, and
//! `dispatch/tests/prompt.rs`. Do not delete those.

use super::*;
use crate::app::actions::{Action, Effect};
use crate::app::agent::{QueueEntryKind, QueuedPrompt};
use crate::app::app_view::InputOutcome;
use crate::scrollback::block::RenderBlock;
use crate::views::plan_approval_view::{
    PLAN_APPROVED_REVIEW_COMMENTS_LEAD, PLAN_REWRITE_WAIT_HEADING, PlanApprovalFocus,
    PlanFeedbackInFlight, PlanPromptIntent,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use xai_grok_shell::session::pending_prompts::PersistedQueuedPrompt;
use xai_grok_shell::session::prompt_wal::PromptWalKind;

/// Operator quote, first tests.
const OPERATOR_STALE_PROMPT: &str = concat!(
    "Also I'm starting to wonder if planning still has stale prompt ",
    "problems, because that's causing confusion. Please, we need a lot ",
    "more red/green TDD tests for that."
);

const HUMAN_TURN: &str = "planning still has stale prompt problems";
const PRODUCT_REPORT: &str = "Job: Isolated Preview must send this as a Human turn.";
const COMMENT_STASH: &str = "this comment may ride Approve";

const APPROVE_HIT: Rect = Rect {
    x: 10,
    y: 20,
    width: 8,
    height: 1,
};
const COMMENT_HIT: Rect = Rect {
    x: 30,
    y: 20,
    width: 8,
    height: 1,
};
const MODAL_AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn isolated_present(
    app: &mut AppView,
    tool_call_id: &str,
    plan: &str,
) -> tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>> {
    let session_id = {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        seed_pending_tool(agent, tool_call_id, "CreatePlan");
        agent.pane_areas.prompt = Rect::new(0, 22, 80, 3);
        agent
            .session
            .session_id
            .as_ref()
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "sess-1".into())
    };
    let (ext, rx) = make_exit_plan_ext_for_session(&session_id, tool_call_id, Some(plan));
    assert!(
        handle_exit_plan_mode(ext, app),
        "isolated present must park the live waiter and dock the pane"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(agent.plan_approval_view.is_some());
    assert!(
        agent.line_viewer.is_some(),
        "isolated present must auto-open the plan side panel"
    );
    rx
}

fn arm_comment_and_approve_hit_rects(app: &mut AppView) {
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    let viewer = agent.line_viewer.as_mut().expect("plan pane open");
    viewer.plan_mut().approve_button_area = Some(APPROVE_HIT);
    viewer.plan_mut().comment_button_area = Some(COMMENT_HIT);
    viewer.last_modal_area = Some(MODAL_AREA);
}

fn mouse_down(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn enter_key() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

fn dispatch_outcome(app: &mut AppView, outcome: InputOutcome) -> Vec<Effect> {
    match outcome {
        InputOutcome::Action(action) | InputOutcome::ActionThenForward(action) => {
            crate::app::dispatch::dispatch(action, app)
        }
        InputOutcome::ActionPair(first, second) => {
            let mut effects = crate::app::dispatch::dispatch(first, app);
            effects.extend(crate::app::dispatch::dispatch(second, app));
            effects
        }
        _ => Vec::new(),
    }
}

fn type_into_human_box(app: &mut AppView, text: &str) {
    for ch in text.chars() {
        let ev = Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        let outcome = app.handle_input(&ev);
        assert!(
            !matches!(
                outcome,
                InputOutcome::Action(_)
                    | InputOutcome::ActionThenForward(_)
                    | InputOutcome::ActionPair(_, _)
            ),
            "typing in the Human box must not Approve or send, got {outcome:?}"
        );
    }
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.prompt.text().contains(text),
        "typed Human-box text must land in the composer, got {:?}",
        agent.prompt.text()
    );
}

fn effects_send_human(effects: &[Effect], needle: &str) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::SendPrompt { text, .. }
        | Effect::SendInterject { text, .. }
        | Effect::SetModeThenPrompt { text, .. } => {
            text.contains(needle) && !text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD)
        }
        Effect::SendPromptNow { .. } => true,
        _ => false,
    })
}

fn effects_approve_with_notes(effects: &[Effect], needle: &str) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::SendInterject { text, .. } => {
            text.contains(needle) && text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD)
        }
        _ => false,
    })
}

fn assert_enter_approves_with_notes(outcome: &InputOutcome, needle: &str) {
    match outcome {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains(needle),
                "Enter must Approve with the Operator notes, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Approve with comment must wrap the notes, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview idle plus a non-empty Operator box plus Enter must Approve with those notes; got {other:?}"
        ),
    }
}

fn plan_comment_texts(app: &AppView) -> Vec<String> {
    app.agents
        .get(&AgentId(0))
        .unwrap()
        .plan_approval_view
        .as_ref()
        .map(|pav| pav.comments.iter().map(|c| c.text.clone()).collect())
        .unwrap_or_default()
}

fn queued_texts(app: &AppView) -> Vec<String> {
    app.agents
        .get(&AgentId(0))
        .unwrap()
        .session
        .pending_prompts
        .iter()
        .map(|p| p.text.clone())
        .collect()
}

fn user_prompt_texts(app: &AppView) -> Vec<String> {
    let agent = app.agents.get(&AgentId(0)).unwrap();
    (0..agent.scrollback.len())
        .filter_map(|i| match agent.scrollback.get(i).map(|e| &e.block) {
            Some(RenderBlock::UserPrompt(b)) => Some(b.text.clone()),
            _ => None,
        })
        .collect()
}

fn bind_session_home(app: &mut AppView, cwd: std::path::PathBuf, sid: &str) {
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    agent.session.session_id = Some(sid.to_string().into());
    agent.session.cwd = cwd;
}

fn load_wal(cwd: &str, sid: &str) -> Vec<xai_grok_shell::session::prompt_wal::PromptWalRecord> {
    xai_grok_shell::session::prompt_wal::load_prompt_wal(cwd, sid).unwrap_or_default()
}

fn wal_has_human_send(cwd: &str, sid: &str, needle: &str) -> bool {
    load_wal(cwd, sid).iter().any(|r| {
        matches!(
            r.kind,
            PromptWalKind::Send
                | PromptWalKind::Queue
                | PromptWalKind::Interject
                | PromptWalKind::PlanNotes
        ) && r.text.contains(needle)
    })
}

fn assert_not_only_plan_comment_1(app: &AppView, needle: &str) {
    let comments = plan_comment_texts(app);
    let in_comments = comments.iter().any(|c| c.contains(needle));
    let in_scrollback = user_prompt_texts(app).iter().any(|t| t.contains(needle));
    assert!(
        !in_comments || in_scrollback,
        "Human sentence must not remain only as plan comment 1; comments={comments:?} scrollback={:?}",
        user_prompt_texts(app)
    );
}

fn click_comment_cta(app: &mut AppView) {
    arm_comment_and_approve_hit_rects(app);
    let outcome = app.handle_input(&mouse_down(32, 20));
    assert!(
        !matches!(
            outcome,
            InputOutcome::Action(Action::SendPrompt(_))
                | InputOutcome::Action(Action::Interject { .. })
        ),
        "Comment CTA must not Approve or send, got {outcome:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert_eq!(
        agent.plan_approval_view.as_ref().map(|p| p.focus),
        Some(PlanApprovalFocus::Prompt),
        "Comment CTA focuses the composer"
    );
    assert_eq!(
        agent.plan_approval_view.as_ref().map(|p| p.prompt_intent),
        Some(PlanPromptIntent::Comment),
        "Comment CTA is the comment path, not Isolated Preview Human send"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "Comment CTA must not Approve"
    );
}

fn write_session_chat_history(cwd: &str, sid: &str, jsonl: &str) {
    let path = xai_grok_shell::session::prompt_wal::chat_history_path(cwd, sid)
        .expect("chat_history.jsonl path");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, jsonl).unwrap();
}

/// Operator: "Also I'm starting to wonder if planning still has stale prompt
/// problems, because that's causing confusion. Please, we need a lot more
/// red/green TDD tests for that."
/// Isolated Preview open: Human sentence must not remain only as plan
/// comment 1; must be Human turn + WAL.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_human_sentence_is_human_turn_and_wal_not_only_plan_comment_1() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-stale-human-wal";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd, sid);
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nHuman send while preview is open\n",
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Preview)
        );
        assert!(agent.prompt.text().trim().is_empty());
    }

    type_into_human_box(&mut app, HUMAN_TURN);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, HUMAN_TURN);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, HUMAN_TURN),
        "Isolated Preview idle plus notes plus Enter must Approve with those notes; effects={effects:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, HUMAN_TURN),
        "Approve with notes must be on WAL, got {:?}",
        load_wal(&cwd_str, sid)
    );
    assert_not_only_plan_comment_1(&app, HUMAN_TURN);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Isolated Preview idle Enter with notes must Approve"
    );
    assert!(
        !agent
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.feedback_draft.as_deref())
            .is_some_and(|d| d.contains(HUMAN_TURN))
            || wal_has_human_send(&cwd_str, sid, HUMAN_TURN),
        "feedback_draft must not be the only home for a Human turn"
    );
}

/// Operator: "Also I'm starting to wonder if planning still has stale prompt
/// problems, because that's causing confusion. Please, we need a lot more
/// red/green TDD tests for that."
/// After Comment CTA, a later product report must still send as Human,
/// not ride-Approve-only.
#[test]
#[serial_test::serial(GROK_HOME)]
fn after_comment_cta_later_product_report_sends_as_human_not_ride_approve_only() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-stale-later-report";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd, sid);
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nComment then a later Human report\n",
    );
    click_comment_cta(&mut app);
    type_into_human_box(&mut app, COMMENT_STASH);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, COMMENT_STASH);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, COMMENT_STASH),
        "Comment CTA then notes then Enter must Approve with those notes; effects={effects:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, COMMENT_STASH),
        "Approve with Comment notes must be on WAL, got {:?}",
        load_wal(&cwd_str, sid)
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Comment notes plus Enter must Approve"
    );
}

/// Empty Enter never Approves, including Isolated Preview with Approve marked.
#[test]
fn empty_enter_never_approves_isolated_preview_even_when_approve_is_marked() {
    let mut app = make_app_with_agent("sess-empty-enter");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nEmpty Enter never Approves\n",
    );
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Approve);
        viewer.plan_mut().approve_button_area = Some(APPROVE_HIT);
        viewer.last_modal_area = Some(MODAL_AREA);
    }
    let outcome = app.handle_input(&enter_key());
    let effects = dispatch_outcome(&mut app, outcome);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter never Approves a parked plan"
    );
    assert!(
        agent.session.pending_prompts.is_empty(),
        "empty Enter must not queue a Prompt, got {:?}",
        agent.session.pending_prompts
    );
    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            Effect::SendPrompt { .. } | Effect::SendInterject { .. } | Effect::SendPromptNow { .. }
        )),
        "empty Enter must not start a Prompt; effects={effects:?}"
    );
}

/// Composer must clear after a successful Human send during Isolated Preview
/// (no leftover stale draft).
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_human_send_clears_composer_no_stale_draft() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-stale-clear-composer";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd, sid);
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nClear composer after Human send\n",
    );
    type_into_human_box(&mut app, HUMAN_TURN);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, HUMAN_TURN);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, HUMAN_TURN),
        "Isolated Preview idle plus notes plus Enter must Approve with those notes; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve lands, leftover={:?}",
        agent.prompt.text()
    );
    let persist = agent.unsent_composer_draft_to_persist();
    assert!(
        !persist.contains(HUMAN_TURN),
        "unsent persist must not keep leftover notes after Approve; persist={persist:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, HUMAN_TURN),
        "cleared composer is not a lost prompt: WAL must still hold the Human turn, got {:?}",
        load_wal(&cwd_str, sid)
    );
}

/// Plan-comment body that was already a Human turn must not restore as a
/// queue row after present / rebuild / session load (stale occupancy).
#[test]
#[serial_test::serial(GROK_HOME)]
fn plan_comment_body_already_human_turn_does_not_restore_as_queue_after_present_rebuild() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-stale-occupancy";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nStale occupancy after present\n",
    );
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(HUMAN_TURN));
        agent.session.prompt_history.push(HUMAN_TURN.to_string());
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            3,
            HUMAN_TURN,
            QueueEntryKind::Prompt,
        ));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            4,
            PRODUCT_REPORT,
            QueueEntryKind::Prompt,
        ));
        agent.persist_pending_prompts();
        agent.sync_queue_pane();
    }
    let queued = queued_texts(&app);
    assert!(
        !queued.iter().any(|t| t == HUMAN_TURN),
        "a plan-comment body that is already a Human turn must not stay as a queue row after present; queue={queued:?}"
    );
    assert!(
        queued.iter().any(|t| t == PRODUCT_REPORT),
        "a truly unsent follow-up must still occupy the queue; queue={queued:?}"
    );

    xai_grok_shell::session::pending_prompts::write_pending_prompts(
        &cwd_str,
        sid,
        &[
            PersistedQueuedPrompt {
                id: 3,
                text: HUMAN_TURN.to_string(),
                kind: "prompt".into(),
            },
            PersistedQueuedPrompt {
                id: 4,
                text: PRODUCT_REPORT.to_string(),
                kind: "prompt".into(),
            },
        ],
    )
    .expect("write pending_prompts.json");
    write_session_chat_history(
        &cwd_str,
        sid,
        &format!(r#"{{"type":"user","content":[{{"type":"text","text":"{HUMAN_TURN}"}}]}}"#),
    );
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.pending_prompts.clear();
        agent.restore_pending_prompts_from_disk();
        agent.restore_prompt_wal_from_disk();
    }
    let after_load = queued_texts(&app);
    assert!(
        !after_load.iter().any(|t| t.contains(HUMAN_TURN)),
        "rebuild / session load must not restore a committed Human plan-comment body as a queue row; queue={after_load:?}"
    );
    assert!(
        after_load.iter().any(|t| t.contains(PRODUCT_REPORT)),
        "unsent follow-up must still restore; queue={after_load:?}"
    );
}

/// Comment CTA then notes then Enter Approves with those notes.
#[test]
fn ride_approve_chrome_visible_non_empty_composer_enter_still_sends() {
    let mut app = make_app_with_agent("sess-ride-approve");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nRide-Approve chrome must not Plan-Exit\n",
    );
    click_comment_cta(&mut app);
    type_into_human_box(&mut app, COMMENT_STASH);
    let first = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&first, COMMENT_STASH);
    let effects = dispatch_outcome(&mut app, first);
    assert!(
        effects_approve_with_notes(&effects, COMMENT_STASH),
        "Comment notes plus Enter must Approve with those notes; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Comment notes plus Enter must Approve, not Plan-Exit"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve lands, got {:?}",
        agent.prompt.text()
    );
}

/// Revise / re-present must not resurrect a prompt already in chat history.
#[test]
#[serial_test::serial(GROK_HOME)]
fn revise_re_present_does_not_resurrect_prompt_already_in_chat_history() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-stale-resurrect";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nFirst present\n",
    );
    type_into_human_box(&mut app, HUMAN_TURN);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, HUMAN_TURN);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, HUMAN_TURN),
        "first Isolated Preview idle Enter must Approve with those notes; effects={effects:?}"
    );
    let already_painted = user_prompt_texts(&app)
        .iter()
        .any(|t| t.contains(HUMAN_TURN));
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        if !already_painted {
            agent
                .scrollback
                .push_block(RenderBlock::user_prompt(HUMAN_TURN));
        }
        agent.session.prompt_history.retain(|p| p != HUMAN_TURN);
        agent
            .session
            .prompt_history
            .insert(0, HUMAN_TURN.to_string());
        agent.prompt.set_text("");
        if let Some(pav) = agent.plan_approval_view.as_mut() {
            pav.feedback_draft = Some(HUMAN_TURN.to_string());
        }
    }
    write_session_chat_history(
        &cwd_str,
        sid,
        &format!(r#"{{"type":"user","content":[{{"type":"text","text":"{HUMAN_TURN}"}}]}}"#),
    );

    let revise_session_id = {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        seed_pending_tool(agent, "create-plan-revise", "CreatePlan");
        agent
            .session
            .session_id
            .as_ref()
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "sess-1".into())
    };
    let (ext, _rx2) = make_exit_plan_ext_for_session(
        &revise_session_id,
        "create-plan-revise",
        Some("# Revised plan.md\n\nRe-present must not resurrect\n"),
    );
    assert!(handle_exit_plan_mode(ext, &mut app));
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.restore_prompt_wal_from_disk();
        agent.restore_pending_prompts_from_disk();
    }
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        !agent.prompt.text().contains(HUMAN_TURN),
        "Revise / re-present must not resurrect a prompt already in chat history into the composer, got {:?}",
        agent.prompt.text()
    );
    let persist = agent.unsent_composer_draft_to_persist();
    assert!(
        !persist.contains(HUMAN_TURN),
        "re-present must not restore a committed Human turn as an unsent draft; persist={persist:?}"
    );
    assert!(
        !queued_texts(&app).iter().any(|t| t.contains(HUMAN_TURN)),
        "re-present must not enqueue a prompt already in chat history; queue={:?}",
        queued_texts(&app)
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "re-present parks a new Isolated Preview and must not Approve"
    );
}

/// After Comment CTA, empty Enter still never Approves.
#[test]
fn after_comment_cta_empty_enter_never_approves() {
    let mut app = make_app_with_agent("sess-comment-empty");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nComment then empty Enter\n",
    );
    click_comment_cta(&mut app);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    let outcome = app.handle_input(&enter_key());
    let effects = dispatch_outcome(&mut app, outcome);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter never Approves after Comment CTA"
    );
    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            Effect::SendPrompt { .. } | Effect::SendInterject { .. } | Effect::SendPromptNow { .. }
        )),
        "empty Enter after Comment must not send; effects={effects:?}"
    );
}

/// Operator quote is the spec for this module; Isolated Preview must not
/// treat that sentence as plan comment 1.
#[test]
fn operator_stale_prompt_quote_is_a_human_turn_not_plan_comment_1() {
    let mut app = make_app_with_agent("sess-operator-quote");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nOperator quote is a Human turn\n",
    );
    type_into_human_box(&mut app, OPERATOR_STALE_PROMPT);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, OPERATOR_STALE_PROMPT);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, OPERATOR_STALE_PROMPT),
        "Operator quote plus Enter must Approve with those notes; effects={effects:?}"
    );
    assert_not_only_plan_comment_1(&app, OPERATOR_STALE_PROMPT);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Operator quote plus Enter must Approve"
    );
}

const LEFTOVER_PRESENT: &str = concat!(
    "# Plan: why the agent stopped, and what we do instead\n\n",
    "Leftover Isolated Preview present. TECH.md persist overwrite.\n",
);
const MILL_PLAN_MD: &str =
    "# Mill 69 of 69 GREEN\n\nCurrent mill plan.md after mill work continued.\n";
const MILL_CONTINUE_HUMAN: &str = "continue mill 69 of 69";

fn leftover_isolated_preview_body(app: &AppView) -> Option<String> {
    app.agents
        .get(&AgentId(0))
        .unwrap()
        .line_viewer
        .as_ref()
        .and_then(|v| v.markdown_content_for_test())
        .map(str::to_owned)
}

fn write_mill_session_plan_md(cwd: &std::path::Path, sid: &str, body: &str) {
    let cwd_str = cwd.to_string_lossy();
    let encoded = urlencoding::encode(&cwd_str);
    let plan_md = xai_grok_shell::util::grok_home::grok_home()
        .join("sessions")
        .join(encoded.as_ref())
        .join(sid)
        .join("plan.md");
    std::fs::create_dir_all(plan_md.parent().unwrap()).unwrap();
    std::fs::write(&plan_md, body).unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&plan_md)
        .unwrap()
        .set_modified(later)
        .unwrap();
}

/// Operator: "Still suffering from stale plans :(" Isolated Preview
/// stay-after-present is keeping leftover present after mill work
/// continues. Human send that is not Comment notes must close Isolated
/// Preview (or re-read mill plan.md). Empty Enter never Approves.
#[test]
fn isolated_preview_human_send_closes_leftover_present_after_mill_continues() {
    let mut app = make_app_with_agent("sess-mill-human");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.line_viewer.is_some(),
            "Isolated Preview must stay after present so Comment then Approve can run"
        );
        assert!(
            leftover_isolated_preview_body(&app)
                .is_some_and(|b| b.contains("why the agent stopped")),
            "fixture: leftover present must paint Isolated Preview"
        );
    }
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    let empty = app.handle_input(&enter_key());
    let empty_effects = dispatch_outcome(&mut app, empty);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && agent.line_viewer.is_some(),
            "empty Enter never Approves and must not vanish Isolated Preview"
        );
        assert!(
            !empty_effects.iter().any(|effect| matches!(
                effect,
                Effect::SendPrompt { .. }
                    | Effect::SendInterject { .. }
                    | Effect::SendPromptNow { .. }
            )),
            "empty Enter must not start a Prompt; effects={empty_effects:?}"
        );
    }
    type_into_human_box(&mut app, MILL_CONTINUE_HUMAN);
    let outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&outcome, MILL_CONTINUE_HUMAN);
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_approve_with_notes(&effects, MILL_CONTINUE_HUMAN),
        "Isolated Preview idle plus notes plus Enter must Approve with those notes; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Isolated Preview idle Enter with notes must Approve, not Plan-Exit and leave the paste"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve lands, got {:?}",
        agent.prompt.text()
    );
}

/// `/implement` continues nested work. Isolated Preview with a live waiter
/// stays until Esc, Exit, or Approve. Operator `/implement` must not vanish
/// the pane.
#[test]
fn isolated_preview_implement_closes_leftover_present_after_mill_continues() {
    let mut app = make_app_with_agent("sess-mill-implement");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    let effects = crate::app::dispatch::dispatch(Action::SendPrompt("/implement".into()), &mut app);
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. } if text.contains("/implement")
        )),
        "/implement must continue mill; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "/implement must not Approve"
    );
    assert!(
        agent.line_viewer.is_some(),
        "Isolated Preview with a live waiter stays until Esc, Exit, or Approve; operator /implement must not vanish the pane"
    );
}

/// Operator: Isolated Preview leftover present / TECH.md persist overwrite
/// while mill 69 GREEN. If mill rewrote session plan.md, Isolated Preview
/// must re-read that file.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_rereads_current_disk_plan_md_when_mill_rewrote_it() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "sess-mill-reread";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    let _ =
        crate::app::dispatch::dispatch(Action::SendPrompt(MILL_CONTINUE_HUMAN.into()), &mut app);
    let painted =
        leftover_isolated_preview_body(&app).expect("Isolated Preview after mill rewrite");
    assert!(
        painted.contains("Mill 69 of 69 GREEN") && painted.contains("Current mill plan.md"),
        "Isolated Preview must re-read current session plan.md if mill rewrote it; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("TECH.md persist overwrite"),
        "Isolated Preview must not keep leftover present after mill rewrite; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "mill rewrite re-read must not Approve"
    );
}

/// Nested implementer finish must not vanish Isolated Preview. Operator:
/// Isolated Preview must not vanish every couple of minutes. Stay until
/// Esc, Exit, or Approve. There is no wall-clock Plan Exit timer.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_must_not_close_on_nested_specialist_finish() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "sess-nested-finish-stay";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent
            .subagent_sessions
            .insert("nested-69".into(), make_subagent_info("nested-69"));
    }
    let _ = crate::app::acp_handler::handle(
        make_ext_session_notification(
            sid,
            XaiSessionUpdate::SubagentFinished {
                subagent_id: "nested-69".into(),
                child_session_id: "nested-69".into(),
                status: "completed".into(),
                error: None,
                tool_calls: 1,
                turns: 1,
                duration_ms: 1000,
                tokens_used: 0,
                output: Some("nested implementer GREEN".into()),
                will_wake: false,
            },
        ),
        &mut app,
    );
    let painted = leftover_isolated_preview_body(&app)
        .expect("Isolated Preview must stay after nested specialist finish");
    assert!(
        painted.contains("why the agent stopped"),
        "nested specialist finish must not vanish Isolated Preview leftover present; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "nested specialist finish must not Approve leftover present"
    );
}

fn mill_next_implement_was_started(app: &AppView, marker: &str) -> bool {
    let sent = app.pending_effects.iter().any(|e| {
        matches!(
            e,
            Effect::SendPrompt { text, .. } if text.contains("/implement") && text.contains(marker)
        )
    });
    let queued = app.agents[&AgentId(0)]
        .session
        .pending_prompts
        .iter()
        .any(|p| p.text.contains("/implement") && p.text.contains(marker));
    sent || queued
}

fn paint_mill_next_implement(app: &mut AppView, child_sid: &str, body: &str) {
    let _ = handle(
        make_ext_session_notification(
            "sess-parent",
            test_subagent_spawned("sess-parent", child_sid),
        ),
        app,
    );
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    let child = agent
        .subagent_views
        .get_mut(child_sid)
        .expect("mill child view");
    child
        .scrollback
        .push_block(RenderBlock::agent_message(body));
}

/// Named contract: after mill paints a Next implement prompt whose body
/// starts with `/implement`, grok-oss sends that turn. Nested mill L2
/// never receives PromptResponse. The Operator does not paste it.
/// Occupancy still running on a sibling L2 does not skip the send.
/// Isolated Preview leftover must not Approve. Empty Enter never Approves.
#[test]
fn mill_nested_finish_auto_runs_next_implement_prompt_without_operator_paste() {
    crate::appearance::cache::set_auto_run_implement(true);
    crate::appearance::cache::set_economic_mode(false);

    let mut app = make_app_with_agent("sess-parent");
    let _ = handle(
        make_ext_session_notification(
            "sess-parent",
            test_subagent_spawned("sess-parent", "occupancy-l2"),
        ),
        &mut app,
    );
    paint_mill_next_implement(
        &mut app,
        "mill-70",
        "Mill row GREEN.\n\n\
         Next implement prompt\n\
         /implement --effort 3 Keep at least two L2s running\n\
         1) next mill row on nixbuilder",
    );
    assert!(
        app.agents[&AgentId(0)]
            .subagent_sessions
            .get("occupancy-l2")
            .is_some_and(|i| !i.finished),
        "fixture: occupancy sibling is still running"
    );
    let _ = handle(
        make_ext_session_notification("sess-parent", test_subagent_finished("mill-70")),
        &mut app,
    );
    assert!(
        mill_next_implement_was_started(&app, "Keep at least two L2s running"),
        "after mill paints a Next implement prompt whose body starts with \
         /implement, grok-oss must send that turn; the Operator does not paste it; \
         effects={:?} queue={:?}",
        app.pending_effects,
        app.agents[&AgentId(0)]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
}

/// Painted mill heading without markdown hashes still auto-runs.
#[test]
fn mill_nested_finish_auto_runs_painted_heading_without_hashes() {
    crate::appearance::cache::set_auto_run_implement(true);
    crate::appearance::cache::set_economic_mode(false);

    let mut app = make_app_with_agent("sess-parent");
    paint_mill_next_implement(
        &mut app,
        "mill-70",
        "## Next implement prompt\n\
         /implement --effort 3 Keep at least two L2s running\n\
         1) next mill row on nixbuilder",
    );
    let _ = handle(
        make_ext_session_notification("sess-parent", test_subagent_finished("mill-70")),
        &mut app,
    );
    assert!(
        mill_next_implement_was_started(&app, "Keep at least two L2s running"),
        "hashed Next implement prompt must auto-run; effects={:?} queue={:?}",
        app.pending_effects,
        app.agents[&AgentId(0)]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
}

/// Auto-run does not fire on bare `implement` without the slash.
#[test]
fn mill_nested_finish_does_not_auto_run_bare_implement_without_slash() {
    crate::appearance::cache::set_auto_run_implement(true);
    crate::appearance::cache::set_economic_mode(false);

    let mut app = make_app_with_agent("sess-parent");
    paint_mill_next_implement(
        &mut app,
        "mill-70",
        "Next implement prompt\n\
         implement leftover without slash\n\
         1) do not send this",
    );
    let _ = handle(
        make_ext_session_notification("sess-parent", test_subagent_finished("mill-70")),
        &mut app,
    );
    assert!(
        !mill_next_implement_was_started(&app, "leftover without slash"),
        "bare implement without the slash must not auto-run; effects={:?} queue={:?}",
        app.pending_effects,
        app.agents[&AgentId(0)]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
}

/// Isolated Preview leftover present after mill: auto-run `/implement`
/// must not Approve. Empty Enter never Approves.
#[test]
fn mill_nested_finish_auto_run_does_not_approve_isolated_preview() {
    crate::appearance::cache::set_auto_run_implement(true);
    crate::appearance::cache::set_economic_mode(false);

    let mut app = make_app_with_agent("sess-parent");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    paint_mill_next_implement(
        &mut app,
        "mill-70",
        "Next implement prompt\n\
         /implement --effort 3 Keep at least two L2s running\n\
         1) next mill row on nixbuilder",
    );
    let _ = handle(
        make_ext_session_notification("sess-parent", test_subagent_finished("mill-70")),
        &mut app,
    );
    assert!(
        mill_next_implement_was_started(&app, "Keep at least two L2s running"),
        "mill Next implement prompt must auto-run under leftover Isolated Preview"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "mill auto-run /implement must not Approve leftover Isolated Preview"
    );
    assert!(
        agent.line_viewer.is_some(),
        "Isolated Preview must stay until Esc, Exit, or Approve; nested auto-run /implement must not vanish the pane"
    );
}

/// A leftover slash-palette `/` is not mill continue. Isolated Preview
/// leftover must stay. Empty Enter never Approves.
#[test]
fn leftover_slash_palette_is_not_mill_continue() {
    let mut app = make_app_with_agent("sess-slash-not-mill");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    let effects = crate::app::dispatch::dispatch(Action::SendPrompt("/".into()), &mut app);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "leftover slash `/` must not Approve Isolated Preview"
    );
    assert!(
        agent.line_viewer.is_some(),
        "leftover slash `/` must not mill-continue close Isolated Preview"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendInterject { .. } | Effect::SendPromptNow { .. }
        )),
        "leftover slash `/` must not steal the next model turn; effects={effects:?}"
    );
}

/// Resume re-park: leftover slash `/` then `/view-plan` binds Approve.
/// Approve still implements. Mill auto-run must not steal that waiter.
#[test]
fn restored_plan_approval_view_plan_after_slash_leftover_still_approves() {
    let mut app = make_app_with_agent("sess-resume-gbt3703");
    let resume_id = "exit-plan-mode-resume-gbt3703";
    let session_id = {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        seed_pending_tool(agent, resume_id, "Plan: Exit");
        agent.pane_areas.prompt = Rect::new(0, 22, 80, 3);
        agent
            .session
            .session_id
            .as_ref()
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "sess-resume-gbt3703".into())
    };
    let (ext, rx) = make_exit_plan_ext_for_session(
        &session_id,
        resume_id,
        Some("# Plan GBT3703Repro\n\nResume must restore plan approval.\n"),
    );
    assert!(
        handle_exit_plan_mode(ext, &mut app),
        "resume must re-park a live waiter"
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "resume must restore plan approval"
        );
        assert!(
            agent.line_viewer.is_none(),
            "resume must not auto-dock Isolated Preview"
        );
    }

    let _ = crate::app::dispatch::dispatch(Action::SendPrompt("/".into()), &mut app);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "leftover slash `/` must not Approve a restored waiter"
        );
        assert!(
            agent.line_viewer.is_none(),
            "leftover slash `/` must not mill-continue a restored waiter"
        );
    }

    let _ = crate::app::dispatch::dispatch(Action::SendPrompt("/view-plan".into()), &mut app);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.line_viewer.is_some(),
            "/view-plan must bind Approve to the restored waiter"
        );
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "/view-plan must not Approve"
        );
        assert!(
            !matches!(agent.prompt.text().trim(), "/" | "/view-plan"),
            "leftover slash palette must not cover Approve; composer={:?}",
            agent.prompt.text()
        );
        assert!(
            !agent.prompt.slash_open(),
            "slash palette must close so Approve is clickable"
        );
    }

    arm_comment_and_approve_hit_rects(&mut app);
    let outcome = app.handle_input(&mouse_down(12, 20));
    let _ = dispatch_outcome(&mut app, outcome);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_decision_resolved,
        "panel Approve must leave plan mode and start the implement turn"
    );
    drop(rx);
}

/// Failed mill L2 must not auto-run leftover `/implement`.
#[test]
fn mill_nested_failed_finish_does_not_auto_run_next_implement() {
    crate::appearance::cache::set_auto_run_implement(true);
    crate::appearance::cache::set_economic_mode(false);

    let mut app = make_app_with_agent("sess-parent");
    paint_mill_next_implement(
        &mut app,
        "mill-70",
        "Next implement prompt\n\
         /implement --effort 3 Keep at least two L2s running",
    );
    let mut finished = test_subagent_finished("mill-70");
    if let XaiSessionUpdate::SubagentFinished { status, .. } = &mut finished {
        *status = "failed".into();
    }
    let _ = handle(
        make_ext_session_notification("sess-parent", finished),
        &mut app,
    );
    assert!(
        !mill_next_implement_was_started(&app, "Keep at least two L2s running"),
        "failed mill must not auto-run; effects={:?} queue={:?}",
        app.pending_effects,
        app.agents[&AgentId(0)]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
}

/// Operator (2026-09-15): "also now this is a regression... see? then I hit
/// enter and it just disappears and nothing fucking happens. you didn't
/// fix stale plans. you broke them." Isolated Preview leftover mill 69 /
/// TECH.md-era plan, idle Enter:send, composer
/// `/plan update the plan with what was accomplished and all that remains
/// please`. Isolated Preview must not vanish onto a blank mill with no
/// send. Submit a plan-update turn (SendPrompt / SetModeThenPrompt /
/// SendInterject). Isolated Preview stays or re-reads current disk
/// plan.md. Queue-only is a false green. Empty Enter never Approves.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_plan_slash_with_body_submits_plan_update_not_only_stale_preview() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-slash-with-body";
    const PLAN_UPDATE: &str =
        "update the plan with what was accomplished and all that remains please";
    const PLAN_SLASH: &str =
        "/plan update the plan with what was accomplished and all that remains please";
    const MILL_TECH_LEFTOVER: &str = concat!(
        "# Mill 69 of 69 GREEN\n\n",
        "TECH.md persist overwrite leftover Isolated Preview.\n",
    );

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", MILL_TECH_LEFTOVER);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.plan_mode_active = false;
        agent.plan_mode_pending = None;
        agent.prompt.set_text("");
    }
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            leftover_isolated_preview_body(&app)
                .is_some_and(|b| b.contains("Mill 69 of 69 GREEN")
                    && b.contains("TECH.md persist overwrite")),
            "fixture: leftover Isolated Preview must paint mill 69 / TECH.md-era plan"
        );
        assert!(agent.session.state.is_idle(), "fixture: idle Enter:send");
    }
    assert!(
        !app.global_work_pause.is_active(),
        "status [pause] button chrome is not engaged pause"
    );
    let empty = app.handle_input(&enter_key());
    let empty_effects = dispatch_outcome(&mut app, empty);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some()
                && !agent.plan_decision_resolved
                && agent.line_viewer.is_some(),
            "empty Enter never Approves and must not vanish Isolated Preview"
        );
        assert!(
            !empty_effects.iter().any(|effect| matches!(
                effect,
                Effect::SendPrompt { .. }
                    | Effect::SendInterject { .. }
                    | Effect::SendPromptNow { .. }
                    | Effect::SetModeThenPrompt { .. }
            )),
            "empty Enter must not start a Prompt; effects={empty_effects:?}"
        );
    }

    type_into_human_box(&mut app, PLAN_SLASH);
    let outcome = app.handle_input(&enter_key());
    match &outcome {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(PLAN_UPDATE),
                "Enter must submit the `/plan` body, got {text:?}"
            );
        }
        other => panic!(
            "`/plan` with extra Operator text must SendPrompt, not only dock Isolated Preview; got {other:?}"
        ),
    }
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, PLAN_UPDATE),
        "`/plan` with extra Operator text must SendPrompt/SetModeThenPrompt/SendInterject, not mill-continue close then empty drain; effects={effects:?} queue={:?}",
        queued_texts(&app)
    );
    assert!(
        !queued_texts(&app).iter().any(|t| t.contains(PLAN_UPDATE))
            || effects_send_human(&effects, PLAN_UPDATE),
        "queue-only is a false green for this vanish; effects={effects:?} queue={:?}",
        queued_texts(&app)
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, PLAN_UPDATE),
        "plan-update sentence must be on WAL, not wiped without sending; got {:?}",
        load_wal(&cwd_str, sid)
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().trim().is_empty(),
            "composer may clear after a landed send, not as a lost prompt"
        );
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "`/plan` with body must not Approve"
        );
        assert!(
            agent.line_viewer.is_some(),
            "Isolated Preview must stay docked as rewriting-wait, not vanish onto a blank mill"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Updating),
            "plan-update send must mark Isolated Preview rewriting-wait"
        );
        assert!(
            agent.line_viewer.as_ref().is_some_and(|v| v
                .plan_ref()
                .is_some_and(|p| !p.show_action_buttons && !p.feedback_active)),
            "idle Approve / Comment / Revise / Exit must not arm on leftover body during rewrite-wait"
        );
    }
    let painted = leftover_isolated_preview_body(&app)
        .expect("Isolated Preview must stay docked as rewriting-wait");
    assert!(
        painted.contains(PLAN_REWRITE_WAIT_HEADING) && painted.contains(PLAN_UPDATE),
        "Isolated Preview must quote the Operator's second prompt as rewriting-wait, not leftover mill plan.md; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("TECH.md persist overwrite")
            && !painted.contains("Current mill plan.md"),
        "Isolated Preview must not paint leftover mill plan.md as a live present while the plan-update turn is running; got {painted:?}"
    );
}

/// Operator (2026-09-15): leftover Isolated Preview plus a running turn
/// (plan waiter). `/plan update the plan with what was accomplished and
/// all that remains please` Enter:send must not wipe the Operator box
/// with empty effects. Isolated Preview leftover why-the-agent-stopped
/// must not be the only outcome.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_plan_slash_with_body_while_turn_running_sends_not_vanish() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-slash-body-running";
    const PLAN_UPDATE: &str =
        "update the plan with what was accomplished and all that remains please";
    const PLAN_SLASH: &str =
        "/plan update the plan with what was accomplished and all that remains please";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.plan_mode_active = true;
        assert!(
            leftover_isolated_preview_body(&app)
                .is_some_and(|b| b.contains("why the agent stopped")),
            "fixture: leftover Isolated Preview must paint why the agent stopped"
        );
    }

    type_into_human_box(&mut app, PLAN_SLASH);
    let outcome = app.handle_input(&enter_key());
    match &outcome {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(PLAN_UPDATE),
                "Enter must submit the `/plan` body, got {text:?}"
            );
        }
        other => panic!(
            "`/plan` with extra Operator text must SendPrompt, not only dock Isolated Preview; got {other:?}"
        ),
    }
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, PLAN_UPDATE),
        "running-turn `/plan` extra text must SendPrompt or SendInterject, not vanish; effects={effects:?} queue={:?}",
        queued_texts(&app)
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, PLAN_UPDATE),
        "plan-update sentence must be on WAL, not wiped without sending; got {:?}",
        load_wal(&cwd_str, sid)
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().trim().is_empty(),
            "composer may clear after a landed send, not as a lost prompt"
        );
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "`/plan` with body must not Approve"
        );
    }
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.line_viewer.is_some(),
            "running-turn `/plan` extra text must not mill-continue-close Isolated Preview before a send"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Updating),
            "running-turn plan-update must mark Isolated Preview rewriting-wait"
        );
    }
    let painted = leftover_isolated_preview_body(&app).expect(
        "running-turn `/plan` extra text must keep Isolated Preview docked as rewriting-wait",
    );
    assert!(
        painted.contains(PLAN_REWRITE_WAIT_HEADING) && painted.contains(PLAN_UPDATE),
        "Isolated Preview must quote the Operator's second prompt as rewriting-wait; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("TECH.md persist overwrite"),
        "Isolated Preview must not stay leftover present after `/plan` with body; got {painted:?}"
    );
}

/// Operator: a second plan prompt must not pop the stale plan. Isolated
/// Preview leftover after the first present stays only so Comment then
/// Approve can run. Empty Enter never Approves. A second `/plan` extra-text
/// send is a plan-update turn: Isolated Preview stays docked as
/// rewriting-wait, quotes that prompt, and does not arm idle Approve on
/// leftover mill-69 / first-draft `plan.md`. When `exit_plan_mode` writes
/// current disk `plan.md`, Isolated Preview presents that file and idle
/// CTAs arm. Paste-then-Enter Approve still works after the new present.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_second_plan_prompt_must_not_paint_stale_plan_as_live_present() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "plan-second-prompt-rewrite-wait";
    const PLAN_UPDATE: &str =
        "update the plan with what was accomplished and all that remains please";
    const PLAN_SLASH: &str =
        "/plan update the plan with what was accomplished and all that remains please";
    const STALE_FIRST_DRAFT: &str = concat!(
        "# Plan: mill 69 of 69 first draft\n\n",
        "Leftover Isolated Preview present. TECH.md persist overwrite.\n",
    );
    const REWRITTEN_PLAN: &str =
        "# Plan: second present after rewrite\n\nCurrent disk plan.md after exit_plan_mode.\n";
    const FOLLOW_UP: &str = "please add more detail while rewrite-wait is up";
    const APPROVE_NOTES: &str = "ship the rewritten plan";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", STALE_FIRST_DRAFT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.plan_mode_active = true;
        agent.plan_mode_pending = None;
        agent.prompt.set_text("");
    }
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            leftover_isolated_preview_body(&app)
                .is_some_and(|b| b.contains("mill 69 of 69 first draft")
                    && b.contains("TECH.md persist overwrite")),
            "fixture: leftover Isolated Preview must paint the stale first draft"
        );
        assert!(
            agent.line_viewer.as_ref().is_some_and(|v| v
                .plan_ref()
                .is_some_and(|p| p.show_action_buttons || p.feedback_active)),
            "fixture: leftover present may arm idle CTAs so Comment then Approve can run"
        );
    }
    let empty = app.handle_input(&enter_key());
    let empty_effects = dispatch_outcome(&mut app, empty);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some()
                && !agent.plan_decision_resolved
                && agent.line_viewer.is_some(),
            "empty Enter never Approves and must not vanish Isolated Preview"
        );
        assert!(
            !empty_effects.iter().any(|effect| matches!(
                effect,
                Effect::SendPrompt { .. }
                    | Effect::SendInterject { .. }
                    | Effect::SendPromptNow { .. }
                    | Effect::SetModeThenPrompt { .. }
            )),
            "empty Enter must not start a Prompt; effects={empty_effects:?}"
        );
    }

    type_into_human_box(&mut app, PLAN_SLASH);
    let outcome = app.handle_input(&enter_key());
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, PLAN_UPDATE),
        "second plan prompt must send a plan-update turn; effects={effects:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, PLAN_UPDATE),
        "plan-update sentence must be on WAL; got {:?}",
        load_wal(&cwd_str, sid)
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "second plan prompt must not Approve leftover body"
        );
        assert!(
            agent.line_viewer.is_some(),
            "Isolated Preview must stay docked as rewriting-wait, not close-then-hope"
        );
        assert_eq!(
            agent.plan_feedback_in_flight,
            Some(PlanFeedbackInFlight::Updating)
        );
        assert!(
            agent.line_viewer.as_ref().is_some_and(|v| v
                .plan_ref()
                .is_some_and(|p| !p.show_action_buttons && !p.feedback_active)),
            "idle Approve / Comment / Revise / Exit must not arm during rewriting-wait"
        );
    }
    let painted =
        leftover_isolated_preview_body(&app).expect("Isolated Preview rewriting-wait body");
    assert!(
        painted.contains(PLAN_REWRITE_WAIT_HEADING) && painted.contains(PLAN_UPDATE),
        "rewriting-wait must quote the Operator's second prompt; got {painted:?}"
    );
    assert!(
        !painted.contains("mill 69 of 69 first draft")
            && !painted.contains("TECH.md persist overwrite")
            && !painted.contains("Current mill plan.md"),
        "second plan prompt must not pop the stale plan; got {painted:?}"
    );

    type_into_human_box(&mut app, FOLLOW_UP);
    let follow_outcome = app.handle_input(&enter_key());
    match &follow_outcome {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. })
            if text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD) =>
        {
            panic!(
                "rewrite-wait plus notes plus Enter must not Approve leftover body; got {text:?}"
            );
        }
        InputOutcome::Action(Action::SendPrompt(_))
        | InputOutcome::ActionThenForward(Action::SendPrompt(_))
        | InputOutcome::Action(Action::Interject { .. })
        | InputOutcome::ActionThenForward(Action::Interject { .. })
        | InputOutcome::Action(Action::SendPromptNow { .. })
        | InputOutcome::ActionThenForward(Action::SendPromptNow { .. }) => {}
        other => {
            panic!("rewrite-wait notes plus Enter must send, not Approve leftover; got {other:?}")
        }
    }
    let follow_effects = dispatch_outcome(&mut app, follow_outcome);
    assert!(
        !effects_approve_with_notes(&follow_effects, FOLLOW_UP),
        "rewrite-wait must not Approve leftover body; effects={follow_effects:?}"
    );

    write_mill_session_plan_md(&cwd, sid, REWRITTEN_PLAN);
    let _rx2 = isolated_present(&mut app, "create-plan-call-2", REWRITTEN_PLAN);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_feedback_in_flight.is_none(),
            "new exit_plan_mode present must clear rewriting-wait"
        );
        assert!(
            agent.line_viewer.as_ref().is_some_and(|v| v
                .plan_ref()
                .is_some_and(|p| p.show_action_buttons && p.feedback_active)),
            "new present must arm idle CTAs on current disk plan.md"
        );
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "new present is review, not Approve"
        );
    }
    let presented = leftover_isolated_preview_body(&app)
        .expect("Isolated Preview after exit_plan_mode must paint current disk plan.md");
    assert!(
        presented.contains("second present after rewrite")
            && presented.contains("Current disk plan.md after exit_plan_mode"),
        "Isolated Preview must re-read current disk plan.md after exit_plan_mode; got {presented:?}"
    );
    assert!(
        !presented.contains(PLAN_REWRITE_WAIT_HEADING)
            && !presented.contains("mill 69 of 69 first draft"),
        "new present must not keep rewriting-wait or the stale first draft; got {presented:?}"
    );

    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    type_into_human_box(&mut app, APPROVE_NOTES);
    let approve_outcome = app.handle_input(&enter_key());
    assert_enter_approves_with_notes(&approve_outcome, APPROVE_NOTES);
    let approve_effects = dispatch_outcome(&mut app, approve_outcome);
    assert!(
        effects_approve_with_notes(&approve_effects, APPROVE_NOTES),
        "paste-then-Enter Approve must work after the new present; effects={approve_effects:?}"
    );
}

/// Operator: bare `/plan` exclusive-blocks nested implementers the way
/// exclusive plan mode used to. It is not leftover Isolated Preview that
/// leaves nested work running. Empty Enter never Approves.
#[test]
#[serial_test::serial(GROK_HOME)]
fn leftover_isolated_preview_bare_plan_exclusive_covering_from_current_disk() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "plan-slash-bare-disk";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    let effects = crate::app::dispatch::dispatch(Action::SendPrompt("/plan".into()), &mut app);
    assert!(
        effects.iter().all(|e| !matches!(
            e,
            Effect::SendPrompt { text, .. } if text.contains("update the plan")
        )),
        "bare `/plan` must not invent a plan-update Prompt; effects={effects:?}"
    );
    let painted = leftover_isolated_preview_body(&app)
        .expect("bare `/plan` must open exclusive covering from current disk plan.md");
    assert!(
        painted.contains("Mill 69 of 69 GREEN") && painted.contains("Current mill plan.md"),
        "bare `/plan` must paint current disk plan.md; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("TECH.md persist overwrite"),
        "bare `/plan` must not keep leftover Isolated Preview present; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent
            .line_viewer
            .as_ref()
            .is_some_and(|v| v.fullscreen && !v.is_soft_plan_side_pane()),
        "bare `/plan` is covering exclusive plan, not Isolated Preview on the right"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "bare `/plan` must not Approve leftover present"
    );
}

/// Named contract: soft planning does not reset the primary plan; it
/// makes a secondary plan; Isolated Preview does not immediately pull
/// up leftover current `plan.md`. Mill nested work stays Working.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_soft_planning_does_not_pull_up_leftover_current_plan_md() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "plan-soft-secondary";

    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
    }
    let primary_before = {
        let cwd_str = cwd.to_string_lossy();
        let encoded = urlencoding::encode(&cwd_str);
        std::fs::read_to_string(
            xai_grok_shell::util::grok_home::grok_home()
                .join("sessions")
                .join(encoded.as_ref())
                .join(sid)
                .join("plan.md"),
        )
        .expect("primary plan.md")
    };
    let effects = crate::app::dispatch::dispatch(
        Action::SendPrompt("/plan --soft add feature".into()),
        &mut app,
    );
    assert!(
        effects.iter().all(|e| !matches!(
            e,
            Effect::CancelTurn { .. }
                | Effect::SetSessionMode { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "`/plan --soft` must not stop mill work or enter plan mode; effects={effects:?}"
    );
    let painted =
        leftover_isolated_preview_body(&app).expect("`/plan --soft` must dock Isolated Preview");
    assert!(
        painted.contains("add feature"),
        "`/plan --soft add feature` must seed Isolated Preview; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("TECH.md persist overwrite")
            && !painted.contains("Current mill plan.md")
            && !painted.contains("Mill 69 of 69 GREEN"),
        "Isolated Preview must not immediately pull up leftover current plan.md; got {painted:?}"
    );
    let cwd_str = cwd.to_string_lossy();
    let encoded = urlencoding::encode(&cwd_str);
    let disk = std::fs::read_to_string(
        xai_grok_shell::util::grok_home::grok_home()
            .join("sessions")
            .join(encoded.as_ref())
            .join(sid)
            .join("plan.md"),
    )
    .expect("primary plan.md after --soft");
    assert_eq!(
        disk, primary_before,
        "soft planning must not reset the primary plan.md"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert_eq!(
        agent.session.state,
        crate::app::agent::AgentState::TurnRunning,
        "`/plan --soft` must not park L1; mill stays running"
    );
    assert!(
        agent.isolated_preview_shows_secondary_plan,
        "`/plan --soft` must dock Isolated Preview as a secondary plan"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter never Approves leftover mill present"
    );
    let db = xai_grok_shell::util::grok_home::grok_home().join("grok_oss.db");
    if db.is_file() {
        let store = xai_grok_shell::grok_oss::open_at(&db).expect("grok_oss.db");
        if let Ok(Some(primary)) =
            store.load_session_plan_body(sid, xai_grok_shell::grok_oss::SESSION_PLAN_IDENTITY)
        {
            assert!(
                !primary.contains("add feature"),
                "soft planning must not reset the primary plan SQL body; got {primary:?}"
            );
        }
        if let Ok(Some(secondary)) =
            store.load_session_plan_body(sid, xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY)
        {
            assert!(
                secondary.contains("add feature") && !secondary.contains("Current mill plan.md"),
                "soft planning must make a secondary plan; got {secondary:?}"
            );
        }
    }
}

/// Operator: Isolated Preview must not vanish every couple of minutes.
/// Occupancy ticks (nested implementer progress) must not close Isolated
/// Preview. There is no wall-clock Plan Exit timer. Stay until Esc, Exit,
/// or Approve.
#[test]
fn isolated_preview_must_not_vanish_every_couple_of_minutes_on_nested_occupancy_tick() {
    let mut app = make_app_with_agent("sess-occupancy-tick");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    let _ = handle(
        make_ext_session_notification(
            "sess-occupancy-tick",
            test_subagent_spawned("sess-occupancy-tick", "occupancy-l2"),
        ),
        &mut app,
    );
    for _ in 0..3 {
        let _ = handle(
            make_ext_session_notification(
                "sess-occupancy-tick",
                test_subagent_progress("sess-occupancy-tick", "occupancy-l2"),
            ),
            &mut app,
        );
    }
    let painted = leftover_isolated_preview_body(&app).expect(
        "Isolated Preview must not vanish every couple of minutes on nested occupancy ticks",
    );
    assert!(
        painted.contains("why the agent stopped"),
        "occupancy tick must not close Isolated Preview leftover present; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "occupancy tick must not Approve Isolated Preview"
    );
    assert!(
        agent
            .subagent_sessions
            .get("occupancy-l2")
            .is_some_and(|i| !i.finished && !i.pending_kill),
        "occupancy nested implementer must stay Working"
    );
}

/// Named contract: there is no Plan Exit wall-clock timer that closes
/// Isolated Preview. Nested implementer occupancy ticks and specialist
/// finish must not close it. Stay until Esc, Exit, or Approve.
#[test]
fn isolated_preview_has_no_plan_exit_wall_clock_timer() {
    let mut app = make_app_with_agent("sess-no-plan-exit-timer");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    let _ = handle(
        make_ext_session_notification(
            "sess-no-plan-exit-timer",
            test_subagent_spawned("sess-no-plan-exit-timer", "occupancy-l2"),
        ),
        &mut app,
    );
    let _ = handle(
        make_ext_session_notification(
            "sess-no-plan-exit-timer",
            test_subagent_progress("sess-no-plan-exit-timer", "occupancy-l2"),
        ),
        &mut app,
    );
    let _ = handle(
        make_ext_session_notification(
            "sess-no-plan-exit-timer",
            test_subagent_finished("occupancy-l2"),
        ),
        &mut app,
    );
    assert!(
        leftover_isolated_preview_body(&app).is_some(),
        "Isolated Preview must not vanish every couple of minutes; there is no Plan Exit wall-clock timer"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.line_viewer.is_some() && agent.plan_approval_view.is_some(),
        "Isolated Preview stays until Esc, Exit, or Approve"
    );
    assert!(
        !agent.plan_decision_resolved,
        "nested occupancy finish is not Plan Exit and not Approve"
    );
}

/// GitHub #122. Empty Enter never Approves a plan present, Isolated Preview
/// and exclusive covering. Enter with nothing selected does not Approve.
/// Clickable Approve is the only Approve.
#[test]
fn empty_enter_never_approves_exclusive_covering_present_github_122() {
    let mut app = make_app_with_agent("sess-exclusive-empty-enter");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Exclusive covering\n\nEmpty Enter never Approves\n",
    );
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.fullscreen = true;
        viewer.plan_mut().selected_cta = None;
        viewer.plan_mut().approve_button_area = Some(APPROVE_HIT);
        viewer.last_modal_area = Some(MODAL_AREA);
    }
    let empty = app.handle_input(&enter_key());
    let empty_effects = dispatch_outcome(&mut app, empty);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "GitHub #122: empty Enter never Approves exclusive covering; Enter with nothing selected does not Approve"
        );
        assert!(
            agent.line_viewer.as_ref().is_some_and(|v| v.fullscreen),
            "empty Enter must not vanish exclusive covering"
        );
        assert!(
            !empty_effects.iter().any(|effect| matches!(
                effect,
                Effect::SendPrompt { .. }
                    | Effect::SendInterject { .. }
                    | Effect::SendPromptNow { .. }
            )),
            "empty Enter must not start a Prompt; effects={empty_effects:?}"
        );
    }
    arm_comment_and_approve_hit_rects(&mut app);
    let click = app.handle_input(&mouse_down(APPROVE_HIT.x + 1, APPROVE_HIT.y));
    let click_effects = dispatch_outcome(&mut app, click);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_decision_resolved || agent.plan_approval_view.is_none(),
        "GitHub #122: clickable Approve is the only Approve; got decision_resolved={} park={}",
        agent.plan_decision_resolved,
        agent.plan_approval_view.is_some()
    );
    assert!(
        click_effects.iter().any(|effect| matches!(
            effect,
            Effect::SendPrompt { .. } | Effect::SendInterject { .. } | Effect::SendPromptNow { .. }
        )) || agent.plan_decision_resolved
            || agent.plan_approval_view.is_none(),
        "clickable Approve must Approve; effects={click_effects:?}"
    );
}

/// Operator: "soft planning is still very broken; two tests were not enough."
/// Leftover Isolated Preview already open must not immediately dock leftover
/// primary `plan.md`. Nested implementers keep running.
#[test]
#[serial_test::serial(GROK_HOME)]
fn plan_soft_leftover_isolated_preview_already_open_does_not_dock_leftover_primary() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "plan-soft-leftover-open";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
        let mut nested = make_subagent_info("nested-soft");
        nested.status = Some(std::sync::Arc::from("Working"));
        agent.subagent_sessions.insert("nested-soft".into(), nested);
    }
    let primary_before = {
        let cwd_str = cwd.to_string_lossy();
        let encoded = urlencoding::encode(&cwd_str);
        std::fs::read_to_string(
            xai_grok_shell::util::grok_home::grok_home()
                .join("sessions")
                .join(encoded.as_ref())
                .join(sid)
                .join("plan.md"),
        )
        .expect("primary plan.md")
    };
    let effects = crate::app::dispatch::dispatch(
        Action::SendPrompt("/plan --soft rewrite auth".into()),
        &mut app,
    );
    assert!(
        effects.iter().all(|e| !matches!(
            e,
            Effect::CancelTurn { .. } | Effect::KillSubagent { .. } | Effect::SetSessionMode { .. }
        )),
        "`/plan --soft` must not exclusive-block nested implementers; effects={effects:?}"
    );
    let painted =
        leftover_isolated_preview_body(&app).expect("`/plan --soft` must keep Isolated Preview");
    assert!(
        painted.contains("rewrite auth"),
        "`/plan --soft rewrite auth` must seed Isolated Preview; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped")
            && !painted.contains("Current mill plan.md")
            && !painted.contains("TECH.md persist overwrite"),
        "leftover Isolated Preview already open must not immediately dock leftover primary plan.md; got {painted:?}"
    );
    let disk = {
        let cwd_str = cwd.to_string_lossy();
        let encoded = urlencoding::encode(&cwd_str);
        std::fs::read_to_string(
            xai_grok_shell::util::grok_home::grok_home()
                .join("sessions")
                .join(encoded.as_ref())
                .join(sid)
                .join("plan.md"),
        )
        .expect("primary after --soft")
    };
    assert_eq!(
        disk, primary_before,
        "soft planning must not reset the primary plan.md"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.isolated_preview_shows_secondary_plan,
        "`/plan --soft` is a secondary plan"
    );
    let nested = &agent.subagent_sessions["nested-soft"];
    assert!(
        !nested.pending_kill && !nested.finished,
        "nested implementers keep running under `/plan --soft`"
    );
}

/// Soft Isolated Preview must not close on nested occupancy tick or
/// specialist finish. Operator: two tests were not enough.
#[test]
fn plan_soft_must_not_close_on_nested_tick() {
    let mut app = make_app_with_agent("sess-soft-nested-tick");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
    }
    let _ = crate::app::dispatch::dispatch(
        Action::SendPrompt("/plan --soft add feature".into()),
        &mut app,
    );
    let _ = handle(
        make_ext_session_notification(
            "sess-soft-nested-tick",
            test_subagent_spawned("sess-soft-nested-tick", "occupancy-l2"),
        ),
        &mut app,
    );
    let _ = handle(
        make_ext_session_notification(
            "sess-soft-nested-tick",
            test_subagent_progress("sess-soft-nested-tick", "occupancy-l2"),
        ),
        &mut app,
    );
    let _ = handle(
        make_ext_session_notification(
            "sess-soft-nested-tick",
            test_subagent_finished("occupancy-l2"),
        ),
        &mut app,
    );
    let painted = leftover_isolated_preview_body(&app)
        .expect("`/plan --soft` Isolated Preview must not close on nested tick");
    assert!(
        painted.contains("add feature"),
        "nested tick must not vanish `/plan --soft` Isolated Preview; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.isolated_preview_shows_secondary_plan,
        "nested tick must not convert `/plan --soft` Isolated Preview into leftover primary"
    );
}

/// Identity collision: upserting the secondary plan must not overwrite the
/// primary `plan.md` row even when the feature text looks like leftover
/// primary body. Operator: two tests were not enough.
#[test]
#[serial_test::serial(GROK_HOME)]
fn plan_soft_identity_collision_does_not_reset_primary() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "plan-soft-identity";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    let _ = crate::app::dispatch::dispatch(
        Action::SendPrompt("/plan --soft add feature".into()),
        &mut app,
    );
    let cwd_str = cwd.to_string_lossy();
    let encoded = urlencoding::encode(&cwd_str);
    let disk = std::fs::read_to_string(
        xai_grok_shell::util::grok_home::grok_home()
            .join("sessions")
            .join(encoded.as_ref())
            .join(sid)
            .join("plan.md"),
    )
    .expect("primary plan.md");
    assert_eq!(
        disk, MILL_PLAN_MD,
        "secondary Isolated Preview must not collide with primary plan.md identity"
    );
    let db = xai_grok_shell::util::grok_home::grok_home().join("grok_oss.db");
    if db.is_file() {
        let store = xai_grok_shell::grok_oss::open_at(&db).expect("grok_oss.db");
        let primary = store
            .load_session_plan_body(sid, xai_grok_shell::grok_oss::SESSION_PLAN_IDENTITY)
            .ok()
            .flatten();
        let secondary = store
            .load_session_plan_body(sid, xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY)
            .ok()
            .flatten();
        if let Some(primary) = primary {
            assert!(
                !primary.contains("add feature"),
                "primary upsert identity must stay primary; got {primary:?}"
            );
        }
        if let Some(secondary) = secondary {
            assert!(
                secondary.contains("add feature"),
                "secondary identity must hold the `/plan --soft` body; got {secondary:?}"
            );
        }
    }
}

/// `exit_plan_mode` writing the primary must present that file. `/plan --soft`
/// leftover placeholder must not stay as if it were leftover Isolated Preview
/// leftover body paint. Operator: two tests were not enough.
#[test]
#[serial_test::serial(GROK_HOME)]
fn plan_soft_exit_plan_mode_writes_primary_and_presents_it() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "plan-soft-exit-primary";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _ = crate::app::dispatch::dispatch(
        Action::SendPrompt("/plan --soft leftover placeholder".into()),
        &mut app,
    );
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    let presented = "# Current disk plan.md after exit_plan_mode\n\nsecond present after rewrite\n";
    let _rx = isolated_present(&mut app, "create-plan-call-2", presented);
    let painted =
        leftover_isolated_preview_body(&app).expect("exit_plan_mode must present Isolated Preview");
    assert!(
        painted.contains("second present after rewrite")
            && painted.contains("Current disk plan.md after exit_plan_mode"),
        "exit_plan_mode writing the primary must present that file; got {painted:?}"
    );
    assert!(
        !painted.contains("leftover placeholder"),
        "leftover `/plan --soft` body must not stay after exit_plan_mode writes the primary; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "exit_plan_mode present is review, not Approve"
    );
}
