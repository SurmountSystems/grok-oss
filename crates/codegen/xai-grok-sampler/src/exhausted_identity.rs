//! Process-local memo of **credit-exhausted** credential fingerprints (dual-auth).
//!
//! After a **credit** hop, the exhausted identity is recorded so parallel and
//! subsequent turns skip dead keys without re-burning a request. Entries
//! expire after [`DEFAULT_TTL`] (or on process restart). No raw secrets —
//! fingerprints only ([`grok_rate_limit::fingerprint_secret`]).
//!
//! **Rate-limit hops do not use this memo.** A plain HTTP 429 is temporary;
//! identity switch uses shared `grok-rate-limit` cooldown for the left
//! fingerprint and distinct toast copy (`format_rate_limit_hop_reason`).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// How long an exhausted fingerprint stays memoized (process-local).
pub const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60); // 1 hour

static MEMO: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Drop expired entries. Called on mark/query so a long-lived process does not
/// retain forever entries that are never re-queried.
fn sweep_expired(map: &mut HashMap<String, Instant>) {
    let now = Instant::now();
    map.retain(|_, until| *until > now);
}

/// Mark `fingerprint` as credit-exhausted until `now + ttl`.
///
/// Empty fingerprints are ignored (never memoize blank credentials).
pub fn mark_exhausted(fingerprint: &str) {
    mark_exhausted_for(fingerprint, DEFAULT_TTL);
}

/// Testable variant with explicit TTL.
pub fn mark_exhausted_for(fingerprint: &str, ttl: Duration) {
    let fp = fingerprint.trim();
    if fp.is_empty() {
        return;
    }
    let until = Instant::now() + ttl;
    if let Ok(mut guard) = MEMO.lock() {
        sweep_expired(&mut guard);
        guard.insert(fp.to_owned(), until);
    }
}

/// True when `fingerprint` is currently memoized as exhausted.
pub fn is_exhausted(fingerprint: &str) -> bool {
    let fp = fingerprint.trim();
    if fp.is_empty() {
        return false;
    }
    let Ok(mut guard) = MEMO.lock() else {
        return false;
    };
    sweep_expired(&mut guard);
    match guard.get(fp) {
        Some(until) if *until > Instant::now() => true,
        Some(_) => {
            // Should already be gone via sweep; belt-and-suspenders.
            guard.remove(fp);
            false
        }
        None => false,
    }
}

/// Clear all memo entries (tests / process restart simulation).
pub fn clear_all() {
    if let Ok(mut guard) = MEMO.lock() {
        guard.clear();
    }
}

/// Serialize tests that mutate the process-global exhausted memo.
///
/// Clears the memo before and after `f`. Hold across the full
/// arrange/act/assert so multi-threaded cargo tests cannot race each other.
#[cfg(test)]
pub fn with_memo_lock<R>(f: impl FnOnce() -> R) -> R {
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_all();
    let out = f();
    clear_all();
    out
}

/// Human labels for dual-auth hop status / toast (no raw keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialLabel {
    SuperGrokSession,
    ConsoleKey,
}

impl CredentialLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SuperGrokSession => "SuperGrok session",
            Self::ConsoleKey => "console key",
        }
    }

    /// All label variants (for allow-list tests).
    #[cfg(test)]
    const ALL: [Self; 2] = [Self::SuperGrokSession, Self::ConsoleKey];
}

/// Why a multi-identity hop ran (toast copy + memo policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopCause {
    /// Credit / spending limit — mark 1h exhausted memo; sticky skip.
    CreditExhausted,
    /// Plain HTTP 429 — temporary; do **not** use credit memo.
    RateLimited,
}

/// Status/toast copy for a successful credit hop. No secrets.
pub fn format_credential_hop_reason(from: CredentialLabel, to: CredentialLabel) -> String {
    format_hop_reason(from, to, HopCause::CreditExhausted)
}

/// Status/toast copy for a successful rate-limit identity hop. No secrets.
pub fn format_rate_limit_hop_reason(from: CredentialLabel, to: CredentialLabel) -> String {
    format_hop_reason(from, to, HopCause::RateLimited)
}

/// Status/toast copy for a multi-identity hop. No secrets.
pub fn format_hop_reason(from: CredentialLabel, to: CredentialLabel, cause: HopCause) -> String {
    let cause_label = match cause {
        HopCause::CreditExhausted => "credit exhausted",
        HopCause::RateLimited => "rate limited",
    };
    if from == to {
        format!("Switched to next {} ({cause_label})", to.as_str())
    } else {
        format!(
            "Switched {} → {} ({cause_label})",
            from.as_str(),
            to.as_str()
        )
    }
}

