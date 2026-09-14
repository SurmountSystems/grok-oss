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
use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};
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
        Effect::SendPrompt { text, .. } | Effect::SendInterject { text, .. } => {
            text.contains(needle)
                && !text
                    .contains(crate::views::plan_approval_view::PLAN_APPROVED_REVIEW_COMMENTS_LEAD)
        }
        Effect::SendPromptNow { .. } => true,
        _ => false,
    })
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
        matches!(r.kind, PromptWalKind::Send | PromptWalKind::Queue) && r.text.contains(needle)
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
    match &outcome {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(HUMAN_TURN),
                "Isolated Preview Human Enter must SendPrompt, got {text:?}"
            );
            assert!(
                !text
                    .contains(crate::views::plan_approval_view::PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Human send must not wrap ride-Approve review comments, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview Human sentence must be a Human turn, not only plan comment 1; got {other:?}"
        ),
    }
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, HUMAN_TURN),
        "dispatch must start a Human Prompt; effects={effects:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, HUMAN_TURN),
        "lost Isolated Preview Human sentence must be on WAL, got {:?}",
        load_wal(&cwd_str, sid)
    );
    assert_not_only_plan_comment_1(&app, HUMAN_TURN);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "Isolated Preview Human Enter must not Approve"
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
    let stash = app.handle_input(&enter_key());
    let stash_effects = dispatch_outcome(&mut app, stash);
    assert!(
        !effects_send_human(&stash_effects, COMMENT_STASH),
        "first Comment Enter may stash; must not send the comment as the only Prompt; effects={stash_effects:?}"
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
            "Comment stash must not Approve"
        );
        assert_eq!(
            agent
                .plan_approval_view
                .as_ref()
                .and_then(|p| p.feedback_draft.as_deref()),
            Some(COMMENT_STASH)
        );
    }

    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    type_into_human_box(&mut app, PRODUCT_REPORT);
    let outcome = app.handle_input(&enter_key());
    match &outcome {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(PRODUCT_REPORT),
                "later product report must SendPrompt as Human, got {text:?}"
            );
            assert!(
                !text
                    .contains(crate::views::plan_approval_view::PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "later product report must not ride-Approve-only, got {text:?}"
            );
        }
        other => panic!(
            "after Comment CTA, a later product report must still send as Human, not ride-Approve-only; got {other:?}"
        ),
    }
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, PRODUCT_REPORT),
        "later product report must dispatch as Human; effects={effects:?}"
    );
    assert!(
        wal_has_human_send(&cwd_str, sid, PRODUCT_REPORT),
        "later product report must be on WAL, got {:?}",
        load_wal(&cwd_str, sid)
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "later product report Enter must not Approve"
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
    assert!(
        matches!(
            &outcome,
            InputOutcome::Action(Action::SendPrompt(text))
                | InputOutcome::ActionThenForward(Action::SendPrompt(text))
            if text.contains(HUMAN_TURN)
        ),
        "successful Isolated Preview Human send must SendPrompt; got {outcome:?}"
    );
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, HUMAN_TURN),
        "successful Human send must dispatch; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer must clear after a successful Human send during Isolated Preview, leftover={:?}",
        agent.prompt.text()
    );
    let persist = agent.unsent_composer_draft_to_persist();
    assert!(
        !persist.contains(HUMAN_TURN),
        "unsent persist must not keep a leftover stale draft of a sent Human turn; persist={persist:?}"
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

/// Ride-Approve chrome visible + non-empty composer Enter still sends.
#[test]
fn ride_approve_chrome_visible_non_empty_composer_enter_still_sends() {
    let mut app = make_app_with_agent("sess-ride-approve");
    let _rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nRide-Approve chrome must not block send\n",
    );
    click_comment_cta(&mut app);
    type_into_human_box(&mut app, COMMENT_STASH);
    let first = app.handle_input(&enter_key());
    let _ = dispatch_outcome(&mut app, first);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert_eq!(
            agent
                .plan_approval_view
                .as_ref()
                .and_then(|p| p.feedback_draft.as_deref()),
            Some(COMMENT_STASH),
            "ride-Approve chrome is the stashed comment"
        );
        agent.prompt.set_text(COMMENT_STASH);
    }
    let second = app.handle_input(&enter_key());
    match &second {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(COMMENT_STASH),
                "non-empty Enter while ride-Approve chrome is visible must still send, got {text:?}"
            );
        }
        other => panic!(
            "ride-Approve chrome visible + non-empty composer Enter still sends; got {other:?}"
        ),
    }
    let effects = dispatch_outcome(&mut app, second);
    assert!(
        effects_send_human(&effects, COMMENT_STASH),
        "second Enter must dispatch as Human; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "non-empty Enter while ride-Approve chrome is visible must not Approve"
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
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, HUMAN_TURN),
        "first Isolated Preview Human send must dispatch; effects={effects:?}"
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
    match &outcome {
        InputOutcome::Action(Action::SendPrompt(text))
        | InputOutcome::ActionThenForward(Action::SendPrompt(text)) => {
            assert!(
                text.contains(OPERATOR_STALE_PROMPT),
                "Operator stale-prompt sentence must SendPrompt, got {text:?}"
            );
        }
        other => {
            panic!("Operator quote must be a Human turn, not only plan comment 1; got {other:?}")
        }
    }
    assert_not_only_plan_comment_1(&app, OPERATOR_STALE_PROMPT);
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "Operator quote Enter must not Approve"
    );
}

