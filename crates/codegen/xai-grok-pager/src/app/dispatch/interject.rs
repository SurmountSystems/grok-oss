//! Mid-turn interjection dispatch: optimistic local echo, the `x.ai/interject` effect, and prompt-history recording.

use super::ctx::NO_SESSION_NOTICE;
use super::voice::voice_stop_on_submit;
use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::block::RenderBlock;

/// Send a mid-turn interjection.
/// Pushes a standard user prompt block locally for instant feedback, records the text in prompt history, and clears the prompt.
/// The id is recorded in `self_interjection_ids` so `handle_interjection` drops the echo instead of rendering a duplicate.
pub(super) fn dispatch_interject(
    app: &mut AppView,
    text: String,
    images: Vec<crate::prompt_images::PastedImage>,
) -> Vec<Effect> {
    // Hard-reset only — `text` may not be from the composer.
    let _ = voice_stop_on_submit(app);
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    dispatch_interject_on(app, id, text, images)
}

/// Same as [`dispatch_interject`], but targets `agent_id` instead of the focused pane.
/// Used when a `/btw` answer lands on a non-active session.
pub(super) fn dispatch_interject_on(
    app: &mut AppView,
    id: AgentId,
    text: String,
    images: Vec<crate::prompt_images::PastedImage>,
) -> Vec<Effect> {
    dispatch_interject_on_inner(app, id, text, images, /* user_submit */ true)
}

/// Wait-start / Enter flush: same wire send, without submit side effects (voice stop, history, toast).
pub(super) fn dispatch_interject_from_held_queue(
    app: &mut AppView,
    id: AgentId,
    text: String,
    images: Vec<crate::prompt_images::PastedImage>,
) -> Vec<Effect> {
    dispatch_interject_on_inner(app, id, text, images, /* user_submit */ false)
}

fn dispatch_interject_on_inner(
    app: &mut AppView,
    id: AgentId,
    text: String,
    images: Vec<crate::prompt_images::PastedImage>,
    user_submit: bool,
) -> Vec<Effect> {
    // Voice is app-wide and bound to the focused composer
    // A /btw answer on another session must not commit interim text or kill dictation on the pane the user is actually talking into
    if user_submit && matches!(app.active_view, ActiveView::Agent(active) if active == id) {
        // Hard-reset only; `text` may not be from the composer
        let _ = voice_stop_on_submit(app);
    }
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };

    if user_submit {
        agent.ephemeral_tip.clear_on_submit();
        agent.release_hook_block_hold();
    }

    match overlay_operator_clarify(agent) {
        OverlayOperatorClarify::L3Unbothered => return refuse_l3_overlay_operator_text(agent),
        OverlayOperatorClarify::L2(session_id) => {
            return paint_and_send_interject(agent, id, session_id, text, images);
        }
        OverlayOperatorClarify::None => {}
    }

    let Some(session_id) = agent.session.session_id.clone() else {
        agent.show_toast(NO_SESSION_NOTICE);
        return vec![];
    };

    if user_submit {
        agent.record_prompt_in_history(&text);
    }

    // Push a standard user prompt block locally for instant feedback
    // Record its id so the broadcast echo (`x.ai/session/interjection`) is deduped instead of rendering a second copy on this pane
    let interjection_id = uuid::Uuid::new_v4().to_string();
    agent.self_interjection_ids.insert(interjection_id.clone());
    let entry_id = agent
        .scrollback
        .push_block(RenderBlock::interjection_prompt(&text));
    agent
        .interjection_painted_blocks
        .insert(interjection_id.clone(), entry_id);
    agent
        .interjection_retry_images
        .insert(interjection_id.clone(), images.clone());

    // The composer is NOT touched here: the producer that consumed composer text (the InterjectPrompt registry arm) clears it at the call site
    // Every other producer (Send now, edit-interject, plan review comments) carries non-composer text and must keep the user's draft/stash
    if user_submit {
        agent.show_toast("Interjection sent");
    }

    // Image-bearing interjection: build text and image content blocks via the same helper as the queued-prompt drain path
    // The helper covers orphan-placeholder recovery, the allowlist, and the size cap. Text-only stays on the legacy wire.
    let blocks = if images.is_empty() {
        None
    } else {
        let build = crate::prompt_images::build_content_blocks_with_workspace_report(
            text.clone(),
            images,
            Some(std::path::Path::new(&agent.session.cwd)),
        );
        app.pending_image_notices
            .extend(agent.skipped_image_send_notice(&build.skipped_display_numbers));
        Some(build.blocks)
    };

    vec![Effect::SendInterject {
        agent_id,
        session_id,
        text,
        interjection_id,
        blocks,
    }]
}

