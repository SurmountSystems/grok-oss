//! Credit / allowance exhausted credential fingerprint memo (dual-auth).
//!
//! After a **credit / allowance** identity switch, the exhausted identity is
//! recorded so parallel and subsequent turns skip dead keys without re-burning
//! a request. Entries expire after [`DEFAULT_TTL`]. No raw secrets —
//! fingerprints only ([`grok_rate_limit::fingerprint_secret`]).
//!
//! ## Process cache + durable store
//!
//! - **Process-local** [`Instant`] map: fast path within one CLI/pager process.
//! - **Durable** under `$GROK_HOME/exhausted_credits/{fingerprint}.json` (TTL
//!   unix-ms), so preemptive skip survives process restart / “new store load”.
//! - Successful **console-key** requests clear that entry (top-up recovery).
//!   SuperGrok **session** success does **not** clear (extras can 200 while
//!   included weekly is 100%; clearing would put SuperGrok back as primary and
//!   burn more extras). Session recovery: billing usage drop via
//!   [`sync_allowance_exhaust_from_usage`], or TTL.
//!
//! **Rate-limit identity switches do not use this memo.** A plain HTTP 429 is
//! temporary; the switch uses shared `grok-rate-limit` cooldown for the left
//! fingerprint and distinct toast copy (`format_rate_limit_hop_reason`).

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// How long an exhausted fingerprint stays memoized (process + durable).
pub const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60); // 1 hour

/// Subdir under `$GROK_HOME` (mirrors `rate_limits/` spirit).
pub const DURABLE_SUBDIR: &str = "exhausted_credits";

static MEMO: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Drop expired entries. Called on mark/query so a long-lived process does not
/// retain forever entries that are never re-queried.
fn sweep_expired(map: &mut HashMap<String, Instant>) {
    let now = Instant::now();
    map.retain(|_, until| *until > now);
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn grok_home_path() -> PathBuf {
    if let Ok(v) = std::env::var("GROK_HOME") {
        return PathBuf::from(v);
    }
    #[allow(deprecated)]
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".grok")
}

fn durable_dir() -> PathBuf {
    grok_home_path().join(DURABLE_SUBDIR)
}

fn durable_path(fingerprint: &str) -> PathBuf {
    // Fingerprints are hex; keep filename safe.
    let safe: String = fingerprint
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    durable_dir().join(format!("{safe}.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurableRecord {
    until_unix_ms: u64,
}

fn write_durable(fingerprint: &str, until_unix_ms: u64) {
    let dir = durable_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::debug!(error = %e, "exhausted_identity: could not create durable dir");
        return;
    }
    let path = durable_path(fingerprint);
    let rec = DurableRecord { until_unix_ms };
    let Ok(data) = serde_json::to_vec_pretty(&rec) else {
        return;
    };
    // Best-effort atomic-ish write (temp + rename).
    let tmp = path.with_extension("json.tmp");
    match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
    {
        Ok(mut f) => {
            if f.write_all(&data).is_ok() {
                let _ = f.sync_all();
                if let Err(e) = fs::rename(&tmp, &path) {
                    tracing::debug!(error = %e, "exhausted_identity: durable rename failed");
                    let _ = fs::remove_file(&tmp);
                }
            } else {
                let _ = fs::remove_file(&tmp);
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "exhausted_identity: durable write open failed");
        }
    }
}

fn read_durable(fingerprint: &str) -> Option<u64> {
    let path = durable_path(fingerprint);
    let mut file = File::open(&path).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    let rec: DurableRecord = serde_json::from_str(buf.trim()).ok()?;
    Some(rec.until_unix_ms)
}

fn remove_durable(fingerprint: &str) {
    let path = durable_path(fingerprint);
    let _ = fs::remove_file(path);
}

/// Wipe all durable files under the current `$GROK_HOME` exhausted dir (tests).
fn clear_durable_dir() {
    let dir = durable_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let _ = fs::remove_file(path);
        }
    }
}

/// Included SuperGrok weekly/monthly allowance floor for pre-request dual-auth
/// switch (billing `usage_pct`).
///
/// Chosen as **100.0** (not 99): pager `billing_poll_wanted` uses ≥99% so the
/// balance is refreshed near the end of the pool; switch only when the included
/// allowance reports fully used. Matches credit_bar / SpendingLimiter floor
/// semantics (values under 100 stay under 100 until truly exhausted). Avoids
/// switching while ~1% included remains, while still leaving SuperGrok before
/// paid **extras** burn when weekly is 100% and requests still succeed (no 402).
pub const INCLUDED_ALLOWANCE_EXHAUST_PCT: f64 = 100.0;

