//! Flock-backed SuperGrok limits snapshot under `$GROK_HOME`.
//!
//! One process holds the exclusive flock and may call SuperGrok
//! `GET …/billing?format=credits` (active and siblings) plus Management
//! prepaid / postpaid / series. Other live TUIs wait, read
//! `limits_snapshot.json`, and apply the meters into the same process maps
//! [`super::remember_supergrok_included_billing`] already fills.
//!
//! No daemon. Rebuild SIGUSR1 is not used (that signal means fleet relaunch).
//! `active_sessions.json` is a hint only; flock is the authority.
//! Honor [`grok_rate_limit::DISABLE_ENV`] so isolated tests stay hermetic
//! (each process fetches; no shared file).
//!
//! Snapshot stores identity ids, included SuperGrok period used percent,
//! reset, SuperGrok dollar extras cents, and poll outcome class. Never JWTs
//! or API keys.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::allowance_exhaust_from_billing::{
    remember_supergrok_billing_poll_failed, remember_supergrok_billing_poll_ok,
    remember_supergrok_build_usage, remember_supergrok_dollar_extras,
    remember_supergrok_included_billing,
};
use super::included_poll_history::record_included_poll_now;
use super::xai_management::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS;

/// Snapshot JSON under `$GROK_HOME`.
pub const SNAPSHOT_FILE_NAME: &str = "limits_snapshot.json";

/// Exclusive flock file under `$GROK_HOME` (not the JSON itself).
pub const LOCK_FILE_NAME: &str = "limits_snapshot.lock";

/// Alias of [`LOCK_FILE_NAME`] for callers that want the snapshot-prefixed name.
pub const SNAPSHOT_LOCK_FILE_NAME: &str = LOCK_FILE_NAME;

/// Shared snapshot freshness window. Matches Management process TTL (60s).
pub const SNAPSHOT_TTL_SECS: u64 = CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS;

/// [`SNAPSHOT_TTL_SECS`] as a [`Duration`].
pub const SNAPSHOT_TTL: Duration = Duration::from_secs(SNAPSHOT_TTL_SECS);

/// Document schema version (integer; bump when fields change meaning).
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Poll outcome class stored on disk (never a secret).
pub const POLL_OUTCOME_OK: &str = "ok";
/// Auth-class credits poll fail.
pub const POLL_OUTCOME_AUTH: &str = "auth";
/// Transport / timeout class.
pub const POLL_OUTCOME_NETWORK: &str = "network";
/// Other non-auth fail.
pub const POLL_OUTCOME_OTHER: &str = "other";
/// Never polled / unknown.
pub const POLL_OUTCOME_NEVER: &str = "never";

/// Whether this collect should bust a fresh snapshot when this process is leader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitsSnapshotMode {
    /// Background TUI poll: reuse a snapshot younger than [`SNAPSHOT_TTL`].
    HonorTtl,
    /// Explicit `grok-oss limits` / `/limits`: fetch if this process holds
    /// exclusive flock without waiting. After waiting on a leader, reuse the
    /// just-written snapshot unless it is still missing or stale.
    ForceRefresh,
}

/// How this process obtained the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitsSnapshotRole {
    /// This process held exclusive flock and called the fetch callback.
    LeaderFetched,
    /// This process read a usable snapshot and did not HTTP.
    FollowerRead,
    /// [`grok_rate_limit::DISABLE_ENV`] is set: fetch without coordination.
    UncoordinatedFetch,
}

/// One SuperGrok principal's meters in the shared snapshot (no tokens).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsSnapshotIdentity {
    pub identity_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grok_build_usage_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unified_billing_user: Option<bool>,
    /// `ok` / `auth` / `network` / `other` / `never`.
    #[serde(default)]
    pub poll_outcome: String,
}

/// Optional console team prepaid / postpaid / series meters (no management key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LimitsSnapshotManagement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepaid_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_period_total_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_oauth_class_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_api_class_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_other_class_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_default_credits_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_default_credits_issued_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_billing_cycle_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postpaid_billing_cycle_month: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_day_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_oauth_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_api_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_other_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_series_limit_reached: Option<bool>,
}

/// On-disk snapshot. Never includes JWTs or API keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsSnapshotDocument {
    pub schema_version: u32,
    pub fetched_at_unix_ms: u64,
    #[serde(default)]
    pub identities: Vec<LimitsSnapshotIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management: Option<LimitsSnapshotManagement>,
}

