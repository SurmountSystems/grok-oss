//! In-memory nested L2 token counts and TECH.md render.
//!
//! Subagents list layout reads only this in-memory map. This module never
//! opens session transcript files on paint. Measured nested L2 tokens are
//! session usage counts, not included SuperGrok period limits, not SuperGrok
//! dollar credits, and not console team prepaid / console API credits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Default TECH.md filename at the workspace root.
pub const TECH_MD_FILENAME: &str = "TECH.md";

/// Suffix shown on a Subagents list row after a usage tick.
pub const MEASURED_TOKENS_SUFFIX_PREFIX: &str = "measured";
pub const MEASURED_TOKENS_SUFFIX_UNIT: &str = "tokens";

/// Billing-truth sentence required in TECH.md (complete thought).
pub const NOT_BILLING_METERS_SENTENCE: &str = "Measured nested L2 tokens are session usage counts, not included SuperGrok period limits, not SuperGrok dollar credits, and not console team prepaid / console API credits. SuperGrok is a paid product. Estimates are estimates, not billing truth.";

/// One nested L2 row tracked in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedL2Tokens {
    /// Nested session id (internal key; not dumped in Subagents list text).
    pub nested_session_id: String,
    /// Operator-facing description label for the Subagents list and TECH.md tree.
    pub description: String,
    /// Measured session usage token count for this nested L2.
    pub measured_tokens: u64,
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
            measured_tokens: 0,
            estimate_tokens: None,
            owner: "L2".to_string(),
            contract_aspect: "nested L2 session usage".to_string(),
            status: NestedL2Status::Spawned,
        }
    }
}

/// In-memory nested L2 token counts keyed by nested session id.
#[derive(Debug, Clone)]
pub struct L2TokenTracker {
    by_id: HashMap<String, NestedL2Tokens>,
    /// TECH.md path. Tests inject a temp file. Production uses workspace root.
    tech_md_path: PathBuf,
}

impl Default for L2TokenTracker {
    fn default() -> Self {
        Self::with_tech_md_path(default_tech_md_path())
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

fn process_tracker() -> &'static Mutex<L2TokenTracker> {
    static TRACKER: OnceLock<Mutex<L2TokenTracker>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(L2TokenTracker::default()))
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
    pub fn record_usage(&mut self, nested_session_id: &str, measured_tokens: u64) {
        if let Some(row) = self.by_id.get_mut(nested_session_id) {
            row.measured_tokens = measured_tokens;
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
        let row = self.by_id.get(nested_session_id)?;
        Some(format_measured_tokens_suffix(row.measured_tokens))
    }

    /// Paint a Subagents list row from in-memory counts only.
    ///
    /// Operator contract: layout must not read the session transcript file.
    /// This function takes the in-memory tracker and a description label. It
    /// does not take a chat history path.
    pub fn format_subagents_list_row(&self, nested_session_id: &str, description: &str) -> String {
        format_subagents_list_row_from_memory(
            description,
            self.format_subagents_list_token_suffix(nested_session_id)
                .as_deref(),
        )
    }

    /// Render TECH.md and write it to the injected path.
    pub fn persist_tech_md(&self) -> std::io::Result<()> {
        let body = render_tech_md(&self.by_id);
        if let Some(parent) = self.tech_md_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.tech_md_path, body)
    }
}

/// Format `measured 12400 tokens` (plain English).
pub fn format_measured_tokens_suffix(measured_tokens: u64) -> String {
    format!("{MEASURED_TOKENS_SUFFIX_PREFIX} {measured_tokens} {MEASURED_TOKENS_SUFFIX_UNIT}")
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
                measured = row.measured_tokens,
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

    /// Operator contract: Subagents list shows measured tokens per nested L2.
    /// After spawn + usage tick of 12400, the row contains `measured 12400 tokens`.
    #[test]
    fn subagents_list_shows_measured_tokens_per_nested_l2() {
        let mut tracker = L2TokenTracker::with_tech_md_path("/tmp/unused-tech.md");
        tracker.record_spawn("nested-l2-session", "rate-limit implementer");
        tracker.record_usage("nested-l2-session", 12400);
        let row = tracker.format_subagents_list_row("nested-l2-session", "rate-limit implementer");
        assert!(
            row.contains("measured 12400 tokens"),
            "Subagents list row must contain measured 12400 tokens, got {row:?}"
        );
        assert!(
            !row.contains("nested-l2-session") || row.contains("rate-limit implementer"),
            "row uses the description label, not UUID speech as the visible name"
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
        assert!(row.contains("measured 42 tokens"));
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
}
