//! Prefer a live failover identity after a memoized-dead console primary.
//!
//! Fail-open SuperGrok: a leftover HTTP 402 memo does not hop reconstruct.
//! Stay reconstruct already restores SuperGrok when the stay sidecar is set.
//! This request hops after SuperGrok HTTP 402.
//!
//! When a console primary is memoized out of allowance and a live failover
//! remains, reorder [`SamplerConfig`] so the first HTTP attempt already uses
//! the next live identity.
//!
//! Shell may call [`prefer_live_identity_after_credit_exhaust`] after rebuilding
//! session config each turn (`reconstruct_full_config` / `prepare_sampler_for_turn`).

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

/// Switch live identity to the console key because the operator asked
/// (`use-console` sidecar). Does **not** mark SuperGrok used up and does not
/// require `[auth] preferred_method = "api_key"`.
pub fn prefer_console_identity_for_use_console_pin(config: &mut SamplerConfig) -> bool {
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    let sess = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_owned();

    let already_console = !active.is_empty()
        && !is_session_identity(config, &active)
        && config.bearer_resolver.is_none();
    if already_console {
        if let Some(url) = config.failover_base_url.clone()
            && looks_like_cli_chat_proxy_url(&config.base_url)
            && !looks_like_cli_chat_proxy_url(&url)
        {
            switch_api_host_with_identity(config, &url);
            return true;
        }
        return false;
    }

    let idx = config.failover_api_keys.iter().position(|k| {
        let t = k.trim();
        !t.is_empty()
            && t != active
            && (sess.is_empty() || t != sess)
            && !is_session_identity(config, t)
    });
    let Some(idx) = idx else {
        return false;
    };
    let console = config.failover_api_keys.remove(idx);
    let console_trim = console.trim().to_owned();
    config
        .failover_api_keys
        .retain(|k| k.trim() != console_trim);
    if !active.is_empty() {
        config.failover_api_keys.retain(|k| k.trim() != active);
        config.failover_api_keys.insert(0, active);
    }
    config.api_key = Some(console);
    if config.bearer_resolver.is_some() {
        config.stashed_bearer_resolver = config.bearer_resolver.take();
    }
    config.bearer_resolver = None;
    if let Some(url) = config.failover_base_url.clone() {
        switch_api_host_with_identity(config, &url);
    }
    true
}

/// Keep or restore SuperGrok as primary because the operator asked
/// (`stay-supergrok` sidecar). Fail-open: does not mark SuperGrok used up.
pub fn prefer_supergrok_identity_for_stay_pin(config: &mut SamplerConfig) -> bool {
    let Some(sess) = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    else {
        return false;
    };
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    if is_session_identity(config, &active) || config.bearer_resolver.is_some() {
        if let Some(url) = config.session_base_url.clone() {
            switch_api_host_with_identity(config, &url);
        }
        return false;
    }

    config.failover_api_keys.retain(|k| k.trim() != sess);
    if !active.is_empty() && active != sess {
        config.failover_api_keys.retain(|k| k.trim() != active);
        config.failover_api_keys.insert(0, active);
    }
    config.api_key = Some(sess);
    if let Some(resolver) = config.stashed_bearer_resolver.take() {
        config.bearer_resolver = Some(resolver);
    } else if let Some(resolver) = config.session_bearer_resolver.clone() {
        config.bearer_resolver = Some(resolver);
    }
    if let Some(url) = config.session_base_url.clone() {
        switch_api_host_with_identity(config, &url);
    }
    true
}

/// If primary is a memoized-dead **console** key and a live failover remains,
/// rotate config to that failover before any HTTP (silent preference).
///
/// Always prunes memoized-dead failover candidates first (even when primary is
/// already a live console key).
///
/// Fail-open SuperGrok: a leftover HTTP 402 memo from an earlier request must
/// not hop reconstruct between turns. Stay reconstruct already restores
/// SuperGrok on the next rebuild when the stay sidecar is set. This request
/// hops after SuperGrok HTTP 402 (`apply_retry_decision`).
///
/// Returns switch reason when rotation applied (callers may log; UI chrome for
/// already-memoized apply is optional, prefer silence so turns do not look
/// like Retrying every prompt). Mid-request credit switches still use the
/// request-task Retrying path.
///
/// Shell: call after `reconstruct_full_config`.
pub fn prefer_live_identity_after_credit_exhaust(config: &mut SamplerConfig) -> Option<String> {
    prune_exhausted_failover_candidates(config);

    if !primary_is_memoized_credit_exhausted(config) {
        return None;
    }
    let active = config.api_key.as_deref().unwrap_or("").trim();
    if is_session_identity(config, active) || config.bearer_resolver.is_some() {
        return None;
    }
    let has_live = config.failover_api_keys.iter().any(|k| {
        let t = k.trim();
        !t.is_empty() && t != active && !exhausted_identity::is_exhausted(&fingerprint_secret(t))
    });
    if !has_live {
        return None;
    }
    rotate_identity_config(config, HopCause::CreditExhausted)
}

