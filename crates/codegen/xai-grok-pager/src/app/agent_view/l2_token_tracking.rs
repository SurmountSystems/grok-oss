//! In-memory nested L2 token counts and one grok-oss sqlite row.
//!
//! Subagents list layout reads only this in-memory map. This module never
//! opens session transcript files on paint and does not write TECH.md.
//! Measured nested L2 tokens are
//! session usage counts, not included SuperGrok period limits, not SuperGrok
//! dollar credits, and not console team prepaid / console API credits.
//! Operator-visible Subagents list chrome omits the word `tokens`. The unit
//! is implicit (`90k`, `112.9k`). Each nested session id is its own window.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// One nested L2 row tracked in memory.
///
/// Grok OSS: `measured_tokens` is an `AtomicU64` high-water so concurrent ACP
/// usage ticks do not race. This diverges from upstream xAI because Grok OSS
/// Subagents list chrome tracks nested L2 session usage in this map. A racy
/// last-write `u64` can drop a later count (10232) when a stale smaller tick
/// lands last.
#[derive(Debug)]
pub struct NestedL2Tokens {
    /// Nested session id (internal key; not dumped in Subagents list text).
    pub nested_session_id: String,
    /// Operator-facing description label for the Subagents list.
    pub description: String,
    /// grok-oss event ULID for this nested row. Not a Grok Build UUID.
    event_ulid: String,
    /// grok-oss work ULID shared by spawn, usage, and exit. One row.
    work_ulid: String,
    /// High-water nested L2 session usage. Not included SuperGrok period
    /// limits, SuperGrok dollar credits, or console team prepaid.
    /// The grok-oss row stores this high-water. List paint reads
    /// `current_tokens` so compact can go down.
    measured_tokens: AtomicU64,
    /// Latest live sample. `store`, not `fetch_max`, so a later compact
    /// replaces a stale larger window on the next paint.
    current_tokens: AtomicU64,
    current_set: AtomicBool,
    /// Status: spawned, running, or exited.
    pub status: NestedL2Status,
}

/// Lifecycle of a nested L2 on the token tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedL2Status {
    Spawned,
    Running,
    Exited,
}

impl NestedL2Tokens {
    fn new(nested_session_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            nested_session_id: nested_session_id.into(),
            description: description.into(),
            event_ulid: xai_grok_tools::util::ulid::mint(),
            work_ulid: xai_grok_tools::util::ulid::mint(),
            measured_tokens: AtomicU64::new(0),
            current_tokens: AtomicU64::new(0),
            current_set: AtomicBool::new(false),
            status: NestedL2Status::Spawned,
        }
    }

    /// Session usage high-water for this nested L2 (not billing meters).
    pub fn measured_tokens(&self) -> u64 {
        self.measured_tokens.load(Ordering::Relaxed)
    }

    /// Current live sample, if a usage tick has stored one. Can be below
    /// [`Self::measured_tokens`] after compact.
    pub fn current_tokens(&self) -> Option<u64> {
        if self.current_set.load(Ordering::Relaxed) {
            Some(self.current_tokens.load(Ordering::Relaxed))
        } else {
            None
        }
    }

    fn store_current(&self, measured_tokens: u64) {
        self.current_tokens
            .store(measured_tokens, Ordering::Relaxed);
        self.current_set.store(true, Ordering::Relaxed);
    }
}

impl Clone for NestedL2Tokens {
    fn clone(&self) -> Self {
        Self {
            nested_session_id: self.nested_session_id.clone(),
            description: self.description.clone(),
            event_ulid: self.event_ulid.clone(),
            work_ulid: self.work_ulid.clone(),
            measured_tokens: AtomicU64::new(self.measured_tokens()),
            current_tokens: AtomicU64::new(self.current_tokens().unwrap_or(0)),
            current_set: AtomicBool::new(self.current_tokens().is_some()),
            status: self.status,
        }
    }
}

impl PartialEq for NestedL2Tokens {
    fn eq(&self, other: &Self) -> bool {
        self.nested_session_id == other.nested_session_id
            && self.description == other.description
            && self.event_ulid == other.event_ulid
            && self.work_ulid == other.work_ulid
            && self.measured_tokens() == other.measured_tokens()
            && self.current_tokens() == other.current_tokens()
            && self.status == other.status
    }
}

impl Eq for NestedL2Tokens {}

/// In-memory nested L2 token counts keyed by nested session id.
#[derive(Debug, Clone)]
pub struct L2TokenTracker {
    by_id: HashMap<String, NestedL2Tokens>,
}

impl Default for L2TokenTracker {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
        }
    }
}

fn process_tracker() -> &'static Mutex<L2TokenTracker> {
    static TRACKER: OnceLock<Mutex<L2TokenTracker>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(L2TokenTracker::default()))
}

fn with_process_tracker(f: impl FnOnce(&mut L2TokenTracker)) {
    let mut tracker = process_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut tracker);
}

fn peek_process_tracker<R>(f: impl FnOnce(&L2TokenTracker) -> R) -> R {
    let tracker = process_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&tracker)
}

/// Production spawn hook. Subagents list paint still uses in-memory counts only.
/// Also calls `insert_local_usage_event`. A zero total is not stored.
pub fn on_nested_l2_spawn(nested_session_id: &str, description: &str) {
    with_process_tracker(|t| t.record_spawn(nested_session_id, description));
    if let Some((store, event)) = nested_l2_sqlite_pair(nested_session_id) {
        let _ = xai_grok_shell::token_economy::insert_local_usage_event(&store, &event);
    }
}

/// Production usage-tick hook (session usage tokens, not billing meters).
///
/// Always stores the live sample with `store_current`, including when the new
/// count is below the in-memory high-water. `record_usage` still `fetch_max`s
/// `measured_tokens` and does not lower that high-water.
pub fn on_nested_l2_usage(nested_session_id: &str, measured_tokens: u64) {
    with_process_tracker(|t| {
        t.record_usage(nested_session_id, measured_tokens);
        if let Some(row) = t.get(nested_session_id) {
            row.store_current(measured_tokens);
        }
    });
    if let Some((store, event)) = nested_l2_sqlite_pair(nested_session_id) {
        let _ = xai_grok_shell::token_economy::insert_local_usage_event(&store, &event);
    }
}