/// Outcome of syncing billing usage into the credit-exhausted memo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowanceExhaustAction {
    /// Session fingerprint marked (or already marked) out of allowance so the
    /// next request prefers the console key.
    Marked,
    /// Session fingerprint cleared because included usage is below the floor.
    Cleared,
    /// No memo change (below threshold without prior mark, no dual-auth, blank token).
    None,
}

/// Sync SuperGrok session allowance-exhaust memo from billing `usage_pct`.
///
/// When dual-auth failover is available and included usage is at/above
/// [`INCLUDED_ALLOWANCE_EXHAUST_PCT`], mark the session identity exhausted so
/// [`crate::actor::request_task`] preemptive skip prefers the console key
/// **without** waiting for a failed 402 (extras would still succeed on SuperGrok).
///
/// When usage drops below the floor (period reset), clear the same fingerprint
/// so SuperGrok can be primary again.
///
/// `session_token` is the raw SuperGrok/session JWT (fingerprinted; never stored
/// raw in the memo). Empty tokens and missing failover are no-ops.
pub fn sync_allowance_exhaust_from_usage(
    usage_pct: f64,
    session_token: Option<&str>,
    has_console_failover: bool,
) -> AllowanceExhaustAction {
    use grok_rate_limit::fingerprint_secret;

    let Some(tok) = session_token.map(str::trim).filter(|s| !s.is_empty()) else {
        return AllowanceExhaustAction::None;
    };
    let fp = fingerprint_secret(tok);

    if usage_pct >= INCLUDED_ALLOWANCE_EXHAUST_PCT {
        if !has_console_failover {
            return AllowanceExhaustAction::None;
        }
        mark_exhausted(&fp);
        return AllowanceExhaustAction::Marked;
    }

    // Period reset / recovery: only clear if this SuperGrok identity was marked out.
    if is_exhausted(&fp) {
        clear_exhausted(&fp);
        return AllowanceExhaustAction::Cleared;
    }
    AllowanceExhaustAction::None
}

/// Mark `fingerprint` as credit-exhausted until `now + ttl`.
///
/// Empty fingerprints are ignored (never memoize blank credentials).
/// Writes process cache **and** durable `$GROK_HOME` record.
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
    let until_ms = now_unix_ms().saturating_add(ttl.as_millis() as u64);
    write_durable(fp, until_ms);
}

/// True when `fingerprint` is currently memoized as exhausted.
///
/// Checks process cache first; on miss, loads durable `$GROK_HOME` record and
/// hydrates the process cache when still live.
pub fn is_exhausted(fingerprint: &str) -> bool {
    let fp = fingerprint.trim();
    if fp.is_empty() {
        return false;
    }
    if process_is_exhausted(fp) {
        return true;
    }
    // Durable reload (survives process restart / process-memo clear).
    match read_durable(fp) {
        Some(until_ms) => {
            let now = now_unix_ms();
            if until_ms > now {
                let remaining = Duration::from_millis(until_ms - now);
                if let Ok(mut guard) = MEMO.lock() {
                    sweep_expired(&mut guard);
                    guard.insert(fp.to_owned(), Instant::now() + remaining);
                }
                true
            } else {
                remove_durable(fp);
                false
            }
        }
        None => false,
    }
}

fn process_is_exhausted(fp: &str) -> bool {
    let Ok(mut guard) = MEMO.lock() else {
        return false;
    };
    sweep_expired(&mut guard);
    match guard.get(fp) {
        Some(until) if *until > Instant::now() => true,
        Some(_) => {
            guard.remove(fp);
            false
        }
        None => false,
    }
}

/// Clear one fingerprint from process + durable memo (e.g. after a successful
/// **console-key** request, or top-up recovery). Callers must not clear SuperGrok
/// session fingerprints on extras-paid 200s — see
/// `clear_exhausted_after_success` in the sampler request task.
pub fn clear_exhausted(fingerprint: &str) {
    let fp = fingerprint.trim();
    if fp.is_empty() {
        return;
    }
    if let Ok(mut guard) = MEMO.lock() {
        guard.remove(fp);
    }
    remove_durable(fp);
}

/// Clear all process-local memo entries.
///
/// Does **not** wipe durable files on disk (use [`clear_all_including_durable`]
/// in tests that own a temp `$GROK_HOME`).
pub fn clear_all() {
    if let Ok(mut guard) = MEMO.lock() {
        guard.clear();
    }
}

