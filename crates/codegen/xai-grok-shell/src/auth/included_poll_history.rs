//! SuperGrok included billing poll history (S1 credits), process + durable.
//!
//! Each successful `GET …/billing?format=credits` appends a sample per
//! SuperGrok `identity_id`. A pure detector marks **included debit unproven**
//! when included %, SuperGrok $ extras, and optional Grok Build product % stay
//! flat across enough polls over a minimum wall-clock window.
//!
//! **Limits before credits:** this module only observes SuperGrok session
//! meters. It does not hop to console to "fix" Usage $ and does not change
//! Design A (console ApiKey omitted while included has headroom).
//!
//! ## Storage
//!
//! - **Process ring** for fast same-process reads.
//! - **Durable** under `$GROK_HOME/included_poll_history/{identity}.json` with
//!   exclusive flock (same spirit as `rate_limits/` and `exhausted_credits/`).
//!   Cold CLI processes share the series so `flat_poll` honesty can fire across
//!   sequential multipolls. Never stores tokens or secrets — only meter
//!   samples (timestamp, usage %, optional Build %, optional extras cents).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// One successful S1 sample for one SuperGrok principal.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludedPollSample {
    /// Wall time when the poll completed (UTC).
    pub ts: DateTime<Utc>,
    /// Top-level `creditUsagePercent` from credits config.
    pub credit_usage_percent: f64,
    /// `productUsage` entry for `PRODUCT_GROK_BUILD` when present.
    pub build_usage_percent: Option<f64>,
    /// SuperGrok Extra Usage Credits (`prepaidBalance.val`) when present.
    pub prepaid_balance_cents: Option<i64>,
}

/// Default minimum successful polls before flat-poll evidence can fire.
pub const DEFAULT_MIN_POLLS: usize = 2;

/// Default minimum span between first and last sample in the detector window.
pub const DEFAULT_MIN_WINDOW: Duration = Duration::from_secs(30);

/// Cap ring length per identity (oldest dropped).
const RING_CAP: usize = 32;

/// Subdir under `$GROK_HOME` (mirrors `rate_limits/` / `exhausted_credits/`).
pub const DURABLE_SUBDIR: &str = "included_poll_history";

/// Process-local ring buffers keyed by SuperGrok `identity_id`.
static POLL_HISTORY_BY_IDENTITY: Mutex<BTreeMap<String, VecDeque<IncludedPollSample>>> =
    Mutex::new(BTreeMap::new());

// ── Pure detector (unchanged contracts) ─────────────────────────────────────

/// Pure detector: included debit unproven when SuperGrok meters stay flat.
///
/// Returns `true` only when:
/// - at least `min_polls` samples,
/// - the last `min_polls` samples span at least `min_window` wall time,
/// - across that window: `credit_usage_percent` unchanged, and when both
///   samples carry SuperGrok $ extras or Build product %, those are unchanged
///   too.
///
/// Any step in included %, Build product %, or extras cents clears flat
/// (returns `false`). Not enough polls or too-short window → `false`
/// (no invented evidence).
/// Select the most recent poll suffix that spans at least `min_window` and has
/// at least `min_polls` samples. Returns `None` when evidence is too thin.
///
/// High-frequency multipoll (many samples a few seconds apart) must still fire
/// once the series covers `min_window`. Looking only at the last `min_polls`
/// points fails when those two points are 2s apart under load, even if free
/// SuperGrok period % has been flat for minutes. Named C4 measurement contract:
/// flat free-period series under load is ticket evidence, not invent debit.
pub fn recent_flat_candidate_window(
    samples: &[IncludedPollSample],
    min_polls: usize,
    min_window: Duration,
) -> Option<&[IncludedPollSample]> {
    if min_polls < 2 || samples.len() < min_polls {
        return None;
    }
    let last_idx = samples.len() - 1;
    let last = &samples[last_idx];
    // Walk newest→oldest until the span from that sample to the last covers
    // min_window. That is the tightest recent window with enough wall time.
    let mut start_idx = None;
    for i in (0..last_idx).rev() {
        let span = (last.ts - samples[i].ts).to_std().unwrap_or(Duration::ZERO);
        if span >= min_window {
            start_idx = Some(i);
            break;
        }
    }
    let start_idx = start_idx?;
    let window = &samples[start_idx..=last_idx];
    if window.len() < min_polls {
        return None;
    }
    Some(window)
}