/// Exact allow-list of dual-auth hop status/toast strings (no heuristic).
///
/// Built from every [`CredentialLabel`] pair × [`HopCause`] via
/// [`format_hop_reason`] so copy edits stay toast-eligible only when the
/// formatter still emits them.
fn hop_reason_allowlist() -> [&'static str; 8] {
    // Credit + rate-limit × (session→key, key→session, key→key, session→session).
    // Keep in sync with format_hop_reason; tests assert equality.
    [
        "Switched SuperGrok session → console key (credit exhausted)",
        "Switched console key → SuperGrok session (credit exhausted)",
        "Switched to next console key (credit exhausted)",
        "Switched to next SuperGrok session (credit exhausted)",
        "Switched SuperGrok session → console key (rate limited)",
        "Switched console key → SuperGrok session (rate limited)",
        "Switched to next console key (rate limited)",
        "Switched to next SuperGrok session (rate limited)",
    ]
}

/// True when `reason` is dual-auth hop chrome (status or toast).
///
/// Exact match against the allow-list of [`format_hop_reason`]
/// outputs — not a loose substring heuristic.
pub fn is_credential_hop_reason(reason: &str) -> bool {
    hop_reason_allowlist().contains(&reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_query_exhausted() {
        with_memo_lock(|| {
            assert!(!is_exhausted("fp-a"));
            mark_exhausted("fp-a");
            assert!(is_exhausted("fp-a"));
            assert!(!is_exhausted("fp-b"));
        });
    }

    #[test]
    fn empty_fingerprint_never_memoized() {
        with_memo_lock(|| {
            mark_exhausted("");
            mark_exhausted("   ");
            assert!(!is_exhausted(""));
            assert!(!is_exhausted("   "));
        });
    }

    #[test]
    fn expired_entry_is_not_exhausted() {
        with_memo_lock(|| {
            mark_exhausted_for("fp-old", Duration::from_millis(1));
            std::thread::sleep(Duration::from_millis(5));
            assert!(!is_exhausted("fp-old"));
        });
    }

    #[test]
    fn sweep_drops_expired_siblings_on_mark() {
        with_memo_lock(|| {
            mark_exhausted_for("fp-old", Duration::from_millis(1));
            std::thread::sleep(Duration::from_millis(5));
            mark_exhausted("fp-new");
            // Mark sweeps; is_exhausted must not resurrect the old entry.
            assert!(!is_exhausted("fp-old"));
            assert!(is_exhausted("fp-new"));
        });
    }

    #[test]
    fn hop_reason_labels_no_secrets() {
        let s = format_credential_hop_reason(
            CredentialLabel::SuperGrokSession,
            CredentialLabel::ConsoleKey,
        );
        assert_eq!(
            s,
            "Switched SuperGrok session → console key (credit exhausted)"
        );
        assert!(is_credential_hop_reason(&s));
        assert!(!s.contains("sk-") && !s.contains("jwt"));

        let reverse = format_credential_hop_reason(
            CredentialLabel::ConsoleKey,
            CredentialLabel::SuperGrokSession,
        );
        assert_eq!(
            reverse,
            "Switched console key → SuperGrok session (credit exhausted)"
        );

        let same =
            format_credential_hop_reason(CredentialLabel::ConsoleKey, CredentialLabel::ConsoleKey);
        assert_eq!(same, "Switched to next console key (credit exhausted)");
        assert!(is_credential_hop_reason(&same));
    }

    #[test]
    fn rate_limit_hop_reason_labels_no_secrets() {
        let s = format_rate_limit_hop_reason(
            CredentialLabel::SuperGrokSession,
            CredentialLabel::ConsoleKey,
        );
        assert_eq!(s, "Switched SuperGrok session → console key (rate limited)");
        assert!(is_credential_hop_reason(&s));
        assert!(!s.contains("credit"));
        assert!(!s.contains("sk-") && !s.contains("jwt"));

        let same =
            format_rate_limit_hop_reason(CredentialLabel::ConsoleKey, CredentialLabel::ConsoleKey);
        assert_eq!(same, "Switched to next console key (rate limited)");
        assert!(is_credential_hop_reason(&same));
    }

    #[test]
    fn all_formatter_outputs_are_toast_eligible() {
        for cause in [HopCause::CreditExhausted, HopCause::RateLimited] {
            for from in CredentialLabel::ALL {
                for to in CredentialLabel::ALL {
                    let reason = format_hop_reason(from, to, cause);
                    assert!(
                        is_credential_hop_reason(&reason),
                        "formatter output must stay toast-eligible: {reason}"
                    );
                    assert!(
                        hop_reason_allowlist().contains(&reason.as_str()),
                        "allow-list out of sync with formatter: {reason}"
                    );
                }
            }
        }
    }

    #[test]
    fn non_hop_reasons_not_flagged() {
        // Bare transport copy is not hop chrome (even though it says "rate limited").
        assert!(!is_credential_hop_reason("rate limited"));
        assert!(!is_credential_hop_reason(
            "credit exhausted on active API key"
        ));
        // Loose heuristic would match; exact allow-list must not.
        assert!(!is_credential_hop_reason(
            "Switched something weird (credit exhausted)"
        ));
        assert!(!is_credential_hop_reason(
            "Switched SuperGrok session → console key (credit exhausted) extra"
        ));
        assert!(!is_credential_hop_reason(
            "Switched SuperGrok session → console key (rate limited) extra"
        ));
    }
}