impl LimitsSnapshotDocument {
    /// Empty document stamped at `fetched_at_unix_ms`.
    pub fn empty(fetched_at_unix_ms: u64) -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            fetched_at_unix_ms,
            identities: Vec::new(),
            management: None,
        }
    }
}

/// Unix milliseconds (0 if the clock is before the epoch).
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// True when the snapshot is missing a usable timestamp or older than TTL.
pub fn snapshot_is_stale(doc: &LimitsSnapshotDocument, now_unix_ms: u64) -> bool {
    if doc.fetched_at_unix_ms == 0 {
        return true;
    }
    now_unix_ms.saturating_sub(doc.fetched_at_unix_ms) >= SNAPSHOT_TTL.as_millis() as u64
}

/// Paths used by the hub under `grok_home`.
pub fn snapshot_paths(grok_home: impl AsRef<Path>) -> (PathBuf, PathBuf) {
    let home = grok_home.as_ref();
    (home.join(SNAPSHOT_FILE_NAME), home.join(LOCK_FILE_NAME))
}

/// Whether shared snapshot coordination is disabled for this process.
pub fn shared_limits_snapshot_disabled() -> bool {
    grok_rate_limit::shared_rate_limits_disabled()
}

/// True when JSON looks like it stored a token, key, or a forbidden substring.
pub fn snapshot_json_contains_secrets(json: &str, extra_forbidden: &[&str]) -> bool {
    let lower = json.to_ascii_lowercase();
    if lower.contains("access_token")
        || lower.contains("accesstoken")
        || lower.contains("authorization")
        || lower.contains("\"jwt\"")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("bearer ")
    {
        return true;
    }
    extra_forbidden
        .iter()
        .any(|s| !s.is_empty() && json.contains(*s))
}

/// Apply a snapshot into the process included-billing maps (no HTTP).
pub fn apply_limits_snapshot(doc: &LimitsSnapshotDocument) {
    for id in &doc.identities {
        let identity = id.identity_id.trim();
        if identity.is_empty() {
            continue;
        }
        if let Some(pct) = id.usage_pct {
            remember_supergrok_included_billing(
                identity,
                pct,
                id.period_end.as_deref(),
                id.period_type.as_deref(),
            );
            record_included_poll_now(identity, pct, id.grok_build_usage_pct, id.extras_cents);
        }
        if let Some(cents) = id.extras_cents {
            remember_supergrok_dollar_extras(identity, cents);
        }
        if let Some(build) = id.grok_build_usage_pct {
            remember_supergrok_build_usage(identity, build);
        }
        match id.poll_outcome.as_str() {
            POLL_OUTCOME_OK => remember_supergrok_billing_poll_ok(identity),
            POLL_OUTCOME_AUTH => remember_supergrok_billing_poll_failed(identity, "auth failed"),
            POLL_OUTCOME_NETWORK => {
                remember_supergrok_billing_poll_failed(identity, "network error")
            }
            POLL_OUTCOME_OTHER => remember_supergrok_billing_poll_failed(identity, "other error"),
            _ => {}
        }
    }
    if let Some(mgmt) = doc.management.as_ref() {
        apply_management_snapshot(mgmt);
    }
}

fn apply_management_snapshot(mgmt: &LimitsSnapshotManagement) {
    use super::xai_management::{
        ConsoleTeamPostpaidPreview, ConsoleTeamUsageSeries, seed_console_team_postpaid_cache,
        seed_console_team_prepaid_cache, seed_console_team_usage_series_cache,
    };
    let team = mgmt.team_id.as_deref().unwrap_or("").trim();
    if team.is_empty() {
        return;
    }
    if let Some(cents) = mgmt.prepaid_cents {
        seed_console_team_prepaid_cache(team, cents);
    }
    if let Some(total) = mgmt.postpaid_period_total_cents {
        seed_console_team_postpaid_cache(ConsoleTeamPostpaidPreview {
            team_id: team.to_owned(),
            period_total_cents: total,
            oauth_class_cents: mgmt.postpaid_oauth_class_cents.unwrap_or(0),
            api_class_cents: mgmt.postpaid_api_class_cents.unwrap_or(0),
            other_class_cents: mgmt.postpaid_other_class_cents.unwrap_or(0),
            default_credits_cents: mgmt.postpaid_default_credits_cents,
            default_credits_issued_cents: mgmt.postpaid_default_credits_issued_cents,
            billing_cycle_year: mgmt.postpaid_billing_cycle_year,
            billing_cycle_month: mgmt.postpaid_billing_cycle_month,
        });
    }
    if let (Some(start), Some(end)) = (
        mgmt.usage_series_start.as_deref(),
        mgmt.usage_series_end.as_deref(),
    ) {
        let day_window = mgmt.usage_series_day_window.unwrap_or(7);
        seed_console_team_usage_series_cache(
            ConsoleTeamUsageSeries {
                team_id: team.to_owned(),
                start_time: start.to_owned(),
                end_time: end.to_owned(),
                timezone: mgmt
                    .usage_series_timezone
                    .clone()
                    .unwrap_or_else(|| "Etc/GMT".into()),
                rows: Vec::new(),
                oauth_class_usd: mgmt.usage_series_oauth_usd.unwrap_or(0.0),
                api_class_usd: mgmt.usage_series_api_usd.unwrap_or(0.0),
                other_class_usd: mgmt.usage_series_other_usd.unwrap_or(0.0),
                limit_reached: mgmt.usage_series_limit_reached.unwrap_or(false),
            },
            day_window,
        );
    }
}