/// True when SuperGrok included % (and any observed Build / extras) stayed flat
/// across a recent multi-poll window of at least `min_window`.
pub fn included_debit_unproven(
    samples: &[IncludedPollSample],
    min_polls: usize,
    min_window: Duration,
) -> bool {
    let Some(window) = recent_flat_candidate_window(samples, min_polls, min_window) else {
        return false;
    };
    let first = &window[0];
    for s in &window[1..] {
        if !float_same(s.credit_usage_percent, first.credit_usage_percent) {
            return false;
        }
        if optional_f64_stepped(first.build_usage_percent, s.build_usage_percent) {
            return false;
        }
        if optional_i64_stepped(first.prepaid_balance_cents, s.prepaid_balance_cents) {
            return false;
        }
    }
    true
}

fn float_same(a: f64, b: f64) -> bool {
    // Billing % is JSON f64; exact match is enough for step detection (server
    // values are coarse enough that real debit moves the printed figure).
    a == b
}

fn optional_f64_stepped(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => !float_same(x, y),
        _ => false,
    }
}

fn optional_i64_stepped(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x != y,
        _ => false,
    }
}

// ── Durable file format (no tokens) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurableSample {
    /// UTC wall time as unix milliseconds.
    ts_unix_ms: i64,
    credit_usage_percent: f64,
    #[serde(default)]
    build_usage_percent: Option<f64>,
    #[serde(default)]
    prepaid_balance_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurableFile {
    /// Original SuperGrok `identity_id` (filename is sanitized).
    identity_id: String,
    samples: Vec<DurableSample>,
}

fn sample_to_durable(s: &IncludedPollSample) -> DurableSample {
    DurableSample {
        ts_unix_ms: s.ts.timestamp_millis(),
        credit_usage_percent: s.credit_usage_percent,
        build_usage_percent: s.build_usage_percent,
        prepaid_balance_cents: s.prepaid_balance_cents,
    }
}

fn sample_from_durable(d: &DurableSample) -> IncludedPollSample {
    let ts = Utc
        .timestamp_millis_opt(d.ts_unix_ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("epoch"));
    IncludedPollSample {
        ts,
        credit_usage_percent: d.credit_usage_percent,
        build_usage_percent: d.build_usage_percent,
        prepaid_balance_cents: d.prepaid_balance_cents,
    }
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

/// Sanitize `identity_id` for a filename (no secrets; ids are labels).
fn safe_identity_filename(identity_id: &str) -> String {
    let safe: String = identity_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "unknown".into()
    } else {
        // Keep paths short if a long opaque id ever lands here.
        if safe.len() > 120 {
            safe.chars().take(120).collect()
        } else {
            safe
        }
    }
}

fn durable_path_for(identity_id: &str) -> PathBuf {
    durable_dir().join(format!("{}.json", safe_identity_filename(identity_id)))
}

// ── Flock-backed disk ring ──────────────────────────────────────────────────

/// Openable store root for multi-process tests (same dir, separate handles).
///
/// Product free functions use [`process_default_root`] (`$GROK_HOME/...`).
#[derive(Debug, Clone)]
pub struct IncludedPollHistoryStore {
    root: PathBuf,
}

