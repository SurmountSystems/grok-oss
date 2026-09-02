//! Drain `$GROK_HOME/l0-enqueue/<session_id>/enqueue.json` into this window.
//!
//! `surmount-coordinator-gui::write_enqueue` writes that drop file. This
//! pager reads it for **this** window's session id, turns it into one human
//! prompt on the same local queue as composer send (`pending_prompts`), then
//! consumes the file so it cannot fire twice. Other session ids are ignored.
//! A missing file is a no-op. The prompt is not written to
//! `active_sessions.json`. L0 is not `/dashboard`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::actions::Effect;
use super::agent::AgentId;
use super::agent_view::AgentView;
use super::app_view::AppView;

const ENQUEUE_DIR: &str = "l0-enqueue";
const ENQUEUE_FILE: &str = "enqueue.json";
const TAKING_FILE: &str = "enqueue.json.taking";

#[derive(Deserialize)]
struct EnqueueDrop {
    prompt: String,
}

/// `{grok_home}/l0-enqueue/{session_id}/enqueue.json`. `None` when
/// `session_id` is empty or not a single path component.
pub(crate) fn enqueue_drop_path(grok_home: &Path, session_id: &str) -> Option<PathBuf> {
    let sid = sanitize_session_id(session_id)?;
    Some(grok_home.join(ENQUEUE_DIR).join(sid).join(ENQUEUE_FILE))
}

fn sanitize_session_id(session_id: &str) -> Option<&str> {
    let sid = session_id.trim();
    if sid.is_empty() || sid == "." || sid == ".." {
        return None;
    }
    if sid.contains('/') || sid.contains('\\') || sid.contains('\0') {
        return None;
    }
    Some(sid)
}

/// Read and consume the drop file for `session_id`. Returns the prompt when
/// present and non-empty. Missing file is `None`. After a successful consume
/// the drop file is gone (renamed aside, then deleted).
pub(crate) fn consume_l0_enqueue(grok_home: &Path, session_id: &str) -> Option<String> {
    let path = enqueue_drop_path(grok_home, session_id)?;
    let taking = path.with_file_name(TAKING_FILE);
    std::fs::rename(&path, &taking).ok()?;
    let raw = std::fs::read_to_string(&taking);
    let _ = std::fs::remove_file(&taking);
    let raw = raw.ok()?;
    let parsed: EnqueueDrop = serde_json::from_str(&raw).ok()?;
    let prompt = parsed.prompt;
    if prompt.trim().is_empty() {
        return None;
    }
    Some(prompt)
}

/// Queue the drop for this agent's bound session id, if any. Does not start
/// a turn; callers that already drain `pending_prompts` keep doing so.
pub(crate) fn drain_into_agent(agent: &mut AgentView, grok_home: &Path) -> bool {
    let Some(sid) = agent.session.session_id.as_ref() else {
        return false;
    };
    let Some(text) = consume_l0_enqueue(grok_home, sid.0.as_ref()) else {
        return false;
    };
    agent.append_prompt_wal(
        xai_grok_shell::session::prompt_wal::PromptWalKind::Queue,
        &text,
        &[],
    );
    agent.session.enqueue_prompt(text);
    agent.persist_pending_prompts();
    true
}

/// Drain every bound agent in this window. Ignores drop files whose session
/// id is not this window. Then tries the same queue drain as composer send.
/// `None` when no drop was consumed.
pub(crate) fn drain_into_app(app: &mut AppView) -> Option<Vec<Effect>> {
    let home = xai_grok_config::grok_home();
    drain_into_app_at(app, &home)
}

fn drain_into_app_at(app: &mut AppView, grok_home: &Path) -> Option<Vec<Effect>> {
    let ids: Vec<AgentId> = app.agents.keys().copied().collect();
    let mut consumed = false;
    let mut effects = Vec::new();
    for id in ids {
        let Some(agent) = app.agents.get_mut(&id) else {
            continue;
        };
        if !drain_into_agent(agent, grok_home) {
            continue;
        }
        consumed = true;
        effects.extend(crate::app::dispatch::maybe_drain_queue_and_note_peek(
            app, id,
        ));
    }
    consumed.then_some(effects)
}

