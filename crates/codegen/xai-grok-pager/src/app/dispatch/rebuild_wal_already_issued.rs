//! Rebuild WAL restore: do not enqueue committed user turns.
//!
//! `/goal <rest>` matches history `A goal has been set: <rest>` inside
//! `<user_query>`. Quoted WAL bodies match JSONL escaped quotes. A send
//! that is not a parsed user turn still restores.

#[cfg(test)]
mod tests {
    use xai_grok_shell::session::pending_prompts::PersistedQueuedPrompt;

    fn write_session_chat_history(cwd: &str, sid: &str, jsonl: &str) {
        let path = xai_grok_shell::session::prompt_wal::chat_history_path(cwd, sid)
            .expect("chat_history.jsonl path");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, jsonl).unwrap();
    }

    fn append_wal_send(cwd: &str, sid: &str, body: &str) {
        let record = xai_grok_shell::session::prompt_wal::PromptWalRecord::new(
            sid,
            xai_grok_shell::session::prompt_wal::PromptWalKind::Send,
            body,
            Vec::new(),
        );
        xai_grok_shell::session::prompt_wal::append_prompt_wal(cwd, sid, &record)
            .expect("write WAL");
    }

    /// Grok OSS Named contract: after `/rebuild`, do not enqueue WAL Send bodies that
    /// are already committed user turns. A truly missing WAL send still
    /// restores. This diverges from upstream xAI because the catalog WAL table pins
    /// restore and skip tests as contracts, not known-good from 2026-09-02.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_prompt_wal_does_not_enqueue_committed_goal_slash_or_quoted_send() {
        use crate::app::agent::AgentId;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "wal-restore-skip-committed";
        let goal_wal = "/goal run just check-remote";
        let quoted_wal = r#"say "noted""#;
        let missing_wal = "operator send that never reached chat history";
        append_wal_send(&cwd_str, sid, goal_wal);
        append_wal_send(&cwd_str, sid, quoted_wal);
        append_wal_send(&cwd_str, sid, missing_wal);
        write_session_chat_history(
            &cwd_str,
            sid,
            concat!(
                r#"{"type":"user","content":[{"type":"text","text":"<user_query>\n# /goal -- pursue an objective\n\nA goal has been set: run just check-remote\n\nStart now.\n</user_query>"}]}"#,
                "\n",
                r#"{"type":"user","content":[{"type":"text","text":"say \"noted\""}]}"#,
                "\n",
                r#"{"type":"assistant","content":"please do not treat this as a user turn: operator send that never reached chat history"}"#,
                "\n",
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.prompt_history.clear();
            agent.session.pending_prompts.clear();
            agent.restore_prompt_wal_from_disk();
            let queued: Vec<&str> = agent
                .session
                .pending_prompts
                .iter()
                .map(|p| p.text.as_str())
                .collect();
            assert!(
                !queued.contains(&goal_wal),
                "pager bind must not enqueue a WAL /goal send already recorded as A goal has been set; queue={queued:?}"
            );
            assert!(
                !queued.contains(&quoted_wal),
                "pager bind must not enqueue a WAL send whose quoted body is already a JSONL user turn; queue={queued:?}"
            );
            assert!(
                queued.contains(&missing_wal),
                "a WAL send whose body is truly absent from parsed user text must still restore; queue={queued:?}"
            );
        }
    }

    fn append_wal_kind(
        cwd: &str,
        sid: &str,
        kind: xai_grok_shell::session::prompt_wal::PromptWalKind,
        body: &str,
    ) {
        let record =
            xai_grok_shell::session::prompt_wal::PromptWalRecord::new(sid, kind, body, Vec::new());
        xai_grok_shell::session::prompt_wal::append_prompt_wal(cwd, sid, &record)
            .expect("write WAL");
    }

    /// Grok OSS Named contract: WAL Interject and Queue kinds that are already parsed
    /// user turns in `chat_history.jsonl` must not restore into the pager
    /// queue. Do not weaken Send, `/goal`, or quoted-body skip tests. This diverges
    /// from upstream xAI because the catalog WAL table pins restore and skip tests.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_prompt_wal_does_not_enqueue_committed_interject_or_queue() {
        use crate::app::agent::AgentId;
        use xai_grok_shell::session::prompt_wal::PromptWalKind;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "wal-restore-skip-interject-queue";
        let issued_interject = "BTW names follow-up that already ran as a Human turn";
        let issued_queue =
            "Convert this LightWave 3D scene to a sprite sheet with two extra lines.";
        let missing_interject = "operator interject that never reached chat history";
        append_wal_kind(&cwd_str, sid, PromptWalKind::Interject, issued_interject);
        append_wal_kind(&cwd_str, sid, PromptWalKind::Queue, issued_queue);
        append_wal_kind(&cwd_str, sid, PromptWalKind::Interject, missing_interject);
        write_session_chat_history(
            &cwd_str,
            sid,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": issued_interject}],
                }),
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": issued_queue}],
                }),
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.prompt_history.clear();
            agent.session.pending_prompts.clear();
            agent.restore_prompt_wal_from_disk();
            let queued: Vec<&str> = agent
                .session
                .pending_prompts
                .iter()
                .map(|p| p.text.as_str())
                .collect();
            assert!(
                !queued.contains(&issued_interject),
                "pager bind must not enqueue a WAL Interject already recorded as a Human turn; queue={queued:?}"
            );
            assert!(
                !queued.contains(&issued_queue),
                "pager bind must not enqueue a WAL Queue body already recorded as a Human turn; queue={queued:?}"
            );
            assert!(
                queued.contains(&missing_interject),
                "a WAL Interject whose body is truly absent from parsed user text must still restore; queue={queued:?}"
            );
        }
    }

    /// Named contract: `pending_prompts.json` rows that are already committed
    /// user turns must not restore into the pager queue after `/rebuild`.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn restore_pending_prompts_skips_bodies_already_recorded_in_chat_history() {
        use crate::app::agent::AgentId;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "pending-skip-committed";
        let goal_wal = "/goal run just check-remote";
        let quoted_wal = r#"say "noted""#;
        let still_queued = "follow-up that is not yet a user turn";
        xai_grok_shell::session::pending_prompts::write_pending_prompts(
            &cwd_str,
            sid,
            &[
                PersistedQueuedPrompt {
                    id: 1,
                    text: goal_wal.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 2,
                    text: quoted_wal.to_string(),
                    kind: "prompt".into(),
                },
                PersistedQueuedPrompt {
                    id: 3,
                    text: still_queued.to_string(),
                    kind: "prompt".into(),
                },
            ],
        )
        .expect("write pending_prompts.json");
        write_session_chat_history(
            &cwd_str,
            sid,
            concat!(
                r#"{"type":"user","content":[{"type":"text","text":"<user_query>\nA goal has been set: run just check-remote\n</user_query>"}]}"#,
                "\n",
                r#"{"type":"user","content":[{"type":"text","text":"say \"noted\""}]}"#,
                "\n",
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.prompt_history.clear();
            agent.session.pending_prompts.clear();
            agent.restore_pending_prompts_from_disk();
            let queued: Vec<&str> = agent
                .session
                .pending_prompts
                .iter()
                .map(|p| p.text.as_str())
                .collect();
            assert!(
                !queued.iter().any(|t| *t == goal_wal || *t == quoted_wal),
                "rebuild persist skip: issued pending_prompts bodies already in chat history must not restore; queue={queued:?}"
            );
            assert!(
                queued.contains(&still_queued),
                "a pending_prompts row that is not a committed user turn must still restore; queue={queued:?}"
            );
        }
    }

    /// Named contract: a second `/rebuild` must not write stale issued queue
    /// bodies into `pending_prompts.json` or rebuild-flush WAL. Unsent rows
    /// still persist.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn persist_rebuild_skips_issued_queue_rows_so_a_second_rebuild_does_not_write_stale_wal() {
        use crate::app::agent::AgentId;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "rebuild-persist-skip-issued";
        let goal_wal = "/goal run just check-remote";
        let quoted_wal = r#"say "noted""#;
        let still_queued = "follow-up that is not yet a user turn";
        write_session_chat_history(
            &cwd_str,
            sid,
            concat!(
                r#"{"type":"user","content":[{"type":"text","text":"<user_query>\nA goal has been set: run just check-remote\n</user_query>"}]}"#,
                "\n",
                r#"{"type":"user","content":[{"type":"text","text":"say \"noted\""}]}"#,
                "\n",
            ),
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.enqueue_prompt(goal_wal.into());
            agent.session.enqueue_prompt(quoted_wal.into());
            agent.session.enqueue_prompt(still_queued.into());
            agent.persist_session_work_to_disk_for_rebuild();
        }

        let rows = xai_grok_shell::session::pending_prompts::load_pending_prompts(&cwd_str, sid)
            .expect("load queue");
        assert!(
            !rows
                .iter()
                .any(|r| r.text == goal_wal || r.text == quoted_wal),
            "second rebuild persist must not write issued /goal or quoted bodies into pending_prompts.json; got {rows:?}"
        );
        assert!(
            rows.iter().any(|r| r.text == still_queued),
            "second rebuild persist must still write a truly unsent queue row; got {rows:?}"
        );
        let wal =
            xai_grok_shell::session::prompt_wal::load_prompt_wal(&cwd_str, sid).expect("load WAL");
        assert!(
            !wal.iter().any(|r| {
                r.kind == xai_grok_shell::session::prompt_wal::PromptWalKind::RebuildFlush
                    && (r.text == goal_wal || r.text == quoted_wal)
            }),
            "second rebuild persist must not append rebuild-flush WAL for issued queue bodies; got {wal:?}"
        );
        assert!(
            wal.iter().any(|r| {
                r.kind == xai_grok_shell::session::prompt_wal::PromptWalKind::RebuildFlush
                    && r.text == still_queued
            }),
            "second rebuild persist must still append rebuild-flush WAL for unsent queue text; got {wal:?}"
        );
    }
}
