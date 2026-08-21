//! `/metadata` dispatch: transcript report, not `/session-info`.

use super::*;
use crate::app::actions::Action;
use crate::app::agent::AgentId;

#[test]
fn show_session_metadata_no_active_agent_is_noop() {
    let mut app = test_app();
    let effects = dispatch(Action::ShowSessionMetadata, &mut app);
    assert!(
        effects.is_empty(),
        "ShowSessionMetadata without an agent is a no-op"
    );
}

#[test]
fn show_session_metadata_commits_transcript_block() {
    let mut app = test_app_with_agent();
    let before = agent_scrollback_len(&app);
    let effects = dispatch(Action::ShowSessionMetadata, &mut app);
    assert!(effects.is_empty(), "got: {effects:?}");
    assert_eq!(agent_scrollback_len(&app), before + 1);
    let text = last_system_text(&app, AgentId(0));
    assert!(
        text.contains("Session metadata"),
        "slash dispatch must commit a metadata report; got {text:?}"
    );
    assert!(
        text.contains("pid:"),
        "metadata report must include pid; got {text:?}"
    );
    assert!(
        !text.contains('\u{2014}'),
        "no em dashes in metadata report; got {text:?}"
    );
}

#[test]
#[serial_test::serial(TOKEN_ECONOMY_LIVE)]
fn show_session_metadata_maps_uuid_and_shows_ulid_when_db_overridden() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db = tmp.path().join("grok_oss.db");
    xai_grok_shell::token_economy::set_token_economy_live(
        xai_grok_shell::token_economy::TokenEconomyConfig {
            grok_oss_database_path: Some(db.clone()),
            ..xai_grok_shell::token_economy::TokenEconomyConfig::default()
        },
    );
    struct Guard {
        prev_ulid_primary: bool,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            xai_grok_shell::token_economy::reset_token_economy_live_to_defaults();
            crate::appearance::cache::set_ulid_session_ids(self.prev_ulid_primary);
        }
    }
    let prev_ulid_primary = crate::appearance::cache::load_ulid_session_ids();
    let _guard = Guard { prev_ulid_primary };

    let uuid = "018f1e2a-3b4c-7d8e-9f01-23456789abcd".to_string();
    let mut app = test_app_with_agent();
    app.agents.get_mut(&AgentId(0)).unwrap().session.session_id =
        Some(acp::SessionId::new(uuid.clone()));
    crate::appearance::cache::set_ulid_session_ids(true);

    let effects = dispatch(Action::ShowSessionMetadata, &mut app);
    assert!(effects.is_empty(), "got: {effects:?}");
    let text = last_system_text(&app, AgentId(0));
    assert!(
        text.contains("Session metadata"),
        "slash dispatch must commit a metadata report; got {text:?}"
    );
    assert!(
        text.contains("grok-oss ULID:"),
        "overridden grok_oss.db must map a ULID; got {text:?}"
    );
    assert!(
        text.contains(&format!("Grok Build UUID: {uuid}")),
        "wire UUID must stay visible; got {text:?}"
    );
    let ulid_at = text.find("grok-oss ULID").expect("ulid line");
    let uuid_at = text.find("Grok Build UUID").expect("uuid line");
    assert!(
        ulid_at < uuid_at,
        "toggle on lists ULID first; got {text:?}"
    );

    let store = xai_grok_shell::grok_oss::open_at(&db).expect("open override db");
    let pair = store
        .lookup_by_uuid(&uuid)
        .expect("lookup")
        .expect("session_id_map row");
    assert_eq!(pair.session_uuid, uuid);
    assert_eq!(pair.session_ulid.len(), 26);
    assert!(text.contains(&pair.session_ulid), "got {text:?}");
    assert_eq!(
        app.agents[&AgentId(0)]
            .session
            .session_id
            .as_ref()
            .map(|id| id.0.to_string())
            .as_deref(),
        Some(uuid.as_str()),
        "wire session id must stay the UUID"
    );

    crate::appearance::cache::set_ulid_session_ids(false);
    let _ = dispatch(Action::ShowSessionMetadata, &mut app);
    let off = last_system_text(&app, AgentId(0));
    let ulid_off = off.find("grok-oss ULID").expect("ulid line off");
    let uuid_off = off.find("Grok Build UUID").expect("uuid line off");
    assert!(
        uuid_off < ulid_off,
        "toggle off lists UUID first; got {off:?}"
    );
}