/// Cancel-and-send: send `text` (and images) as a fresh `sendNow` prompt so the shell cancels the running turn and runs it next.
/// The user block paints at dispatch (the arm hides the queue echo; the adoption reuses the block).
pub(super) fn dispatch_send_prompt_now(
    app: &mut AppView,
    text: String,
    images: Vec<crate::prompt_images::PastedImage>,
    image_notice: Option<String>,
) -> Vec<Effect> {
    // The composer that raised the notice is already cleared, so it shows even when the send bails below.
    app.pending_image_notices.extend(image_notice);
    // Hard-reset only; `text` may be a queue row, not the composer
    let _ = voice_stop_on_submit(app);
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let reconnect_pending = app.reconnect_pending;
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    agent.release_hook_block_hold();
    // Composer / queue-row / paste Send now all land here, not in dispatch_send_prompt_inner. Drop the credit-limit stash so Try Again cannot resubmit the blocked prompt after the user has moved on.
    // dispatch_send_prompt_inner. Drop the credit-limit stash so Try Again
    // cannot resubmit the blocked prompt after the user has moved on.
    agent.credit_limit_stashed_prompt = None;

    // Mid-outage guard (mirrors the plain prompt path): the producers already consumed the payload (composer text / queue row)
    // Requeue it locally instead of firing into a dead channel and losing the message
    if reconnect_pending {
        let queue_id = agent.session.next_queue_id;
        agent.session.next_queue_id += 1;
        agent
            .session
            .pending_prompts
            .push_front(crate::app::agent::QueuedPrompt {
                images,
                ..crate::app::agent::QueuedPrompt::plain(
                    queue_id,
                    &text,
                    crate::app::agent::QueueEntryKind::Prompt,
                )
            });
        agent.show_toast("Reconnecting, please wait...");
        return vec![];
    }

    // Submitting retires any edit-contextual ephemeral tip.
    agent.ephemeral_tip.clear_on_submit();

    let Some(session_id) = agent.session.session_id.clone() else {
        agent.show_toast(NO_SESSION_NOTICE);
        return vec![];
    };

    agent.record_prompt_in_history(&text);

    let prompt_id = uuid::Uuid::new_v4().to_string();
    // Self-originated: the ACP gate must treat this prompt's deltas as ours.
    agent.note_self_originated_prompt(&prompt_id);
    // Expect the shell's send-now cancel so the turn-end handling suppresses its marker
    super::queue::arm_send_now_and_paint_dispatched(agent, &prompt_id, &text);

    let build = crate::prompt_images::build_content_blocks_with_workspace_report(
        text.clone(),
        images,
        Some(std::path::Path::new(&agent.session.cwd)),
    );
    app.pending_image_notices
        .extend(agent.skipped_image_send_notice(&build.skipped_display_numbers));
    let blocks = build.blocks;

    // Optimistic queue-pane echo, reconciled by the shell's queue broadcast.
    let sid_str = session_id.0.to_string();
    super::queue::push_server_queue_echo(app, id, &sid_str, &prompt_id, &text, "prompt");
    crate::unified_log::info(
        "prompt.send_now",
        Some(&sid_str),
        Some(serde_json::json!({ "len": text.len(), "prompt_id": prompt_id })),
    );

    vec![Effect::SendPromptNow {
        agent_id: id,
        session_id,
        blocks,
        prompt_id,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::actions::Action;
    use crate::app::agent::AgentId;
    use crate::app::app_view::{InputOutcome, PendingAction};
    use crate::app::dispatch::router::dispatch;
    use crate::app::dispatch::tests::test_app_with_agent;
    use crate::input::key::KeyShortcut;
    use agent_client_protocol as acp;
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    /// Composer-clear ownership: dispatch NEVER touches the composer.
    /// The only composer-text producer (the InterjectPrompt registry arm) clears it at the call site.
    /// Every other producer (Send now, edit-interject, plan review comments) carries non-composer text whose draft/stash must survive dispatch.
    #[test]
    fn interject_dispatch_never_touches_the_composer() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);

        // Unrelated draft survives a plain interject.
        app.agents
            .get_mut(&id)
            .unwrap()
            .prompt
            .set_text("stashed draft");
        let effects = dispatch(
            Action::Interject {
                text: "edited body".into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(matches!(effects.as_slice(), [Effect::SendInterject { .. }]));
        assert_eq!(app.agents.get(&id).unwrap().prompt.text(), "stashed draft");

        // Edited-queued interject: fire-and-forget, composer untouched.
        let effects = dispatch(
            Action::QueueInterjectShared {
                id: "p1".into(),
                expected_version: 1,
                new_text: Some("edited body".into()),
            },
            &mut app,
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::QueueInterject { .. }]
        ));
        assert_eq!(app.agents.get(&id).unwrap().prompt.text(), "stashed draft");

        // Even a composer that equals the interjected text is preserved; the InterjectPrompt arm already cleared it for the composer path
        app.agents.get_mut(&id).unwrap().prompt.set_text("send me");
        let _ = dispatch(
            Action::Interject {
                text: "send me".into(),
                images: vec![],
            },
            &mut app,
        );
        assert_eq!(app.agents.get(&id).unwrap().prompt.text(), "send me");
    }

    /// Interjecting is a submit: it retires the active ephemeral tip.
    #[test]
    fn interject_clears_active_ephemeral_tip() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);

        let agent = app.agents.get_mut(&id).unwrap();
        let _ = agent.ephemeral_tip.show(
            crate::tips::EphemeralTip::new("t", ratatui::text::Line::from("hint")),
            &mut std::collections::HashMap::new(),
        );
        assert!(agent.ephemeral_tip.is_active());

        let _ = dispatch(
            Action::Interject {
                text: "mid-turn note".into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(
            !app.agents.get(&id).unwrap().ephemeral_tip.is_active(),
            "interject submit must clear the tip"
        );
    }

    /// A no-session interject still retires the tip: the clear runs before the "No active session" early return, matching the other submit paths.
    #[test]
    fn interject_without_session_still_clears_ephemeral_tip() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);

        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = None;
        let _ = agent.ephemeral_tip.show(
            crate::tips::EphemeralTip::new("t", ratatui::text::Line::from("hint")),
            &mut std::collections::HashMap::new(),
        );
        assert!(agent.ephemeral_tip.is_active());

        let effects = dispatch(
            Action::Interject {
                text: "mid-turn note".into(),
                images: vec![],
            },
            &mut app,
        );

        let agent = app.agents.get(&id).unwrap();
        assert!(
            !agent.ephemeral_tip.is_active(),
            "no-session interject must still clear the tip"
        );
        assert!(
            effects.is_empty(),
            "no-session interject dispatches no effects"
        );
        assert_eq!(
            agent.toast.as_ref().map(|(m, _)| m.as_str()),
            Some("No active session"),
            "no-session interject takes the 'No active session' path"
        );
    }

    /// Image-bearing interject builds structured blocks (Text first with the placeholder intact, then one Image block).
    /// No-image stays legacy (`blocks: None`) so the wire shape is byte-identical.
    #[test]
    fn interject_with_images_builds_blocks_text_first() {
        let mut app = test_app_with_agent();

        let mut img = crate::prompt_images::from_clipboard_data(&crate::clipboard::ImageData {
            data: vec![1, 2, 3],
            mime_type: "image/png".into(),
        });
        img.display_number = 1;

        let effects = dispatch(
            Action::Interject {
                text: "look at [Image #1] please".into(),
                images: vec![img],
            },
            &mut app,
        );
        match effects.as_slice() {
            [
                Effect::SendInterject {
                    text,
                    blocks: Some(blocks),
                    ..
                },
            ] => {
                assert_eq!(text, "look at [Image #1] please");
                assert_eq!(blocks.len(), 2);
                match &blocks.first().unwrap_or_else(|| panic!("missing index")) {
                    acp::ContentBlock::Text(tb) => {
                        assert!(tb.text.contains("[Image #1]"), "got {:?}", tb.text)
                    }
                    other => panic!("expected Text first, got {other:?}"),
                }
                assert!(matches!(
                    &blocks.get(1).unwrap_or_else(|| panic!("missing index")),
                    acp::ContentBlock::Image(_)
                ));
            }
            other => panic!("expected SendInterject with blocks, got {other:?}"),
        }

        let effects = dispatch(
            Action::Interject {
                text: "plain".into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::SendInterject { blocks: None, .. }]
        ));
    }

    /// Surmount / grok-oss fork; tests are contracts.
    /// Mid-turn `x.ai/interject` appends the operator text to the WAL before
    /// the interject effect is returned.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn prompt_wal_appends_on_mid_turn_interject() {
        use crate::app::agent::AgentState;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "wal-mid-turn-interject";
        let body = "mid-turn interject that must hit the WAL";

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::TurnRunning;
        }

        let effects = dispatch(
            Action::Interject {
                text: body.into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendInterject { text, .. } if text == body)),
            "interject must still fire; WAL is extra durability, got {effects:?}"
        );
        let rows =
            xai_grok_shell::session::prompt_wal::load_prompt_wal(&cwd_str, sid).expect("load WAL");
        assert!(
            rows.iter().any(|r| {
                r.kind == xai_grok_shell::session::prompt_wal::PromptWalKind::Interject
                    && r.text == body
            }),
            "prompt_wal.jsonl must contain the mid-turn interject, got {rows:?}"
        );
    }

    /// Surmount / grok-oss fork; named tests are contracts, not optional chrome.
    /// Mid-turn Ctrl+Enter must dispatch `SendInterject` (`x.ai/interject`).
    /// It must not drop the composer text, queue-only, no-op, or cancel-and-send
    /// (`SendPromptNow`). This is the explicit send-now chord; Enter is the
    /// separate soft-interject path. Grok OSS 1.0.3 is not last-known-good.
    ///
    /// Red before product (code reading): `agent_view/prompt.rs` InterjectPrompt
    /// arm returned `Action::SendPromptNow`, so dispatch emitted
    /// `Effect::SendPromptNow` instead of `Effect::SendInterject`.
    #[test]
    fn ctrl_enter_mid_turn_dispatches_send_interject() {
        use crate::app::agent::AgentState;
        use crate::app::agent_view::ActivePane;

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let body = "steer this running turn";
        let action = {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::TurnRunning;
            agent.set_active_pane(ActivePane::Prompt, true);
            agent.prompt.set_text(body);
            match agent
                .handle_prompt_key_for_test(&KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
            {
                InputOutcome::Action(action) => action,
                other => panic!(
                    "Ctrl+Enter mid-turn must emit Interject, not drop or queue-only, got {other:?}"
                ),
            }
        };
        assert!(
            matches!(&action, Action::Interject { text, .. } if text == body),
            "Ctrl+Enter must be Action::Interject, not SendPromptNow, got {action:?}"
        );
        let effects = dispatch(action, &mut app);
        match effects.as_slice() {
            [Effect::SendInterject { text, .. }] => assert_eq!(text, body),
            other => panic!("expected SendInterject, got {other:?}"),
        }
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptNow { .. })),
            "must not serial-queue or cancel-and-send, got {effects:?}"
        );
        assert!(
            app.agents[&id].prompt.text().is_empty(),
            "composer must clear at the InterjectPrompt call site"
        );
        assert!(
            app.agents[&id].session.pending_prompts.is_empty(),
            "must not land in pending_prompts as the next serial prompt"
        );
        assert!(
            app.agents[&id].session.state.is_turn_running(),
            "current turn must keep running"
        );
    }

    /// Named contract: mid-turn Interject paints and returns SendInterject
    /// without waiting a minute and without cancel-and-send. Dispatch is
    /// local; the ACP send is an effect, not a join on this thread.
    #[test]
    fn interject_does_not_wait_minutes_or_block_paint() {
        use std::time::{Duration, Instant};

        use crate::app::agent::AgentState;

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::TurnRunning;
            agent.session.current_prompt_id = Some("live-turn".into());
        }
        let started = Instant::now();
        let effects = dispatch(
            Action::Interject {
                text: "steer now".into(),
                images: vec![],
            },
            &mut app,
        );
        match effects.as_slice() {
            [Effect::SendInterject { text, .. }] => assert_eq!(text, "steer now"),
            other => panic!("expected SendInterject, got {other:?}"),
        }
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptNow { .. })),
            "interject must not wait on cancel-and-send, got {effects:?}"
        );
        let agent = app.agents.get(&id).unwrap();
        assert!(
            agent.session.state.is_turn_running(),
            "interject must not sit on Cancelling, got {:?}",
            agent.session.state
        );
        assert!(
            agent
                .scrollback
                .entries_in_range(0..agent.scrollback.len())
                .iter()
                .any(|e| matches!(&e.block, RenderBlock::UserPrompt(p) if p.text.contains("steer now"))),
            "interject must paint locally before the ACP send"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "interject dispatch must not wait minutes; elapsed={:?}",
            started.elapsed()
        );
    }

    /// Surmount / grok-oss fork; named tests are contracts, not optional chrome.
    /// Empty composer + Ctrl+Enter must not send an empty interject.
    #[test]
    fn empty_ctrl_enter_mid_turn_does_not_send() {
        use crate::app::agent::AgentState;
        use crate::app::agent_view::ActivePane;

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let outcome = {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::TurnRunning;
            agent.session.pending_prompts.clear();
            agent.set_active_pane(ActivePane::Prompt, true);
            agent.prompt.set_text("");
            agent.handle_prompt_key_for_test(&KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
        };
        assert!(
            !matches!(
                outcome,
                InputOutcome::Action(Action::Interject { .. })
                    | InputOutcome::Action(Action::SendPromptNow { .. })
                    | InputOutcome::Action(Action::SendPrompt(_))
            ),
            "empty composer must not send, got {outcome:?}"
        );
        assert!(
            app.agents[&id].session.pending_prompts.is_empty(),
            "empty Ctrl+Enter must not invent a queued row"
        );
    }

    /// Surmount / grok-oss fork; named tests are contracts, not optional chrome.
    /// Queue `#1 [Send now]` mouse Down must send a local queued row as
    /// `SendInterject`. Key and click must not diverge.
    ///
    /// Red before product (code reading): `force_interject_queue_row` returned
    /// `Action::SendPromptNow` for local rows (`agent_view/queue.rs`). Mouse
    /// Down on `[Send now]` (`app/mouse.rs`) calls that function, so a click
    /// would have dispatched `SendPromptNow` instead of `SendInterject`.
    #[test]
    fn queue_send_now_click_dispatches_send_interject() {
        use crate::app::agent::AgentState;

        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let body = "queued follow-up";
        let action = {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::TurnRunning;
            agent.session.enqueue_prompt(body.into());
            agent.sync_queue_pane();
            let row_id = *agent
                .queue
                .entry_ids()
                .first()
                .expect("queued row must paint");
            agent.queue.list_state.select_by_id(row_id);
            let area = Rect::new(0, 0, 80, 6);
            let mut buf = Buffer::empty(area);
            let layout_cfg = crate::appearance::LayoutConfig::default();
            agent
                .queue
                .render(area, &mut buf, true, &layout_cfg, None, true);
            agent.pane_areas.queue = area;
            let mut found = None;
            'find: for row in area.y..area.y + area.height {
                for col in area.x..area.x + area.width {
                    if agent.queue.send_now_click(col, row) == Some(row_id) {
                        found = Some((col, row));
                        break 'find;
                    }
                }
            }
            let (col, row) = found.expect("queue [Send now] must paint on the local row");
            match agent.handle_mouse(&MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::empty(),
            }) {
                InputOutcome::Action(action) => action,
                other => panic!("Send now on a local queue row must emit Interject, got {other:?}"),
            }
        };
        assert!(
            matches!(&action, Action::Interject { text, .. } if text == body),
            "Send now must be Action::Interject, not SendPromptNow, got {action:?}"
        );
        let effects = dispatch(action, &mut app);
        match effects.as_slice() {
            [Effect::SendInterject { text, .. }] => assert_eq!(text, body),
            other => panic!("expected SendInterject from Send now, got {other:?}"),
        }
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptNow { .. })),
            "Send now must not cancel-and-send, got {effects:?}"
        );
        assert!(
            app.agents[&id].session.pending_prompts.is_empty(),
            "queued row must be consumed"
        );
        assert!(
            app.agents[&id].session.state.is_turn_running(),
            "current turn must keep running"
        );
    }

    /// After a successful image interject, the sent prompt is in scrollback
    /// once and is not leftover in pending_prompts.
    #[test]
    fn image_interject_leaves_one_prompt_and_empty_queue() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let mut img = crate::prompt_images::from_clipboard_data(&crate::clipboard::ImageData {
            data: vec![1, 2, 3],
            mime_type: "image/png".into(),
        });
        img.display_number = 1;
        let _ = dispatch(
            Action::Interject {
                text: "Also why am I waiting here? [Image #1]".into(),
                images: vec![img],
            },
            &mut app,
        );
        let agent = app.agents.get(&id).unwrap();
        let count = (0..agent.scrollback.len())
            .filter(|i| {
                matches!(
                    agent.scrollback.get(*i).map(|e| &e.block),
                    Some(crate::scrollback::block::RenderBlock::UserPrompt(b))
                        if b.text.contains("[Image #1]")
                )
            })
            .count();
        assert_eq!(
            count, 1,
            "Surmount / grok-oss fork: after send the image prompt must appear once, got {count}"
        );
        assert!(
            agent.session.pending_prompts.is_empty(),
            "successful interject must not leave a leftover queued prompt remaining"
        );
    }

    fn overlay_info(
        child_sid: &str,
        parent_sid: &str,
        depth: u32,
    ) -> crate::app::subagent::SubagentInfo {
        crate::app::subagent::SubagentInfo {
            subagent_id: child_sid.into(),
            child_session_id: child_sid.into(),
            description: "coordinate the slice".into(),
            subagent_type: "general-purpose".into(),
            persona: None,
            role: None,
            model: None,
            context_source: None,
            resumed_from: None,
            capability_mode: None,
            workflow_run_id: None,
            context_normalized: false,
            parent_prompt_id: None,
            parent_session_id: Some(parent_sid.into()),
            depth: Some(depth),
            started_at: std::time::Instant::now(),
            last_progress_at: std::time::Instant::now(),
            finished: false,
            status: None,
            error: None,
            duration_ms: None,
            tool_calls: None,
            turns: None,
            turn_count: None,
            tool_call_count: None,
            tokens_used: None,
            context_window_tokens: None,
            context_usage_pct: None,
            tools_used: Vec::new(),
            error_count: None,
            activity_label: None,
            is_background: false,
            pending_kill: false,
            kill_requested_at: None,
            scrollback_entry_id: None,
            prompt: None,
            child_cwd: None,
            worktree_path: None,
            child_updates_replayed: false,
        }
    }

    fn app_with_overlay(child_sid: &str, depth: u32) -> AppView {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let l1_sid = app.agents[&id]
            .session
            .session_id
            .as_ref()
            .expect("l1 session")
            .0
            .to_string();
        let parent_sid = if depth >= 2 {
            "l2-coord"
        } else {
            l1_sid.as_str()
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let child_session = crate::app::agent::AgentSession {
            id: AgentId(0),
            acp_tx: tx,
            session_id: Some(acp::SessionId::new(child_sid)),
            models: crate::acp::model_state::ModelState::default(),
            state: crate::app::agent::AgentState::TurnRunning,
            tracker: crate::acp::tracker::AcpUpdateTracker::new(),
            cwd: std::path::PathBuf::from("/tmp"),
            is_worktree: false,
            forked_from: None,
            pending_prompts: std::collections::VecDeque::new(),
            next_queue_id: 0,
            yolo_mode: false,
            auto_mode: false,
            context_only_mode: false,
            prompt_history: Vec::new(),
            prompt_history_loading: false,
            loading_replay: false,
            restore_degree: None,
            rate_limited: false,
            model_incompatible: false,
            credit_limit_blocked: false,
            free_usage_blocked: false,
            available_commands: Vec::new(),
            available_commands_generation: 0,
            available_tools: None,
            model_switch_pending: false,
            user_model_preference: None,
            deferred_model_switch: None,
            bg_tasks: std::collections::BTreeMap::new(),
            bg_tool_call_to_task: std::collections::HashMap::new(),
            scheduled_tasks: std::collections::HashMap::new(),
            in_flight_prompt: None,
            compact_held_prompt: None,
            current_prompt_id: None,
            created_via_new: false,
            session_notes: crate::app::agent::SessionNotes::default(),
        };
        let child = crate::app::agent_view::AgentView::new(
            child_session,
            crate::scrollback::state::ScrollbackState::new(),
        );
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
        if depth >= 2 {
            agent
                .subagent_sessions
                .insert("l2-coord".into(), overlay_info("l2-coord", &l1_sid, 1));
        }
        agent
            .subagent_sessions
            .insert(child_sid.into(), overlay_info(child_sid, parent_sid, depth));
        agent.insert_subagent_view(child_sid.to_string(), Box::new(child));
        agent.open_subagent_fullscreen(child_sid.to_string());
        app
    }

    #[test]
    fn l2_overlay_send_prompt_interjects_l2_not_l1() {
        let mut app = app_with_overlay("l2-coord", 1);
        let l1 = app.agents[&AgentId(0)]
            .session
            .session_id
            .clone()
            .expect("l1");
        let effects = dispatch(Action::SendPrompt("clarify the slice".into()), &mut app);
        match effects.as_slice() {
            [
                Effect::SendInterject {
                    session_id, text, ..
                },
            ] => {
                assert_eq!(session_id.0.as_ref(), "l2-coord");
                assert_ne!(session_id, &l1);
                assert_eq!(text, "clarify the slice");
            }
            other => panic!("expected interject to the L2 session, got {other:?}"),
        }
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::SendPrompt { .. })),
            "L2 overlay must not SendPrompt on L1"
        );
    }

    #[test]
    fn l3_overlay_send_prompt_does_not_reach_l3_or_l1() {
        let mut app = app_with_overlay("l3-specialist", 2);
        let l1 = app.agents[&AgentId(0)]
            .session
            .session_id
            .clone()
            .expect("l1");
        let effects = dispatch(Action::SendPrompt("do not barge in".into()), &mut app);
        assert!(
            effects.is_empty(),
            "operator text on an L3 overlay must not send, got {effects:?}"
        );
        assert!(
            effects.iter().all(|effect| match effect {
                Effect::SendInterject { session_id, .. }
                | Effect::SendPrompt { session_id, .. }
                | Effect::SendPromptNow { session_id, .. } => {
                    session_id.0.as_ref() != "l3-specialist" && session_id != &l1
                }
                _ => true,
            }),
            "must never target a live L3 or fall through to L1"
        );
    }

    #[test]
    fn interject_while_cancelling_aborts_cancel() {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = crate::app::agent::AgentState::TurnCancelling;
            agent.pending_cancel_resend = Some(crate::app::agent_view::PendingCancelResend {
                prompt_id: Some("pid-keep".into()),
                sent_at: std::time::Instant::now(),
                attempts: 1,
                confirmed: false,
                cancel_subagents: true,
                trigger: crate::app::actions::CancelTrigger::Esc,
            });
        }
        let effects = dispatch(
            Action::Interject {
                text: "keep working".into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(
            matches!(effects.as_slice(), [Effect::SendInterject { .. }]),
            "keep-working interject must still send, got {effects:?}"
        );
        let agent = app.agents.get(&id).unwrap();
        assert!(
            agent.session.state.is_turn_running(),
            "interject must abort cancel while the turn is still cancellable"
        );
        assert!(
            agent.pending_cancel_resend.is_none(),
            "aborting cancel must stop the Cancelling retry"
        );
        assert!(agent.cancel_trigger_hint.is_none());
    }

    #[test]
    fn l2_overlay_interject_while_child_cancelling_aborts_child_cancel() {
        let mut app = app_with_overlay("l2-coord", 1);
        let id = AgentId(0);
        {
            let child = app
                .agents
                .get_mut(&id)
                .unwrap()
                .subagent_views
                .get_mut("l2-coord")
                .unwrap();
            child.session.state = crate::app::agent::AgentState::TurnCancelling;
            child.pending_cancel_resend = Some(crate::app::agent_view::PendingCancelResend {
                prompt_id: Some("l2-pid".into()),
                sent_at: std::time::Instant::now(),
                attempts: 1,
                confirmed: false,
                cancel_subagents: true,
                trigger: crate::app::actions::CancelTrigger::Esc,
            });
        }
        let effects = dispatch(
            Action::Interject {
                text: "keep working".into(),
                images: vec![],
            },
            &mut app,
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::SendInterject { session_id, .. }] if session_id.0.as_ref() == "l2-coord"
        ));
        let parent = app.agents.get(&id).unwrap();
        let child = parent.subagent_views.get("l2-coord").unwrap();
        assert!(child.session.state.is_turn_running());
        assert!(child.pending_cancel_resend.is_none());
        assert!(parent.session.state.is_turn_running());
    }

    #[test]
    fn l2_overlay_app_esc_dismisses_without_cancel_or_cancelling() {
        let mut app = app_with_overlay("l2-coord", 1);
        let id = AgentId(0);
        {
            let child = app
                .agents
                .get_mut(&id)
                .unwrap()
                .subagent_views
                .get_mut("l2-coord")
                .unwrap();
            child.vim_mode = false;
            child.prompt.set_text("");
            child.prompt.set_cursor(0);
        }
        let outcome =
            app.handle_input(&Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(
            matches!(outcome, InputOutcome::Changed),
            "AppView overlay Esc must dismiss, got {outcome:?}"
        );
        assert!(
            !matches!(outcome, InputOutcome::Action(Action::CancelTurn)),
            "overlay Esc must not emit CancelTurn, got {outcome:?}"
        );
        assert!(
            !matches!(
                app.pending_action.as_ref().map(|p| &p.action),
                Some(Action::CancelTurn)
            ),
            "overlay Esc must not arm Cancelling confirm"
        );
        let parent = app.agents.get(&id).unwrap();
        assert!(
            parent.active_subagent.is_none(),
            "Esc must close the nested view"
        );
        let child = parent.subagent_views.get("l2-coord").unwrap();
        assert!(
            child.session.state.is_turn_running(),
            "L2 must keep working"
        );
        assert!(
            !child.session.state.is_cancelling(),
            "overlay Esc must not start Cancelling chrome"
        );
        assert!(child.cancel_trigger_hint.is_none());
    }

    #[test]
    fn l2_overlay_esc_does_not_fire_armed_parent_cancel() {
        let mut app = app_with_overlay("l2-coord", 1);
        let id = AgentId(0);
        {
            let child = app
                .agents
                .get_mut(&id)
                .unwrap()
                .subagent_views
                .get_mut("l2-coord")
                .unwrap();
            child.vim_mode = false;
        }
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        app.pending_action = Some(PendingAction::with_ttl(
            Action::CancelTurn,
            KeyShortcut::from(esc),
            Some("cancel"),
            PendingAction::ESC_DOUBLE_PRESS_TTL,
        ));
        let outcome = app.handle_input(&Event::Key(esc));
        assert!(
            !matches!(outcome, InputOutcome::Action(Action::CancelTurn)),
            "viewing the overlay, Esc must not confirm a prior cancel arm, got {outcome:?}"
        );
        let parent = app.agents.get(&id).unwrap();
        assert!(parent.active_subagent.is_none());
        let child = parent.subagent_views.get("l2-coord").unwrap();
        assert!(child.session.state.is_turn_running());
        assert!(!child.session.state.is_cancelling());
    }

    #[test]
    fn l2_overlay_send_prompt_now_is_still_a_mid_turn_ask_to_l2() {
        let mut app = app_with_overlay("l2-coord", 1);
        let effects = dispatch(
            Action::SendPromptNow {
                text: "steer the coordinator".into(),
                images: vec![],
            },
            &mut app,
        );
        match effects.as_slice() {
            [
                Effect::SendInterject {
                    session_id, text, ..
                },
            ] => {
                assert_eq!(session_id.0.as_ref(), "l2-coord");
                assert_eq!(text, "steer the coordinator");
            }
            other => panic!("expected mid-turn ask to L2, got {other:?}"),
        }
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::SendPromptNow { .. })),
            "must not cancel-and-send the L1 turn from an L2 overlay"
        );
    }
}
