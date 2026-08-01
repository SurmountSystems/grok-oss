//! Prefer a live dual-auth identity after SuperGrok (or primary) is out of allowance.
//!
//! When SuperGrok session (or any primary) is memoized out of allowance and a
//! live failover remains, reorder [`SamplerConfig`] so the **first** HTTP
//! attempt already uses the console key (or next live identity) — no SuperGrok
//! try, no per-turn Retrying switch chrome.
//!
//! Shell must call [`prefer_live_identity_after_credit_exhaust`] after rebuilding
//! session config each turn (`reconstruct_full_config` / `prepare_sampler_for_turn`);
//! otherwise resolve always re-pins SuperGrok as primary and the request task
//! re-switches every prompt.

use grok_rate_limit::fingerprint_secret;

use crate::config::SamplerConfig;
use crate::exhausted_identity::{self, CredentialLabel, HopCause};

/// Cli-chat-proxy header names injected by the shell for session hosts.
/// Stripped when hopping to the public console API; re-added for session host.
const CLI_CHAT_PROXY_HEADER_NAMES: &[&str] = &[
    "X-XAI-Token-Auth",
    "x-authenticateresponse",
    "x-grok-client-mode",
];

fn looks_like_cli_chat_proxy_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("cli-chat-proxy") || lower.contains("cli_chat_proxy")
}

fn strip_cli_chat_proxy_headers(headers: &mut indexmap::IndexMap<String, String>) {
    headers.retain(|k, _| {
        !CLI_CHAT_PROXY_HEADER_NAMES
            .iter()
            .any(|n| k.eq_ignore_ascii_case(n))
    });
}

fn ensure_cli_chat_proxy_headers(headers: &mut indexmap::IndexMap<String, String>) {
    headers
        .entry("X-XAI-Token-Auth".to_string())
        .or_insert_with(|| "xai-grok-cli".to_string());
    headers
        .entry("x-authenticateresponse".to_string())
        .or_insert_with(|| "authenticate-response".to_string());
    headers
        .entry("x-grok-client-mode".to_string())
        .or_insert_with(|| "interactive".to_string());
}

/// Switch API host with the identity (SuperGrok proxy ↔ `api.x.ai`) and adjust
/// cli-chat-proxy headers.
pub(crate) fn switch_api_host_with_identity(config: &mut SamplerConfig, new_base_url: &str) {
    let prev = config.base_url.clone();
    if prev.trim_end_matches('/') == new_base_url.trim_end_matches('/') {
        return;
    }
    config.base_url = new_base_url.to_owned();
    strip_cli_chat_proxy_headers(&mut config.extra_headers);
    if looks_like_cli_chat_proxy_url(new_base_url) {
        ensure_cli_chat_proxy_headers(&mut config.extra_headers);
    }
    tracing::info!(
        target: crate::sampling_log::TARGET,
        from_host = %prev,
        to_host = %new_base_url,
        "switching API host with identity (SuperGrok proxy ↔ console API)"
    );
}

pub(crate) fn is_session_identity(config: &SamplerConfig, token: &str) -> bool {
    config
        .session_identity_key
        .as_deref()
        .is_some_and(|s| s.trim() == token.trim())
}

/// Label for hop status/toast: session JWT vs console API key (no secrets).
pub(crate) fn credential_label(
    config: &SamplerConfig,
    token: &str,
    is_session_side: bool,
) -> CredentialLabel {
    if is_session_side || is_session_identity(config, token) {
        CredentialLabel::SuperGrokSession
    } else {
        CredentialLabel::ConsoleKey
    }
}

/// True when the configured primary should be left alone (skipped) because it is
/// memoized out of allowance / credit.
///
/// Matches either:
/// - live `api_key` fingerprint memoized exhausted, or
/// - SuperGrok session side (bearer resolver / session_identity_key) when the
///   **session identity** fingerprint is exhausted — covers OIDC refresh where
///   the live JWT differs from the memoized prior token but is still SuperGrok.
pub fn primary_is_memoized_credit_exhausted(config: &SamplerConfig) -> bool {
    let active = config.api_key.as_deref().unwrap_or("").trim();
    if !active.is_empty() && exhausted_identity::is_exhausted(&fingerprint_secret(active)) {
        return true;
    }
    let Some(sess) = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    if !exhausted_identity::is_exhausted(&fingerprint_secret(sess)) {
        return false;
    }
    // Session-side primary: live token may have rotated after mark.
    config.bearer_resolver.is_some()
        || is_session_identity(config, active)
        || active.is_empty()
        || active == sess
}