/// After a **console** credit/spend death, make SuperGrok recovery hoppable once.
///
/// Free-period-first ExhaustedAll marks SuperGrok out of allowance so the next
/// turn starts on console (prefer_live). That preemptive memo also pruned
/// SuperGrok from failover, so a console team 403 had nowhere to go. On credit
/// exhaust while active is console and `session_identity_key` is set:
/// clear the SuperGrok memo once and put that JWT first in failover so rotate
/// can hop to free SuperGrok period (wire re-marks if SuperGrok is still dead).
///
/// No-op when active is already SuperGrok session, or no session identity key.
pub fn ensure_supergrok_recovery_after_console_credit_exhaust(config: &mut SamplerConfig) {
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    if active.is_empty() {
        return;
    }
    // Active SuperGrok session (JWT match or live bearer) is not console-dead.
    if is_session_identity(config, &active) || config.bearer_resolver.is_some() {
        return;
    }
    let Some(sess) = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
    else {
        return;
    };
    if sess == active {
        return;
    }
    let sess_fp = fingerprint_secret(&sess);
    if exhausted_identity::is_exhausted(&sess_fp) {
        // One recovery attempt after console team credit death. Period may have
        // reset; if SuperGrok is still full, wire credit error re-marks.
        exhausted_identity::clear_exhausted(&sess_fp);
    }
    // Prefer SuperGrok recovery first on the credit path (before other console keys).
    config.failover_api_keys.retain(|k| k.trim() != sess);
    config.failover_api_keys.insert(0, sess);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SamplerConfig;

    /// Named contract: memoized-dead console primary + live failover hops before
    /// HTTP. SuperGrok leftover HTTP 402 does not use this path.
    #[test]
    fn prefer_live_dead_console_hops_to_next_console_before_request() {
        exhausted_identity::with_memo_lock(|| {
            let dead = "prefer-live-dead-console-key";
            let live = "prefer-live-next-console-key";
            exhausted_identity::mark_exhausted(&fingerprint_secret(dead));

            let mut config = SamplerConfig {
                api_key: Some(dead.into()),
                failover_api_keys: vec![live.into()],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                ..Default::default()
            };
            let reason = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must prefer next console when console primary is memoized exhausted");
            assert_eq!(config.api_key.as_deref(), Some(live));
            assert!(config.bearer_resolver.is_none());
            assert!(exhausted_identity::is_credential_hop_reason(&reason));
            assert!(reason.contains("console key"), "{reason}");
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "already on live console; no further switch"
            );
            assert_eq!(config.api_key.as_deref(), Some(live));
        });
    }

    /// OIDC refresh: leftover SuperGrok HTTP 402 memo on session_identity_key
    /// must not hop reconstruct when the live api_key is a rotated jwt and a
    /// bearer_resolver is present.
    #[test]
    fn leftover_supergrok_http_402_memo_bearer_only_does_not_hop_prefer_live_between_turns() {
        exhausted_identity::with_memo_lock(|| {
            let old_sess = "session-jwt-before-refresh";
            let new_sess = "session-jwt-after-refresh";
            let console = "console-after-refresh";
            exhausted_identity::mark_exhausted(&fingerprint_secret(old_sess));

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
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "bearer-only SuperGrok leftover HTTP 402 memo must not hop reconstruct"
            );
            assert_eq!(config.api_key.as_deref(), Some(new_sess));
            assert!(
                config.bearer_resolver.is_some(),
                "live SuperGrok bearer must remain"
            );
            assert!(
                config.base_url.contains("cli-chat-proxy"),
                "must stay on SuperGrok host: {}",
                config.base_url
            );
            assert!(
                !exhausted_identity::is_exhausted(&fingerprint_secret(new_sess)),
                "rotated SuperGrok jwt must not be marked by reconstruct leftover 402"
            );
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

    /// Leftover SuperGrok HTTP 402 memo from an earlier request must not hop
    /// reconstruct prefer_live between turns. Stay reconstruct already restores
    /// SuperGrok on the next rebuild when the stay sidecar is set. This request
    /// still hops after SuperGrok HTTP 402 (`apply_retry_decision`).
    #[test]
    fn leftover_supergrok_http_402_memo_does_not_hop_prefer_live_between_turns() {
        exhausted_identity::with_memo_lock(|| {
            let session = "reconstruct-leftover-402-session-jwt";
            let console = "reconstruct-leftover-402-console-key";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));
            assert!(exhausted_identity::is_exhausted(&fingerprint_secret(
                session
            )));

            // Stay reconstruct already put SuperGrok back as primary.
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
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "leftover SuperGrok HTTP 402 memo must not hop reconstruct between turns"
            );
            assert_eq!(
                config.api_key.as_deref(),
                Some(session),
                "SuperGrok must stay primary until this request fails with HTTP 402"
            );
            assert!(
                config.base_url.contains("cli-chat-proxy"),
                "must stay on SuperGrok host: {}",
                config.base_url
            );
        });
    }

    /// Fail-open default: a memo that only exists because the client printed
    /// included SuperGrok period limits at 100% (remaining 0) is unproven.
    /// Operator Usage / Billing pages they can see win. Do not skip SuperGrok
    /// or hop to the console key from that printout. A real SuperGrok HTTP 402
    /// on this request still hops after send (`apply_retry_decision`).
    #[test]
    fn prefer_live_does_not_skip_supergrok_on_false_exhaust_memo_when_fail_open() {
        exhausted_identity::with_memo_lock(|| {
            let session = "prefer-live-false-printout-jwt";
            let console = "prefer-live-false-printout-console";
            // Current product marks from this printout. Fail-open must still
            // keep SuperGrok primary (do not skip on that unproven memo).
            let _ =
                exhausted_identity::sync_allowance_exhaust_from_usage(100.0, Some(session), true);

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
            assert!(
                prefer_live_identity_after_credit_exhaust(&mut config).is_none(),
                "prefer_live must not skip SuperGrok on an unproven client 100% printout memo"
            );
            assert_eq!(
                config.api_key.as_deref(),
                Some(session),
                "SuperGrok must stay primary; console hop is 402 or preferred_method=api_key only"
            );
            assert!(
                config.base_url.contains("cli-chat-proxy"),
                "must stay on SuperGrok host: {}",
                config.base_url
            );
        });
    }

    /// Multi console keys stay ordered: dead console primary hops to the first
    /// live console; remaining console keys stay in list; memoized SuperGrok
    /// session is not re-queued.
    #[test]
    fn prefer_live_multi_console_keys_stable_order_first_live() {
        exhausted_identity::with_memo_lock(|| {
            let session = "b2-session-jwt";
            let business = "b2-console-business";
            let personal = "b2-console-personal";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));
            exhausted_identity::mark_exhausted(&fingerprint_secret(business));

            let mut config = SamplerConfig {
                api_key: Some(business.into()),
                failover_api_keys: vec![session.into(), personal.into()],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            let reason = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must prefer first live console key");
            assert_eq!(config.api_key.as_deref(), Some(personal));
            assert!(
                config.failover_api_keys.is_empty(),
                "remaining console keys keep multi-add / collect order: {:?}",
                config.failover_api_keys
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

    /// First console key also out of allowance: skip to next live console.
    #[test]
    fn prefer_live_skips_exhausted_first_console_to_next() {
        exhausted_identity::with_memo_lock(|| {
            let dead_console = "b2-dead-console";
            let also_dead = "b2-also-dead-console";
            let live_console = "b2-live-console";
            exhausted_identity::mark_exhausted(&fingerprint_secret(dead_console));
            exhausted_identity::mark_exhausted(&fingerprint_secret(also_dead));

            let mut config = SamplerConfig {
                api_key: Some(dead_console.into()),
                failover_api_keys: vec![also_dead.into(), live_console.into()],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                ..Default::default()
            };
            let _ = prefer_live_identity_after_credit_exhaust(&mut config)
                .expect("must reach second live console");
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

    /// Named contract: console credit death with SuperGrok memoized exhausted
    /// must reinject SuperGrok recovery (clear preemptive memo once).
    #[test]
    fn ensure_supergrok_recovery_after_console_credit_clears_memo_and_queues() {
        exhausted_identity::with_memo_lock(|| {
            let session = "recovery-session-jwt";
            let console = "recovery-console-key";
            exhausted_identity::mark_exhausted(&fingerprint_secret(session));

            let mut config = SamplerConfig {
                api_key: Some(console.into()),
                failover_api_keys: vec![],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                session_identity_key: Some(session.into()),
                failover_base_url: Some("https://api.x.ai/v1".into()),
                session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
                ..Default::default()
            };
            ensure_supergrok_recovery_after_console_credit_exhaust(&mut config);
            assert!(
                !exhausted_identity::is_credential_exhausted(session),
                "preemptive SuperGrok memo must clear for one recovery attempt"
            );
            assert_eq!(
                config.failover_api_keys.first().map(String::as_str),
                Some(session),
                "SuperGrok recovery must be first failover: {:?}",
                config.failover_api_keys
            );

            let reason = rotate_identity_config(&mut config, HopCause::CreditExhausted)
                .expect("console→SuperGrok recovery hop");
            assert_eq!(config.api_key.as_deref(), Some(session));
            assert!(
                config.base_url.contains("cli-chat-proxy"),
                "must switch to SuperGrok host: {}",
                config.base_url
            );
            assert!(reason.contains("out of allowance"), "{reason}");
            assert!(
                reason.contains("SuperGrok") || reason.contains("session"),
                "{reason}"
            );
        });
    }

    /// No hop invent when SuperGrok session identity is absent (also dead).
    #[test]
    fn ensure_supergrok_recovery_noop_without_session_identity() {
        exhausted_identity::with_memo_lock(|| {
            let mut config = SamplerConfig {
                api_key: Some("console-only".into()),
                failover_api_keys: vec![],
                base_url: "https://api.x.ai/v1".into(),
                model: "grok-4".into(),
                session_identity_key: None,
                ..Default::default()
            };
            ensure_supergrok_recovery_after_console_credit_exhaust(&mut config);
            assert!(config.failover_api_keys.is_empty());
            assert!(rotate_identity_config(&mut config, HopCause::CreditExhausted).is_none());
        });
    }
}