/// Production L2-exit hook. Keeps the last measured count and writes
/// the same ULID row into grok-oss sqlite.
pub fn on_nested_l2_exit(nested_session_id: &str) {
    with_process_tracker(|t| t.record_exit(nested_session_id));
    if let Some((store, event)) = nested_l2_sqlite_pair(nested_session_id) {
        let _ = xai_grok_shell::token_economy::insert_local_usage_event(&store, &event);
    }
}

/// One grok-oss `local_usage_event` for this nested L2. Not the session sqlite.
///
/// Paint does not call this. Tests skip unless `grok_oss_database_path` is
/// set, so crate tests do not write the operator store. A zero total still
/// reaches the ledger write, which does not store it.
fn nested_l2_sqlite_pair(
    nested_session_id: &str,
) -> Option<(
    xai_grok_shell::grok_oss::GrokOssStore,
    xai_grok_shell::token_economy::LocalUsageEvent,
)> {
    let event = peek_process_tracker(|t| {
        let row = t.get(nested_session_id)?;
        let total = i64::try_from(row.measured_tokens()).ok()?;
        Some(xai_grok_shell::token_economy::LocalUsageEvent {
            event_ulid: row.event_ulid.clone(),
            session_id: row.nested_session_id.clone(),
            work_ulid: Some(row.work_ulid.clone()),
            timestamp_utc: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            turn_type: "nested".to_string(),
            agent_kind: "l2".to_string(),
            model_id: None,
            input_tokens: None,
            output_tokens: None,
            cached_tokens: None,
            reasoning_tokens: None,
            total_tokens: Some(total),
            cost_usd_ticks: None,
            cost_missing: true,
            incomplete: false,
            sampling_identity: None,
        })
    })?;
    let cfg = xai_grok_shell::token_economy::token_economy_from_disk();
    if cfg!(test) && cfg.grok_oss_database_path.is_none() {
        return None;
    }
    let store = xai_grok_shell::grok_oss::try_open_from_token_economy_config(&cfg)?;
    Some((store, event))
}

impl L2TokenTracker {
    /// Record a nested L2 spawn. Description is the Subagents list label.
    pub fn record_spawn(
        &mut self,
        nested_session_id: impl Into<String>,
        description: impl Into<String>,
    ) {
        let id = nested_session_id.into();
        let row = NestedL2Tokens::new(id.clone(), description);
        self.by_id.insert(id, row);
    }

    /// Record a usage tick (measured session tokens, not billing meters).
    ///
    /// ACP `SubagentProgress` `tokens_used` is that nested session's live
    /// sampling window (`context_tokens_used`). `fetch_max` keeps an in-memory
    /// high-water so concurrent ticks cannot lose a later count. The current
    /// sample is stored separately and can go down after compact. Subagents
    /// list paint reads that current sample, not the high-water.
    pub fn record_usage(&mut self, nested_session_id: &str, measured_tokens: u64) {
        if let Some(row) = self.by_id.get_mut(nested_session_id) {
            let _previous = row
                .measured_tokens
                .fetch_max(measured_tokens, Ordering::Relaxed);
            // Not fetch_max. A later 40100 still replaces a stale 90000 live sample.
            row.store_current(measured_tokens);
            if row.status == NestedL2Status::Spawned {
                row.status = NestedL2Status::Running;
            }
        }
    }

    /// Record nested L2 exit. Keeps the last measured count.
    pub fn record_exit(&mut self, nested_session_id: &str) {
        if let Some(row) = self.by_id.get_mut(nested_session_id) {
            row.status = NestedL2Status::Exited;
        }
    }

    pub fn get(&self, nested_session_id: &str) -> Option<&NestedL2Tokens> {
        self.by_id.get(nested_session_id)
    }

    /// Subagents list token suffix. Plain English. No UUID dump. No billing-meter words.
    pub fn format_subagents_list_token_suffix(&self, nested_session_id: &str) -> Option<String> {
        let row = self.get(nested_session_id)?;
        let measured = row.measured_tokens();
        if row.status == NestedL2Status::Spawned && measured == 0 {
            return None;
        }
        Some(format_measured_tokens_suffix(measured))
    }
}

/// Compact Subagents list suffix: `53.4k`, `90k`, `112.9k`.
///
/// Same compact count style as the rest of grok-oss (`format_tokens_compact`).
/// The unit is implicit. Must not contain the word `tokens` or `measured`.
/// Must not paint a raw integer like 53407. Under 1000 stays `42`.
pub fn format_measured_tokens_suffix(measured_tokens: u64) -> String {
    crate::views::agent_status::format_tokens_compact(
        i64::try_from(measured_tokens).unwrap_or(i64::MAX),
    )
}

/// Subagents list row paint. In-memory count only. Never opens the session transcript file.
pub fn format_subagents_list_row_from_memory(
    description: &str,
    measured_tokens_suffix: Option<&str>,
) -> String {
    match measured_tokens_suffix {
        Some(suffix) if !suffix.is_empty() => format!("{description} ({suffix})"),
        _ => description.to_string(),
    }
}

/// Current live sample for this nested id, if a tick has stored one.
///
/// List paint calls this (via [`format_live_subagents_list_suffix`] with no
/// override) so a later sample replaces a stale row without another event.
/// Absent means the list may fall back to the in-memory high-water.
pub fn current_live_sample(nested_session_id: &str) -> Option<u64> {
    peek_process_tracker(|t| {
        t.get(nested_session_id)
            .and_then(|row| row.current_tokens())
    })
}

/// Compact suffix for a nested session window.
///
/// An explicit live sample still wins over the tracker high-water, and is
/// stored as the current sample. With no override, paint reads
/// [`current_live_sample`], then the high-water only when no sample has
/// been stored.
/// Does not open the session transcript file. Never the word `tokens`.
pub fn format_live_subagents_list_suffix(
    nested_session_id: &str,
    tokens_used: Option<u64>,
) -> Option<String> {
    if let Some(live) = tokens_used {
        with_process_tracker(|t| {
            if let Some(row) = t.by_id.get(nested_session_id) {
                row.store_current(live);
            }
        });
        return Some(format_measured_tokens_suffix(live));
    }
    if let Some(current) = current_live_sample(nested_session_id) {
        return Some(format_measured_tokens_suffix(current));
    }
    peek_process_tracker(|t| t.format_subagents_list_token_suffix(nested_session_id))
}

