//! `/unstick` resends the last L1 prompt as if the network dropped it.
//!
//! Surmount / grok-oss fork; tests are contracts.
//! Not `/resume`. Do not paint a second Human line. Do not unwind work.

use super::*;
use crate::app::dispatch::unstick::NO_LAST_PROMPT_TOAST;

fn user_prompt_texts(agent: &AgentView) -> Vec<String> {
    (0..agent.scrollback.len())
        .filter_map(|i| match agent.scrollback.entry(i).map(|e| &e.block) {
            Some(RenderBlock::UserPrompt(b)) => Some(b.text.clone()),
            _ => None,
        })
        .collect()
}

fn unstick_resend_text(effects: &[Effect]) -> Option<&str> {
    effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt { text, .. } | Effect::SendPrompt { text, .. } => {
            Some(text.as_str())
        }
        _ => None,
    })
}

/// Operator: resend the last L1 prompt as if the network had been interrupted.
/// Not a duplicate prompt (do not paint a second Human line).
#[test]
#[serial_test::serial(GROK_HOME)]
fn unstick_resends_last_l1_prompt_without_duplicate_human_line() {
    let grok_home = tempfile::tempdir().unwrap();
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "unstick-l1-sess";
    let cwd = grok_home.path().join("proj");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let hung = "do the hung work [Image #1]";
    let child_session = make_test_agent_session(&app, AgentId(1), "nested-overlay");
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("prompt-hung".into());
        agent.scrollback.push_block(RenderBlock::user_prompt(hung));
        let mut child = AgentView::new(child_session, ScrollbackState::new());
        child
            .scrollback
            .push_block(RenderBlock::user_prompt("nested overlay prompt"));
        agent
            .subagent_views
            .insert("nested-overlay".into(), Box::new(child));
        agent.active_subagent = Some("nested-overlay".into());
    }
    let notes = xai_grok_shell::session::prompt_wal::PromptWalRecord::new(
        sid,
        xai_grok_shell::session::prompt_wal::PromptWalKind::PlanNotes,
        "plan notes must not win",
        vec![],
    );
    let send = xai_grok_shell::session::prompt_wal::PromptWalRecord::new(
        sid,
        xai_grok_shell::session::prompt_wal::PromptWalKind::Send,
        hung,
        vec![xai_grok_shell::session::prompt_wal::PromptWalImage {
            n: 1,
            file_id: "img-1.png".into(),
        }],
    );
    xai_grok_shell::session::prompt_wal::append_prompt_wal(&cwd_str, sid, &notes)
        .expect("wal notes");
    xai_grok_shell::session::prompt_wal::append_prompt_wal(&cwd_str, sid, &send).expect("wal send");

    let effects = dispatch(Action::SendPrompt("/unstick".into()), &mut app);

    let agent = app.agents.get(&id).unwrap();
    let humans = user_prompt_texts(agent);
    assert_eq!(
        humans.iter().filter(|t| t.contains("[Image #1]")).count(),
        1,
        "must not paint a second Human line; humans={humans:?}"
    );
    assert_eq!(
        unstick_resend_text(&effects),
        Some(hung),
        "must resend last L1 prompt from WAL; effects={effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::UnstickResendPrompt { text, .. } if text.contains("nested overlay"))),
        "must not resend nested overlay text: {effects:?}"
    );
    assert!(
        !app.session_picker_loading,
        "/unstick must not open the session picker"
    );
    let hung_pid = "prompt-hung";
    let resend_pid = effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt { prompt_id, .. } => Some(prompt_id.as_str()),
        _ => None,
    });
    assert_ne!(
        resend_pid,
        Some(hung_pid),
        "unstick must mint a new prompt id so the hung RPC cannot end the retry; effects={effects:?}"
    );
    let images = effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt { images, .. } => Some(images.as_slice()),
        _ => None,
    });
    assert!(
        images.is_some_and(|imgs| imgs.iter().any(|i| i.file_id == "img-1.png" && i.n == 1)),
        "WAL image file ids must ride the unstick effect; effects={effects:?}"
    );
}