const LEFTOVER_PRESENT: &str = concat!(
    "# Plan: why the agent stopped, and what we do instead\n\n",
    "Leftover Isolated Preview present. TECH.md persist overwrite.\n",
);
const MILL_PLAN_MD: &str = "# Mill 69 of 69 GREEN\n\nCurrent mill plan.md after mill work continued.\n";
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
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects_send_human(&effects, MILL_CONTINUE_HUMAN),
        "Isolated Preview Human send must continue mill; effects={effects:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "Human send must not Approve"
    );
    assert!(
        agent.line_viewer.is_none(),
        "Isolated Preview must not stay parked on leftover present after mill work continues via Human send"
    );
}

/// `/implement` continues mill. Isolated Preview leftover present must close.
#[test]
fn isolated_preview_implement_closes_leftover_present_after_mill_continues() {
    let mut app = make_app_with_agent("sess-mill-implement");
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    let effects = crate::app::dispatch::dispatch(
        Action::SendPrompt("/implement".into()),
        &mut app,
    );
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
        agent.line_viewer.is_none(),
        "Isolated Preview must not stay parked on leftover present after /implement"
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
    type_into_human_box(&mut app, MILL_CONTINUE_HUMAN);
    let outcome = app.handle_input(&enter_key());
    let _ = dispatch_outcome(&mut app, outcome);
    let painted = leftover_isolated_preview_body(&app).expect("Isolated Preview after mill rewrite");
    assert!(
        painted.contains("Mill 69 of 69 GREEN") && painted.contains("Current mill plan.md"),
        "Isolated Preview must re-read current session plan.md if mill rewrote it; got {painted:?}"
    );
    assert!(
        !painted.contains("why the agent stopped") && !painted.contains("TECH.md persist overwrite"),
        "Isolated Preview must not keep leftover present after mill rewrite; got {painted:?}"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "mill rewrite re-read must not Approve"
    );
}

/// After mill completion, Isolated Preview must not still paint leftover
/// present / TECH.md persist overwrite while chat is mill 69 GREEN.
#[test]
#[serial_test::serial(GROK_HOME)]
fn isolated_preview_after_mill_completion_must_not_paint_leftover_present_or_tech_md() {
    let grok_home = tempfile::tempdir().expect("home");
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let proj = tempfile::tempdir().expect("cwd");
    let cwd = proj.path().to_path_buf();
    let sid = "sess-mill-green";
    let mut app = make_app_with_agent(sid);
    bind_session_home(&mut app, cwd.clone(), sid);
    let _rx = isolated_present(&mut app, "create-plan-call", LEFTOVER_PRESENT);
    write_mill_session_plan_md(&cwd, sid, MILL_PLAN_MD);
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent
            .subagent_sessions
            .insert("mill-69".into(), make_subagent_info("mill-69"));
    }
    let _ = crate::app::acp_handler::handle(
        make_ext_session_notification(
            sid,
            XaiSessionUpdate::SubagentFinished {
                subagent_id: "mill-69".into(),
                child_session_id: "mill-69".into(),
                status: "completed".into(),
                error: None,
                tool_calls: 1,
                turns: 1,
                duration_ms: 1000,
                tokens_used: 0,
                output: Some("mill 69 of 69 GREEN".into()),
                will_wake: false,
            },
        ),
        &mut app,
    );
    let painted = leftover_isolated_preview_body(&app);
    if let Some(painted) = painted {
        assert!(
            painted.contains("Mill 69 of 69 GREEN") && !painted.contains("why the agent stopped"),
            "after mill completion Isolated Preview must paint mill plan.md, not leftover present; got {painted:?}"
        );
        assert!(
            !painted.contains("TECH.md persist overwrite"),
            "after mill completion Isolated Preview must not paint TECH.md persist overwrite; got {painted:?}"
        );
    }
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "mill completion must not Approve leftover present"
    );
}