/// Fetch Management prepaid / postpaid / series into snapshot fields (no keys).
///
/// Returns `None` when no management key is configured. Call only from a
/// hub leader fetch callback so followers do not stampede the Management API.
pub async fn fetch_management_into_snapshot() -> Option<LimitsSnapshotManagement> {
    use super::xai_management::{
        USAGE_SERIES_DEFAULT_DAY_WINDOW, fetch_console_team_postpaid_preview_default,
        fetch_console_team_prepaid_balance_default, fetch_console_team_usage_series_default,
        resolve_management_api_key_default,
    };
    resolve_management_api_key_default()?;
    let prepaid = fetch_console_team_prepaid_balance_default().await;
    let postpaid = fetch_console_team_postpaid_preview_default().await;
    let series = fetch_console_team_usage_series_default(USAGE_SERIES_DEFAULT_DAY_WINDOW).await;
    let team_id = prepaid
        .as_ref()
        .map(|m| m.team_id.clone())
        .or_else(|| postpaid.as_ref().map(|m| m.team_id.clone()))
        .or_else(|| series.as_ref().map(|m| m.team_id.clone()));
    if team_id.is_none() && prepaid.is_none() && postpaid.is_none() && series.is_none() {
        return None;
    }
    Some(LimitsSnapshotManagement {
        team_id,
        prepaid_cents: prepaid.as_ref().map(|m| m.balance_cents),
        postpaid_period_total_cents: postpaid.as_ref().map(|m| m.period_total_cents),
        postpaid_oauth_class_cents: postpaid.as_ref().map(|m| m.oauth_class_cents),
        postpaid_api_class_cents: postpaid.as_ref().map(|m| m.api_class_cents),
        postpaid_other_class_cents: postpaid.as_ref().map(|m| m.other_class_cents),
        postpaid_default_credits_cents: postpaid.as_ref().and_then(|m| m.default_credits_cents),
        postpaid_default_credits_issued_cents: postpaid
            .as_ref()
            .and_then(|m| m.default_credits_issued_cents),
        postpaid_billing_cycle_year: postpaid.as_ref().and_then(|m| m.billing_cycle_year),
        postpaid_billing_cycle_month: postpaid.as_ref().and_then(|m| m.billing_cycle_month),
        usage_series_day_window: series.as_ref().map(|_| USAGE_SERIES_DEFAULT_DAY_WINDOW),
        usage_series_start: series.as_ref().map(|s| s.start_time.clone()),
        usage_series_end: series.as_ref().map(|s| s.end_time.clone()),
        usage_series_timezone: series.as_ref().map(|s| s.timezone.clone()),
        usage_series_oauth_usd: series.as_ref().map(|s| s.oauth_class_usd),
        usage_series_api_usd: series.as_ref().map(|s| s.api_class_usd),
        usage_series_other_usd: series.as_ref().map(|s| s.other_class_usd),
        usage_series_limit_reached: series.as_ref().map(|s| s.limit_reached),
    })
}

/// Write `doc` to `limits_snapshot.json` under `grok_home` (no flock).
pub fn write_limits_snapshot_file(
    grok_home: impl AsRef<Path>,
    doc: &LimitsSnapshotDocument,
) -> io::Result<()> {
    let home = grok_home.as_ref();
    fs::create_dir_all(home)?;
    let (snap_path, _) = snapshot_paths(home);
    write_snapshot_atomic(&snap_path, doc)
}

/// Read the snapshot file when it parses; `None` if missing or corrupt.
pub fn read_limits_snapshot_file(grok_home: impl AsRef<Path>) -> Option<LimitsSnapshotDocument> {
    let (snap_path, _) = snapshot_paths(grok_home);
    read_snapshot_at(&snap_path)
}