/// Clear process memo **and** durable files under current `$GROK_HOME`.
pub fn clear_all_including_durable() {
    clear_all();
    clear_durable_dir();
}

/// Serialize tests that mutate the process-global exhausted memo.
///
/// Isolates `$GROK_HOME` to a temp dir so durable writes never touch the
/// operator's real home. Clears process + durable before and after `f`.
#[cfg(test)]
pub fn with_memo_lock<R>(f: impl FnOnce() -> R) -> R {
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = TempDir::new().expect("temp GROK_HOME for exhausted memo tests");
    let _home = EnvGuard::set("GROK_HOME", dir.path());
    clear_all_including_durable();
    let out = f();
    clear_all_including_durable();
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

/// Why a multi-identity switch ran (toast copy + memo policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopCause {
    /// Credit / spending / SuperGrok allowance limit — mark 1h exhausted memo;
    /// stay on the replacement identity on later turns.
    CreditExhausted,
    /// Plain HTTP 429 — temporary; do **not** use credit memo.
    RateLimited,
}

/// Status/toast copy for a successful credit / allowance identity switch. No secrets.
pub fn format_credential_hop_reason(from: CredentialLabel, to: CredentialLabel) -> String {
    format_hop_reason(from, to, HopCause::CreditExhausted)
}

/// Status/toast copy for a successful rate-limit identity switch. No secrets.
pub fn format_rate_limit_hop_reason(from: CredentialLabel, to: CredentialLabel) -> String {
    format_hop_reason(from, to, HopCause::RateLimited)
}

