//! Apply client-wide `[inference]` / `GROK_API_*` / `OPENAI_*` onto sampler credentials.
//!
//! When endpoint+key resolve, every caller of `resolve_credentials` uses them.
//! Nested agents inherit the parent `SamplerConfig` that this overlay already filled.
//! When they are unset, SuperGrok / console ranking is unchanged.

use super::config::ResolvedCredentials;
use xai_chat_state::AuthType;
use xai_grok_config::ResolvedInference;
use xai_grok_sampler::AuthScheme;

pub(crate) fn credentials_from_inference(inf: ResolvedInference) -> ResolvedCredentials {
    ResolvedCredentials {
        api_key: Some(inf.api_key),
        failover_api_keys: Vec::new(),
        base_url: inf.endpoint,
        auth_type: AuthType::ApiKey,
        auth_scheme: AuthScheme::Bearer,
        failover_base_url: None,
        session_base_url: None,
        session_identity_key: None,
    }
}

/// Overlay a resolved inference model id onto the sampler config when set.
pub(crate) fn overlay_inference_model(model: &mut String) {
    if let Some(id) = xai_grok_config::resolve_configured_inference().and_then(|inf| inf.model) {
        *model = id;
    }
}

/// True when client-wide `[inference]` / `GROK_API_*` / `OPENAI_*` resolved and
/// `api_key` is that Bearer key. An OIDC `preferred_method` pin must not clear
/// it. Same keep as OpenRouter: that host does not take an xAI session.
pub(crate) fn inference_request_keeps_resolved_api_key(api_key: Option<&str>) -> bool {
    xai_grok_config::resolve_configured_inference().is_some_and(|inf| {
        api_key.is_some_and(|key| !key.trim().is_empty() && key.trim() == inf.api_key)
    })
}

/// Ranked SuperGrok / console auto credentials for one reconstruct turn.
/// Callers that already resolved `[inference]` must not invoke this.
pub(crate) fn apply_ranked_auto_turn_credentials(
    home: &std::path::Path,
    api_key: &mut Option<String>,
    failover: &mut Vec<String>,
    session_identity: &mut Option<String>,
) {
    let sessions = crate::auth::load_supergrok_session_candidates(home);
    let console = crate::auth::xai_console::load_stored_console_api_keys(
        &crate::auth::credentials_store::CredentialsStore::at_grok_home(home),
    )
    .unwrap_or_default();
    let order = crate::auth::supergrok_identity_rank::order_credentials_for_preferred_auto(
        &sessions, &console,
    );
    if let Some(primary) = order.primary {
        *api_key = Some(primary);
    }
    *failover = order.failover;
    if let Some(identity) = order.session_identity_key {
        *session_identity = Some(identity);
    }
}

/// Live `reconstruct_full_config` overlay: ranked auto then limits pins.
/// When `[inference]` endpoint+key resolve, skip ranked auto so the custom
/// Bearer key stays. Limits pins still run (they early-return on resolved
/// inference). Unset inference still applies ranked auto.
pub(crate) fn apply_reconstruct_turn_credentials(
    sampling: &mut xai_grok_sampler::SamplerConfig,
    ranked_auto_home: Option<&std::path::Path>,
) {
    if let Some(home) = ranked_auto_home
        && xai_grok_config::resolve_configured_inference().is_none()
    {
        apply_ranked_auto_turn_credentials(
            home,
            &mut sampling.api_key,
            &mut sampling.failover_api_keys,
            &mut sampling.session_identity_key,
        );
    }
    crate::auth::limits_pins::apply_limits_pins_to_sampler_config(sampling);
}

/// SamplingClient stamps InheritedSessionBearerResolver as the sole auth
/// source. A resolved inference request keeps SamplerConfig.api_key
/// (`AuthScheme::Bearer`) instead of the AuthManager SuperGrok JWT.
pub(crate) fn reconstruct_installs_inherited_session_bearer_resolver(
    use_bearer_resolver: bool,
) -> bool {
    use_bearer_resolver && xai_grok_config::resolve_configured_inference().is_none()
}

