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
//! reset, SuperGrok dollar credits, and poll outcome class. Never JWTs
//! or API keys.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::allowance_exhaust_from_billing::{
    remember_supergrok_billing_poll_failed, remember_supergrok_billing_poll_ok,
    remember_supergrok_build_usage, remember_supergrok_dollar_credits,
    remember_supergrok_included_billing,
};
use super::included_poll_history::record_included_poll_now;

/// Snapshot JSON under `$GROK_HOME`.
pub const SNAPSHOT_FILE_NAME: &str = "limits_snapshot.json";

/// Exclusive flock file under `$GROK_HOME` (not the JSON itself).
pub const LOCK_FILE_NAME: &str = "limits_snapshot.lock";

/// Alias of [`LOCK_FILE_NAME`] for callers that want the snapshot-prefixed name.
pub const SNAPSHOT_LOCK_FILE_NAME: &str = LOCK_FILE_NAME;

/// Shared snapshot freshness window for HonorTtl automatic/background checks.
///
/// One hour per machine: SuperGrok credits and Management credits APIs are
/// checked at most once an hour unless the operator ForceRefresh (`/limits`,
/// `/limits refresh`, `grok-oss limits`, `grok-oss limits refresh`). This is
/// not the Management in-process paint cache
/// ([`super::xai_management::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`], 60s).
pub const SNAPSHOT_TTL_SECS: u64 = 3600;

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
    /// Background TUI poll: reuse a snapshot younger than [`SNAPSHOT_TTL`]
    /// (one hour). Does not clear Management process caches.
    HonorTtl,
    /// Explicit `grok-oss limits` / `/limits`: fetch if this process holds
    /// exclusive flock without waiting, even when the snapshot is younger
    /// than one hour. After waiting on a leader, reuse the just-written
    /// snapshot unless it is still missing or stale.
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
    pub dollar_credits_cents: Option<i64>,
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
    /// Billing Credits card from GetAmountToPay remaining. Never filled from
    /// [`Self::prepaid_cents`].
    #[serde(default)]
    pub billing_credits_card: xai_grok_sampling_types::BillingCreditsCard,
    /// Remaining USD cents when [`Self::billing_credits_card`] is `fetched`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub billing_credits_cents: Option<i64>,
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
            record_included_poll_now(
                identity,
                pct,
                id.grok_build_usage_pct,
                id.dollar_credits_cents,
            );
        }
        if let Some(cents) = id.dollar_credits_cents {
            remember_supergrok_dollar_credits(identity, cents);
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
            billing_credits_remaining_cents: mgmt.billing_credits_cents,
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

/// Whether `fetch_management_into_snapshot` must also call Management
/// `GET /v1/billing/teams/{team_id}/prepaid/balance`.
///
/// True only when the inference-key prepaid read returned no cents and a
/// management key plus team id exist. A successful inference balance is not
/// replaced. No inference key uses the Management read as the only prepaid
/// read, which is not this also-call.
pub fn should_also_call_management_prepaid_balance(
    inference_key_present: bool,
    inference_returned_meter: bool,
    management_key_present: bool,
    management_team_id_present: bool,
) -> bool {
    inference_key_present
        && !inference_returned_meter
        && management_key_present
        && management_team_id_present
}

/// Team prepaid remaining after the inference-key read and the Management
/// `prepaid/balance` `total.val` read.
///
/// A successful inference meter wins. An empty inference read keeps the
/// Management meter when that read returned cents. Both empty stays `None`
/// (team prepaid unavailable). Neither meter is the Billing Credits card.
pub fn select_team_prepaid_meter(
    inference: Option<super::xai_management::ConsoleTeamPrepaidMeter>,
    management: Option<super::xai_management::ConsoleTeamPrepaidMeter>,
) -> Option<super::xai_management::ConsoleTeamPrepaidMeter> {
    if inference.is_some() {
        inference
    } else {
        management
    }
}

/// Fetch Management prepaid / postpaid / series into snapshot fields (no keys).
///
/// Returns `None` when neither a management key nor a console inference key
/// can supply a meter. Console API credits use the inference key on
/// `api.x.ai` and do not call `management-api.x.ai`. Postpaid and the usage
/// series still use the management key. Call only from a
/// hub leader fetch callback so followers do not stampede the Management API.
/// Automatic HonorTtl leader HTTP is at most once an hour per machine
/// ([`SNAPSHOT_TTL_SECS`]). ForceRefresh still fetches. Process Mutex 60s
/// caches may paint in-process; they must not cause extra leader HTTP under
/// HonorTtl.
pub async fn fetch_management_into_snapshot() -> Option<LimitsSnapshotManagement> {
    use super::xai_management::{
        USAGE_SERIES_DEFAULT_DAY_WINDOW, fetch_console_api_credits_with_inference_key,
        fetch_console_team_postpaid_preview_default, fetch_console_team_prepaid_balance_default,
        fetch_console_team_usage_series_default, resolve_management_api_key_default,
        resolve_management_team_id_default,
    };
    let inference_key_present = super::xai_console::console_inference_key_present_default();
    let inference_prepaid = if inference_key_present {
        fetch_console_api_credits_with_inference_key().await
    } else {
        None
    };
    let management_key = resolve_management_api_key_default();
    if management_key.is_none() && !inference_key_present {
        return None;
    }
    let management_team_present = resolve_management_team_id_default().is_some();
    // No inference key: Management prepaid/balance is the only prepaid read.
    // Inference key with no cents, plus a management key and team id: also
    // call GET /v1/billing/teams/{team_id}/prepaid/balance. Do not call that
    // route to replace a successful inference balance.
    let management_prepaid = if !inference_key_present
        || should_also_call_management_prepaid_balance(
            inference_key_present,
            inference_prepaid.is_some(),
            management_key.is_some(),
            management_team_present,
        ) {
        fetch_console_team_prepaid_balance_default().await
    } else {
        None
    };
    let prepaid = select_team_prepaid_meter(inference_prepaid, management_prepaid);
    let postpaid = if management_key.is_some() {
        fetch_console_team_postpaid_preview_default().await
    } else {
        None
    };
    let series = if management_key.is_some() {
        fetch_console_team_usage_series_default(USAGE_SERIES_DEFAULT_DAY_WINDOW).await
    } else {
        None
    };
    let team_id = prepaid
        .as_ref()
        .map(|m| m.team_id.clone())
        .or_else(|| postpaid.as_ref().map(|m| m.team_id.clone()))
        .or_else(|| series.as_ref().map(|m| m.team_id.clone()));
    if team_id.is_none() && prepaid.is_none() && postpaid.is_none() && series.is_none() {
        return None;
    }
    let remaining = postpaid
        .as_ref()
        .and_then(|m| m.billing_credits_remaining_cents);
    Some(LimitsSnapshotManagement {
        team_id,
        prepaid_cents: prepaid.as_ref().map(|m| m.balance_cents),
        billing_credits_card: match remaining {
            Some(_) => xai_grok_sampling_types::BillingCreditsCard::Fetched,
            None if postpaid.is_none() => xai_grok_sampling_types::BillingCreditsCard::Error,
            None => xai_grok_sampling_types::BillingCreditsCard::NotFetched,
        },
        billing_credits_cents: remaining,
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

struct LiveAskSocket {
    path: PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    doc: std::sync::Arc<std::sync::Mutex<LimitsSnapshotDocument>>,
}

fn live_ask_socket() -> &'static std::sync::Mutex<Option<LiveAskSocket>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<LiveAskSocket>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Publish the last details on `$GROK_HOME/limits_billing_ask.sock`.
///
/// Not `leader.sock`. The listener stays up after this call returns so another
/// process can ask. Answering does not call the billing API.
fn publish_limits_billing_ask_socket(home: &Path, doc: &LimitsSnapshotDocument) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::atomic::Ordering;

    let path = home.join("limits_billing_ask.sock");
    let mut slot = live_ask_socket()
        .lock()
        .map_err(|err| io::Error::other(err.to_string()))?;
    if let Some(live) = slot.as_ref() {
        if live.path == path {
            *live
                .doc
                .lock()
                .map_err(|err| io::Error::other(err.to_string()))? = doc.clone();
            return Ok(());
        }
        live.stop.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(&live.path);
    }
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(bind_err) => {
            // A path that does not accept is a leftover file, not a live leader.
            if UnixStream::connect(&path).is_ok() {
                return Err(bind_err);
            }
            let _ = std::fs::remove_file(&path);
            UnixListener::bind(&path)?
        }
    };
    listener.set_nonblocking(true)?;
    let doc_slot = std::sync::Arc::new(std::sync::Mutex::new(doc.clone()));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let doc_thread = std::sync::Arc::clone(&doc_slot);
    let stop_thread = std::sync::Arc::clone(&stop);
    let thread_path = path.clone();
    std::thread::spawn(move || {
        loop {
            if stop_thread.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    if let Ok(current) = doc_thread.lock() {
                        if let Ok(bytes) = serde_json::to_vec(&*current) {
                            let _ = stream.write_all(&bytes);
                        }
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
        let _ = std::fs::remove_file(&thread_path);
    });
    *slot = Some(LiveAskSocket {
        path,
        stop,
        doc: doc_slot,
    });
    Ok(())
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
            let inside_minute = existing.as_ref().is_some_and(|doc| {
                doc.fetched_at_unix_ms > 0
                    && now_unix_ms.saturating_sub(doc.fetched_at_unix_ms) < 60_000
            });
            // A disk snapshot at 100% is not a successful call. Explicit
            // refresh must still fetch so that field is not applied.
            let disk_usage_at_100 = existing.as_ref().is_some_and(|doc| {
                doc.identities
                    .iter()
                    .any(|identity| identity.usage_pct == Some(100.0))
            });
            if waited {
                !fresh
            } else {
                !inside_minute || disk_usage_at_100
            }
        }
    };

    let (role, doc) = if should_fetch {
        let mut doc = fetch().await;
        if doc.fetched_at_unix_ms == 0 {
            doc.fetched_at_unix_ms = now_unix_ms;
        }
        write_snapshot_atomic(&snap_path, &doc)?;
        publish_limits_billing_ask_socket(home, &doc)?;
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
            dollar_credits_cents: Some(250),
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

    /// Ninety seconds is older than the Management process cache (60s) and
    /// younger than the shared snapshot hour. HonorTtl must not HTTP.
    const WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS: u64 = 90_000;

    #[test]
    fn snapshot_ttl_secs_is_one_hour_not_management_process_cache() {
        assert_eq!(
            SNAPSHOT_TTL_SECS, 3600,
            "HonorTtl automatic SuperGrok credits and Management credits checks are at most once an hour per machine"
        );
        assert_eq!(
            crate::auth::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS,
            60,
            "Management process cache stays 60s as an in-process paint helper; it is not the shared hourly fetch"
        );
        assert_ne!(
            SNAPSHOT_TTL_SECS,
            crate::auth::CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS
        );
    }

    /// Empty inference read plus Management `total.val` is team prepaid.
    /// A successful inference balance is not replaced. Both empty stays unset.
    /// `total.val` is not copied onto the Billing Credits card by this choice.
    #[test]
    fn empty_inference_read_plus_management_total_val_is_team_prepaid_not_the_card() {
        use crate::auth::{
            PrepaidBalanceResponse, UsdCentsVal, console_team_prepaid_from_response,
        };

        assert!(should_also_call_management_prepaid_balance(
            true, false, true, true
        ));
        assert!(!should_also_call_management_prepaid_balance(
            true, true, true, true
        ));
        assert!(!should_also_call_management_prepaid_balance(
            true, false, true, false
        ));
        assert!(!should_also_call_management_prepaid_balance(
            true, false, false, true
        ));

        let management = console_team_prepaid_from_response(
            "team-1",
            &PrepaidBalanceResponse {
                total: UsdCentsVal {
                    val: "-11245".into(),
                },
                changes: vec![],
            },
        )
        .expect("total.val");
        assert_eq!(management.balance_cents, 11_245);
        let chosen = select_team_prepaid_meter(None, Some(management.clone())).expect("prepaid");
        assert_eq!(chosen.balance_cents, 11_245);
        let kept = select_team_prepaid_meter(
            Some(crate::auth::ConsoleTeamPrepaidMeter {
                team_id: "team-1".into(),
                balance_cents: 5_000,
            }),
            Some(management),
        )
        .expect("inference wins");
        assert_eq!(kept.balance_cents, 5_000);
        assert!(select_team_prepaid_meter(None, None).is_none());
        assert_eq!(
            crate::auth::billing_credits_card_from_prepaid_total("-11245"),
            xai_grok_sampling_types::BillingCreditsCard::NotFetched,
            "prepaid total.val must not become the Billing Credits card"
        );
    }

    #[test]
    fn snapshot_younger_than_one_hour_is_not_stale_including_ninety_seconds() {
        let now: u64 = 1_700_000_000_000;
        let ninety = sample_doc(
            now.saturating_sub(WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS),
            10.0,
        );
        assert!(
            !snapshot_is_stale(&ninety, now),
            "90s is within the hour; a 60s shared TTL would wrongly treat this as stale"
        );
        let almost_hour = sample_doc(now.saturating_sub((SNAPSHOT_TTL_SECS - 1) * 1000), 10.0);
        assert!(!snapshot_is_stale(&almost_hour, now));
    }

    #[test]
    fn snapshot_at_one_hour_is_stale() {
        let now: u64 = 1_700_000_000_000;
        let hour = sample_doc(now.saturating_sub(SNAPSHOT_TTL_SECS * 1000), 10.0);
        assert!(
            snapshot_is_stale(&hour, now),
            "after 3600s a waiter or leader may fetch once"
        );
    }

    /// Second grok-oss process within the hour does not hit the SuperGrok
    /// credits API or Management credits APIs.
    #[tokio::test]
    // Grok OSS: a second process within the hour reads limits_snapshot.json and does not HTTP SuperGrok credits or Management credits APIs. This diverges from upstream xAI because automatic limits fetch is at most once an hour per machine through the snapshot hub.
    async fn limits_snapshot_second_process_within_the_hour_does_not_http() {
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
            now.saturating_add(WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS),
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
            "second process within the hour must read the flock snapshot, not HTTP"
        );
        assert_eq!(
            http.load(Ordering::SeqCst),
            1,
            "second process must not call SuperGrok credits or Management credits APIs"
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

    /// Second session asks the first over a billing socket. It does not call the API.
    #[tokio::test]
    async fn second_session_asks_the_first_over_ipc_and_does_not_call_the_api() {
        if std::env::var("GROK_LIMITS_ASK_CHILD").ok().as_deref() == Some("1") {
            let home = std::path::PathBuf::from(std::env::var("GROK_HOME").expect("GROK_HOME"));
            let sock = home.join("limits_billing_ask.sock");
            assert_ne!(
                sock.file_name().and_then(|name| name.to_str()),
                Some("leader.sock"),
                "ask socket is a new path under GROK_HOME, not leader.sock"
            );
            let mut stream = std::os::unix::net::UnixStream::connect(&sock).expect(
                "second session asks the first over limits_billing_ask.sock; the hub has no ask socket",
            );
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .ok();
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut stream, &mut buf)
                .expect("details from the ask socket");
            assert!(
                buf.contains("user-personal"),
                "second process got details by asking, not by calling fetch_credits_config_with_session: {buf}",
            );
            let hits = std::fs::read_to_string(home.join("http_hits")).unwrap_or_default();
            assert_eq!(hits.trim(), "1", "HTTP counter stays at 1");
            return;
        }

        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        let http = Arc::new(AtomicU32::new(0));
        let hits_path = home.join("http_hits");
        let http1 = Arc::clone(&http);
        let hits_for_fetch = hits_path.clone();
        let (role1, _) =
            coordinate_limits_snapshot(home, LimitsSnapshotMode::HonorTtl, now, || {
                let http1 = Arc::clone(&http1);
                let hits_for_fetch = hits_for_fetch.clone();
                async move {
                    let n = http1.fetch_add(1, Ordering::SeqCst) + 1;
                    std::fs::write(&hits_for_fetch, n.to_string()).expect("http counter");
                    sample_doc(now, 24.0)
                }
            })
            .await
            .expect("first process");
        assert_eq!(role1, LimitsSnapshotRole::LeaderFetched);
        assert_eq!(http.load(Ordering::SeqCst), 1, "HTTP counter stays at 1");

        let sock = home.join("limits_billing_ask.sock");
        assert_ne!(
            sock.file_name().and_then(|name| name.to_str()),
            Some("leader.sock")
        );
        let child = std::process::Command::new(std::env::current_exe().expect("test bin"))
            .env("GROK_HOME", home)
            .env("GROK_LIMITS_ASK_CHILD", "1")
            .env("GROK_SKIP_EDIT_VERIFY", "1")
            .args([
                "--exact",
                "auth::limits_snapshot_hub::tests::second_session_asks_the_first_over_ipc_and_does_not_call_the_api",
                "--test-threads",
                "1",
            ])
            .output()
            .expect("second process");
        assert!(
            child.status.success(),
            "second process asks the first over the socket and does not call fetch_credits_config_with_session\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr),
        );
        let hits = std::fs::read_to_string(&hits_path).unwrap_or_default();
        assert_eq!(hits.trim(), "1", "HTTP counter stays at 1");
    }

    #[tokio::test]
    async fn forced_refresh_inside_one_minute_does_not_call_the_api_again() {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        let http = Arc::new(AtomicU32::new(0));
        let http1 = Arc::clone(&http);
        let (_role1, doc1) =
            coordinate_limits_snapshot(home, LimitsSnapshotMode::ForceRefresh, now, || {
                let http1 = Arc::clone(&http1);
                async move {
                    http1.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now, 24.0)
                }
            })
            .await
            .expect("first force refresh");
        assert_eq!(http.load(Ordering::SeqCst), 1);
        assert_eq!(doc1.identities[0].usage_pct, Some(24.0));

        let http2 = Arc::clone(&http);
        let (_role2, doc2) = coordinate_limits_snapshot(
            home,
            LimitsSnapshotMode::ForceRefresh,
            now.saturating_add(30_000),
            || {
                let http2 = Arc::clone(&http2);
                async move {
                    http2.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now.saturating_add(30_000), 99.0)
                }
            },
        )
        .await
        .expect("force refresh inside one minute");
        assert_eq!(
            http.load(Ordering::SeqCst),
            1,
            "ForceRefresh less than a minute after a successful call must not call the API again"
        );
        assert_eq!(doc2.identities[0].usage_pct, Some(24.0));
    }

    #[tokio::test]
    async fn when_the_first_session_exits_exactly_one_successor_calls_the_api() {
        if std::env::var("GROK_LIMITS_SUCCESSOR_CHILD").ok().as_deref() == Some("1") {
            let home = std::path::PathBuf::from(std::env::var("GROK_HOME").expect("GROK_HOME"));
            let hits_path = home.join("http_hits");
            let asked_path = home.join("asked.ok");
            let now = now_unix_ms();
            let hits_for_fetch = hits_path.clone();
            let (role, doc) =
                coordinate_limits_snapshot(&home, LimitsSnapshotMode::ForceRefresh, now, || {
                    let hits_for_fetch = hits_for_fetch.clone();
                    async move {
                        let prev = std::fs::read_to_string(&hits_for_fetch).unwrap_or_default();
                        let n = prev.trim().parse::<u32>().unwrap_or(0).saturating_add(1);
                        std::fs::write(&hits_for_fetch, n.to_string()).expect("http counter");
                        sample_doc(now, 24.0)
                    }
                })
                .await
                .expect("contender");
            let after = std::fs::read_to_string(&hits_path).unwrap_or_default();
            if role == LimitsSnapshotRole::LeaderFetched {
                for _ in 0..100 {
                    if asked_path.exists() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                panic!("third process did not ask the successor socket");
            }
            assert_ne!(role, LimitsSnapshotRole::LeaderFetched);
            assert_eq!(doc.identities[0].usage_pct, Some(24.0));
            let mut stream =
                std::os::unix::net::UnixStream::connect(home.join("limits_billing_ask.sock"))
                    .expect("stale socket file is not a live leader; ask the successor");
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut stream, &mut buf).expect("ask reply");
            assert!(
                buf.contains("user-personal"),
                "third process got details by asking, not by a second API call: {buf}"
            );
            std::fs::write(&asked_path, b"ok").expect("asked");
            assert_eq!(after.trim(), "2", "HTTP counter increases by at most one");
            return;
        }

        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        let stale_at = now.saturating_sub(3_600_000 + 5_000);
        write_limits_snapshot_file(home, &sample_doc(stale_at, 24.0)).expect("stale snapshot");
        std::fs::write(home.join("limits_billing_ask.sock"), b"stale").expect("stale socket file");
        std::fs::write(home.join("http_hits"), b"1").expect("first already called");

        fn spawn_contender(home_owned: &std::path::Path) -> std::process::Output {
            std::process::Command::new(std::env::current_exe().expect("test bin"))
                .env("GROK_HOME", home_owned)
                .env("GROK_LIMITS_SUCCESSOR_CHILD", "1")
                .env("GROK_SKIP_EDIT_VERIFY", "1")
                .args([
                    "--exact",
                    "auth::limits_snapshot_hub::tests::when_the_first_session_exits_exactly_one_successor_calls_the_api",
                    "--test-threads",
                    "1",
                ])
                .output()
                .expect("contender process")
        }
        let left_home = home.to_path_buf();
        let right_home = left_home.clone();
        let left = std::thread::spawn(move || spawn_contender(&left_home));
        let right = std::thread::spawn(move || spawn_contender(&right_home));
        let left = left.join().expect("left");
        let right = right.join().expect("right");
        assert!(
            left.status.success(),
            "left failed\n{}\n{}",
            String::from_utf8_lossy(&left.stdout),
            String::from_utf8_lossy(&left.stderr)
        );
        assert!(
            right.status.success(),
            "right failed\n{}\n{}",
            String::from_utf8_lossy(&right.stdout),
            String::from_utf8_lossy(&right.stderr)
        );
        let hits = std::fs::read_to_string(home.join("http_hits")).unwrap_or_default();
        assert_eq!(hits.trim(), "2", "exactly one successor calls the API");
    }

    /// HonorTtl with a snapshot younger than one hour does not HTTP.
    #[tokio::test]
    // Grok OSS: HonorTtl with a snapshot younger than one hour does not HTTP SuperGrok credits or Management credits APIs. Hourly flock: HonorTtl once an hour; ForceRefresh still fetches; never write access tokens.
    async fn limits_snapshot_honor_ttl_fresh_within_hour_does_not_http() {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        write_limits_snapshot_file(home, &sample_doc(now, 18.0))
            .expect("seed within-hour snapshot");
        let http = Arc::new(AtomicU32::new(0));
        let http1 = Arc::clone(&http);
        let (role, doc) = coordinate_limits_snapshot(
            home,
            LimitsSnapshotMode::HonorTtl,
            now.saturating_add(WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS),
            || {
                let http1 = Arc::clone(&http1);
                async move {
                    http1.fetch_add(1, Ordering::SeqCst);
                    sample_doc(now, 90.0)
                }
            },
        )
        .await
        .expect("honor ttl within hour");
        assert_eq!(role, LimitsSnapshotRole::FollowerRead);
        assert_eq!(
            http.load(Ordering::SeqCst),
            0,
            "HonorTtl must not HTTP SuperGrok credits or Management credits APIs while the snapshot is younger than one hour"
        );
        assert_eq!(doc.identities[0].usage_pct, Some(18.0));
    }

    /// ForceRefresh leader HTTP-fetches even when the snapshot file is
    /// younger than one hour (explicit `/limits` / `grok-oss limits`).
    #[tokio::test]
    // Grok OSS: ForceRefresh leader HTTP-fetches even when the snapshot is younger than one hour. This diverges from upstream xAI because explicit /limits still fetches while HonorTtl stays hourly.
    async fn limits_snapshot_force_refresh_leader_http_fetches_when_snapshot_is_younger_than_one_hour()
     {
        let tmp = tempfile::TempDir::new().expect("temp home");
        let home = tmp.path();
        let _env = SharedSnapshotEnvGuard::acquire(home);
        clear_included_billing_cache();
        let now = now_unix_ms();
        write_limits_snapshot_file(home, &sample_doc(now, 100.0))
            .expect("seed within-hour snapshot");
        let http = Arc::new(AtomicU32::new(0));
        let http1 = Arc::clone(&http);
        let (role, doc) = coordinate_limits_snapshot(
            home,
            LimitsSnapshotMode::ForceRefresh,
            now.saturating_add(WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS),
            || {
                let http1 = Arc::clone(&http1);
                async move {
                    http1.fetch_add(1, Ordering::SeqCst);
                    sample_doc(
                        now.saturating_add(WITHIN_HOUR_BUT_PAST_PROCESS_CACHE_MS),
                        41.0,
                    )
                }
            },
        )
        .await
        .expect("force refresh within hour");
        assert_eq!(role, LimitsSnapshotRole::LeaderFetched);
        assert_eq!(
            http.load(Ordering::SeqCst),
            1,
            "ForceRefresh must fetch even when the snapshot is younger than one hour"
        );
        assert_eq!(doc.identities[0].usage_pct, Some(41.0));
    }

    /// After the hour, a waiter/leader may fetch once. Stale means 3600s, not 60s.
    #[tokio::test]
    // Grok OSS: after the hour a waiter or leader may fetch once; stale means 3600s, not 60s. This diverges from upstream xAI because the snapshot hub uses a one-hour flock, not a per-process cache.
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
    // Grok OSS: the shared snapshot never stores JWTs or API keys. This diverges from upstream xAI because Grok OSS writes a machine-wide limits_snapshot.json that must not hold access tokens.
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
