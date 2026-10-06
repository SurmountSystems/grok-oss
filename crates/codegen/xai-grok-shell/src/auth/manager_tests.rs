//! Unit tests for [`super::AuthManager`]. Wired in via
//! `#[path = "manager_tests.rs"] mod tests;` in `manager.rs`.
//!
//! This file ports the #50 auto-use contract. It is not a checkout of the
//! whole historical test module.

use super::{
    AuthManager, AuthMode, AuthStore, GrokAuth, GrokComConfig, read_auth_json,
    upsert_supergrok_session, write_auth_json,
};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;

fn make_auth(expires_at: Option<DateTime<Utc>>, create_time: DateTime<Utc>) -> GrokAuth {
    GrokAuth {
        auth_mode: AuthMode::External,
        create_time,
        user_id: String::new(),
        expires_at,
        ..GrokAuth::test_default()
    }
}

/// Dual SuperGrok sticky Team base (last business login) for included SuperGrok
/// period rank tests. Rank prefers the personal SuperGrok JWT while both have remaining.
fn dual_supergrok_sticky_team_store(base_scope: &str) -> (GrokAuth, GrokAuth, AuthStore) {
    let personal = GrokAuth {
        key: "tok-personal-free-period".into(),
        auth_mode: AuthMode::Oidc,
        principal_type: Some("User".into()),
        principal_id: Some("536d3388-0735-4e09-bf46-a33da0ed54aa".into()),
        team_id: Some("58c5f686-4270-4d6d-9c3b-df44559f8457".into()),
        user_id: "58c5f686-4270-4d6d-9c3b-df44559f8457".into(),
        ..make_auth(Some(Utc::now() + Duration::hours(1)), Utc::now())
    };
    let team = GrokAuth {
        key: "tok-business-team-base".into(),
        auth_mode: AuthMode::Oidc,
        principal_type: Some("Team".into()),
        principal_id: Some("61fab250-b2c1-40cf-b5b8-628e673a2eeb".into()),
        team_id: Some("61fab250-b2c1-40cf-b5b8-628e673a2eeb".into()),
        user_id: "user-on-team".into(),
        ..make_auth(Some(Utc::now() + Duration::hours(1)), Utc::now())
    };
    let mut map = AuthStore::default();
    upsert_supergrok_session(&mut map, base_scope, personal.clone());
    upsert_supergrok_session(&mut map, base_scope, team.clone());
    (personal, team, map)
}

/// Named contract: sticky personal base stays on the personal SuperGrok JWT
/// when both stored logins still have remaining. Team JWT is not the paying
/// source (Billing Credits / team OAuth settlement).
/// Named contract: with `[auth] auto_use_included_limits = true` (default),
/// AuthManager::new must load ranked included SuperGrok period primary (personal
/// SuperGrok JWT when both stored logins still have remaining), not Team JWT.
#[test]
fn auth_manager_new_auto_use_aligns_sticky_team_base_to_ranked_free_period_primary() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = GrokComConfig::default();
    assert!(
        cfg.auto_use_included_limits,
        "default must prefer included SuperGrok period limits"
    );
    let base_scope = cfg.auth_scope();
    let (personal, team, _) = dual_supergrok_sticky_team_store(&base_scope);
    let mut map = AuthStore::default();
    upsert_supergrok_session(&mut map, &base_scope, team);
    upsert_supergrok_session(&mut map, &base_scope, personal);
    write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

    let mgr = Arc::new(AuthManager::new(dir.path(), cfg).with_proxy_base_url("http://127.0.0.1:1"));
    assert_eq!(
        mgr.current_wire_valid().map(|a| a.key),
        Some("tok-personal-free-period".into()),
        "AuthManager::new with auto_use must wire ranked personal SuperGrok included paying primary, not Team JWT"
    );
    let store = read_auth_json(&dir.path().join("auth.json")).unwrap();
    assert_eq!(
        store.get(&base_scope).map(|a| a.key.as_str()),
        Some("tok-personal-free-period"),
        "disk base after new must match ranked personal SuperGrok included paying primary"
    );
    let trace = mgr
        .session_wire_bearer_trace()
        .expect("wire bearer after new");
    assert_ne!(
        trace["principal_type"], "Team",
        "must not wire Team JWT while a personal SuperGrok login exists: {trace:?}"
    );
}
