use super::*;

/// Operator: support inference over an endpoint and an API key from
/// `~/.grok/config.toml` `[inference]` (`endpoint` and `api_key`).
#[test]
fn inference_config_reads_endpoint_and_api_key_from_config_toml() {
    let parsed = parse_inference_table(
        r#"
[inference]
endpoint = "https://llm.example.test/v1"
api_key = "fixture-config-key"
model = "example-model"
"#,
    )
    .expect("parse [inference] from config.toml");
    assert_eq!(
        parsed.endpoint.as_deref(),
        Some("https://llm.example.test/v1")
    );
    assert_eq!(parsed.api_key.as_deref(), Some("fixture-config-key"));
    assert_eq!(parsed.model.as_deref(), Some("example-model"));

    let resolved =
        resolve_inference(parsed, |_| None).expect("file endpoint and api_key together resolve");
    assert_eq!(resolved.endpoint, "https://llm.example.test/v1");
    assert_eq!(resolved.api_key, "fixture-config-key");
    assert_eq!(resolved.model.as_deref(), Some("example-model"));
}

/// Operator: env overrides the file. GROK_API_BASE / GROK_API_KEY win, then
/// OPENAI_BASE_URL / OPENAI_API_KEY when the grok-oss names are unset.
#[test]
fn inference_config_env_overrides_config_file() {
    let file = parse_inference_table(
        r#"
[inference]
endpoint = "https://from-file.example.test/v1"
api_key = "fixture-file-key"
"#,
    )
    .expect("parse file [inference]");

    let grok_env = resolve_inference(file.clone(), |name| match name {
        GROK_API_BASE_ENV => Some("https://from-grok-env.example.test/v1/".to_owned()),
        GROK_API_KEY_ENV => Some("fixture-grok-env-key".to_owned()),
        OPENAI_BASE_URL_ENV => Some("https://from-openai-env.example.test/v1".to_owned()),
        OPENAI_API_KEY_ENV => Some("fixture-openai-env-key".to_owned()),
        _ => None,
    })
    .expect("GROK_* env must win over file and OPENAI_*");
    assert_eq!(grok_env.endpoint, "https://from-grok-env.example.test/v1");
    assert_eq!(grok_env.api_key, "fixture-grok-env-key");

    let openai_fallback = resolve_inference(file, |name| match name {
        OPENAI_BASE_URL_ENV => Some("https://from-openai-env.example.test/v1".to_owned()),
        OPENAI_API_KEY_ENV => Some("fixture-openai-env-key".to_owned()),
        _ => None,
    })
    .expect("OPENAI_* when GROK_* unset");
    assert_eq!(
        openai_fallback.endpoint,
        "https://from-openai-env.example.test/v1"
    );
    assert_eq!(openai_fallback.api_key, "fixture-openai-env-key");
}

/// Operator: OpenAI-compatible Chat Completions POST `{base}/chat/completions`
/// with header `Authorization: Bearer <api_key>`. Normalize trailing slash once.
#[test]
fn inference_request_posts_openai_compatible_chat_completions_with_bearer() {
    let file = parse_inference_table(
        r#"
[inference]
endpoint = "https://compat.example.test/v1/"
api_key = "fixture-bearer-key"
"#,
    )
    .expect("parse");
    let resolved = resolve_inference(file, |_| None).expect("resolve");
    let request = inference_chat_completions_request(&resolved);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        "https://compat.example.test/v1/chat/completions"
    );
    assert_eq!(request.authorization, "Bearer fixture-bearer-key");
    assert_eq!(
        bearer_authorization("fixture-bearer-key"),
        "Bearer fixture-bearer-key"
    );
}

/// Operator: when endpoint+key are unset, existing SuperGrok / console behavior stays.
#[test]
fn unset_inference_endpoint_keeps_existing_supergrok_or_console_path() {
    assert!(
        resolve_inference(InferenceFileConfig::default(), |_| None).is_none(),
        "empty file and env must not take over SuperGrok or console"
    );
    let endpoint_only = parse_inference_table(
        r#"
[inference]
endpoint = "https://compat.example.test/v1"
"#,
    )
    .expect("parse endpoint-only");
    assert!(
        resolve_inference(endpoint_only, |_| None).is_none(),
        "endpoint without api_key is unset"
    );
    let key_only = parse_inference_table(
        r#"
[inference]
api_key = "fixture-key-only"
"#,
    )
    .expect("parse key-only");
    assert!(
        resolve_inference(key_only, |_| None).is_none(),
        "api_key without endpoint is unset"
    );

    let mut endpoint = "https://cli-chat-proxy.grok.com/v1".to_owned();
    let mut api_key = Some("session-jwt-fixture".to_owned());
    let overlaid = overlay_inference_endpoint_and_key(&mut endpoint, &mut api_key, None);
    assert!(!overlaid);
    assert_eq!(endpoint, "https://cli-chat-proxy.grok.com/v1");
    assert_eq!(api_key.as_deref(), Some("session-jwt-fixture"));
}

#[test]
fn overlay_inference_endpoint_and_key_applies_when_resolved() {
    let resolved = resolve_inference(
        InferenceFileConfig {
            endpoint: Some("https://compat.example.test/v1/".to_owned()),
            api_key: Some("fixture-overlay-key".to_owned()),
            model: None,
        },
        |_| None,
    )
    .expect("both endpoint and key resolve");
    let mut endpoint = "https://cli-chat-proxy.grok.com/v1".to_owned();
    let mut api_key = Some("session-jwt-fixture".to_owned());
    let applied = overlay_inference_endpoint_and_key(&mut endpoint, &mut api_key, Some(&resolved));
    assert!(applied);
    assert_eq!(endpoint, "https://compat.example.test/v1");
    assert_eq!(api_key.as_deref(), Some("fixture-overlay-key"));
}

#[test]
fn inference_http_is_only_for_loopback() {
    let remote_http = parse_inference_table(
        r#"
[inference]
endpoint = "http://evil.example.test/v1"
api_key = "fixture-key"
"#,
    )
    .expect("parse");
    assert!(
        resolve_inference(remote_http, |_| None).is_none(),
        "plain HTTP to a non-loopback host is not an inference endpoint"
    );

    for loopback in [
        "http://127.0.0.1:8080/v1",
        "http://localhost:8080/v1",
        "http://[::1]:8080/v1",
    ] {
        let file = InferenceFileConfig {
            endpoint: Some(loopback.to_owned()),
            api_key: Some("fixture-loopback-key".to_owned()),
            model: None,
        };
        let resolved = resolve_inference(file, |_| None)
            .unwrap_or_else(|| panic!("HTTP loopback must be allowed: {loopback}"));
        assert_eq!(resolved.endpoint, loopback.trim_end_matches('/'));
    }
}

#[test]
fn inference_missing_scheme_defaults_to_https() {
    let file = InferenceFileConfig {
        endpoint: Some("compat.example.test/v1".to_owned()),
        api_key: Some("fixture-key".to_owned()),
        model: None,
    };
    let resolved = resolve_inference(file, |_| None).expect("https default");
    assert_eq!(resolved.endpoint, "https://compat.example.test/v1");
}

#[test]
fn inference_debug_redacts_api_key() {
    let file = InferenceFileConfig {
        endpoint: Some("https://compat.example.test/v1".to_owned()),
        api_key: Some("fixture-must-not-appear-in-debug".to_owned()),
        model: None,
    };
    let debug = format!("{file:?}");
    assert!(
        !debug.contains("fixture-must-not-appear-in-debug"),
        "Debug must not print the API key: {debug}"
    );
}
