//! Client-wide OpenAI-compatible inference: `[inference]` in `config.toml`, plus env.
//!
//! This is an additional method beside SuperGrok OAuth and console API keys.
//! Both an endpoint and an API key must resolve, or the existing SuperGrok /
//! console path stays.
//!
//! Env precedence: `GROK_API_BASE` / `GROK_API_KEY`, then `OPENAI_BASE_URL` /
//! `OPENAI_API_KEY` when the grok-oss names are unset, then the file table.
//! Do not print keys.

use serde::{Deserialize, Serialize};
use xai_grok_env::env_string;

pub const GROK_API_BASE_ENV: &str = "GROK_API_BASE";
pub const GROK_API_KEY_ENV: &str = "GROK_API_KEY";
pub const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";
pub const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";

/// `[inference]` table from layered `config.toml`. Not `[auth]`.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct InferenceFileConfig {
    #[serde(
        alias = "base_url",
        alias = "api_base",
        skip_serializing_if = "Option::is_none"
    )]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional model id. When unset, the existing product model field stays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl std::fmt::Debug for InferenceFileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceFileConfig")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .finish()
    }
}

/// Resolved OpenAI-compatible inference destination. `api_key` is a fixture or
/// operator secret; never log it.
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedInference {
    pub endpoint: String,
    pub api_key: String,
    pub model: Option<String>,
}

impl std::fmt::Debug for ResolvedInference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedInference")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .finish()
    }
}

/// Logical Chat Completions request the live sampler posts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceChatCompletionsRequest {
    pub method: &'static str,
    pub url: String,
    pub authorization: String,
}

fn blank_as_unset(opt: Option<String>) -> Option<String> {
    opt.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Parse the `[inference]` table from a TOML document (user or merged config).
pub fn parse_inference_table(toml_src: &str) -> Result<InferenceFileConfig, String> {
    let value: toml::Value =
        toml::from_str(toml_src).map_err(|e| crate::toml_error_detail(toml_src, &e))?;
    Ok(InferenceFileConfig::from_config_value(&value))
}

impl InferenceFileConfig {
    /// `[inference]` from a merged config value. Missing table is empty.
    pub fn from_config_value(config: &toml::Value) -> Self {
        let Some(table) = config.get("inference") else {
            return Self::default();
        };
        table.clone().try_into().unwrap_or_default()
    }
}

/// File < OPENAI_* < GROK_*. Both endpoint and key must resolve.
pub fn resolve_inference(
    file: InferenceFileConfig,
    mut getenv: impl FnMut(&str) -> Option<String>,
) -> Option<ResolvedInference> {
    let grok_base = blank_as_unset(getenv(GROK_API_BASE_ENV));
    let openai_base = blank_as_unset(getenv(OPENAI_BASE_URL_ENV));
    let file_base = blank_as_unset(file.endpoint);
    let endpoint_raw = grok_base.or(openai_base).or(file_base)?;

    let grok_key = blank_as_unset(getenv(GROK_API_KEY_ENV));
    let openai_key = blank_as_unset(getenv(OPENAI_API_KEY_ENV));
    let file_key = blank_as_unset(file.api_key);
    let api_key = grok_key.or(openai_key).or(file_key)?;

    let endpoint = normalize_inference_base(&endpoint_raw)?;
    let model = blank_as_unset(file.model);
    Some(ResolvedInference {
        endpoint,
        api_key,
        model,
    })
}

/// Process env plus the effective layered `config.toml`.
pub fn resolve_configured_inference() -> Option<ResolvedInference> {
    let file = match crate::effective_config::load_effective_config() {
        Ok(cfg) => InferenceFileConfig::from_config_value(&cfg),
        Err(_) => InferenceFileConfig::default(),
    };
    resolve_inference(file, env_string)
}

/// HTTPS default. HTTP only for localhost / 127.0.0.1 / ::1. Strip one trailing slash.
pub fn normalize_inference_base(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = url::Url::parse(&with_scheme).ok()?;
    match parsed.scheme() {
        "https" => {}
        "http" if http_host_is_loopback(&parsed) => {}
        _ => return None,
    }
    parsed.host_str()?;
    let mut normalized = with_scheme;
    while normalized.ends_with('/') {
        normalized.pop();
    }
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn http_host_is_loopback(url: &url::Url) -> bool {
    // `host_str()` is `[::1]` with brackets or `::1` without; `Host` is ::1 either way.
    match url.host() {
        Some(url::Host::Domain(host)) => host == "localhost",
        Some(url::Host::Ipv4(addr)) => addr.is_loopback(),
        Some(url::Host::Ipv6(addr)) => addr.is_loopback(),
        None => false,
    }
}

/// `{base}/chat/completions`. A base that already ends with that path is kept.
pub fn chat_completions_url(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_owned()
    } else {
        format!("{base}/chat/completions")
    }
}

pub fn bearer_authorization(api_key: &str) -> String {
    format!("Bearer {api_key}")
}

pub fn inference_chat_completions_request(
    resolved: &ResolvedInference,
) -> InferenceChatCompletionsRequest {
    InferenceChatCompletionsRequest {
        method: "POST",
        url: chat_completions_url(&resolved.endpoint),
        authorization: bearer_authorization(&resolved.api_key),
    }
}

/// When `resolved` is `None`, leave SuperGrok / console values in place.
/// Returns whether the overlay applied.
pub fn overlay_inference_endpoint_and_key(
    endpoint: &mut String,
    api_key: &mut Option<String>,
    resolved: Option<&ResolvedInference>,
) -> bool {
    let Some(inf) = resolved else {
        return false;
    };
    *endpoint = inf.endpoint.clone();
    *api_key = Some(inf.api_key.clone());
    true
}

#[cfg(test)]
#[path = "inference_tests.rs"]
mod tests;