/// Live `reconstruct_full_config` `align_to_ranked_free_period_primary`.
/// When `[inference]` endpoint+key resolve, skip so AuthManager is not
/// aligned to a SuperGrok JWT that the seed would then copy. Unset
/// inference still aligns on default limits mode (`auto_use_included_limits`
/// on, `preferred_method` not `api_key`).
pub(crate) fn reconstruct_aligns_ranked_free_period_primary(
    use_bearer_resolver: bool,
    auto_use_included_limits: bool,
    preferred_method_is_api_key: bool,
) -> bool {
    reconstruct_installs_inherited_session_bearer_resolver(use_bearer_resolver)
        && auto_use_included_limits
        && !preferred_method_is_api_key
}

/// Live `reconstruct_full_config` api_key seed. When `[inference]` endpoint+key
/// resolve, keep `creds.api_key` (the custom Bearer key). Do not copy
/// `AuthManager.current_wire_valid()` onto SamplerConfig. Unset inference still
/// seeds the session JWT when the session gate is on.
pub(crate) fn reconstruct_turn_api_key(
    use_bearer_resolver: bool,
    auth_manager_jwt: Option<String>,
    creds_api_key: Option<String>,
) -> Option<String> {
    let creds_api_key = creds_api_key.filter(|key| !key.is_empty());
    if use_bearer_resolver && xai_grok_config::resolve_configured_inference().is_none() {
        auth_manager_jwt
            .filter(|key| !key.is_empty())
            .or(creds_api_key)
    } else {
        creds_api_key
    }
}

/// Live `refresh_token_if_expired` session-JWT path. True only when the session
/// gate is on and `[inference]` is unset. A resolved inference request must not
/// call `AuthManager.get_valid_token` or strip chat-state credentials.
pub(crate) fn refresh_token_applies_session_jwt(session_gate_active: bool) -> bool {
    session_gate_active && xai_grok_config::resolve_configured_inference().is_none()
}