/// Status/toast copy for a multi-identity switch. No secrets.
pub fn format_hop_reason(from: CredentialLabel, to: CredentialLabel, cause: HopCause) -> String {
    let cause_label = match cause {
        HopCause::CreditExhausted => "out of allowance",
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

/// Exact allow-list of dual-auth identity-switch status/toast strings (no heuristic).
///
/// Built from every [`CredentialLabel`] pair × [`HopCause`] via
/// [`format_hop_reason`] so copy edits stay toast-eligible only when the
/// formatter still emits them.
fn hop_reason_allowlist() -> [&'static str; 8] {
    // Allowance + rate-limit × (session→key, key→session, key→key, session→session).
    // Keep in sync with format_hop_reason; tests assert equality.
    [
        "Switched SuperGrok session → console key (out of allowance)",
        "Switched console key → SuperGrok session (out of allowance)",
        "Switched to next console key (out of allowance)",
        "Switched to next SuperGrok session (out of allowance)",
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

/// Path of the durable file for `fingerprint` (tests / diagnostics).
#[cfg(test)]
fn durable_path_for_test(fingerprint: &str) -> PathBuf {
    durable_path(fingerprint)
}

/// Whether a durable record exists on disk (tests).
#[cfg(test)]
fn durable_exists_for_test(fingerprint: &str) -> bool {
    durable_path(fingerprint).is_file()
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
    fn durable_memo_survives_process_memo_clear() {
        with_memo_lock(|| {
            mark_exhausted("fp-durable");
            assert!(is_exhausted("fp-durable"));
            assert!(
                durable_exists_for_test("fp-durable"),
                "mark must write under $GROK_HOME/{DURABLE_SUBDIR}"
            );
            let expected = grok_home_path()
                .join(DURABLE_SUBDIR)
                .join("fp-durable.json");
            assert_eq!(durable_path_for_test("fp-durable"), expected);

            // Simulate process restart: drop process cache only.
            clear_all();
            assert!(
                is_exhausted("fp-durable"),
                "preemptive skip must survive process-memo clear via durable $GROK_HOME load"
            );
        });
    }

    #[test]
    fn clear_exhausted_removes_process_and_durable() {
        with_memo_lock(|| {
            mark_exhausted("fp-clear");
            assert!(is_exhausted("fp-clear"));
            clear_exhausted("fp-clear");
            assert!(!is_exhausted("fp-clear"));
            assert!(
                !durable_exists_for_test("fp-clear"),
                "clear must remove durable file"
            );
            // Process clear alone should not resurrect.
            clear_all();
            assert!(!is_exhausted("fp-clear"));
        });
    }

    #[test]
    fn durable_expired_entry_is_not_exhausted_after_process_clear() {
        with_memo_lock(|| {
            mark_exhausted_for("fp-exp", Duration::from_millis(1));
            std::thread::sleep(Duration::from_millis(5));
            clear_all();
            assert!(
                !is_exhausted("fp-exp"),
                "expired durable record must not keep skipping this identity"
            );
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
            "Switched SuperGrok session → console key (out of allowance)"
        );
        assert!(is_credential_hop_reason(&s));
        assert!(!s.contains("sk-") && !s.contains("jwt"));

        let reverse = format_credential_hop_reason(
            CredentialLabel::ConsoleKey,
            CredentialLabel::SuperGrokSession,
        );
        assert_eq!(
            reverse,
            "Switched console key → SuperGrok session (out of allowance)"
        );

        let same =
            format_credential_hop_reason(CredentialLabel::ConsoleKey, CredentialLabel::ConsoleKey);
        assert_eq!(same, "Switched to next console key (out of allowance)");
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
            "out of allowance on active API key"
        ));
        // Loose heuristic would match; exact allow-list must not.
        assert!(!is_credential_hop_reason(
            "Switched something weird (out of allowance)"
        ));
        assert!(!is_credential_hop_reason(
            "Switched SuperGrok session → console key (out of allowance) extra"
        ));
        assert!(!is_credential_hop_reason(
            "Switched SuperGrok session → console key (rate limited) extra"
        ));
    }

    /// Named contract: included SuperGrok usage ≥ 100% + dual-auth failover →
    /// mark SuperGrok out of allowance so the next request prefers the console
    /// key without waiting for HTTP 402.
    #[test]
    fn allowance_exhaust_marks_session_at_100_pct_with_failover() {
        use grok_rate_limit::fingerprint_secret;

        with_memo_lock(|| {
            let session = "session-jwt-weekly-full";
            let fp = fingerprint_secret(session);
            assert!(!is_exhausted(&fp));

            let action = sync_allowance_exhaust_from_usage(100.0, Some(session), true);
            assert_eq!(action, AllowanceExhaustAction::Marked);
            assert!(
                is_exhausted(&fp),
                "usage 100% + dual-auth must mark SuperGrok session out of allowance"
            );
            assert!(
                durable_exists_for_test(&fp),
                "mark from billing usage must write durable memo under $GROK_HOME"
            );
        });
    }

    #[test]
    fn allowance_exhaust_does_not_mark_at_99_pct() {
        use grok_rate_limit::fingerprint_secret;

        with_memo_lock(|| {
            let session = "session-jwt-almost";
            let fp = fingerprint_secret(session);
            // 99% is the poll floor, not the switch floor.
            let action = sync_allowance_exhaust_from_usage(99.0, Some(session), true);
            assert_eq!(action, AllowanceExhaustAction::None);
            assert!(
                !is_exhausted(&fp),
                "must not leave SuperGrok while included allowance still reports < 100%"
            );
        });
    }

    #[test]
    fn allowance_exhaust_requires_console_failover() {
        use grok_rate_limit::fingerprint_secret;

        with_memo_lock(|| {
            let session = "session-jwt-solo";
            let fp = fingerprint_secret(session);
            let action = sync_allowance_exhaust_from_usage(100.0, Some(session), false);
            assert_eq!(action, AllowanceExhaustAction::None);
            assert!(
                !is_exhausted(&fp),
                "session-only: no console key target → do not mark SuperGrok out"
            );
        });
    }

    #[test]
    fn allowance_exhaust_clears_when_usage_drops_after_period_reset() {
        use grok_rate_limit::fingerprint_secret;

        with_memo_lock(|| {
            let session = "session-jwt-reset";
            let fp = fingerprint_secret(session);
            assert_eq!(
                sync_allowance_exhaust_from_usage(100.0, Some(session), true),
                AllowanceExhaustAction::Marked
            );
            assert!(is_exhausted(&fp));

            let action = sync_allowance_exhaust_from_usage(0.0, Some(session), true);
            assert_eq!(action, AllowanceExhaustAction::Cleared);
            assert!(
                !is_exhausted(&fp),
                "period reset must clear memo so SuperGrok can be primary again"
            );
        });
    }

    #[test]
    fn allowance_exhaust_ignores_blank_session_token() {
        with_memo_lock(|| {
            assert_eq!(
                sync_allowance_exhaust_from_usage(100.0, None, true),
                AllowanceExhaustAction::None
            );
            assert_eq!(
                sync_allowance_exhaust_from_usage(100.0, Some("  "), true),
                AllowanceExhaustAction::None
            );
        });
    }
}
