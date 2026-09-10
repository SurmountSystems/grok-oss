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
}