/// Live `refresh_token_if_expired` overwrite. Returns `Some` JWT only when
/// chat-state credentials should be replaced. When `[inference]` resolves,
/// returns `None` so the custom Bearer key stays. Unset inference still
/// overwrites when the session gate is on and the JWT differs.
pub(crate) fn refresh_token_overwrite_chat_api_key(
    session_gate_active: bool,
    current_chat_api_key: Option<&str>,
    auth_manager_jwt: &str,
) -> Option<String> {
    if !refresh_token_applies_session_jwt(session_gate_active) {
        return None;
    }
    if current_chat_api_key == Some(auth_manager_jwt) {
        return None;
    }
    Some(auth_manager_jwt.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::{
        EndpointsConfig, ModelEntry, resolve_credentials, sampling_config_for_model,
    };
    use serial_test::serial;
    use xai_grok_config::{GROK_API_BASE_ENV, GROK_API_KEY_ENV};
    use xai_grok_test_support::EnvGuard;

    fn isolated_home() -> (
        tempfile::TempDir,
        EnvGuard,
        EnvGuard,
        EnvGuard,
        EnvGuard,
        EnvGuard,
        EnvGuard,
    ) {
        let dir = tempfile::tempdir().expect("temp GROK_HOME");
        let home = EnvGuard::set("GROK_HOME", dir.path());
        let openai_base = EnvGuard::unset("OPENAI_BASE_URL");
        let openai_key = EnvGuard::unset("OPENAI_API_KEY");
        let xai = EnvGuard::unset("XAI_API_KEY");
        let grok_config = EnvGuard::unset("GROK_CONFIG");
        let grok_config_path = EnvGuard::unset("GROK_CONFIG_PATH");
        (
            dir,
            home,
            openai_base,
            openai_key,
            xai,
            grok_config,
            grok_config_path,
        )
    }

    #[test]
    #[serial]
    fn unset_inference_endpoint_keeps_existing_supergrok_or_console_path() {
        let (_dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        let model = ModelEntry::fallback("session-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-fixture"));
        assert_eq!(creds.api_key.as_deref(), Some("session-jwt-fixture"));
        assert_eq!(creds.auth_type, AuthType::SessionToken);
        assert_ne!(
            creds.base_url.trim_end_matches('/'),
            "https://compat.example.test/v1",
            "unset inference must not replace the SuperGrok or console host"
        );
    }

    #[test]
    #[serial]
    fn inference_env_overrides_session_credentials_for_the_whole_client() {
        let (_dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-client-key");
        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        assert_eq!(creds.api_key.as_deref(), Some("fixture-client-key"));
        assert_eq!(creds.base_url, "https://compat.example.test/v1");
        assert_eq!(creds.auth_type, AuthType::ApiKey);
        assert_eq!(creds.auth_scheme, AuthScheme::Bearer);
        let sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        assert_eq!(sampling.base_url, "https://compat.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-client-key"));
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
    }

    /// Operator: `~/.grok/config.toml` `[inference]` must reach the live credential
    /// seam, not only the parse helpers. Env is unset so the file is the source.
    #[test]
    #[serial]
    fn inference_config_toml_reaches_the_whole_client() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        std::fs::write(
            dir.path().join("config.toml"),
            r#"
[inference]
endpoint = "https://from-file.example.test/v1/"
api_key = "fixture-file-key"
model = "file-model"
"#,
        )
        .expect("write $GROK_HOME/config.toml");
        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        assert_eq!(creds.api_key.as_deref(), Some("fixture-file-key"));
        assert_eq!(creds.base_url, "https://from-file.example.test/v1");
        assert_eq!(creds.auth_type, AuthType::ApiKey);
        assert_eq!(creds.auth_scheme, AuthScheme::Bearer);
        let sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        assert_eq!(sampling.base_url, "https://from-file.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-file-key"));
        assert_eq!(sampling.model, "file-model");
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
    }

    /// Nested spawn copies parent sampling then picks a bearer. Inference must
    /// win over a SuperGrok JWT so nested agents hit the same custom endpoint.
    #[test]
    #[serial]
    fn nested_request_bearer_uses_inference_key_not_supergrok_jwt() {
        let (_dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        let got = crate::agent::auth_method::nested_request_bearer(
            Some("stale-chat-jwt".into()),
            Some("recovered-am-token".into()),
            Some("inherited-jwt".into()),
        );
        assert_eq!(got.as_deref(), Some("fixture-inference-key"));
    }

    fn store_live_personal_session(home: &std::path::Path, token: &str) {
        use crate::auth::{AuthMode, GrokAuth, upsert_supergrok_session};
        let live = chrono::Utc::now() + chrono::Duration::days(7);
        let mut map = std::collections::BTreeMap::new();
        upsert_supergrok_session(
            &mut map,
            "https://auth.x.ai::inference-limits-pin",
            GrokAuth {
                key: token.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "u-inference-pin".into(),
                principal_type: Some("User".into()),
                principal_id: Some("u-inference-pin".into()),
                team_id: None,
                expires_at: Some(live),
                ..GrokAuth::default()
            },
        );
        std::fs::write(
            home.join("auth.json"),
            serde_json::to_vec_pretty(&map).expect("auth.json"),
        )
        .expect("write auth.json");
    }

    /// Operator: when `[inference]` endpoint+key (or GROK_API_BASE / GROK_API_KEY)
    /// resolve, the live TUI/CLI turn must keep that custom host and Bearer API
    /// key. `apply_limits_pins_to_sampler_config` must not replace them with a
    /// stored personal SuperGrok JWT.
    #[test]
    #[serial]
    fn apply_limits_pins_does_not_replace_resolved_inference_endpoint_and_key() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        crate::auth::limits_pins::apply_limits_pins_to_sampler_config(&mut sampling);

        assert_eq!(sampling.base_url, "https://compat.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-inference-key"));
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
        assert_ne!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "limits pins must not POST a SuperGrok JWT to the custom inference host"
        );
        assert_ne!(
            sampling.session_identity_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "limits pins must not install the SuperGrok JWT as the session identity on a custom host"
        );
    }

    /// Operator: when endpoint+key are unset, existing SuperGrok / console
    /// behavior stays, including limits-pin SuperGrok session install.
    #[test]
    #[serial]
    fn unset_inference_still_applies_limits_pins_for_stored_supergrok_session() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("session-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-fixture"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        crate::auth::limits_pins::apply_limits_pins_to_sampler_config(&mut sampling);

        assert_eq!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "unset inference must still let limits pins install the stored SuperGrok session"
        );
        assert_ne!(
            sampling.base_url.trim_end_matches('/'),
            "https://compat.example.test/v1",
            "unset inference must not replace the SuperGrok or console host"
        );
    }

    /// Operator: keep an OIDC `preferred_method` pin from clearing a custom
    /// inference key the same way OpenRouter already keeps a resolved key.
    #[test]
    #[serial]
    fn oidc_pin_keeps_a_resolved_inference_api_key() {
        let (_dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        {
            let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1");
            let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
            assert!(inference_request_keeps_resolved_api_key(Some(
                "fixture-inference-key"
            )));
            assert!(!inference_request_keeps_resolved_api_key(None));
            assert!(!inference_request_keeps_resolved_api_key(Some("   ")));
            assert!(!inference_request_keeps_resolved_api_key(Some("other-key")));
        }
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        assert!(
            !inference_request_keeps_resolved_api_key(Some("fixture-inference-key")),
            "unset inference must not keep a key from the OIDC clear"
        );
    }

    /// Operator: when `[inference]` endpoint+key (or GROK_API_BASE / GROK_API_KEY)
    /// resolve, the live TUI/CLI turn must keep that custom host and Bearer API
    /// key. `reconstruct_full_config` must not run ranked auto before the
    /// limits-pin early return and overwrite `sampling.api_key` from stored
    /// SuperGrok sessions. This is the entire-client contract, including a
    /// stored SuperGrok session with default limits mode.
    #[test]
    #[serial]
    fn ranked_auto_turn_credentials_does_not_replace_resolved_inference_key() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        // Live prepare_sampler_for_turn / prepare_chat_completion order for a
        // logged-in SuperGrok operator (auto_use_included_limits on,
        // preferred_method unset): refresh_token_if_expired, then reconstruct
        // AuthManager seed and align, then ranked auto / limits pins.
        // Operator: entire client inference over an endpoint and an API key.
        assert!(
            !refresh_token_applies_session_jwt(true),
            "resolved inference must not write a SuperGrok JWT into chat-state credentials"
        );
        assert_eq!(
            refresh_token_overwrite_chat_api_key(
                true,
                sampling.api_key.as_deref(),
                "stored-personal-supergrok-jwt",
            ),
            None,
            "refresh_token_if_expired must not replace the inference Bearer key with a SuperGrok JWT"
        );
        sampling.api_key = reconstruct_turn_api_key(
            true,
            Some("stored-personal-supergrok-jwt".into()),
            sampling.api_key.clone(),
        );
        assert!(
            !reconstruct_aligns_ranked_free_period_primary(true, true, false),
            "resolved inference must not call align_to_ranked_free_period_primary"
        );
        assert!(
            !reconstruct_installs_inherited_session_bearer_resolver(true),
            "resolved inference must not install InheritedSessionBearerResolver"
        );
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert_eq!(sampling.base_url, "https://compat.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-inference-key"));
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
        assert_ne!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "ranked auto must not POST a SuperGrok JWT to the custom inference host"
        );
        assert_ne!(
            sampling.session_identity_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "ranked auto must not install the SuperGrok JWT as the session identity on a custom host"
        );
        assert!(
            !sampling
                .failover_api_keys
                .iter()
                .any(|k| k == "stored-personal-supergrok-jwt"),
            "ranked auto must not queue the SuperGrok JWT as failover on a custom host"
        );
    }

    /// Operator: SamplingClient stamps InheritedSessionBearerResolver as the
    /// sole auth source. A resolved inference request must keep the Bearer key
    /// on SamplerConfig instead of the AuthManager SuperGrok JWT.
    #[test]
    #[serial]
    fn inherited_session_bearer_resolver_is_not_installed_when_inference_is_resolved() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert!(
            !reconstruct_installs_inherited_session_bearer_resolver(true),
            "resolved inference must not install InheritedSessionBearerResolver"
        );
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-inference-key"));
        assert!(
            sampling.bearer_resolver.is_none(),
            "reconstruct overlay must leave bearer_resolver unset so SamplingClient stamps AuthScheme::Bearer from SamplerConfig.api_key"
        );
    }

    /// Operator: when endpoint+key are unset, existing SuperGrok / console
    /// ranked-auto and bearer-resolver behavior stays.
    #[test]
    #[serial]
    fn unset_inference_still_applies_ranked_auto_turn_credentials() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("session-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-fixture"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        sampling.failover_api_keys = vec!["console-must-wait".into()];
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert_eq!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "unset inference must still let ranked auto install the stored SuperGrok session"
        );
        assert!(
            !sampling
                .failover_api_keys
                .iter()
                .any(|k| k == "console-must-wait"),
            "unset inference must still let ranked auto replace console failover: {:?}",
            sampling.failover_api_keys
        );
        assert_ne!(
            sampling.base_url.trim_end_matches('/'),
            "https://compat.example.test/v1",
            "unset inference must not replace the SuperGrok or console host"
        );
        assert!(
            reconstruct_installs_inherited_session_bearer_resolver(true),
            "unset inference must still install InheritedSessionBearerResolver on the session path"
        );
        assert!(
            !reconstruct_installs_inherited_session_bearer_resolver(false),
            "unset inference must not invent a bearer resolver when the session gate is off"
        );
        assert!(
            reconstruct_aligns_ranked_free_period_primary(true, true, false),
            "unset inference must still align AuthManager on default limits mode"
        );
        assert!(
            !reconstruct_aligns_ranked_free_period_primary(true, true, true),
            "unset inference must not align when preferred_method is api_key"
        );
    }

    /// Operator entire-client contract: when `[inference]` endpoint+key (or
    /// GROK_API_BASE / GROK_API_KEY) resolve, the live TUI/CLI turn must keep
    /// that custom host and Bearer API key. OpenAI-compatible Chat Completions
    /// POST `{base}/chat/completions` with `Authorization: Bearer <api_key>`.
    /// `reconstruct_full_config` must not seed `sampling.api_key` from
    /// `AuthManager.current_wire_valid()` when `use_bearer_resolver` is true
    /// (catalog `NotByok`, including a third-party host). Default logged-in
    /// SuperGrok: `auto_use_included_limits` true, `preferred_method` unset.
    #[test]
    #[serial]
    fn reconstruct_does_not_seed_auth_manager_jwt_when_inference_is_resolved() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        // Live reconstruct_full_config order: AuthManager JWT seed, then ranked
        // auto skip, then limits-pin early return.
        sampling.api_key = reconstruct_turn_api_key(
            true,
            Some("stored-personal-supergrok-jwt".into()),
            sampling.api_key.clone(),
        );
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert_eq!(sampling.base_url, "https://compat.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-inference-key"));
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
        assert_ne!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "AuthManager.current_wire_valid() must not POST a SuperGrok JWT to the custom inference host"
        );
        assert!(
            !reconstruct_installs_inherited_session_bearer_resolver(true),
            "resolved inference must not install InheritedSessionBearerResolver after the seed skip"
        );
    }

    /// Operator entire-client contract: `prepare_sampler_for_turn` and
    /// `prepare_chat_completion` call `refresh_token_if_expired` first. When
    /// `[inference]` endpoint+key resolve, that refresh must not write a
    /// SuperGrok JWT into chat-state credentials while `base_url` stays the
    /// custom host. Ranked-auto skip then leaves whatever refresh wrote.
    #[test]
    #[serial]
    fn refresh_token_does_not_overwrite_inference_key_with_supergrok_jwt() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::set(GROK_API_BASE_ENV, "https://compat.example.test/v1/");
        let _key = EnvGuard::set(GROK_API_KEY_ENV, "fixture-inference-key");
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("catalog-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("session-jwt-must-not-win"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        // Live order: refresh_token_if_expired, then reconstruct seed, then
        // ranked auto / limits pins.
        assert!(
            !refresh_token_applies_session_jwt(true),
            "resolved inference must not write a SuperGrok JWT into chat-state credentials"
        );
        assert_eq!(
            refresh_token_overwrite_chat_api_key(
                true,
                sampling.api_key.as_deref(),
                "stored-personal-supergrok-jwt",
            ),
            None,
            "refresh must not replace the inference Bearer key with a SuperGrok JWT"
        );
        sampling.api_key = reconstruct_turn_api_key(
            true,
            Some("stored-personal-supergrok-jwt".into()),
            sampling.api_key.clone(),
        );
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert_eq!(sampling.base_url, "https://compat.example.test/v1");
        assert_eq!(sampling.api_key.as_deref(), Some("fixture-inference-key"));
        assert_eq!(sampling.auth_scheme, AuthScheme::Bearer);
        assert_ne!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "refresh then reconstruct must not POST a SuperGrok JWT to the custom inference host"
        );
    }

    /// Operator: when endpoint+key are unset, existing SuperGrok reconstruct
    /// still seeds `AuthManager.current_wire_valid()` on the session gate.
    #[test]
    #[serial]
    fn unset_inference_reconstruct_still_seeds_auth_manager_jwt() {
        let (dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);
        store_live_personal_session(dir.path(), "stored-personal-supergrok-jwt");

        let model = ModelEntry::fallback("session-model", &EndpointsConfig::default());
        let creds = resolve_credentials(&model, Some("stale-chat-jwt"));
        let mut sampling = sampling_config_for_model(&model, creds, None, None, None, None);
        sampling.api_key = reconstruct_turn_api_key(
            true,
            Some("stored-personal-supergrok-jwt".into()),
            sampling.api_key.clone(),
        );
        apply_reconstruct_turn_credentials(&mut sampling, Some(dir.path()));

        assert_eq!(
            sampling.api_key.as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "unset inference must still seed the AuthManager SuperGrok JWT on the session path"
        );
        assert_ne!(
            sampling.base_url.trim_end_matches('/'),
            "https://compat.example.test/v1",
            "unset inference must not replace the SuperGrok or console host"
        );
        assert_eq!(
            reconstruct_turn_api_key(
                false,
                Some("stored-personal-supergrok-jwt".into()),
                Some("byok-key".into()),
            )
            .as_deref(),
            Some("byok-key"),
            "unset inference must not invent an AuthManager seed when the session gate is off"
        );
    }

    /// Operator: when endpoint+key are unset, `refresh_token_if_expired` still
    /// writes the SuperGrok JWT into chat-state credentials on the session gate.
    #[test]
    #[serial]
    fn unset_inference_refresh_token_still_overwrites_chat_key_with_session_jwt() {
        let (_dir, _home, _ob, _ok, _xai, _gc, _gcp) = isolated_home();
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _base = EnvGuard::unset(GROK_API_BASE_ENV);
        let _key = EnvGuard::unset(GROK_API_KEY_ENV);

        assert!(
            refresh_token_applies_session_jwt(true),
            "unset inference must still refresh SuperGrok chat-state credentials on the session path"
        );
        assert!(
            !refresh_token_applies_session_jwt(false),
            "unset inference must not refresh SuperGrok credentials when the session gate is off"
        );
        assert_eq!(
            refresh_token_overwrite_chat_api_key(
                true,
                Some("stale-chat-jwt"),
                "stored-personal-supergrok-jwt",
            )
            .as_deref(),
            Some("stored-personal-supergrok-jwt"),
            "unset inference must still overwrite chat-state with the SuperGrok JWT"
        );
        assert_eq!(
            refresh_token_overwrite_chat_api_key(
                true,
                Some("stored-personal-supergrok-jwt"),
                "stored-personal-supergrok-jwt",
            ),
            None,
            "unset inference must not rewrite chat-state when the JWT is already current"
        );
        assert_eq!(
            refresh_token_overwrite_chat_api_key(
                false,
                Some("stale-chat-jwt"),
                "stored-personal-supergrok-jwt",
            ),
            None,
            "unset inference must not overwrite when the session gate is off"
        );
    }
}
