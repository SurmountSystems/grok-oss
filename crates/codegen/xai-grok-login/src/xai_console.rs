//! First-party console API key store helpers used by credential read-modify-write.

use crate::auth_method::has_xai_api_key_env;
use crate::credentials_store::{BEARER_USERNAME, CredentialsStore, CredentialsStoreError};

/// Default first-party inference base URL used as the store key.
pub const XAI_CONSOLE_API_URL: &str = "https://api.x.ai/v1";

/// Normalize the credential URL used as the store key.
pub fn credential_url(base_url: Option<&str>) -> String {
    let url = base_url
        .unwrap_or(XAI_CONSOLE_API_URL)
        .trim_end_matches('/');
    if url.is_empty() {
        XAI_CONSOLE_API_URL.to_owned()
    } else {
        url.to_owned()
    }
}

fn split_api_key_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split(|c| c == ',' || c == '\n' || c == '\r') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == part) {
            out.push(part.to_owned());
        }
    }
    out
}

/// Append a console API key. Fail-closed when the keyring read errors.
///
/// Returns `true` when the key was newly added.
pub fn add_console_api_key(
    store: &CredentialsStore,
    api_key: &str,
) -> Result<bool, XaiConsoleAuthError> {
    if has_xai_api_key_env() {
        return Err(XaiConsoleAuthError::EnvVarSet);
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err(XaiConsoleAuthError::EmptyKey);
    }
    let url = credential_url(None);
    let mut keys = match store.read_for_update(&url)? {
        Some((_, raw)) => split_api_key_list(&raw),
        None => Vec::new(),
    };
    if keys.iter().any(|existing| existing == key) {
        return Ok(false);
    }
    keys.push(key.to_owned());
    store.write(&url, BEARER_USERNAME, &keys.join(","))?;
    Ok(true)
}

#[derive(Debug, thiserror::Error)]
pub enum XaiConsoleAuthError {
    #[error("XAI_API_KEY is set; refuse to write the secret store (env wins)")]
    EnvVarSet,
    #[error("API key is empty")]
    EmptyKey,
    #[error(transparent)]
    Store(#[from] CredentialsStoreError),
}