/// Session bind: pull a waiting L0 drop into `pending_prompts` so the
/// following `maybe_drain_queue` can send it. Tests skip disk so they do
/// not read the operator grok home.
pub(crate) fn drain_into_agent_on_bind(agent: &mut AgentView) {
    if cfg!(test) {
        return;
    }
    let home = xai_grok_config::grok_home();
    let _ = drain_into_agent(agent, &home);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::make_agent_view;
    use std::fs;

    fn write_drop(home: &Path, session_id: &str, prompt: &str) -> PathBuf {
        let path = enqueue_drop_path(home, session_id).expect("safe session id");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("enqueue dir");
        }
        let body = serde_json::json!({ "prompt": prompt });
        fs::write(&path, serde_json::to_vec_pretty(&body).expect("json")).expect("write drop");
        path
    }

    #[test]
    fn drain_l0_enqueue_for_this_session_id_queues_one_human_line() {
        let home = tempfile::tempdir().expect("home");
        let mut agent = make_agent_view(Some("sess-this"), "/tmp");
        let path = write_drop(home.path(), "sess-this", "do the selected work");
        write_drop(home.path(), "sess-other", "must not run");

        assert!(drain_into_agent(&mut agent, home.path()));
        assert_eq!(agent.session.pending_prompts.len(), 1);
        assert_eq!(
            agent.session.pending_prompts[0].text,
            "do the selected work"
        );
        assert!(
            !path.exists(),
            "this session's drop file must be consumed so it cannot fire twice"
        );
        assert!(
            enqueue_drop_path(home.path(), "sess-other")
                .expect("other path")
                .exists(),
            "other session drop must stay until that window drains"
        );
        assert!(
            !home.path().join("active_sessions.json").exists(),
            "L0 drain must not write the prompt into active_sessions.json"
        );
        assert!(!drain_into_agent(&mut agent, home.path()));
        assert_eq!(agent.session.pending_prompts.len(), 1);
    }

    #[test]
    fn drain_l0_enqueue_ignores_other_session_id() {
        let home = tempfile::tempdir().expect("home");
        let mut agent = make_agent_view(Some("sess-this"), "/tmp");
        let other = write_drop(home.path(), "sess-other", "foreign prompt");

        assert!(!drain_into_agent(&mut agent, home.path()));
        assert!(
            agent.session.pending_prompts.is_empty(),
            "this window must ignore another session's drop; queue={:?}",
            agent
                .session
                .pending_prompts
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
        );
        assert!(other.exists(), "ignored drop file must not be consumed");
    }

    /// Surmount / grok-oss fork; tests are contracts.
    /// L0 drain enqueue writes a `queue` WAL line before the model is asked.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn prompt_wal_appends_on_queue_enqueue() {
        let grok_home = tempfile::tempdir().expect("home");
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().expect("cwd");
        let cwd = proj.path().to_string_lossy().into_owned();
        let sid = "wal-queue-enqueue";
        let body = "l0 queued prompt that must hit the WAL";
        let mut agent = make_agent_view(Some(sid), &cwd);
        write_drop(grok_home.path(), sid, body);

        assert!(drain_into_agent(&mut agent, grok_home.path()));
        assert_eq!(agent.session.pending_prompts[0].text, body);
        let rows =
            xai_grok_shell::session::prompt_wal::load_prompt_wal(&cwd, sid).expect("load WAL");
        assert!(
            rows.iter().any(|r| {
                r.kind == xai_grok_shell::session::prompt_wal::PromptWalKind::Queue
                    && r.text == body
            }),
            "prompt_wal.jsonl must contain the queued body, got {rows:?}"
        );
    }

    #[test]
    fn drain_l0_enqueue_missing_file_is_noop() {
        let home = tempfile::tempdir().expect("home");
        let mut agent = make_agent_view(Some("sess-this"), "/tmp");

        assert!(!drain_into_agent(&mut agent, home.path()));
        assert!(agent.session.pending_prompts.is_empty());
        assert!(
            !home.path().join(ENQUEUE_DIR).exists(),
            "missing drop must not create l0-enqueue"
        );
    }
}