/// One nested session's shown count.
///
/// A live sample replaces a stale present window, then `past` is added once.
/// After compact the live sample can still hold the pre-compact window while
/// `past` already keeps the dropped units, so that larger sample is not added
/// on top of `past`. `None` when the host has no present sample and no past.
pub fn shown_nested_count(session_id: &str, present: Option<u64>, past: u64) -> Option<u64> {
    let live = current_live_sample(session_id);
    let present_window = match (live, present) {
        (Some(live), Some(snapshot)) if past > 0 && live > snapshot => Some(snapshot),
        (Some(live), _) => Some(live),
        (None, snapshot) => snapshot,
    };
    match present_window {
        Some(window) => Some(window.saturating_add(past)),
        None if past > 0 => Some(past),
        None => None,
    }
}

/// L2 row total: that session's shown count plus each L3 count, once.
///
/// `None` when every count is absent. A missing count adds nothing.
pub fn sum_shown_counts_once(own: Option<u64>, l3_counts: &[Option<u64>]) -> Option<u64> {
    let mut total = 0u64;
    let mut any = false;
    if let Some(count) = own {
        total = total.saturating_add(count);
        any = true;
    }
    for count in l3_counts.iter().copied().flatten() {
        total = total.saturating_add(count);
        any = true;
    }
    any.then_some(total)
}

/// Grok 4.6-era wrap average. It stays an estimate until the host returns a figure.
/// Not an actual token count. Not billing truth.
pub const STANDING_WRAP_ESTIMATE_WALL: &str = "19.4 minutes";
/// Grok 4.6-era nested-token estimate. Not an actual token count.
pub const STANDING_WRAP_ESTIMATE_TOKENS: &str = "167.0k";

/// Display text for one live job row. Pure. No L1 total and no grok-oss sqlite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveJobRowDisplay {
    pub job: String,
    /// Wall estimate, labeled as an estimate.
    pub estimate_wall: String,
    /// Token estimate, labeled as an estimate.
    pub estimate_tokens: String,
    pub elapsed: String,
    /// Host figure. Empty when the host has not returned a figure.
    /// Empty omits the token clause. It is not a placeholder and not an estimate.
    pub actual_tokens: String,
    /// Always 0. Display must not add this row into the L1 sampling window.
    pub l1_tokens_added: u64,
    /// Always false. Display must not write grok-oss sqlite.
    pub wrote_grok_oss_sqlite: bool,
}

/// Inputs for [`display_live_job_row`].
///
/// `host_tokens` is the host session-usage figure for this row. `None` means
/// the host has not returned a figure yet. Do not pass the standing estimate
/// in place of a missing figure.
pub struct LiveJobRowInput<'a> {
    pub job: &'a str,
    pub estimate_wall: &'a str,
    pub estimate_tokens: &'a str,
    pub elapsed: &'a str,
    pub host_tokens: Option<u64>,
}

fn label_as_estimate(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.to_ascii_lowercase().contains("estimate") {
        trimmed.to_string()
    } else {
        format!("{trimmed} (estimate)")
    }
}

