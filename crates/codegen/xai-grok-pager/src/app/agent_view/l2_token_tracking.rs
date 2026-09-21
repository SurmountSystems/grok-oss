//! In-memory nested L2 token counts and TECH.md render.
//!
//! Subagents list layout reads only this in-memory map. This module never
//! opens session transcript files on paint. Measured nested L2 tokens are
//! session usage counts, not included SuperGrok period limits, not SuperGrok
//! dollar credits, and not console team prepaid / console API credits.
//! Operator-visible Subagents list chrome omits the word `tokens`. The unit
//! is implicit (`90k`, `112.9k`). Each nested session id is its own window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Default TECH.md filename at the workspace root.
pub const TECH_MD_FILENAME: &str = "TECH.md";

/// Billing-truth sentence required in TECH.md (complete thought).
pub const NOT_BILLING_METERS_SENTENCE: &str = "Measured nested L2 tokens are session usage counts, not included SuperGrok period limits, not SuperGrok dollar credits, and not console team prepaid / console API credits. SuperGrok is a paid product. Estimates are estimates, not billing truth.";

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
    /// Operator-facing description label for the Subagents list and TECH.md tree.
    pub description: String,
    /// High-water nested L2 session usage. Not included SuperGrok period
    /// limits, SuperGrok dollar credits, or console team prepaid.
    measured_tokens: AtomicU64,
    /// Optional estimate (not billing truth).
    pub estimate_tokens: Option<u64>,
    /// Owner of the contract/aspect row in TECH.md.
    pub owner: String,
    /// Contract or aspect name for the TECH.md table.
    pub contract_aspect: String,
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

impl NestedL2Status {
    fn as_str(self) -> &'static str {
        match self {
            NestedL2Status::Spawned => "spawned",
            NestedL2Status::Running => "running",
            NestedL2Status::Exited => "exited",
        }
    }
}

impl NestedL2Tokens {
    fn new(nested_session_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            nested_session_id: nested_session_id.into(),
            description: description.into(),
            measured_tokens: AtomicU64::new(0),
            estimate_tokens: None,
            owner: "L2".to_string(),
            contract_aspect: "nested L2 session usage".to_string(),
            status: NestedL2Status::Spawned,
        }
    }

    /// Session usage high-water for this nested L2 (not billing meters).
    pub fn measured_tokens(&self) -> u64 {
        self.measured_tokens.load(Ordering::Relaxed)
    }
}

impl Clone for NestedL2Tokens {
    fn clone(&self) -> Self {
        Self {
            nested_session_id: self.nested_session_id.clone(),
            description: self.description.clone(),
            measured_tokens: AtomicU64::new(self.measured_tokens()),
            estimate_tokens: self.estimate_tokens,
            owner: self.owner.clone(),
            contract_aspect: self.contract_aspect.clone(),
            status: self.status,
        }
    }
}

impl PartialEq for NestedL2Tokens {
    fn eq(&self, other: &Self) -> bool {
        self.nested_session_id == other.nested_session_id
            && self.description == other.description
            && self.measured_tokens() == other.measured_tokens()
            && self.estimate_tokens == other.estimate_tokens
            && self.owner == other.owner
            && self.contract_aspect == other.contract_aspect
            && self.status == other.status
    }
}

impl Eq for NestedL2Tokens {}

/// In-memory nested L2 token counts keyed by nested session id.
#[derive(Debug, Clone)]
pub struct L2TokenTracker {
    by_id: HashMap<String, NestedL2Tokens>,
    /// TECH.md path. Tests inject a temp file. Production uses workspace root.
    tech_md_path: PathBuf,
}

impl Default for L2TokenTracker {
    fn default() -> Self {
        production_tracker()
    }
}

/// Workspace-root TECH.md, or `GROK_TECH_MD_PATH` when tests/production inject a path.
pub fn default_tech_md_path() -> PathBuf {
    if let Some(p) = std::env::var_os("GROK_TECH_MD_PATH") {
        return PathBuf::from(p);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(TECH_MD_FILENAME)
}

/// Production TECH.md tracker: workspace-root path, or `GROK_TECH_MD_PATH`.
fn production_tracker() -> L2TokenTracker {
    if let Some(p) = std::env::var_os("GROK_TECH_MD_PATH") {
        L2TokenTracker::with_tech_md_path(PathBuf::from(p))
    } else {
        match std::env::current_dir() {
            Ok(root) => L2TokenTracker::at_workspace_root(root),
            Err(_) => L2TokenTracker::with_tech_md_path(default_tech_md_path()),
        }
    }
}

fn process_tracker() -> &'static Mutex<L2TokenTracker> {
    static TRACKER: OnceLock<Mutex<L2TokenTracker>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(production_tracker()))
}