/// Pop the next distinct live failover key and switch API host / bearer with it.
/// Returns switch reason (no secrets). Does **not** rebuild a client.
///
/// For [`HopCause::CreditExhausted`], marks the left identity exhausted.
pub(crate) fn rotate_identity_config(
    config: &mut SamplerConfig,
    cause: HopCause,
) -> Option<String> {
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    let active_fp = if active.is_empty() {
        String::new()
    } else {
        fingerprint_secret(&active)
    };
    // Process-local credit memo: do not re-select keys already known dead.
    // Drop blanks + active duplicate + memoized fingerprints.
    config.failover_api_keys.retain(|k| {
        let t = k.trim();
        if t.is_empty() || t == active {
            return false;
        }
        !exhausted_identity::is_exhausted(&fingerprint_secret(t))
    });
    let next_key = config.failover_api_keys.first().cloned()?;
    config.failover_api_keys.remove(0);
    // Also drop any further duplicates of the key we are switching to.
    let next_trim = next_key.trim().to_owned();
    config.failover_api_keys.retain(|k| k.trim() != next_trim);
    let prev_fp = fingerprint_secret(&active);
    let next_fp = fingerprint_secret(&next_key);
    let next_is_session = is_session_identity(config, &next_trim);
    let active_is_session =
        is_session_identity(config, &active) || config.bearer_resolver.is_some();
    let log_msg = match cause {
        HopCause::CreditExhausted => {
            "out of allowance on active credential; switching to next identity"
        }
        HopCause::RateLimited => "rate limited on active credential; switching to next identity",
    };
    tracing::info!(
        target: crate::sampling_log::TARGET,
        from_key = %prev_fp,
        to_key = %next_fp,
        remaining_failover = config.failover_api_keys.len(),
        next_is_session,
        ?cause,
        "{log_msg}"
    );
    config.api_key = Some(next_key);

    if next_is_session {
        // Key → session: restore session host + live bearer when available.
        if let Some(url) = config.session_base_url.clone() {
            switch_api_host_with_identity(config, &url);
        }
        if let Some(resolver) = config.stashed_bearer_resolver.take() {
            config.bearer_resolver = Some(resolver);
        } else if let Some(resolver) = config.session_bearer_resolver.clone() {
            // Live re-bind without prior stash (key-primary mid-switch / next turn).
            config.bearer_resolver = Some(resolver);
        } else {
            // No live resolver available; wire the session JWT as a static key.
            config.bearer_resolver = None;
        }
    } else {
        // Session → key (or key → key): never re-inject exhausted session JWT.
        if config.bearer_resolver.is_some() {
            config.stashed_bearer_resolver = config.bearer_resolver.take();
        }
        config.bearer_resolver = None;
        // Use console host when dual-auth split hosts are configured.
        if active_is_session || is_session_identity(config, &active) {
            if let Some(url) = config.failover_base_url.clone() {
                switch_api_host_with_identity(config, &url);
            }
        } else if let Some(url) = config.failover_base_url.clone() {
            // Console→console stays on console host; if somehow still on session host, fix.
            if looks_like_cli_chat_proxy_url(&config.base_url)
                && !looks_like_cli_chat_proxy_url(&url)
            {
                switch_api_host_with_identity(config, &url);
            }
        }
    }

    let from_label = credential_label(config, &active, active_is_session);
    let to_label = credential_label(config, &next_trim, next_is_session);
    let hop_reason = exhausted_identity::format_hop_reason(from_label, to_label, cause);

    // Credit / allowance only: 1h memo so subsequent turns stay off dead keys.
    // Rate-limit switches rely on shared cooldown, not this memo.
    if matches!(cause, HopCause::CreditExhausted) {
        if !active_fp.is_empty() {
            exhausted_identity::mark_exhausted(&active_fp);
        }
        // JWT refresh: also mark session_identity_key when leaving session.
        if active_is_session
            && let Some(sess) = config
                .session_identity_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != active.as_str())
        {
            exhausted_identity::mark_exhausted(&fingerprint_secret(sess));
        }
    }

    Some(hop_reason)
}