impl IncludedPollHistoryStore {
    /// Store under `grok_home/included_poll_history`.
    pub fn open(grok_home: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = grok_home.as_ref().join(DURABLE_SUBDIR);
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path_for(&self, identity_id: &str) -> PathBuf {
        self.root
            .join(format!("{}.json", safe_identity_filename(identity_id)))
    }

    /// Append a sample for one identity (flock merge + cap). No process map.
    pub fn record(&self, identity_id: &str, sample: IncludedPollSample) -> std::io::Result<()> {
        let id = identity_id.trim();
        if id.is_empty() {
            return Ok(());
        }
        let path = self.path_for(id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock_exclusive()?;
        let mut ring = read_ring_from_file(&mut file, id);
        if let Some(prev) = ring.back() {
            log_poll_delta_if_stepped(id, prev, &sample);
        }
        ring.push_back(sample);
        while ring.len() > RING_CAP {
            ring.pop_front();
        }
        write_ring_to_file(&mut file, id, &ring)?;
        let _ = file.unlock();
        Ok(())
    }

    /// Load ring for one identity (oldest → newest). Empty if never polled.
    pub fn history_for(&self, identity_id: &str) -> Vec<IncludedPollSample> {
        let id = identity_id.trim();
        if id.is_empty() {
            return Vec::new();
        }
        let path = self.path_for(id);
        // VecDeque → Vec for callers that index/slice samples.
        load_ring_at_path(&path, id)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Flat-poll evidence across all identity files under this store.
    pub fn flat_evidence(&self, min_polls: usize, min_window: Duration) -> FlatPollEvidence {
        for id in list_identity_ids_in_dir(&self.root) {
            let samples = self.history_for(&id);
            let ev = flat_poll_evidence_for_samples(&samples, min_polls, min_window);
            if ev.unproven {
                return ev;
            }
        }
        FlatPollEvidence::default()
    }
}

fn process_default_root() -> PathBuf {
    durable_dir()
}

fn read_ring_from_file(file: &mut File, identity_id: &str) -> VecDeque<IncludedPollSample> {
    file.seek(SeekFrom::Start(0)).ok();
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
        return VecDeque::new();
    }
    match serde_json::from_str::<DurableFile>(buf.trim()) {
        Ok(df) => {
            // Prefer file's identity_id when present; caller passes expected id.
            let _ = df.identity_id;
            let _ = identity_id;
            df.samples.iter().map(sample_from_durable).collect()
        }
        Err(_) => VecDeque::new(),
    }
}

fn write_ring_to_file(
    file: &mut File,
    identity_id: &str,
    ring: &VecDeque<IncludedPollSample>,
) -> std::io::Result<()> {
    let df = DurableFile {
        identity_id: identity_id.to_owned(),
        samples: ring.iter().map(sample_to_durable).collect(),
    };
    let data = serde_json::to_vec_pretty(&df)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(&data)?;
    file.sync_all()?;
    Ok(())
}

fn load_ring_at_path(path: &Path, identity_id: &str) -> Option<VecDeque<IncludedPollSample>> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(path)
        .ok()?;
    // Shared lock for multi-reader safety with exclusive writers.
    file.lock_shared().ok()?;
    let ring = read_ring_from_file(&mut file, identity_id);
    let _ = file.unlock();
    Some(ring)
}

fn list_identity_ids_in_dir(dir: &Path) -> Vec<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Prefer identity_id from file body (filename is sanitized).
        if let Ok(mut file) = File::open(&path) {
            let mut buf = String::new();
            if file.read_to_string(&mut buf).is_ok()
                && let Ok(df) = serde_json::from_str::<DurableFile>(buf.trim())
            {
                let id = df.identity_id.trim();
                if !id.is_empty() {
                    out.insert(id.to_owned());
                    continue;
                }
            }
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && !stem.is_empty()
        {
            out.insert(stem.to_owned());
        }
    }
    out.into_iter().collect()
}

fn clear_durable_dir_at(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let _ = fs::remove_file(path);
        }
    }
}