fn with_process_tracker(f: impl FnOnce(&mut L2TokenTracker)) {
    let mut tracker = process_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut tracker);
    // cargo test must not write repo-root TECH.md. Production persist is the binary.
    #[cfg(not(test))]
    {
        let _ = tracker.persist_tech_md();
    }
}

fn peek_process_tracker<R>(f: impl FnOnce(&L2TokenTracker) -> R) -> R {
    let tracker = process_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&tracker)
}

/// Production spawn hook. Subagents list paint still uses in-memory counts only.
pub fn on_nested_l2_spawn(nested_session_id: &str, description: &str) {
    with_process_tracker(|t| t.record_spawn(nested_session_id, description));
}

/// Production usage-tick hook (session usage tokens, not billing meters).
pub fn on_nested_l2_usage(nested_session_id: &str, measured_tokens: u64) {
    with_process_tracker(|t| t.record_usage(nested_session_id, measured_tokens));
}

/// Production L2-exit hook. Keeps the last measured count for TECH.md.
pub fn on_nested_l2_exit(nested_session_id: &str) {
    with_process_tracker(|t| t.record_exit(nested_session_id));
}

impl L2TokenTracker {
    /// Production default: `workspace_root/TECH.md`.
    pub fn at_workspace_root(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            by_id: HashMap::new(),
            tech_md_path: workspace_root.as_ref().join(TECH_MD_FILENAME),
        }
    }

    /// Tests inject a temp TECH.md path.
    pub fn with_tech_md_path(tech_md_path: impl Into<PathBuf>) -> Self {
        Self {
            by_id: HashMap::new(),
            tech_md_path: tech_md_path.into(),
        }
    }

    pub fn tech_md_path(&self) -> &Path {
        &self.tech_md_path
    }

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
    /// sampling window (`context_tokens_used`). `fetch_max` keeps a TECH.md
    /// high-water so concurrent ticks cannot lose a later count. Subagents
    /// list paint uses the live sample passed into
    /// [`format_live_subagents_list_suffix`], not this high-water, so a later
    /// compact cannot leave a stale 90k leftover.
    pub fn record_usage(&mut self, nested_session_id: &str, measured_tokens: u64) {
        if let Some(row) = self.by_id.get_mut(nested_session_id) {
            let _previous = row
                .measured_tokens
                .fetch_max(measured_tokens, Ordering::Relaxed);
            if row.status == NestedL2Status::Spawned {
                row.status = NestedL2Status::Running;
            }
        }
    }

    /// Record nested L2 exit. Keeps the last measured count for TECH.md.
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

    /// Render TECH.md and write it to the injected path.
    pub fn persist_tech_md(&self) -> std::io::Result<()> {
        let body = render_tech_md(&self.by_id);
        let path = self.tech_md_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)
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

/// Compact suffix for a nested session window.
///
/// Live `SubagentProgress` (`tokens_used`) wins over the tracker high-water
/// so compact cannot leave a stale leftover. Falls back to the tracker when
/// this nested id has no live sample yet. Does not open the session
/// transcript file. Never the word `tokens`.
pub fn format_live_subagents_list_suffix(
    nested_session_id: &str,
    tokens_used: Option<u64>,
) -> Option<String> {
    match tokens_used {
        Some(live) => Some(format_measured_tokens_suffix(live)),
        None => peek_process_tracker(|t| t.format_subagents_list_token_suffix(nested_session_id)),
    }
}

