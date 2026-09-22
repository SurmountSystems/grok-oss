//! Stale pager-queue occupancy: committed Human turns must not stay as
//! queue rows, and consecutive identical queue bodies collapse to one.
//!
//! Live occupancy is independent of `/rebuild` WAL restore. A mid-turn
//! Responding window can still paint `#3` / `#4` / `#5` `[Send now]` after
//! those bodies already ran as Human turns.

#[cfg(test)]
mod tests {
    use crate::app::agent::{AgentId, AgentState, QueueEntryKind, QueuedPrompt};
    use crate::app::dispatch::queue::maybe_drain_queue;
    use crate::scrollback::block::RenderBlock;
    use xai_grok_shell::session::pending_prompts::PersistedQueuedPrompt;

    const LAKE_BUILD: &str = "Lake-build time-preference note";
    const LIGHTWAVE: &str =
        "Convert this LightWave 3D scene to a sprite sheet with two extra lines.";
    const STILL_UNSENT: &str = "follow-up that is not yet a Human turn";

    fn queued_texts(agent: &crate::app::agent_view::AgentView) -> Vec<String> {
        agent
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.clone())
            .collect()
    }

    /// Named contract: a prompt that is already a Human turn is not shown as
    /// a queue row. Mid-turn Responding still holding `#3` with `[Send now]`
    /// after that body ran is stale occupancy, not only `/rebuild` WAL.
    #[test]
    fn pending_prompts_row_matching_committed_human_turn_is_not_kept_as_queue_row() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(LAKE_BUILD));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            3,
            LAKE_BUILD,
            QueueEntryKind::Prompt,
        ));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            6,
            STILL_UNSENT,
            QueueEntryKind::Prompt,
        ));
        agent.persist_pending_prompts();
        agent.sync_queue_pane();
        let queued = queued_texts(agent);
        assert!(
            !queued.iter().any(|t| t == LAKE_BUILD),
            "a pending_prompts row that is already a committed Human turn must not stay as a queue row after drain or restore; queue={queued:?}"
        );
        assert!(
            queued.iter().any(|t| t == STILL_UNSENT),
            "a truly unsent follow-up must still occupy the queue; queue={queued:?}"
        );
        assert_eq!(
            agent.queue.entry_ids().len(),
            1,
            "queue pane must not paint the committed Human turn as a [Send now] row"
        );
    }

    /// Named contract: consecutive identical queue bodies collapse to one.
    /// `#4` and `#5` with the same LightWave 3D-to-sprite text are duplicate
    /// occupancy, not two unsent prompts.
    #[test]
    fn consecutive_identical_queue_bodies_are_deduped() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            4,
            LIGHTWAVE,
            QueueEntryKind::Prompt,
        ));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            5,
            format!("{LIGHTWAVE}\n"),
            QueueEntryKind::Prompt,
        ));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            6,
            STILL_UNSENT,
            QueueEntryKind::Prompt,
        ));
        agent.persist_pending_prompts();
        agent.sync_queue_pane();
        let queued = queued_texts(agent);
        assert_eq!(
            queued.iter().filter(|t| t.trim() == LIGHTWAVE).count(),
            1,
            "consecutive identical queue bodies must collapse to one row; queue={queued:?}"
        );
        assert_eq!(
            queued.len(),
            2,
            "unsent follow-up stays after consecutive dedup; queue={queued:?}"
        );
        assert_eq!(queued[1], STILL_UNSENT);
    }

    /// After drain paints a Human turn, a leftover identical copy in
    /// `pending_prompts` is stale occupancy and must not remain.
    #[test]
    fn drain_drops_remaining_copy_of_the_prompt_that_just_became_a_human_turn() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.enqueue_prompt(LIGHTWAVE.into());
        agent.session.enqueue_prompt(LIGHTWAVE.into());
        agent.session.enqueue_prompt(STILL_UNSENT.into());
        let drain = maybe_drain_queue(agent);
        assert!(
            !drain.effects.is_empty(),
            "idle drain must start the first LightWave copy as a Human turn"
        );
        let queued = queued_texts(agent);
        assert!(
            !queued.iter().any(|t| t.trim() == LIGHTWAVE),
            "after drain, a leftover identical copy of the committed Human turn must not stay queued; queue={queued:?}"
        );
        assert_eq!(
            queued,
            vec![STILL_UNSENT.to_string()],
            "the truly unsent follow-up is the only remaining queue row; queue={queued:?}"
        );
    }

    /// Restore from `pending_prompts.json` must drop rows that already exist
    /// as Human turns in live scrollback even when `chat_history.jsonl` is
    /// empty. This is the live window, not only WAL chat-history skip.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_pending_prompts_drops_rows_already_painted_as_human_turns() {
        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "stale-queue-live-scrollback";
        xai_grok_shell::session::pending_prompts::write_pending_prompts(
            &cwd_str,
            sid,
            &[
                PersistedQueuedPrompt {
                    id: 3,
                    text: LAKE_BUILD.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 4,
                    text: LIGHTWAVE.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 5,
                    text: LIGHTWAVE.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 6,
                    text: STILL_UNSENT.to_string(),
                    kind: "prompt".into(),
                },
            ],
        )
        .expect("write pending_prompts.json");

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd;
        agent.session.prompt_history.clear();
        agent.session.pending_prompts.clear();
        agent.session.state = AgentState::TurnRunning;
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(LAKE_BUILD));
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(LIGHTWAVE));
        agent.restore_pending_prompts_from_disk();
        let queued = queued_texts(agent);
        assert!(
            !queued
                .iter()
                .any(|t| t == LAKE_BUILD || t.trim() == LIGHTWAVE),
            "restore must not keep pending_prompts rows that are already Human turns in the transcript; queue={queued:?}"
        );
        assert_eq!(
            queued,
            vec![STILL_UNSENT.to_string()],
            "a pending_prompts row that is not a committed Human turn must still restore; queue={queued:?}"
        );
    }

    /// Named contract: restore with a live in-memory queue plus disk rows
    /// that are already Human turns must not leave those rows. Quote: stale
    /// prompts continue to be a problem after rebuild.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_pending_prompts_from_disk_drops_human_turns_when_memory_queue_is_nonempty() {
        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "stale-queue-nonempty-memory";
        xai_grok_shell::session::pending_prompts::write_pending_prompts(
            &cwd_str,
            sid,
            &[
                PersistedQueuedPrompt {
                    id: 3,
                    text: LAKE_BUILD.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 4,
                    text: LIGHTWAVE.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 6,
                    text: STILL_UNSENT.to_string(),
                    kind: "prompt".into(),
                },
            ],
        )
        .expect("write pending_prompts.json");

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd;
        agent.session.prompt_history.clear();
        agent.session.state = AgentState::TurnRunning;
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(LAKE_BUILD));
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(LIGHTWAVE));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            3,
            LAKE_BUILD,
            QueueEntryKind::Prompt,
        ));
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            6,
            STILL_UNSENT,
            QueueEntryKind::Prompt,
        ));
        agent.restore_pending_prompts_from_disk();
        let queued = queued_texts(agent);
        assert!(
            !queued
                .iter()
                .any(|t| t == LAKE_BUILD || t.trim() == LIGHTWAVE),
            "stale prompts continue to be a problem after rebuild: restore must drop Human-turn rows even when memory queue is already non-empty; queue={queued:?}"
        );
        assert_eq!(
            queued,
            vec![STILL_UNSENT.to_string()],
            "a still-unsent follow-up must remain after occupancy drop; queue={queued:?}"
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

    fn shared_prompt_wire(
        id: &str,
        text: &str,
        position: usize,
    ) -> crate::app::prompt_queue::QueueEntryWire {
        crate::app::prompt_queue::QueueEntryWire {
            id: id.into(),
            version: 1,
            owner: None,
            last_editor: None,
            kind: "prompt".into(),
            text: text.into(),
            position,
            combined_texts: None,
        }
    }

    /// Named contract: a prompt that already issued or already has a Human
    /// turn in this session must not come back as a queued stale Prompt after
    /// rebuild, occupancy drop, Compact, or session reload. Duplicate occupancy
    /// rows for the same already-issued text must not remain in the pager
    /// queue. After Compact empties live scrollback, `sync_queue_pane` (the
    /// paint path the Operator sees) must still drop `shared_queue` Prompt
    /// wires whose text is already a parsed user turn in `chat_history.jsonl`.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn sync_queue_pane_drops_shared_queue_rows_already_in_chat_history_when_scrollback_is_empty() {
        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "stale-paint-compact-emptied-scrollback";
        write_session_chat_history(
            &cwd_str,
            sid,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": LIGHTWAVE}],
                })
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd;
        agent.session.prompt_history.clear();
        agent.session.pending_prompts.clear();
        agent.session.state = AgentState::TurnRunning;
        assert_eq!(
            agent.scrollback.len(),
            0,
            "Compact emptied live scrollback; this fixture must not push a UserPrompt block"
        );
        agent.shared_queue = vec![
            shared_prompt_wire("occ-4", LIGHTWAVE, 0),
            shared_prompt_wire("occ-5", LIGHTWAVE, 1),
        ];
        agent.session.pending_prompts.push_back(QueuedPrompt::plain(
            6,
            STILL_UNSENT,
            QueueEntryKind::Prompt,
        ));
        agent.sync_queue_pane();
        let painted = agent.queue.entry_texts();
        assert!(
            !painted.iter().any(|t| t.trim() == LIGHTWAVE),
            "a prompt that already issued or already has a Human turn in this session must not come back as a queued stale Prompt after rebuild, occupancy drop, Compact, or session reload; painted={painted:?}"
        );
        assert_eq!(
            painted.iter().filter(|t| t.trim() == LIGHTWAVE).count(),
            0,
            "duplicate occupancy rows for the same already-issued text must not remain in the pager queue; painted={painted:?}"
        );
        assert!(
            painted.contains(&STILL_UNSENT),
            "a truly unsent follow-up must still paint as a queue row; painted={painted:?}"
        );
        assert_eq!(
            agent.queue.entry_ids().len(),
            1,
            "queue pane must not paint the compacted Human turn as a [Send now] row; ids={:?} painted={painted:?}",
            agent.queue.entry_ids()
        );
        assert!(
            !agent
                .shared_queue
                .iter()
                .any(|w| w.text.trim() == LIGHTWAVE),
            "paint-path occupancy drop must remove already-issued shared_queue Prompt wires even when scrollback is empty"
        );
    }

    /// Occupancy drop after rebuild must not wipe still-running nested work.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn still_running_nested_work_is_not_occupancy_dropped_after_rebuild() {
        use crate::app::subagent::live_subagent_list;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let sid = "occupancy-drop-keeps-nested";

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd;
        agent.session.state = AgentState::TurnRunning;
        agent.subagent_sessions.insert(
            "cs-nested-live".into(),
            crate::app::agent_view::AgentView::live_nested_occupancy_row_for_tests(
                "cs-nested-live",
                "sa-nested-live",
                "still running nested implementor",
                Some("implementer"),
            ),
        );
        agent.persist_session_work_to_disk_for_rebuild();
        agent.subagent_sessions.clear();
        agent.restore_nested_occupancy_from_disk();
        agent.drop_stale_queue_occupancy();
        agent.drop_stale_queue_occupancy_with_chat_history();
        let live = live_subagent_list(agent.subagent_sessions.values());
        assert_eq!(
            live.len(),
            1,
            "occupancy drop after rebuild must not wipe still-running nested work; live={live:?}"
        );
        assert_eq!(live[0].child_session_id.as_ref(), "cs-nested-live");
        assert!(!live[0].finished);
    }

    /// Occupancy snapshot always has finished: false. Restore must not
    /// un-finish a host that already exited, and persist-after-finish must
    /// not revive that dead host as a live list row.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_nested_occupancy_does_not_unfinish_or_revive_dead_host() {
        use crate::app::subagent::live_subagent_list;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let sid = "occupancy-restore-dead-host";

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd;
        agent.session.state = AgentState::TurnRunning;
        agent.subagent_sessions.insert(
            "cs-dead-host".into(),
            crate::app::agent_view::AgentView::live_nested_occupancy_row_for_tests(
                "cs-dead-host",
                "sa-dead-host",
                "finished paused implementor",
                Some("implementer"),
            ),
        );
        agent.persist_session_work_to_disk_for_rebuild();
        {
            let info = agent.subagent_sessions.get_mut("cs-dead-host").unwrap();
            info.finished = true;
            info.status = Some(std::sync::Arc::from("completed"));
        }
        agent.restore_nested_occupancy_from_disk();
        let dead = agent
            .subagent_sessions
            .get("cs-dead-host")
            .expect("finished host must stay in the registry");
        assert!(
            dead.finished,
            "/rebuild restore must not un-finish a dead host from occupancy"
        );
        assert_eq!(dead.status.as_deref(), Some("completed"));
        assert!(
            live_subagent_list(agent.subagent_sessions.values()).is_empty(),
            "dead host must not return to the live Subagents list; live={:?}",
            live_subagent_list(agent.subagent_sessions.values())
        );

        agent.persist_session_work_to_disk_for_rebuild();
        agent.subagent_sessions.clear();
        agent.restore_nested_occupancy_from_disk();
        assert!(
            !agent.subagent_sessions.contains_key("cs-dead-host"),
            "persist after finish must not revive a dead host as a running occupancy row"
        );
        assert!(live_subagent_list(agent.subagent_sessions.values()).is_empty());
    }

    /// Named contract: Compact-fail unstick after occupancy drop must not
    /// leave the last Human turn as a Prompt row. Command `/compact` only.
    #[test]
    fn compact_fail_unstick_after_occupancy_drop_requeues_compact_only_not_last_human_turn() {
        use crate::app::actions::{Action, Effect};
        use crate::app::dispatch::dispatch;
        use crate::scrollback::blocks::SessionEvent;

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::Idle;
            agent.session.in_flight_prompt = None;
            agent.session.compact_held_prompt = None;
            agent.session.pending_prompts.clear();
            agent.session_sampling_window = Some(500_000);
            agent.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
                507_000, 500_000,
            ));
            agent
                .scrollback
                .push_block(RenderBlock::user_prompt(LIGHTWAVE));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::session_event(
                    SessionEvent::CompactionFailed {
                        error: "Compaction sampler got a spending-limit response.".into(),
                    },
                ));
        }
        let effects = dispatch(Action::ToggleGlobalPause, &mut app);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.persist_pending_prompts();
            agent.sync_queue_pane();
        }
        let agent = app.agents.get(&id).unwrap();
        let queued = queued_texts(agent);
        let painted = agent.queue.entry_texts();
        let human_prompt = queued.iter().any(|t| t.trim() == LIGHTWAVE)
            || painted.iter().any(|t| t.trim() == LIGHTWAVE)
            || effects
                .iter()
                .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.trim() == LIGHTWAVE));
        assert!(
            !human_prompt,
            "Compact-fail unstick after occupancy drop must not leave the last Human turn as a Prompt row; queue={queued:?} painted={painted:?} effects={effects:?}"
        );
        let compact_only = queued.iter().all(|t| t.trim() == "/compact")
            && effects.iter().any(|e| matches!(e, Effect::Compact { .. }));
        assert!(
            compact_only,
            "Human-turn requeues Compact only; queue={queued:?} effects={effects:?}"
        );
        assert!(
            !painted.iter().any(|t| t.trim() == LIGHTWAVE),
            "the Operator-visible [Send now] pane must not show the last Human turn after compact-fail unstick; painted={painted:?}"
        );
    }

    /// Named contract: `apply_canceled_turn_resume_on_load` after a finished
    /// Human turn in chat history must not `enqueue_prompt_front` that text.
    /// Compact may have removed the UserPrompt from live scrollback.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn session_load_cancel_resume_does_not_enqueue_human_turn_already_in_chat_history() {
        use crate::app::actions::{Action, Effect, TaskResult};
        use crate::app::dispatch::dispatch;
        use agent_client_protocol as acp;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "stale-cancel-resume-chat-history";
        write_session_chat_history(
            &cwd_str,
            sid,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": LIGHTWAVE}],
                })
            ),
        );
        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
        let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
            LIGHTWAVE,
            Some("pid-stale-compacted-resume"),
            "2026-09-09T00:00:00Z",
        )
        .expect("marker");
        xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
            &cwd_str, sid, &marker,
        )
        .expect("write leftover marker");

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        app.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
            assert_eq!(
                agent.scrollback.len(),
                0,
                "Compact emptied UserPrompt from live scrollback"
            );
        }
        let load_effects = dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut app,
        );
        let agent = app.agents.get(&agent_id).unwrap();
        let queued = queued_texts(agent);
        let painted = agent.queue.entry_texts();
        assert!(
            !queued.iter().any(|t| t.trim() == LIGHTWAVE),
            "session load must not enqueue_prompt_front a finished Human turn that is already in chat history; queue={queued:?}"
        );
        assert!(
            !painted.iter().any(|t| t.trim() == LIGHTWAVE),
            "the Operator-visible queue must not show that Human turn as a [Send now] Prompt; painted={painted:?}"
        );
        let refired = load_effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.trim() == LIGHTWAVE));
        assert!(
            !refired,
            "cancel-resume must not re-fire a Human turn already recorded in chat history; effects={load_effects:?}"
        );
        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
    }

    /// Operator: "Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary."
    ///
    /// Last-session open. No `canceled_turn_resume.json`. Chat history already
    /// has the Operator prompt, including `/goal do the thing` recorded as
    /// `A goal has been set: do the thing`. Disk pending has those bodies
    /// plus a truly unsent follow-up. Occupancy already marked the goal
    /// `continue_prior_work`, which the ordinary drop spares. Restore through
    /// session bind must not re-queue the recorded prompts.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn last_session_restore_without_canceled_turn_file_does_not_requeue_recorded_operator_prompt() {
        use crate::app::actions::{Action, Effect, TaskResult};
        use crate::app::dispatch::dispatch;
        use agent_client_protocol as acp;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "last-session-no-canceled-marker";
        let goal = "/goal do the thing";
        let recorded = "already recorded operator prompt";
        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
        let marker =
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .expect("load canceled-turn marker");
        assert!(
            marker.is_none(),
            "fixture is last-session open with no canceled_turn_resume.json"
        );
        xai_grok_shell::session::pending_prompts::write_pending_prompts(
            &cwd_str,
            sid,
            &[
                PersistedQueuedPrompt {
                    id: 1,
                    text: goal.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 2,
                    text: recorded.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 3,
                    text: STILL_UNSENT.to_string(),
                    kind: "prompt".into(),
                },
            ],
        )
        .expect("write pending_prompts.json");
        write_session_chat_history(
            &cwd_str,
            sid,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": recorded}],
                }),
                serde_json::json!({
                    "type": "user",
                    "content": [{
                        "type": "text",
                        "text": "<user_query>\nA goal has been set: do the thing\n</user_query>",
                    }],
                }),
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
            agent.session.prompt_history.clear();
            agent
                .session
                .enqueue_continue_prior_work_front(goal.to_string());
            assert_eq!(
                agent.scrollback.len(),
                0,
                "last-session open starts with empty scrollback"
            );
        }
        let load_effects = dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut app,
        );
        let agent = app.agents.get(&agent_id).unwrap();
        let queued = queued_texts(agent);
        let stale = |text: &str| {
            text.contains(goal) || text.contains("do the thing") || text.trim() == recorded
        };
        assert!(
            !queued.iter().any(|t| stale(t)),
            "Operator: \"Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary.\" last-session restore must not re-queue an Operator prompt already in history, including /goal mapped to A goal has been set; queue={queued:?}"
        );
        assert!(
            !load_effects
                .iter()
                .any(|e| { matches!(e, Effect::SendPrompt { text, .. } if stale(text)) }),
            "Operator: \"Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary.\" last-session restore must not send that recorded prompt; effects={load_effects:?}"
        );
        let kept_unsent = queued.iter().any(|t| t == STILL_UNSENT)
            || load_effects
                .iter()
                .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == STILL_UNSENT));
        assert!(
            kept_unsent,
            "a pending row that is not yet a Human turn must still restore; queue={queued:?} effects={load_effects:?}"
        );
        let still_absent =
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .expect("load marker after restore");
        assert!(
            still_absent.is_none(),
            "this restore must not invent canceled_turn_resume.json"
        );
    }
}
