//! `/metadata`: live session ids and context as a transcript system block.

use xai_grok_shell::grok_oss::try_open_from_token_economy_config;

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::block::RenderBlock;
use crate::slash::commands::metadata::{SessionMetadataFields, format_session_metadata};

/// Commit a refresh-on-open transcript block of live session metadata.
pub(super) fn dispatch_show_session_metadata(app: &mut AppView) -> Vec<Effect> {
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        let text = slash_report_for_agent(agent);
        agent.scrollback.push_block(RenderBlock::system(text));
    }
    vec![]
}

fn slash_report_for_agent(agent: &crate::app::agent_view::AgentView) -> String {
    let session_uuid = agent
        .session
        .session_id
        .as_ref()
        .map(|id| id.0.to_string())
        .filter(|s| !s.is_empty());
    let session_ulid = session_uuid.as_deref().and_then(lookup_or_map_session_ulid);
    let cwd = {
        let display = agent.session.cwd.display().to_string();
        if display.is_empty() {
            None
        } else {
            Some(display)
        }
    };
    let model = agent.session.models.current_model_name();
    let pid = std::process::id();
    let started = this_window_opened_at(pid);
    let fields = SessionMetadataFields {
        session_ulid,
        session_uuid,
        cwd,
        model,
        started,
        pid,
        ulid_primary: crate::appearance::cache::load_ulid_session_ids(),
    };
    format_session_metadata(&fields)
}

fn lookup_or_map_session_ulid(session_uuid: &str) -> Option<String> {
    let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
    if cfg!(test) && cfg.grok_oss_database_path.is_none() {
        return None;
    }
    let store = try_open_from_token_economy_config(&cfg)?;
    match store.ensure_session_ids(session_uuid) {
        Ok(pair) => Some(pair.session_ulid),
        Err(e) => {
            tracing::debug!(error = %e, session_uuid, "session_id_map lookup failed (fail-open)");
            None
        }
    }
}

fn this_window_opened_at(pid: u32) -> Option<String> {
    let rows = crate::running_sessions::list_running_sessions().ok()?;
    rows.into_iter()
        .find(|row| row.pid == pid)
        .map(|row| row.opened_at.to_rfc3339())
}

/// Named for tests that want the grok_oss module in scope without opening the operator db.
#[cfg(test)]
#[allow(dead_code)]
fn _schema_owner() -> i64 {
    xai_grok_shell::grok_oss::SCHEMA_VERSION
}
