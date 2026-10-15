//! Deliver an already-chosen included SuperGrok period limits pin to other
//! grok-oss processes on this host with these accounts.
//!
//! The click writes the local pin first. This module writes that same choice
//! into each other process's own `limits_pins.json`. It does not re-exec and
//! it does not send SIGUSR1. Stock `preferred_method = "api_key"` stays in
//! each process's config. `use_console` stays false, so the console API key
//! stays unused.
//!
//! A live leader applies the same pin when it receives
//! [`ControlCommand::ApplyIncludedLimits`](xai_grok_shell::leader::ControlCommand::ApplyIncludedLimits).
//! The L0 sleep row in the peer test has no leader socket, so this function
//! writes that row's own pin file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use xai_grok_shell::auth::SupergrokIdentityPin;
use xai_grok_shell::auth::limits_pins::{MeterSource, apply_included_limits_choice_under};

/// Deliver `account`'s included SuperGrok period limits to every other
/// grok-oss process on this host with these accounts.
///
/// `meter` is the meter the click already chose. Any meter other than
/// included SuperGrok period limits writes nothing. This function leaves the
/// clicker's own `limits_pins.json` alone. A process on another host stays.
/// The surmount-1 builder row stays. A grok-oss logged into a different
/// account stays.
pub fn broadcast_included_limits_choice(account: SupergrokIdentityPin, meter: MeterSource) {
    if meter != MeterSource::Included {
        return;
    }
    let clicker_home = live_grok_home();
    let this_accounts = account_keys(&clicker_home);
    if this_accounts.is_empty() {
        return;
    }
    let Ok(sessions) = xai_grok_active_sessions::list_live_in(&clicker_home) else {
        return;
    };
    let roster = crate::views::dashboard::row::l0_roster_from_active_sessions(&sessions, None);
    let self_pid = std::process::id();
    for session in &sessions {
        if !xai_grok_shell::util::is_grok_process(session.pid) {
            continue;
        }
        let session_id = session.session_id.0.to_string();
        let host = roster
            .iter()
            .find(|entry| entry.session_id == session_id)
            .and_then(|entry| entry.origin.host.as_deref());
        let Some(peer_home) = grok_home_from_process_environ(session.pid) else {
            continue;
        };
        if same_home(&clicker_home, &peer_home) {
            continue;
        }
        let accounts = account_keys(&peer_home);
        if !crate::views::dashboard::row::limits_choice_reaches_l0_row(
            host,
            &accounts,
            &this_accounts,
            session.pid,
            self_pid,
        ) {
            continue;
        }
        let _ = apply_included_limits_choice_under(&peer_home, account);
    }
}

fn live_grok_home() -> PathBuf {
    if let Some(value) = std::env::var_os("GROK_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(value);
    }
    xai_dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
}

fn account_keys(home: &Path) -> BTreeSet<(String, String)> {
    let path = home.join("auth.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    let Ok(store) = serde_json::from_str::<xai_grok_login::model::AuthStore>(text.trim()) else {
        return BTreeSet::new();
    };
    xai_grok_shell::auth::list_supergrok_principal_listings(&store)
        .into_iter()
        .map(|listing| (listing.role_label.to_string(), listing.identity_id))
        .collect()
}

fn same_home(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (dunce::canonicalize(left), dunce::canonicalize(right)) {
        (Ok(left_home), Ok(right_home)) => left_home == right_home,
        _ => false,
    }
}

fn grok_home_from_process_environ(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let data = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
        for entry in data.split(|byte| *byte == 0) {
            let Ok(text) = std::str::from_utf8(entry) else {
                continue;
            };
            let Some(value) = text.strip_prefix("GROK_HOME=") else {
                continue;
            };
            if value.is_empty() {
                return None;
            }
            return Some(PathBuf::from(value));
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}