/// Coordinate: only the exclusive-flock holder may invoke `fetch`.
///
/// `fetch` must not include JWTs or API keys in the returned document.
pub async fn coordinate_limits_snapshot<F, Fut>(
    grok_home: impl AsRef<Path>,
    mode: LimitsSnapshotMode,
    now_unix_ms: u64,
    fetch: F,
) -> io::Result<(LimitsSnapshotRole, LimitsSnapshotDocument)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = LimitsSnapshotDocument>,
{
    let home = grok_home.as_ref();
    if shared_limits_snapshot_disabled() {
        let mut doc = fetch().await;
        if doc.fetched_at_unix_ms == 0 {
            doc.fetched_at_unix_ms = now_unix_ms;
        }
        apply_limits_snapshot(&doc);
        return Ok((LimitsSnapshotRole::UncoordinatedFetch, doc));
    }

    fs::create_dir_all(home)?;
    let (snap_path, lock_path) = snapshot_paths(home);
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;

    let waited = match lock_file.try_lock_exclusive() {
        Ok(()) => false,
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            lock_file.lock_exclusive()?;
            true
        }
        Err(e) => return Err(e),
    };

    let existing = read_snapshot_at(&snap_path);
    let fresh = existing
        .as_ref()
        .is_some_and(|d| !snapshot_is_stale(d, now_unix_ms));

    let should_fetch = match mode {
        LimitsSnapshotMode::HonorTtl => !fresh,
        LimitsSnapshotMode::ForceRefresh => {
            if waited {
                !fresh
            } else {
                true
            }
        }
    };

    let (role, doc) = if should_fetch {
        let mut doc = fetch().await;
        if doc.fetched_at_unix_ms == 0 {
            doc.fetched_at_unix_ms = now_unix_ms;
        }
        write_snapshot_atomic(&snap_path, &doc)?;
        (LimitsSnapshotRole::LeaderFetched, doc)
    } else {
        let doc = existing.expect("fresh snapshot exists when not fetching");
        (LimitsSnapshotRole::FollowerRead, doc)
    };

    apply_limits_snapshot(&doc);
    let _ = lock_file.unlock();
    Ok((role, doc))
}

fn write_snapshot_atomic(path: &Path, doc: &LimitsSnapshotDocument) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(doc)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(&data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn read_snapshot_at(path: &Path) -> Option<LimitsSnapshotDocument> {
    let mut file = File::open(path).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    if buf.trim().is_empty() {
        return None;
    }
    serde_json::from_str(buf.trim()).ok()
}

/// Serializes GROK_HOME + included-billing cache for hub tests.
#[cfg(test)]
pub(crate) struct SharedSnapshotEnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev_disable: Option<std::ffi::OsString>,
    prev_home: Option<std::ffi::OsString>,
}

#[cfg(test)]
static SNAPSHOT_TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
impl SharedSnapshotEnvGuard {
    pub(crate) fn acquire(grok_home: &Path) -> Self {
        let lock = SNAPSHOT_TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev_disable = std::env::var_os(grok_rate_limit::DISABLE_ENV);
        if prev_disable.is_some() {
            // SAFETY: exclusive via SNAPSHOT_TEST_ENV_LOCK; restored on drop.
            unsafe { std::env::remove_var(grok_rate_limit::DISABLE_ENV) };
        }
        let prev_home = std::env::var_os("GROK_HOME");
        unsafe { std::env::set_var("GROK_HOME", grok_home) };
        Self {
            _lock: lock,
            prev_disable,
            prev_home,
        }
    }
}