/// Operator: without unwinding any work or tokens. Do not cancel nested
/// agents, rewind tool results, drop the transcript, reset sampler usage
/// meters, or compact-away the turn.
#[test]
fn unstick_does_not_cancel_nested_subagents_or_rewind_tokens() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let hung = "finish the stuck turn";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("prompt-tokens".into());
        agent.scrollback.push_block(RenderBlock::user_prompt(hung));
        agent
            .scrollback
            .push_block(RenderBlock::tool_call("edit", "src/x.rs", true));
        agent.context_state = Some(xai_grok_shell::session::ContextInfo {
            used: 4242,
            total: 128_000,
            ..Default::default()
        });
    }
    let child = Box::new(AgentView::new(
        make_test_agent_session(&app, AgentId(1), "live-nested"),
        ScrollbackState::new(),
    ));
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.subagent_views.insert("live-nested".into(), child);
        agent.subagent_sessions.insert(
            "live-nested".into(),
            make_test_subagent("live-nested", "sa-1"),
        );
    }

    let effects = dispatch(Action::SendPrompt("/unstick".into()), &mut app);

    assert!(
        unstick_resend_text(&effects).is_some(),
        "unstick must still resend; effects={effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "must not cancel nested agents or the hung turn: {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RewindExecute { .. })),
        "must not rewind tool results: {effects:?}"
    );
    assert!(
        !effects.iter().any(|e| match e {
            Effect::UnstickResendPrompt { text, .. } | Effect::SendPrompt { text, .. } => {
                text.contains("/compact")
            }
            _ => false,
        }),
        "must not compact-away the turn: {effects:?}"
    );
    let agent = app.agents.get(&id).unwrap();
    assert!(
        agent.subagent_views.contains_key("live-nested"),
        "nested subagent view must stay"
    );
    assert!(
        !agent
            .subagent_sessions
            .get("live-nested")
            .expect("nested session")
            .pending_kill,
        "must not mark nested subagents pending kill"
    );
    assert_eq!(
        agent.context_state.as_ref().map(|c| c.used),
        Some(4242),
        "must not reset sampler usage meters"
    );
    let still_has_tool = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::ToolCall(_))
        )
    });
    assert!(
        still_has_tool,
        "must not drop the transcript or rewind tools"
    );
    assert_eq!(user_prompt_texts(agent).len(), 1);
}

/// Operator: must not conflict with resume. `/resume` / continue interrupted
/// turn stay as they are.
#[test]
fn unstick_does_not_collide_with_resume_slash() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let resume = dispatch(Action::SendPrompt("/resume".into()), &mut app);
    let resume_opens_picker = matches!(
        app.agents[&id].active_modal,
        Some(crate::views::modal::ActiveModal::SessionPicker { .. })
    ) || resume
        .iter()
        .any(|e| matches!(e, Effect::FetchSessionList { .. }));
    assert!(
        resume_opens_picker,
        "/resume must still open the session picker; effects={resume:?} session_picker={resume_opens_picker}"
    );

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("stuck parent"));
        agent.session.state = AgentState::TurnRunning;
    }
    let unstick = dispatch(Action::SendPrompt("/unstick".into()), &mut app);
    assert!(
        !matches!(
            app.agents[&id].active_modal,
            Some(crate::views::modal::ActiveModal::SessionPicker { .. })
        ),
        "/unstick must not open the session picker"
    );
    assert!(
        unstick_resend_text(&unstick).is_some(),
        "/unstick must resend, not pick a session; effects={unstick:?}"
    );
    assert!(
        !unstick
            .iter()
            .any(|e| matches!(e, Effect::FetchSessionList { .. })),
        "/unstick must not fetch the session list: {unstick:?}"
    );
}