/// Drop blank, active-duplicate, and memoized-out-of-allowance keys from the
/// failover list (no secrets logged).
///
/// Always safe to call after resolve rebuild. Keeps exhausted SuperGrok session
/// JWT from sitting as a silent next hop when console is already primary
/// (`preferred_method = api_key` + included weekly marked used up).
pub fn prune_exhausted_failover_candidates(config: &mut SamplerConfig) {
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    config.failover_api_keys.retain(|k| {
        let t = k.trim();
        if t.is_empty() || (!active.is_empty() && t == active) {
            return false;
        }
        !exhausted_identity::is_exhausted(&fingerprint_secret(t))
    });
}

/// If primary is memoized out of allowance and a live failover remains, rotate
/// config to that failover **before** any HTTP (silent preference; stay on the
/// console key after switch).
///
/// Always prunes memoized-dead failover candidates first (even when primary is
/// already a live console key) so SuperGrok extras are not the silent next hop.
///
/// Returns switch reason when rotation applied (callers may log; UI chrome for
/// already-memoized apply is optional — prefer silence so turns do not look
/// like Retrying every prompt). Mid-request credit switches still use the
/// request-task Retrying path.
///
/// Shell: call after `reconstruct_full_config` so actor + aux clients start on
/// console key when SuperGrok is remembered out of allowance.
pub fn prefer_live_identity_after_credit_exhaust(config: &mut SamplerConfig) -> Option<String> {
    // Prune first: console-primary dual-auth must not keep an exhausted SuperGrok
    // session JWT queued after included weekly is marked used up.
    prune_exhausted_failover_candidates(config);

    if !primary_is_memoized_credit_exhausted(config) {
        return None;
    }
    // Need at least one non-empty live failover candidate after prune.
    let active = config.api_key.as_deref().unwrap_or("").trim();
    let has_live = config.failover_api_keys.iter().any(|k| {
        let t = k.trim();
        !t.is_empty() && t != active && !exhausted_identity::is_exhausted(&fingerprint_secret(t))
    });
    if !has_live {
        return None;
    }
    rotate_identity_config(config, HopCause::CreditExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SamplerConfig;

    /// Named contract: memoized SuperGrok exhaust + dual-auth → config primary
    /// is already console key before any request (no SuperGrok attempt).
    #[test]
    fn prefer_live_session_to_console_before_request() {
        exhausted_identity::with_memo_lock(|| {
            let session = "prefer-live-session-jwt";
            let console = "prefer-live-console-key";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));

            let mut config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![console.into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            let reason = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must prefer console when session memoized exhausted");
            assert_eq!(config.api_key.as_deref(), Some(console));
            assert!(
                config.base_url.contains("api.x.ai"),
                "must switch host: {}",
                config.base_url
            );
            assert!(config.bearer_resolver.is_none());
            assert!(exhausted_identity::is_credential_hop_reason(&reason));
            assert!(reason.contains("console key"), "{reason}");
            // Second apply: primary is console (live) → no further switch.
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "already on console; seamless subsequent turns"
            );
            assert_eq!(config.api_key.as_deref(), Some(console));
        });
    }

    /// OIDC refresh: live primary JWT differs from memoized session_identity_key.
    #[test]
    fn prefer_live_session_jwt_refresh_still_skips() {
        exhausted_identity::with_memo_lock(|| {
            let old_sess = "session-jwt-before-refresh";
            let new_sess = "session-jwt-after-refresh";
            let console = "console-after-refresh";
            exhausted_identity::mark_exhausted(&fingerprint_secret(old_sess));

            // Bearer resolver present marks us session-side even when live key is new.
            #[derive(Debug)]
            struct FixedBearer(&'static str);
            impl crate::config::BearerResolver for FixedBearer {
                fn current_bearer(&self) -> Option<String> {
                    Some(self.0.to_owned())
                }
            }

            let mut config = SamplerConfig {
                api_key: Some(new_sess.into()),
                failover_api_keys: vec![console.into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(old_sess.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                bearer_resolver: Some(std::sync::Arc::new(FixedBearer(new_sess))),
                session_bearer_resolver: Some(std::sync::Arc::new(FixedBearer(new_sess))),
                ..Default::default()
            };
            assert!(primary_is_memoized_credit_exhausted(&config));
            let _ = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("skip after JWT refresh");
            assert_eq!(config.api_key.as_deref(), Some(console));
            assert!(config.bearer_resolver.is_none());
            // Live refreshed JWT also marked so a rebuild with new token alone still skips.
            assert!(exhausted_identity::is_exhausted(&fingerprint_secret(
                new_sess
            )));
        });
    }

    #[test]
    fn prefer_live_noop_when_primary_live() {
        exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("live-session".into()),
                failover_api_keys: vec!["console".into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some("live-session".into()),
                ..Default::default()
            };
            assert!(prefer_live_identity_after_credit_exhaust(&mut config).is_none());
            assert_eq!(config.api_key.as_deref(), Some("live-session"));
        });
    }

    /// B2: multi console keys stay ordered; first live key becomes primary;
    /// remaining console keys stay in list; SuperGrok session is not re-queued.
    #[test]
    fn prefer_live_multi_console_keys_stable_order_first_live() {
        exhausted_identity::with_memo_lock(|| {
            let session = "b2-session-jwt";
            let business = "b2-console-business";
            let personal = "b2-console-personal";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));

            let mut config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![business.into(), personal.into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            let reason = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must prefer first console key");
            assert_eq!(config.api_key.as_deref(), Some(business));
            assert_eq!(
                config.failover_api_keys,
                vec![personal.to_string()],
                "remaining console keys keep multi-add / collect order"
            );
            assert!(
                !config.failover_api_keys.iter().any(|k| k.trim() == session),
                "exhausted SuperGrok must not sit before remaining console keys"
            );
            assert!(
                config.base_url.contains("api.x.ai"),
                "must use console host: {}",
                config.base_url
            );
            assert!(reason.contains("console key"), "{reason}");
        });
    }

    /// B2: first console key also out of allowance → skip to next live console
    /// (never SuperGrok extras while a usable console key remains).
    #[test]
    fn prefer_live_skips_exhausted_first_console_to_next() {
        exhausted_identity::with_memo_lock(|| {
            let session = "b2-skip-session";
            let dead_console = "b2-dead-console";
            let live_console = "b2-live-console";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));
            exhausted_identity::mark_exhausted(&fingerprint_secret(dead_console));

            let mut config = SamplerConfig {
                api_key: Some(session.into()),
                failover_api_keys: vec![dead_console.into(), live_console.into()],
                base_url: "https://cli-chat-proxy.grok.com/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            let _ = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must reach second console");
            assert_eq!(config.api_key.as_deref(), Some(live_console));
            assert!(config.failover_api_keys.is_empty());
            assert!(config.base_url.contains("api.x.ai"));
        });
    }

    /// B2: console already primary (`preferred_method=api_key`) + SuperGrok
    /// session memoized out of allowance in failover → drop session so extras
    /// are not the silent next hop; primary stays on console.
    #[test]
    fn prefer_live_console_primary_prunes_exhausted_session_from_failover() {
        exhausted_identity::with_memo_lock(|| {
            let session = "b2-prune-session-jwt";
            let business = "b2-prune-business";
            let other = "b2-prune-other-console";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));

            let mut config = SamplerConfig {
                api_key: Some(business.into()),
                failover_api_keys: vec![other.into(), session.into()],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "console primary is live; no rotate"
            );
            assert_eq!(config.api_key.as_deref(), Some(business));
            assert_eq!(
                config.failover_api_keys,
                vec![other.to_string()],
                "exhausted SuperGrok session must be pruned; other console kept"
            );
        });
    }
}