/// Flock-append under process default `$GROK_HOME` durable dir; update process map.
fn durable_record_and_mirror(identity_id: &str, sample: IncludedPollSample) {
    let store = match IncludedPollHistoryStore::open(grok_home_path()) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(
                error = %e,
                "included_poll_history: could not open durable dir; process ring only"
            );
            // Process-only fallback.
            let mut map = POLL_HISTORY_BY_IDENTITY
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let ring = map.entry(identity_id.to_owned()).or_default();
            if let Some(prev) = ring.back() {
                log_poll_delta_if_stepped(identity_id, prev, &sample);
            }
            ring.push_back(sample);
            while ring.len() > RING_CAP {
                ring.pop_front();
            }
            return;
        }
    };
    if let Err(e) = store.record(identity_id, sample.clone()) {
        tracing::debug!(
            error = %e,
            identity_id,
            "included_poll_history: durable record failed; process ring only"
        );
        let mut map = POLL_HISTORY_BY_IDENTITY
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let ring = map.entry(identity_id.to_owned()).or_default();
        if let Some(prev) = ring.back() {
            log_poll_delta_if_stepped(identity_id, prev, &sample);
        }
        ring.push_back(sample);
        while ring.len() > RING_CAP {
            ring.pop_front();
        }
        return;
    }
    // Mirror full durable ring into process map (authoritative after write).
    let samples = store.history_for(identity_id);
    let mut map = POLL_HISTORY_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.insert(identity_id.to_owned(), samples.into_iter().collect());
}

/// Load durable ring for identity and merge into process map (disk wins order).
fn load_durable_into_process(identity_id: &str) -> Vec<IncludedPollSample> {
    let path = durable_path_for(identity_id);
    let disk = load_ring_at_path(&path, identity_id).unwrap_or_default();
    let mut map = POLL_HISTORY_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if disk.is_empty() {
        return map
            .get(identity_id)
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default();
    }
    // Disk is multi-process SoT after a successful write. Prefer disk when present.
    map.insert(identity_id.to_owned(), disk.clone());
    disk.into_iter().collect()
}

// ── Public process API ──────────────────────────────────────────────────────

/// Append a poll sample for one SuperGrok principal (process + durable).
///
/// No-op on empty `identity_id`. Caps ring at [`RING_CAP`]. When the new
/// sample steps meters vs the previous sample for this identity, logs
/// `billing: poll_delta` (optional observability; not a hop signal).
///
/// Writes under `$GROK_HOME/included_poll_history/` so cold processes share
/// the series. Never stores tokens.
pub fn record_included_poll_sample(identity_id: &str, sample: IncludedPollSample) {
    let id = identity_id.trim();
    if id.is_empty() {
        return;
    }
    durable_record_and_mirror(id, sample);
}

/// Record a sample stamped with `Utc::now()` (product S1 success path).
pub fn record_included_poll_now(
    identity_id: &str,
    credit_usage_percent: f64,
    build_usage_percent: Option<f64>,
    prepaid_balance_cents: Option<i64>,
) {
    record_included_poll_sample(
        identity_id,
        IncludedPollSample {
            ts: Utc::now(),
            credit_usage_percent,
            build_usage_percent,
            prepaid_balance_cents,
        },
    );
}

fn log_poll_delta_if_stepped(
    identity_id: &str,
    prev: &IncludedPollSample,
    next: &IncludedPollSample,
) {
    let included_stepped = !float_same(prev.credit_usage_percent, next.credit_usage_percent);
    let build_stepped = optional_f64_stepped(prev.build_usage_percent, next.build_usage_percent);
    let extras_stepped =
        optional_i64_stepped(prev.prepaid_balance_cents, next.prepaid_balance_cents);
    if !(included_stepped || build_stepped || extras_stepped) {
        return;
    }
    tracing::info!(
        target: "xai_grok_shell::auth",
        identity_id,
        prev_credit_usage_percent = prev.credit_usage_percent,
        next_credit_usage_percent = next.credit_usage_percent,
        prev_build_usage_percent = ?prev.build_usage_percent,
        next_build_usage_percent = ?next.build_usage_percent,
        prev_prepaid_balance_cents = ?prev.prepaid_balance_cents,
        next_prepaid_balance_cents = ?next.prepaid_balance_cents,
        "billing: poll_delta"
    );
}