/// Operator: if there is no last L1 prompt, fail loud with a short toast.
/// Do not invent text.
#[test]
fn unstick_with_no_last_prompt_fails_loud() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(user_prompt_texts(&app.agents[&id]).is_empty());

    let effects = dispatch(Action::SendPrompt("/unstick".into()), &mut app);

    assert!(
        unstick_resend_text(&effects).is_none(),
        "must not invent a prompt; effects={effects:?}"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::UnstickResendPrompt { .. }
        )),
        "no last prompt must not send: {effects:?}"
    );
    let toast = app.agents[&id]
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains(NO_LAST_PROMPT_TOAST) || toast.to_ascii_lowercase().contains("no last"),
        "must fail loud with a short toast; toast={toast:?}"
    );
    assert!(
        app.agents[&id].session.pending_prompts.is_empty(),
        "must not enqueue invented text"
    );
}

/// Operator: WAL image file ids resend as resource links, not only
/// `[Image #N]` text, and never as data URLs.
#[test]
#[serial_test::serial(GROK_HOME)]
fn unstick_resends_wal_images_as_resource_blocks_not_data_urls() {
    let grok_home = tempfile::tempdir().unwrap();
    let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "unstick-img-sess";
    let cwd = grok_home.path().join("proj");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let hung = "see [Image #1]";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("prompt-hung".into());
        agent.scrollback.push_block(RenderBlock::user_prompt(hung));
    }
    let send = xai_grok_shell::session::prompt_wal::PromptWalRecord::new(
        sid,
        xai_grok_shell::session::prompt_wal::PromptWalKind::Send,
        hung,
        vec![
            xai_grok_shell::session::prompt_wal::PromptWalImage {
                n: 1,
                file_id: "img-1.png".into(),
            },
            xai_grok_shell::session::prompt_wal::PromptWalImage {
                n: 2,
                file_id: "data:image/png;base64,AAAA".into(),
            },
        ],
    );
    xai_grok_shell::session::prompt_wal::append_prompt_wal(&cwd_str, sid, &send).expect("wal send");
    let session_id = app.agents[&id].session.session_id.clone().expect("sid");
    let images_dir = crate::prompt_images::session_images_dir(Some(&session_id), &cwd)
        .expect("session images dir");
    std::fs::create_dir_all(&images_dir).expect("mkdir images");
    std::fs::write(images_dir.join("img-1.png"), b"png-bytes").expect("write image");

    let effects = dispatch(Action::SendPrompt("/unstick".into()), &mut app);
    let (images, dir) = effects
        .iter()
        .find_map(|e| match e {
            Effect::UnstickResendPrompt {
                images,
                images_dir,
                text,
                ..
            } => {
                assert!(
                    text.contains("[Image #1]"),
                    "text must keep the image token: {text}"
                );
                assert!(
                    !text.to_ascii_lowercase().contains("data:"),
                    "text must not inline a data URL: {text}"
                );
                Some((images.clone(), images_dir.clone()))
            }
            _ => None,
        })
        .expect("unstick must resend");
    assert!(
        images.iter().any(|i| i.file_id == "img-1.png"),
        "safe WAL file id must be on the effect: {images:?}"
    );
    assert!(
        images
            .iter()
            .all(|i| !i.file_id.to_ascii_lowercase().starts_with("data:")),
        "data URL file ids must not be sent: {images:?}"
    );
    let dir = dir.expect("unstick must pass the session images dir");
    let blocks = crate::app::dispatch::unstick::wal_image_resource_blocks(&dir, &images);
    assert_eq!(
        blocks.len(),
        1,
        "exactly the safe on-disk file becomes a resource block: {blocks:?}"
    );
    let acp::ContentBlock::ResourceLink(link) = &blocks[0] else {
        panic!("must send a resource link, not inline image bytes: {blocks:?}");
    };
    assert!(
        link.uri.starts_with("file://") && link.uri.contains("img-1.png"),
        "resource uri must point at the WAL file id: {}",
        link.uri
    );
    assert!(
        !link.uri.to_ascii_lowercase().contains("data:"),
        "resource uri must not be a data URL: {}",
        link.uri
    );
}