/// Paint one live job row.
///
/// A host figure is shown with the same compact count as Subagents list
/// chrome. No host figure leaves the token text empty so the row omits the
/// token clause. Estimates stay labeled as estimates. This function does not
/// copy [`STANDING_WRAP_ESTIMATE_WALL`] or [`STANDING_WRAP_ESTIMATE_TOKENS`]
/// into the token count, does not add the figure into the L1 total, and does
/// not write grok-oss sqlite.
pub fn display_live_job_row(input: LiveJobRowInput<'_>) -> LiveJobRowDisplay {
    let actual_tokens = match input.host_tokens {
        Some(figure) => format_measured_tokens_suffix(figure),
        None => String::new(),
    };
    LiveJobRowDisplay {
        job: input.job.to_string(),
        estimate_wall: label_as_estimate(input.estimate_wall),
        estimate_tokens: label_as_estimate(input.estimate_tokens),
        elapsed: input.elapsed.to_string(),
        actual_tokens,
        l1_tokens_added: 0,
        wrote_grok_oss_sqlite: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Subagents list omits the word tokens. Live paint helper
    /// [`format_subagents_list_row_from_memory`] paints compact count after
    /// an in-memory usage tick. Must not contain `measured`. Must not paint
    /// a raw integer like 12400. No usage tick means no suffix.
    #[test]
    fn format_subagents_list_description_shows_measured_tokens_suffix() {
        let suffix = format_measured_tokens_suffix(12400);
        let with_tick =
            format_subagents_list_row_from_memory("rate-limit implementer", Some(&suffix));
        assert!(
            with_tick.contains("12.4k"),
            "Subagents description must contain 12.4k, got {with_tick:?}"
        );
        assert!(
            !with_tick.contains("tokens") && !with_tick.contains("token"),
            "Subagents list omits the word tokens; got {with_tick:?}"
        );
        assert!(
            !with_tick.contains("measured"),
            "Contract A: Subagents nested token chrome must not contain measured, got {with_tick:?}"
        );
        assert!(
            !with_tick.contains("12400"),
            "must not paint a raw integer token count, got {with_tick:?}"
        );
        assert_eq!(format_measured_tokens_suffix(12400), "12.4k");
        assert_eq!(format_measured_tokens_suffix(112_900), "112.9k");
        let before_tick = format_subagents_list_row_from_memory("rate-limit implementer", None);
        assert_eq!(before_tick, "rate-limit implementer");
        assert!(
            !before_tick.contains("tokens"),
            "no compact suffix before the first usage tick, got {before_tick:?}"
        );
    }

    /// Subagents list omits the word tokens. Nested L2 chrome uses the same
    /// compact count style as the rest of grok-oss (K/M). Must contain
    /// `53.4k`. Must not contain `measured`. Must not paint a raw integer
    /// like 53407. Match `format_tokens_compact`.
    #[test]
    fn subagents_list_shows_measured_tokens_per_nested_l2() {
        let mut tracker = L2TokenTracker::default();
        tracker.record_spawn("nested-l2-session", "Stale prompt still live");
        tracker.record_usage("nested-l2-session", 53407);
        let row = format_subagents_list_row_from_memory(
            "Stale prompt still live",
            tracker
                .format_subagents_list_token_suffix("nested-l2-session")
                .as_deref(),
        );
        let compact = crate::views::agent_status::format_tokens_compact(53407);
        assert_eq!(compact, "53.4k");
        assert!(
            row.contains("53.4k"),
            "Subagents list row must contain 53.4k, got {row:?}"
        );
        assert!(
            !row.contains("tokens") && !row.contains("token"),
            "Subagents list omits the word tokens; got {row:?}"
        );
        assert!(
            !row.contains("measured"),
            "Contract A: Subagents nested token chrome must not contain measured, got {row:?}"
        );
        assert!(
            !row.contains("53407"),
            "must not paint a raw integer token count, got {row:?}"
        );
        assert!(
            !row.contains("nested-l2-session") || row.contains("Stale prompt still live"),
            "row uses the description label, not UUID speech as the visible name"
        );
        assert_eq!(format_measured_tokens_suffix(42), "42");
        assert_eq!(format_measured_tokens_suffix(12400), "12.4k");
        assert_eq!(format_measured_tokens_suffix(90_000), "90k");
        assert_eq!(format_measured_tokens_suffix(112_900), "112.9k");
        assert_eq!(format_measured_tokens_suffix(112_600), "112.6k");
        assert_eq!(format_measured_tokens_suffix(1_500_000), "1.5M");
    }

    /// Subagents list omits the word tokens. Truncation of a long job name
    /// must not become `112.6k token...`. Iso still showed `112.6k tokens`.
    #[test]
    fn subagents_list_omits_the_word_tokens() {
        let suffix = format_measured_tokens_suffix(112_600);
        let row = format_subagents_list_row_from_memory("Isolated Preview", Some(&suffix));
        assert!(
            row.contains("112.6k"),
            "Subagents list must paint 112.6k, got {row:?}"
        );
        assert!(
            !row.contains("tokens") && !row.contains("token"),
            "Subagents list omits the word tokens; truncation must not become 112.6k token...; got {row:?}"
        );
        assert_eq!(row, "Isolated Preview (112.6k)");
    }

    /// Live sampling wins over tracker high-water. A later smaller window
    /// (compact) must not leave a stale 90k leftover on the list.
    #[test]
    fn format_live_subagents_list_row_uses_live_sample_not_tracker_high_water() {
        on_nested_l2_spawn("nested-l2-live", "Residual");
        on_nested_l2_usage("nested-l2-live", 90_000);
        let stale = format_subagents_list_row_from_memory(
            "Residual",
            format_live_subagents_list_suffix("nested-l2-live", None).as_deref(),
        );
        assert!(
            stale.contains("90k"),
            "tracker fallback paints 90k before a live sample, got {stale:?}"
        );
        let live = format_subagents_list_row_from_memory(
            "Residual",
            format_live_subagents_list_suffix("nested-l2-live", Some(40_100)).as_deref(),
        );
        assert!(
            live.contains("40.1k"),
            "live sampling must paint 40.1k, got {live:?}"
        );
        assert!(
            !live.contains("90k"),
            "live sampling must not leave a stale 90k leftover, got {live:?}"
        );
        assert!(
            !live.contains("tokens"),
            "Subagents list omits the word tokens; got {live:?}"
        );
    }

    /// Operator: live-update from the current atomic token counters. Not a snapshot.
    #[test]
    fn subagents_list_and_compact_chrome_live_update_from_current_atomic_counters_not_a_frozen_snapshot()
     {
        let id = "nested-l2-live-atomic-paint";
        on_nested_l2_spawn(id, "Residual");
        let before = format_subagents_list_row_from_memory(
            "Residual",
            format_live_subagents_list_suffix(id, None).as_deref(),
        );
        on_nested_l2_usage(id, 90_000);
        let high_water = format_subagents_list_row_from_memory(
            "Residual",
            format_live_subagents_list_suffix(id, None).as_deref(),
        );
        assert!(
            high_water.contains("90k") && !before.contains("40.1k"),
            "Operator: live-update from the current atomic token counters. Not a snapshot. got {high_water:?}"
        );
        on_nested_l2_usage(id, 40_100);
        assert_eq!(
            current_live_sample(id),
            Some(40_100),
            "Operator: live-update from the current atomic token counters. Not a snapshot."
        );
        let compact = format_live_subagents_list_suffix(id, None);
        assert_eq!(
            compact.as_deref(),
            Some("40.1k"),
            "Operator: live-update from the current atomic token counters. Not a snapshot. got {compact:?}"
        );
        let live = format_subagents_list_row_from_memory("Residual", compact.as_deref());
        assert_eq!(
            live, "Residual (40.1k)",
            "Operator: live-update from the current atomic token counters. Not a snapshot. got {live:?}"
        );
        assert!(
            !live.contains("90.0k") && !live.contains("90k"),
            "Not a snapshot. compact suffix must paint 40.1k, not 90.0k, got {live:?}"
        );
        let high_water_tokens =
            peek_process_tracker(|t| t.get(id).map(|row| row.measured_tokens()).unwrap_or(0));
        assert_eq!(
            high_water_tokens, 90_000,
            "fetch_max high-water stays 90000 after compact"
        );
    }

    /// Operator contract: spawn, usage, and exit record one grok-oss sqlite
    /// row and do not create TECH.md. Measured tokens stay on that one row.
    #[test]
    #[serial_test::serial(TOKEN_ECONOMY_LIVE)]
    fn tech_md_write_records_measured_tokens_on_spawn_usage_tick_and_l2_exit() {
        let dir = std::env::temp_dir().join(format!(
            "grok-l2-token-tracking-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let tech_path = dir.join("TECH.md");
        let db_path = dir.join("grok_oss.db");
        let cwd_tech = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("TECH.md");
        let cwd_before = fs::read(&cwd_tech).ok();

        let cfg = xai_grok_shell::token_economy::TokenEconomyConfig {
            grok_oss_database_path: Some(db_path.clone()),
            ..xai_grok_shell::token_economy::TokenEconomyConfig::default()
        };
        xai_grok_shell::token_economy::set_token_economy_live(cfg);
        struct ResetLiveTokenEconomy;
        impl Drop for ResetLiveTokenEconomy {
            fn drop(&mut self) {
                xai_grok_shell::token_economy::reset_token_economy_live_to_defaults();
            }
        }
        let _reset_live_token_economy = ResetLiveTokenEconomy;

        let id = format!(
            "nested-l2-sqlite-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        on_nested_l2_spawn(&id, "occupancy extras writer");
        on_nested_l2_usage(&id, 12400);
        on_nested_l2_exit(&id);

        assert!(
            !tech_path.exists(),
            "the product must not create TECH.md at {tech_path:?}"
        );
        assert_eq!(
            fs::read(&cwd_tech).ok(),
            cwd_before,
            "the product must not write TECH.md"
        );

        let (event_ulid, work_ulid, measured, status) = peek_process_tracker(|t| {
            let row = t.get(&id).expect("spawned nested L2 row");
            (
                row.event_ulid.clone(),
                row.work_ulid.clone(),
                row.measured_tokens(),
                row.status,
            )
        });
        assert_eq!(measured, 12400, "in-memory atomic high-water stays 12400");
        assert_eq!(status, NestedL2Status::Exited);
        assert!(
            event_ulid.len() == 26 && work_ulid.len() == 26,
            "ULID session rows"
        );
        assert!(
            !event_ulid.contains('-') && !work_ulid.contains('-'),
            "ULID session rows are grok-oss ULIDs, not Grok Build UUIDs"
        );
        assert!(
            xai_grok_tools::util::ulid::is_valid(&event_ulid)
                && xai_grok_tools::util::ulid::is_valid(&work_ulid),
            "event and work ids must be ULIDs"
        );

        let store = xai_grok_shell::grok_oss::open_at(&db_path).expect("grok_oss.db");
        let rows =
            xai_grok_shell::token_economy::ledger::local_usage_events_for_session(&store, &id)
                .expect("read local_usage_event");
        assert_eq!(rows.len(), 1, "one nested row");
        let row = &rows[0];
        assert_eq!(row.event_ulid, event_ulid);
        assert_eq!(row.work_ulid.as_deref(), Some(work_ulid.as_str()));
        assert_eq!(row.agent_kind, "l2");
        assert_ne!(row.agent_kind.to_ascii_lowercase(), "l3");
        assert_eq!(
            row.total_tokens,
            Some(12400),
            "one sqlite row records the measured tokens"
        );

        let src = include_str!("l2_token_tracking.rs");
        let product = src.split("mod tests").next().expect("product before tests");
        let tracker_fn = fn_body(
            product,
            "fn with_process_tracker",
            "fn peek_process_tracker",
        );
        assert!(
            !tracker_fn.contains("persist_tech_md"),
            "live path must not write TECH.md"
        );
        assert!(
            !tracker_fn.contains("insert_local_usage_event"),
            "paint shares with_process_tracker and must not insert"
        );
        let hooks = fn_body(
            product,
            "pub fn on_nested_l2_spawn",
            "fn nested_l2_sqlite_pair",
        );
        assert_eq!(
            hooks.matches("insert_local_usage_event").count(),
            3,
            "spawn, usage, and exit each call insert_local_usage_event"
        );
        let paint = fn_body(
            product,
            "pub fn format_live_subagents_list_suffix",
            "pub const STANDING_WRAP_ESTIMATE_WALL",
        );
        assert!(
            !paint.contains("insert_local_usage_event"),
            "do not insert during paint"
        );
        assert!(
            product.contains("AtomicU64") && product.contains("fetch_max"),
            "keep the in-memory atomic counter"
        );
        assert!(
            !product.contains("fn render_tech_md")
                && !product.contains("fn persist_tech_md")
                && !product.contains("NOT_BILLING_METERS_SENTENCE"),
            "the product must not keep a TECH.md writer"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// In-memory `get` feeds the Subagents list row. Spawn and usage do not
    /// create TECH.md. One grok-oss sqlite row records the measured tokens.
    #[test]
    #[serial_test::serial(TOKEN_ECONOMY_LIVE)]
    fn at_workspace_root_tech_md_path_and_get_feed_subagents_list_row() {
        let dir = std::env::temp_dir().join(format!(
            "grok-l2-token-tracking-workspace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let tech_path = dir.join("TECH.md");
        let db_path = dir.join("grok_oss.db");
        let cwd_tech = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("TECH.md");
        let cwd_before = fs::read(&cwd_tech).ok();

        let cfg = xai_grok_shell::token_economy::TokenEconomyConfig {
            grok_oss_database_path: Some(db_path.clone()),
            ..xai_grok_shell::token_economy::TokenEconomyConfig::default()
        };
        xai_grok_shell::token_economy::set_token_economy_live(cfg);
        struct ResetLiveTokenEconomy;
        impl Drop for ResetLiveTokenEconomy {
            fn drop(&mut self) {
                xai_grok_shell::token_economy::reset_token_economy_live_to_defaults();
            }
        }
        let _reset_live_token_economy = ResetLiveTokenEconomy;

        let id = format!(
            "nested-l2-list-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        on_nested_l2_spawn(&id, "Stale prompt still live");
        on_nested_l2_usage(&id, 53407);

        let mut tracker = L2TokenTracker::default();
        tracker.record_spawn("nested-l2-session", "Stale prompt still live");
        tracker.record_usage("nested-l2-session", 53407);
        let measured = tracker
            .get("nested-l2-session")
            .expect("spawned nested L2 row")
            .measured_tokens();
        assert_eq!(measured, 53407);
        let suffix = tracker
            .format_subagents_list_token_suffix("nested-l2-session")
            .expect("suffix after usage tick");
        assert_eq!(suffix, format_measured_tokens_suffix(53407));
        assert_eq!(suffix, "53.4k");
        let row = format_subagents_list_row_from_memory("Stale prompt still live", Some(&suffix));
        assert_eq!(row, "Stale prompt still live (53.4k)");
        assert!(
            row.contains("53.4k"),
            "Subagents list row must contain 53.4k, got {row:?}"
        );
        assert!(
            !row.contains("tokens"),
            "Subagents list omits the word tokens; got {row:?}"
        );
        assert!(
            !row.contains("measured"),
            "Contract A: Subagents nested token chrome must not contain measured, got {row:?}"
        );
        assert!(
            !row.contains("53407"),
            "must not paint a raw integer token count, got {row:?}"
        );
        assert!(
            !tech_path.exists(),
            "the product must not create TECH.md at {tech_path:?}"
        );
        assert_eq!(
            fs::read(&cwd_tech).ok(),
            cwd_before,
            "the product must not write TECH.md"
        );

        let store = xai_grok_shell::grok_oss::open_at(&db_path).expect("grok_oss.db");
        let rows =
            xai_grok_shell::token_economy::ledger::local_usage_events_for_session(&store, &id)
                .expect("read local_usage_event");
        assert_eq!(rows.len(), 1, "one nested row");
        assert_eq!(rows[0].agent_kind, "l2");
        assert_eq!(
            rows[0].total_tokens,
            Some(53407),
            "one sqlite row records the measured tokens"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Operator contract: layout_must_not_read the session transcript jsonl.
    /// The Subagents list paint function takes the in-memory count only.
    /// Never open that transcript file in this module.
    #[test]
    fn subagents_list_layout_does_not_read_chat_history_jsonl() {
        let mut tracker = L2TokenTracker::default();
        tracker.record_spawn("id-only-in-memory", "reviewer");
        tracker.record_usage("id-only-in-memory", 42);
        // Unit-level: call the formatter with only the in-memory struct.
        // Signature has no chat history path.
        let row = format_subagents_list_row_from_memory(
            "reviewer",
            tracker
                .format_subagents_list_token_suffix("id-only-in-memory")
                .as_deref(),
        );
        assert!(row.contains("42"));
        assert!(
            !row.contains("tokens"),
            "Subagents list omits the word tokens; got {row:?}"
        );
        assert!(
            !row.contains("measured"),
            "Contract A: Subagents nested token chrome must not contain measured, got {row:?}"
        );
        let src = include_str!("l2_token_tracking.rs");
        let forbidden = concat!("chat_history", ".jsonl");
        let product = src.split("mod tests").next().expect("product before tests");
        assert!(
            !product.contains(forbidden),
            "product code in this module must never open {forbidden}"
        );
        assert!(
            !product.contains("std::fs::read")
                && !product.contains("File::open")
                && !product.contains("fn render_tech_md")
                && !product.contains("fn persist_tech_md"),
            "paint path must not open files and must not write TECH.md"
        );
    }

    /// Grok OSS: nested L2 token accumulator is AtomicU64 so concurrent ACP
    /// usage ticks do not race. This diverges from upstream xAI because Grok
    /// OSS Subagents list tracks nested L2 session usage in-memory
    /// (`l2_token_tracking`) and paints compact chrome. A racy last-write
    /// u64 can drop 10232 when a stale smaller tick lands last. Named tests
    /// are contracts: the high-water is 10232, chrome is `10.2k`, and
    /// the outcome must not be fitted to a racy last-write. The Subagents
    /// list omits the word tokens.
    #[test]
    fn concurrent_nested_l2_usage_ticks_keep_atomic_u64_high_water() {
        let src = include_str!("l2_token_tracking.rs");
        let product = src.split("mod tests").next().expect("product before tests");
        assert!(
            product.contains("AtomicU64"),
            "nested L2 token accumulator must be AtomicU64, not a racy u64"
        );
        assert!(
            product.contains("fetch_max"),
            "concurrent usage ticks must keep the high-water with fetch_max"
        );
        assert!(
            !product.contains("pub measured_tokens: u64"),
            "do not store nested L2 usage in a racy public u64"
        );

        let tracker = std::sync::Arc::new(std::sync::Mutex::new(L2TokenTracker::default()));
        {
            let mut t = tracker.lock().expect("spawn lock");
            t.record_spawn("nested-l2-session", "Atomic usage ticks");
        }
        const HIGH_WATER: u64 = 10232;
        const STALE: u64 = 8000;
        let mut joins = Vec::new();
        for i in 0..8 {
            let tracker = std::sync::Arc::clone(&tracker);
            joins.push(std::thread::spawn(move || {
                let tick = if i % 2 == 0 { HIGH_WATER } else { STALE };
                for _ in 0..64 {
                    let mut t = tracker.lock().expect("usage lock");
                    t.record_usage("nested-l2-session", tick);
                }
            }));
        }
        for j in joins {
            j.join().expect("usage thread");
        }
        let t = tracker.lock().expect("read lock");
        let measured = t
            .get("nested-l2-session")
            .expect("spawned nested L2 row")
            .measured_tokens();
        assert_eq!(
            measured, HIGH_WATER,
            "concurrent usage ticks must keep 10232, not a stale last-write like 8000"
        );
        let row = format_subagents_list_row_from_memory(
            "Atomic usage ticks",
            t.format_subagents_list_token_suffix("nested-l2-session")
                .as_deref(),
        );
        assert!(
            row.contains("10.2k"),
            "Subagents list must paint compact 10.2k after 10232, got {row:?}"
        );
        assert!(
            !row.contains("tokens"),
            "Subagents list omits the word tokens; got {row:?}"
        );
        assert!(
            !row.contains("measured"),
            "Contract A: Subagents nested token chrome must not contain measured, got {row:?}"
        );
        assert!(
            !row.contains("10232"),
            "must not paint a raw integer token count, got {row:?}"
        );
        assert!(
            !row.contains("8000"),
            "must not paint a stale concurrent tick, got {row:?}"
        );
        assert_eq!(format_measured_tokens_suffix(HIGH_WATER), "10.2k");
    }

    /// Operator: when the host has a real token count, the row shows that
    /// count. An estimate stays labeled as an estimate. When the host has
    /// no count, the row omits the token clause. Do not invent 167.0k.
    /// KernelLinearTheorems shows its host figure. Do not copy 2.6k / 500k.
    #[test]
    fn live_job_row_omits_the_token_clause_without_a_host_figure_and_shows_a_real_count() {
        const CONTRACT: &str = "When the host has a real token count, the row shows that count. An estimate stays labeled as an estimate and is not copied into the count. When the host has no count, the row omits the token clause. Do not print a placeholder. Do not invent 167.0k. A fetched host figure is shown. Do not copy 2.6k / 500k onto the row.";

        let sqlite_path = std::env::temp_dir().join(format!(
            "grok-oss-live-job-row-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_file(&sqlite_path);

        let id = "live-job-row-display-does-not-record";
        on_nested_l2_spawn(id, "Import-graph driver");
        let measured_before =
            peek_process_tracker(|t| t.get(id).map(|row| row.measured_tokens()).unwrap_or(0));

        let missing = display_live_job_row(LiveJobRowInput {
            job: "Import-graph driver",
            estimate_wall: STANDING_WRAP_ESTIMATE_WALL,
            estimate_tokens: STANDING_WRAP_ESTIMATE_TOKENS,
            elapsed: "19.4 minutes",
            host_tokens: None,
        });
        assert!(
            missing.actual_tokens.is_empty(),
            "{CONTRACT} omit the token clause when the host has no figure, got {:?}",
            missing.actual_tokens
        );
        assert!(
            !missing.actual_tokens.contains("not fetched")
                && !missing.actual_tokens.contains("not_fetched")
                && !missing.actual_tokens.contains("tokens not fetched"),
            "{CONTRACT} do not print a placeholder, got {:?}",
            missing.actual_tokens
        );
        let missing_row = format!(
            "{} · {} · {}{}",
            missing.estimate_wall, missing.estimate_tokens, missing.elapsed, missing.actual_tokens
        );
        assert!(
            !missing_row.contains("not fetched") && !missing_row.ends_with(" · "),
            "{CONTRACT} the row omits the token clause, got {missing_row:?}"
        );
        assert!(
            missing_row.contains("167.0k (estimate)")
                && missing_row.contains("19.4 minutes (estimate)")
                && missing_row.matches("167.0k").count() == 1,
            "{CONTRACT} 167.0k stays an estimate and is not copied into the count, got {missing_row:?}"
        );
        assert!(
            !missing_row.contains("2.6k") && !missing_row.contains("500k"),
            "{CONTRACT} do not copy the main-thread footer onto the row, got {missing_row:?}"
        );
        assert!(
            missing.estimate_wall.contains("estimate")
                && missing.estimate_wall.contains(STANDING_WRAP_ESTIMATE_WALL),
            "{CONTRACT} label the estimate, got {:?}",
            missing.estimate_wall
        );
        assert!(
            missing.estimate_tokens.contains("estimate")
                && missing
                    .estimate_tokens
                    .contains(STANDING_WRAP_ESTIMATE_TOKENS),
            "{CONTRACT} label the estimate, got {:?}",
            missing.estimate_tokens
        );
        assert!(
            !missing.actual_tokens.contains("19.4")
                && !missing.actual_tokens.contains("167.0")
                && missing.actual_tokens != STANDING_WRAP_ESTIMATE_WALL
                && missing.actual_tokens != STANDING_WRAP_ESTIMATE_TOKENS,
            "{CONTRACT} never copy 19.4 minutes or 167.0k into Actual tokens, got {:?}",
            missing.actual_tokens
        );
        assert_eq!(missing.l1_tokens_added, 0, "{CONTRACT}");
        assert!(!missing.wrote_grok_oss_sqlite, "{CONTRACT}");

        // 357_000 is evidence a fetch can work (KernelLinearTheorems, 357.0k).
        // It is not the only success value. Another host figure must show too.
        let evidence = 357_000_u64;
        let also_fetched = 112_400_u64;
        let evidence_row = display_live_job_row(LiveJobRowInput {
            job: "KernelLinearTheorems",
            estimate_wall: STANDING_WRAP_ESTIMATE_WALL,
            estimate_tokens: STANDING_WRAP_ESTIMATE_TOKENS,
            elapsed: "finished",
            host_tokens: Some(evidence),
        });
        let other_row = display_live_job_row(LiveJobRowInput {
            job: "another fetched row",
            estimate_wall: STANDING_WRAP_ESTIMATE_WALL,
            estimate_tokens: STANDING_WRAP_ESTIMATE_TOKENS,
            elapsed: "finished",
            host_tokens: Some(also_fetched),
        });
        assert_eq!(
            evidence_row.actual_tokens,
            format_measured_tokens_suffix(evidence),
            "{CONTRACT} show the fetched host figure, got {:?}",
            evidence_row.actual_tokens
        );
        assert_eq!(
            other_row.actual_tokens,
            format_measured_tokens_suffix(also_fetched),
            "{CONTRACT} 357.0k is not the only success value, got {:?}",
            other_row.actual_tokens
        );
        assert_ne!(
            evidence_row.actual_tokens, other_row.actual_tokens,
            "{CONTRACT} do not hardcode one success string"
        );
        assert!(!evidence_row.actual_tokens.is_empty());
        assert!(!other_row.actual_tokens.is_empty());
        assert!(
            !evidence_row.actual_tokens.contains("not fetched")
                && !other_row.actual_tokens.contains("not fetched"),
            "{CONTRACT} a real count is not a placeholder"
        );
        let evidence_clause = format!(" · {}", evidence_row.actual_tokens);
        assert_eq!(
            evidence_clause,
            format!(" · {}", format_measured_tokens_suffix(evidence)),
            "{CONTRACT} a real count stays on the row, got {evidence_clause:?}"
        );
        assert!(
            !evidence_clause.contains("2.6k") && !evidence_clause.contains("500k"),
            "{CONTRACT} do not copy the main-thread footer onto the row"
        );
        assert!(
            !evidence_row.actual_tokens.contains("19.4")
                && evidence_row.actual_tokens != STANDING_WRAP_ESTIMATE_TOKENS,
            "{CONTRACT} do not copy the estimate into Actual tokens, got {:?}",
            evidence_row.actual_tokens
        );
        assert!(
            evidence_row.estimate_tokens.contains("estimate")
                && other_row.estimate_wall.contains("estimate"),
            "{CONTRACT} a fetched row still labels the estimate as an estimate"
        );

        let mut goal = crate::app::agent::GoalDisplayState::test_stub();
        goal.status = crate::app::agent::GoalDisplayStatus::Active;
        goal.tokens_used = 1_000;
        goal.token_baseline = 100;
        goal.finished_subagent_tokens = 50;
        let context = Some(270_000_u64);
        let l1_before = goal.live_tokens_used(context, 0);
        let l1_after_display = goal.live_tokens_used(context, evidence_row.l1_tokens_added);
        let l1_if_caller_passed_the_row_figure = goal.live_tokens_used(context, evidence);
        assert_eq!(
            l1_before, l1_after_display,
            "{CONTRACT} displaying the row does not add that figure again into the L1 total"
        );
        assert_eq!(
            l1_before, l1_if_caller_passed_the_row_figure,
            "{CONTRACT} the row figure stays off the L1 total"
        );
        assert_eq!(goal.tokens_used, 1_000, "{CONTRACT}");
        assert_eq!(goal.token_baseline, 100, "{CONTRACT}");
        assert_eq!(goal.finished_subagent_tokens, 50, "{CONTRACT}");

        let measured_after =
            peek_process_tracker(|t| t.get(id).map(|row| row.measured_tokens()).unwrap_or(0));
        assert_eq!(
            measured_before, measured_after,
            "{CONTRACT} displaying the row does not record the figure again"
        );
        assert!(
            !sqlite_path.exists(),
            "{CONTRACT} displaying the row does not write grok-oss sqlite at {sqlite_path:?}"
        );

        let src = include_str!("l2_token_tracking.rs");
        let product = src.split("mod tests").next().expect("product before tests");
        let start = product
            .find("pub fn display_live_job_row")
            .expect("display fn");
        let fn_src = &product[start..];
        assert!(
            !fn_src.contains("fn render_tech_md") && !fn_src.contains("persist_tech_md"),
            "{CONTRACT} display must not write TECH.md"
        );
        assert!(
            !fn_src.contains("rusqlite")
                && !fn_src.contains("Connection::open")
                && !fn_src.contains("fs::write")
                && !fn_src.contains(".execute("),
            "{CONTRACT} display must not write grok-oss sqlite"
        );
        assert!(
            !fn_src.contains("on_nested_l2_usage")
                && !fn_src.contains("finished_subagent_tokens")
                && !fn_src.contains("token_baseline"),
            "{CONTRACT} display must not add the row figure into the L1 total"
        );
        assert!(
            !fn_src.contains("not fetched") && !fn_src.contains("tokens not fetched"),
            "{CONTRACT} the row formatter must not paint a placeholder"
        );
    }

    /// Operator: when the host has a real token count, the row shows that
    /// count. When the host has no count, the row omits the token clause.
    /// An estimate stays labeled as an estimate. Do not invent 167.0k.
    ///
    /// The tasks pane row that `TasksPane::render` paints, and the `/tasks`
    /// block, must call [`display_live_job_row`] and use the returned text.
    #[test]
    fn live_job_row_paint_path_calls_display_live_job_row() {
        const CONTRACT: &str = "When the host has a real token count, the row shows that count. An estimate stays labeled as an estimate and is not copied into the count. When the host has no count, the row omits the token clause. Do not print a placeholder. Do not invent 167.0k. A fetched host figure is shown. Do not copy 2.6k / 500k onto the row.";

        let tasks_src = include_str!("../../views/tasks_pane.rs");
        let tasks_fn = fn_body(
            tasks_src,
            "fn from_subagent_with_l3_count",
            "\n    fn from_workflow_run",
        );
        assert_paint_calls_formatter("tasks pane", tasks_fn, CONTRACT);

        let tasks_block = include_str!("../status_blocks.rs");
        let block_fn = fn_body(
            tasks_block,
            "pub(crate) fn tasks_block_text",
            "\npub(crate) fn session_usage_block_text",
        );
        assert_paint_calls_formatter("/tasks block", block_fn, CONTRACT);
    }

    fn fn_body<'a>(src: &'a str, start_needle: &str, end_needle: &str) -> &'a str {
        let start = src.find(start_needle).expect("paint function");
        let rest = &src[start..];
        let end = rest.find(end_needle).expect("following function");
        &rest[..end]
    }

    fn assert_paint_calls_formatter(surface: &str, paint_fn: &str, contract: &str) {
        assert!(
            paint_fn.contains("display_live_job_row"),
            "{contract} {surface} must call display_live_job_row"
        );
        assert!(
            paint_fn.contains("shown.actual_tokens")
                && paint_fn.contains("shown.estimate_wall")
                && paint_fn.contains("shown.estimate_tokens"),
            "{contract} {surface} must use the returned actual tokens and the labeled estimate"
        );
        assert!(
            !paint_fn.contains("not fetched") && !paint_fn.contains("tokens not fetched"),
            "{contract} {surface} must not paint a placeholder"
        );
        assert!(
            paint_fn.contains("subagent_list_row_usage"),
            "{contract} {surface} must take the host figure, not invent one"
        );
        assert!(
            !paint_fn.contains("357.0k")
                && !paint_fn.contains("357_000")
                && !paint_fn.contains("357000"),
            "{contract} {surface} must not hardcode 357.0k"
        );
        assert!(
            !paint_fn.contains("live_tokens_used")
                && !paint_fn.contains("on_nested_l2_usage")
                && !paint_fn.contains("rusqlite")
                && !paint_fn.contains("finished_subagent_tokens"),
            "{contract} {surface} must not add the row figure to the L1 total or grok-oss sqlite"
        );
    }
}