/// Snapshot of one identity's poll ring (oldest → newest). Empty if never polled.
///
/// Loads durable `$GROK_HOME` history so a cold process sees samples recorded
/// by other processes.
pub fn included_poll_history_for(identity_id: &str) -> Vec<IncludedPollSample> {
    let id = identity_id.trim();
    if id.is_empty() {
        return Vec::new();
    }
    load_durable_into_process(id)
}

/// Which SuperGrok meters were observed flat in a flat-poll window.
///
/// `unproven` is true only when the detector fires. `observed_build` /
/// `observed_extras` are true only when **every** sample in that window
/// carried the field (and it stayed flat). Honesty copy must not name Build
/// or SuperGrok $ extras as flat unless the matching flag is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlatPollEvidence {
    pub unproven: bool,
    pub observed_build: bool,
    pub observed_extras: bool,
}

/// Pure evidence for one sample series (same window rules as
/// [`included_debit_unproven`]).
pub fn flat_poll_evidence_for_samples(
    samples: &[IncludedPollSample],
    min_polls: usize,
    min_window: Duration,
) -> FlatPollEvidence {
    if !included_debit_unproven(samples, min_polls, min_window) {
        return FlatPollEvidence::default();
    }
    // Same recent window as [`included_debit_unproven`] (not only last N points).
    let window = recent_flat_candidate_window(samples, min_polls, min_window)
        .expect("included_debit_unproven true implies a candidate window");
    let observed_build = window.iter().all(|s| s.build_usage_percent.is_some());
    let observed_extras = window.iter().all(|s| s.prepaid_balance_cents.is_some());
    FlatPollEvidence {
        unproven: true,
        observed_build,
        observed_extras,
    }
}

/// Flat-poll evidence from process + durable history (defaults thresholds).
///
/// When multiple identities have history, returns the first identity whose
/// series is unproven (OR across identities for `unproven`; observed flags
/// come from that same series so honesty copy stays honest).
///
/// Scans `$GROK_HOME/included_poll_history/` so a cold process with an empty
/// process ring still sees multi-process samples.
pub fn flat_poll_evidence_from_history() -> FlatPollEvidence {
    flat_poll_evidence_from_history_with(DEFAULT_MIN_POLLS, DEFAULT_MIN_WINDOW)
}

/// Like [`flat_poll_evidence_from_history`] with explicit thresholds.
pub fn flat_poll_evidence_from_history_with(
    min_polls: usize,
    min_window: Duration,
) -> FlatPollEvidence {
    let mut ids: BTreeSet<String> = {
        let map = POLL_HISTORY_BY_IDENTITY
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        map.keys().cloned().collect()
    };
    for id in list_identity_ids_in_dir(&process_default_root()) {
        ids.insert(id);
    }
    for id in ids {
        let samples = included_poll_history_for(&id);
        let ev = flat_poll_evidence_for_samples(&samples, min_polls, min_window);
        if ev.unproven {
            return ev;
        }
    }
    FlatPollEvidence::default()
}

/// True when any SuperGrok identity's history meets default flat-poll criteria.
///
/// Used to set `LimitsSnapshot.flat_poll_unproven_debit` on `/limits` and
/// `limits --json` from real process + durable history (not a test-only
/// setter alone).
pub fn flat_poll_unproven_debit_from_history() -> bool {
    flat_poll_evidence_from_history().unproven
}

/// Like [`flat_poll_unproven_debit_from_history`] with explicit thresholds.
pub fn flat_poll_unproven_debit_from_history_with(min_polls: usize, min_window: Duration) -> bool {
    flat_poll_evidence_from_history_with(min_polls, min_window).unproven
}