#[cfg(test)]
impl Drop for SharedSnapshotEnvGuard {
    fn drop(&mut self) {
        match self.prev_disable.take() {
            Some(v) => unsafe { std::env::set_var(grok_rate_limit::DISABLE_ENV, v) },
            None => unsafe { std::env::remove_var(grok_rate_limit::DISABLE_ENV) },
        }
        match self.prev_home.take() {
            Some(v) => unsafe { std::env::set_var("GROK_HOME", v) },
            None => unsafe { std::env::remove_var("GROK_HOME") },
        }
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)] // SNAPSHOT_TEST_ENV_LOCK serializes GROK_HOME.
mod tests {
    use super::*;
    use crate::auth::allowance_exhaust_from_billing::{
        clear_included_billing_cache, included_billing_fields_snapshot,
        supergrok_billing_poll_outcome,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn sample_identity(usage_pct: f64) -> LimitsSnapshotIdentity {
        LimitsSnapshotIdentity {
            identity_id: "user-personal".into(),
            usage_pct: Some(usage_pct),
            period_end: Some("2026-09-01T00:00:00Z".into()),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            extras_cents: Some(250),
            grok_build_usage_pct: Some(12.0),
            is_unified_billing_user: Some(false),
            poll_outcome: POLL_OUTCOME_OK.into(),
        }
    }

    fn sample_doc(now_ms: u64, usage_pct: f64) -> LimitsSnapshotDocument {
        let mut doc = LimitsSnapshotDocument::empty(now_ms);
        doc.identities.push(sample_identity(usage_pct));
        doc
    }

    #[tokio::test]
    async fn limits_snapshot_second_process_reads_file_and_does_not_http() {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        let http = Arc::new(AtomicU32::new(0));

        let http1 = Arc::clone(&http);
        let (role1, _) =
            coordinate_limits_snapshot(home, LimitsSnapshotMode::HonorTtl, now, || {
                let http1 = Arc::clone(&http1);
                async move {
                    http1.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now, 24.0)
                }
            })
            .await
            .expect("first coordinate");
        assert_eq!(role1, LimitsSnapshotRole::LeaderFetched);
        assert_eq!(http.load(Ordering::SeqCst), 1);

        clear_included_billing_cache();
        assert!(included_billing_fields_snapshot().is_empty());

        let http2 = Arc::clone(&http);
        let (role2, doc2) = coordinate_limits_snapshot(
            home,
            LimitsSnapshotMode::HonorTtl,
            now.saturating_add(1_000),
            || {
                let http2 = Arc::clone(&http2);
                async move {
                    http2.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now, 99.0)
                }
            },
        )
        .await
        .expect("second coordinate");
        assert_eq!(
            role2,
            LimitsSnapshotRole::FollowerRead,
            "second process must read the flock snapshot, not HTTP"
        );
        assert_eq!(
            http.load(Ordering::SeqCst),
            1,
            "second process must not call SuperGrok billing HTTP"
        );
        assert_eq!(doc2.identities[0].usage_pct, Some(24.0));
        let remembered = included_billing_fields_snapshot();
        let fields = remembered
            .get("user-personal")
            .expect("follower applies remember maps");
        assert_eq!(fields.usage_pct, Some(24.0));
        assert_eq!(fields.prepaid_balance_cents, Some(250));
        assert!(supergrok_billing_poll_outcome("user-personal").is_ok());
    }

    #[tokio::test]
    async fn limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once() {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        let stale_at = now.saturating_sub((SNAPSHOT_TTL_SECS + 5) * 1000);
        write_limits_snapshot_file(home, &sample_doc(stale_at, 10.0)).expect("seed stale snapshot");

        let http = Arc::new(AtomicU32::new(0));
        let http1 = Arc::clone(&http);
        let (role, doc) =
            coordinate_limits_snapshot(home, LimitsSnapshotMode::HonorTtl, now, || {
                let http1 = Arc::clone(&http1);
                async move {
                    http1.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now, 41.0)
                }
            })
            .await
            .expect("stale waiter coordinate");
        assert_eq!(role, LimitsSnapshotRole::LeaderFetched);
        assert_eq!(http.load(Ordering::SeqCst), 1);
        assert_eq!(doc.identities[0].usage_pct, Some(41.0));
        let on_disk = read_limits_snapshot_file(home).expect("leader rewrote snapshot");
        assert_eq!(on_disk.identities[0].usage_pct, Some(41.0));
        assert!(!snapshot_is_stale(&on_disk, now));
    }

    #[tokio::test]
    async fn limits_snapshot_never_writes_access_tokens() {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        // Realistic JWT shape. Must never appear on disk.
        let leaked = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.e30.signature-not-a-real-token";
        let _ = leaked;
        let (_role, _doc) =
            coordinate_limits_snapshot(home, LimitsSnapshotMode::HonorTtl, now, || async move {
                sample_doc(now, 7.0)
            })
            .await
            .expect("coordinate");
        let (snap_path, _) = snapshot_paths(home);
        let bytes = fs::read_to_string(&snap_path).expect("leader must write limits_snapshot.json");
        assert!(
            !snapshot_json_contains_secrets(&bytes, &[leaked]),
            "snapshot must not store JWTs or access_token keys: {bytes}"
        );
        assert!(
            !bytes.contains("eyJ"),
            "snapshot must not contain JWT header prefixes: {bytes}"
        );
    }
}