fn render_tech_md(by_id: &HashMap<String, NestedL2Tokens>) -> String {
    let mut out = String::new();
    out.push_str("# Nested L2 token tracking\n\n");
    out.push_str(NOT_BILLING_METERS_SENTENCE);
    out.push_str("\n\n");
    out.push_str("## Dependency tree\n\n");
    out.push_str("- L1 main session\n");
    let mut rows: Vec<&NestedL2Tokens> = by_id.values().collect();
    rows.sort_by(|a, b| {
        a.description
            .cmp(&b.description)
            .then(a.nested_session_id.cmp(&b.nested_session_id))
    });
    if rows.is_empty() {
        out.push_str("  - (no nested L2 sessions)\n");
    } else {
        for row in &rows {
            let label = if row.description.is_empty() {
                "nested L2"
            } else {
                row.description.as_str()
            };
            out.push_str(&format!("  - L2 {label}\n"));
            out.push_str("    - L3 specialists (when spawned)\n");
        }
    }
    out.push('\n');
    out.push_str("## Nested L2 session usage\n\n");
    out.push_str("| id | contract/aspect | owner | measured tokens | estimate | status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    if rows.is_empty() {
        out.push_str("| - | - | - | - | - | - |\n");
    } else {
        for row in &rows {
            let estimate = row
                .estimate_tokens
                .map(|n| n.to_string())
                .unwrap_or_else(|| "estimate".to_string());
            // Table id is the description label, not UUID speech, when a description exists.
            let table_id = if row.description.is_empty() {
                row.nested_session_id.as_str()
            } else {
                row.description.as_str()
            };
            out.push_str(&format!(
                "| {table_id} | {aspect} | {owner} | {measured} | {estimate} | {status} |\n",
                aspect = row.contract_aspect,
                owner = row.owner,
                measured = row.measured_tokens(),
                status = row.status.as_str(),
            ));
        }
    }
    out.push('\n');
    out
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
        let mut tracker = L2TokenTracker::with_tech_md_path("/tmp/unused-tech.md");
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

    /// Operator contract: TECH.md write records measured tokens on spawn,
    /// usage tick, and L2 exit. File contains the measured number, a
    /// dependency tree, a table with columns id | contract/aspect | owner |
    /// measured tokens | estimate | status, and the sentence that this is
    /// not included SuperGrok period limits / SuperGrok dollar credits /
    /// console credits.
    #[test]
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
        let mut tracker = L2TokenTracker::with_tech_md_path(&tech_path);
        tracker.record_spawn("l2-a", "occupancy extras writer");
        tracker.persist_tech_md().expect("write after spawn");
        tracker.record_usage("l2-a", 12400);
        tracker.persist_tech_md().expect("write after usage tick");
        tracker.record_exit("l2-a");
        tracker.persist_tech_md().expect("write after L2 exit");

        let body = fs::read_to_string(&tech_path).expect("read TECH.md");
        assert!(
            body.contains("12400"),
            "TECH.md must contain the measured token number, got {body}"
        );
        assert!(
            body.contains("L1") && body.contains("L2") && body.contains("L3"),
            "TECH.md must contain the L1 -> L2 -> L3 dependency tree, got {body}"
        );
        assert!(
            body.contains("| id | contract/aspect | owner | measured tokens | estimate | status |"),
            "TECH.md must have the aspect table columns, got {body}"
        );
        assert!(
            body.contains("not included SuperGrok period limits")
                && body.contains("not SuperGrok dollar credits")
                && body.contains("console team prepaid / console API credits"),
            "TECH.md must say measured tokens are not billing meters, got {body}"
        );
        assert!(
            body.contains("exited"),
            "TECH.md after L2 exit must record exited status, got {body}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Workspace-root constructor, TECH.md path getter, and in-memory `get`
    /// feed the same Subagents list row formatter as live chrome. Direct
    /// calls so lib-test `-D dead-code` sees `at_workspace_root`,
    /// `tech_md_path`, and `get` even when other tests use a temp path.
    #[test]
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
        let mut tracker = L2TokenTracker::at_workspace_root(&dir);
        assert_eq!(tracker.tech_md_path(), dir.join(TECH_MD_FILENAME).as_path());
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
        let _ = fs::remove_dir_all(&dir);
    }

    /// Operator contract: layout_must_not_read the session transcript jsonl.
    /// The Subagents list paint function takes the in-memory count only.
    /// Never open that transcript file in this module.
    #[test]
    fn subagents_list_layout_does_not_read_chat_history_jsonl() {
        let mut tracker = L2TokenTracker::with_tech_md_path("/tmp/unused-tech.md");
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
            !product.contains("std::fs::read") && !product.contains("File::open"),
            "paint path must not open files; persist_tech_md writes TECH.md only"
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

        let tracker = std::sync::Arc::new(std::sync::Mutex::new(
            L2TokenTracker::with_tech_md_path("/tmp/unused-tech-atomic.md"),
        ));
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
}