/// Clear process poll history **and** durable files under current `$GROK_HOME`.
///
/// Prefer isolating `$GROK_HOME` in tests (see [`with_history_lock`]) so this
/// never wipes an operator's live series.
pub fn clear_included_poll_history() {
    POLL_HISTORY_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    clear_durable_dir_at(&process_default_root());
}

/// Clear process ring only; leave durable files (multi-process / cold restart tests).
pub fn clear_process_included_poll_history_only() {
    POLL_HISTORY_BY_IDENTITY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
}

/// Path of the durable file for `identity_id` under current `$GROK_HOME` (tests).
#[cfg(test)]
fn durable_path_for_test(identity_id: &str) -> PathBuf {
    durable_path_for(identity_id)
}

/// Serialize tests that mutate process-global poll history.
///
/// Isolates `$GROK_HOME` to a temp dir so durable writes never touch the
/// operator's real home. Clears process + durable before and after `f`.
#[cfg(test)]
pub fn with_history_lock<R>(f: impl FnOnce() -> R) -> R {
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = TempDir::new().expect("temp GROK_HOME for included poll history tests");
    let _home = EnvGuard::set("GROK_HOME", dir.path());
    clear_included_poll_history();
    let out = f();
    clear_included_poll_history();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).single().expect("valid unix ts")
    }

    fn sample(secs: i64, pct: f64, build: Option<f64>, extras: Option<i64>) -> IncludedPollSample {
        IncludedPollSample {
            ts: ts(secs),
            credit_usage_percent: pct,
            build_usage_percent: build,
            prepaid_balance_cents: extras,
        }
    }

    /// Named contract: flat included % + flat SuperGrok $ extras → unproven.
    #[test]
    fn poll_history_marks_flat_when_included_and_extras_unchanged() {
        let samples = vec![
            sample(1_000, 65.0, Some(54.0), Some(10029)),
            sample(1_060, 65.0, Some(54.0), Some(10029)),
        ];
        assert!(
            included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "two polls 60s apart with same included % and extras must mark flat"
        );
    }

    /// Named contract: included % step clears flat (debit at least observed).
    #[test]
    fn poll_history_clears_flat_when_included_pct_steps() {
        let samples = vec![
            sample(1_000, 65.0, Some(54.0), Some(10029)),
            sample(1_060, 66.5, Some(54.0), Some(10029)),
        ];
        assert!(
            !included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "included % step must clear flat-poll unproven"
        );
    }

    /// Named contract: Grok Build productUsage step clears flat even if top-level % flat.
    #[test]
    fn poll_history_clears_flat_when_build_product_usage_steps() {
        let samples = vec![
            sample(1_000, 65.0, Some(54.0), Some(10029)),
            sample(1_060, 65.0, Some(55.2), Some(10029)),
        ];
        assert!(
            !included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "Build product % step must clear flat even when creditUsagePercent is flat"
        );
    }

    /// Named contract: SuperGrok $ extras drop clears flat.
    #[test]
    fn poll_history_clears_flat_when_extras_cents_drop() {
        let samples = vec![
            sample(1_000, 100.0, Some(80.0), Some(10029)),
            sample(1_060, 100.0, Some(80.0), Some(9900)),
        ];
        assert!(
            !included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "extras cents drop must clear flat-poll unproven"
        );
    }

    #[test]
    fn not_flat_when_fewer_than_min_polls() {
        let samples = vec![sample(1_000, 65.0, None, Some(100))];
        assert!(!included_debit_unproven(
            &samples,
            2,
            Duration::from_secs(0)
        ));
    }

    #[test]
    fn not_flat_when_window_too_short() {
        let samples = vec![
            sample(1_000, 65.0, None, Some(100)),
            sample(1_010, 65.0, None, Some(100)),
        ];
        assert!(
            !included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "10s span must not satisfy 30s min_window"
        );
    }

    /// Named contract (C4 multipoll under load): many dense samples a few
    /// seconds apart with free SuperGrok period % flat for ≥30s wall must mark
    /// unproven debit. Looking only at the last two points (often ~2s apart)
    /// used to miss this and hide ticket evidence.
    #[test]
    fn dense_high_frequency_flat_series_marks_unproven_when_wall_spans_min_window() {
        // 20 samples, 2s apart → 38s wall, all free period 6.0%.
        let samples: Vec<_> = (0..20)
            .map(|i| sample(1_000 + i * 2, 6.0, None, Some(10029)))
            .collect();
        assert!(
            included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "dense multipoll flat free-period series spanning ≥30s must mark unproven"
        );
        let ev = flat_poll_evidence_for_samples(&samples, 2, Duration::from_secs(30));
        assert!(ev.unproven);
        assert!(!ev.observed_build, "Build never on wire in this series");
        assert!(ev.observed_extras, "extras present on every dense sample");
    }

    /// Named contract: dense series that only recently stepped must clear.
    #[test]
    fn dense_flat_then_step_in_recent_window_clears_unproven() {
        let mut samples: Vec<_> = (0..15)
            .map(|i| sample(1_000 + i * 2, 6.0, None, Some(10029)))
            .collect();
        // Last two seconds: free period steps to 7% (still dense).
        samples.push(sample(1_000 + 15 * 2, 7.0, None, Some(10029)));
        samples.push(sample(1_000 + 16 * 2, 7.0, None, Some(10029)));
        // Recent 30s window still includes the 6→7 step.
        assert!(
            !included_debit_unproven(&samples, 2, Duration::from_secs(30)),
            "step inside the recent min_window must clear flat-poll unproven"
        );
    }

    #[test]
    fn process_ring_feeds_flat_from_history() {
        with_history_lock(|| {
            record_included_poll_sample("team-a", sample(1_000, 65.0, None, Some(500)));
            record_included_poll_sample("team-a", sample(1_060, 65.0, None, Some(500)));
            assert!(
                flat_poll_unproven_debit_from_history_with(2, Duration::from_secs(30)),
                "process history with flat meters must surface unproven debit"
            );
            // Step included % → clear.
            record_included_poll_sample("team-a", sample(1_120, 70.0, None, Some(500)));
            assert!(
                !flat_poll_unproven_debit_from_history_with(2, Duration::from_secs(30)),
                "latest window with step must not stay flat"
            );
        });
    }

    #[test]
    fn empty_identity_is_noop() {
        with_history_lock(|| {
            record_included_poll_sample("  ", sample(1_000, 1.0, None, None));
            assert!(included_poll_history_for("  ").is_empty());
        });
    }

    /// Named contract (Issue 1): included-only flat series → unproven but
    /// observed_build / observed_extras false (honesty must not claim them).
    #[test]
    fn flat_evidence_included_only_does_not_mark_build_or_extras_observed() {
        let samples = vec![
            sample(1_000, 65.0, None, None),
            sample(1_060, 65.0, None, None),
        ];
        let ev = flat_poll_evidence_for_samples(&samples, 2, Duration::from_secs(30));
        assert!(ev.unproven, "included-only flat must still be unproven");
        assert!(
            !ev.observed_build,
            "Build never on wire → not observed flat"
        );
        assert!(
            !ev.observed_extras,
            "extras never on wire → not observed flat"
        );
    }

    /// Full dogfood-shaped series: all three meters observed flat.
    #[test]
    fn flat_evidence_all_meters_observed_when_present() {
        let samples = vec![
            sample(1_000, 65.0, Some(54.0), Some(10029)),
            sample(1_060, 65.0, Some(54.0), Some(10029)),
        ];
        let ev = flat_poll_evidence_for_samples(&samples, 2, Duration::from_secs(30));
        assert!(ev.unproven);
        assert!(ev.observed_build);
        assert!(ev.observed_extras);
    }

    /// Named contract: two store handles (two processes) share samples on disk.
    #[test]
    fn two_store_handles_share_poll_samples() {
        let dir = tempfile::TempDir::new().expect("temp grok home");
        let a = IncludedPollHistoryStore::open(dir.path()).expect("open a");
        let b = IncludedPollHistoryStore::open(dir.path()).expect("open b");
        a.record("team-shared", sample(1_000, 65.0, Some(54.0), Some(10029)))
            .expect("record a");
        let from_b = b.history_for("team-shared");
        assert_eq!(
            from_b.len(),
            1,
            "peer B must load A's sample from durable file"
        );
        assert_eq!(from_b[0].credit_usage_percent, 65.0);
        b.record("team-shared", sample(1_060, 65.0, Some(54.0), Some(10029)))
            .expect("record b");
        let from_a = a.history_for("team-shared");
        assert_eq!(from_a.len(), 2, "A must see B's second sample");
        let ev = a.flat_evidence(2, Duration::from_secs(30));
        assert!(
            ev.unproven && ev.observed_build && ev.observed_extras,
            "flat evidence must fire across two store handles"
        );
        // No secrets: file is JSON with only meters + identity_id.
        let path = dir.path().join(DURABLE_SUBDIR).join("team-shared.json");
        let body = fs::read_to_string(&path).expect("read durable");
        assert!(body.contains("credit_usage_percent"));
        assert!(body.contains("identity_id"));
        assert!(
            !body.to_ascii_lowercase().contains("token")
                && !body.to_ascii_lowercase().contains("bearer")
                && !body.contains("sk-"),
            "durable poll history must not store secrets: {body}"
        );
    }

    /// Named contract: cold process (empty process map) sees prior process
    /// samples via `$GROK_HOME` and can surface flat after a second spaced poll.
    #[test]
    fn cold_process_surfaces_flat_from_prior_process_disk() {
        with_history_lock(|| {
            // Process 1: one poll only (not enough for flat alone).
            record_included_poll_sample("team-cold", sample(1_000, 65.0, Some(54.0), Some(10029)));
            assert!(
                durable_path_for_test("team-cold").is_file(),
                "record must write under $GROK_HOME/{DURABLE_SUBDIR}"
            );
            assert!(
                !flat_poll_unproven_debit_from_history_with(2, Duration::from_secs(30)),
                "single sample must not invent flat"
            );

            // Simulate process exit / cold CLI: drop process ring only.
            clear_process_included_poll_history_only();
            assert!(
                included_poll_history_for("team-cold").len() == 1,
                "cold process must load the prior sample from disk"
            );

            // Process 2: second spaced poll with same meters.
            record_included_poll_sample("team-cold", sample(1_060, 65.0, Some(54.0), Some(10029)));
            // Cold evidence path also scans disk when process map was empty
            // before the second record; after record process is warm — clear
            // process again and require disk-only evidence.
            clear_process_included_poll_history_only();
            assert!(
                flat_poll_unproven_debit_from_history_with(2, Duration::from_secs(30)),
                "cold process must surface flat when multi-sample series lives only on disk"
            );
        });
    }

    /// Ring cap preserved on durable write.
    #[test]
    fn durable_ring_caps_at_thirty_two() {
        let dir = tempfile::TempDir::new().expect("temp");
        let store = IncludedPollHistoryStore::open(dir.path()).expect("open");
        for i in 0..40 {
            store
                .record("cap-id", sample(1_000 + i, 50.0, None, None))
                .expect("record");
        }
        let hist = store.history_for("cap-id");
        assert_eq!(hist.len(), RING_CAP, "durable ring must cap at {RING_CAP}");
        assert_eq!(hist[0].ts, ts(1_000 + 8), "oldest kept is sample index 8");
    }
}
