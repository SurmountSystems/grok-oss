//! Rebuild Grok OSS from a local source tree and soft-relaunch live processes.
//!
//! This is the product path for `/rebuild` and `grok-oss rebuild`. It does
//! **not** use the SpaceXAI auto-updater channel. Install default is
//! `just install` → `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss`.
//!
//! Identity SHA in `version (sha)` is a **git object id**, not a SHA-1
//! security hash of a downloaded artifact. Verify is `binary --version`.
//! Failed verify must not signal peers.
//!
//! After install it:
//! 1. Soft-signals reachable leaders (`RelaunchForUpdate`).
//! 2. Writes a cooperative rebuild-relaunch request under `$GROK_HOME`.
//! 3. Nudges **all** other live product TUI PIDs in `active_sessions` with
//!    `SIGUSR1` so they re-exec onto the new binary (same session), not only
//!    the window that typed `/rebuild`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use xai_grok_active_sessions::{self as active_sessions, ActiveSession};
use xai_grok_shell::leader::{self, LeaderRelaunchOutcome};

/// Package that produces the `grok-oss` binary.
const PAGER_BIN_PACKAGE: &str = "xai-grok-pager-bin";

/// Relative path markers that identify this workspace root.
const JUSTFILE_NAME: &str = "justfile";
const PAGER_BIN_MANIFEST: &str = "crates/codegen/xai-grok-pager-bin/Cargo.toml";

/// Disk file under `$GROK_HOME` that tells peer TUIs to re-exec after rebuild.
const REBUILD_RELAUNCH_REQUEST_FILENAME: &str = "rebuild_relaunch_request.json";

/// Ignore requests older than this so a stale file cannot thrash forever.
const REBUILD_RELAUNCH_REQUEST_MAX_AGE_SECS: u64 = 15 * 60;

/// Summary of one rebuild + relaunch attempt (for CLI, slash scrollback, tests).
#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub source_root: PathBuf,
    pub installed_path: PathBuf,
    /// Full identity when known, e.g. `0.1.100 (abc123)`.
    pub installed_identity: String,
    pub install_backend: InstallBackend,
    pub leader_outcomes: Vec<LeaderRelaunchOutcome>,
    /// Outcomes of cooperative peer TUI relaunch signals (`SIGUSR1`).
    pub peer_outcomes: Vec<PeerRelaunchOutcome>,
    /// Alive active_sessions rows after optional crash hygiene.
    pub live_sessions: Vec<ActiveSession>,
    /// Lines suitable for operator scrollback / stdout.
    pub summary_lines: Vec<String>,
}

/// Outcome of asking one live active-session process to re-exec for rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerRelaunchOutcome {
    /// `SIGUSR1` (or platform equivalent) delivered.
    Signaled { pid: u32, session_id: String },
    /// Skipped (self, not grok, dead, or signal error).
    Skipped {
        pid: u32,
        session_id: String,
        reason: String,
    },
}

/// Cooperative request so peer TUIs re-exec onto the newly installed binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildRelaunchRequest {
    /// Absolute path of the installed `grok-oss` binary.
    pub installed_exe: PathBuf,
    /// Full identity, e.g. `0.1.100 (abc123)`.
    pub installed_identity: String,
    /// Unix epoch seconds when the request was written.
    pub requested_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallBackend {
    JustInstall,
    CargoFixedArgv,
}

impl InstallBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::JustInstall => "just install",
            Self::CargoFixedArgv => "cargo build + install (fixed argv)",
        }
    }
}

/// Walk from `start` upward until a checkout with this repo's install recipe
/// is found (`justfile` + `crates/codegen/xai-grok-pager-bin/Cargo.toml`).
pub fn resolve_source_root(start: &Path) -> Result<PathBuf> {
    let mut cur = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().context("resolve cwd")?.join(start)
    };
    if let Ok(canon) = dunce::canonicalize(&cur) {
        cur = canon;
    }
    loop {
        if looks_like_grok_oss_root(&cur) {
            return Ok(cur);
        }
        if !cur.pop() {
            bail!(
                "Could not find a Grok OSS source tree (need `{JUSTFILE_NAME}` and \
                 `{PAGER_BIN_MANIFEST}`) walking up from {}. \
                 Run from a checkout of this repo, or `cd` there first.",
                start.display()
            );
        }
    }
}

fn looks_like_grok_oss_root(dir: &Path) -> bool {
    dir.join(JUSTFILE_NAME).is_file() && dir.join(PAGER_BIN_MANIFEST).is_file()
}

/// Default install destination for `just install` / cargo install path.
pub fn default_install_path() -> PathBuf {
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".cargo")
        });
    cargo_home.join("bin").join("grok-oss")
}

/// Whether `just` is available on PATH.
pub fn just_available() -> bool {
    Command::new("just")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// How install child processes attach stdio.
///
/// **Capture is mandatory** for both TUI `/rebuild` and CLI `grok-oss rebuild`.
/// Inheriting the parent TTY while ratatui owns the alt-screen destroys footer
/// / composer layout (raw cargo ANSI, `\r` progress bars, multi-line just
/// echoes paint over the TUI). Progress must go through a sanitized single-line
/// callback into TUI toast / CLI println, never raw child bytes on the PTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStdioPolicy {
    /// Pipe stdout and stderr; never inherit the parent TTY.
    Capture,
}

/// Contract: product install always captures. There is no Inherit path.
pub fn install_stdio_policy() -> InstallStdioPolicy {
    InstallStdioPolicy::Capture
}

/// Max length of a TUI/CLI progress line after sanitize (stable footer height).
pub const REBUILD_PROGRESS_LINE_MAX_CHARS: usize = 160;

// ---------------------------------------------------------------------------
// Structured rebuild progress (weighted stages + cargo compile counts)
// ---------------------------------------------------------------------------

/// One progress sample for `/rebuild` and `grok-oss rebuild`.
///
/// `fraction` is overall job progress in `0.0..=1.0` (clamped). Prefer
/// monotonic advances; stage boundaries may jump forward but must not go
/// backwards.
#[derive(Debug, Clone, PartialEq)]
pub struct RebuildProgressEvent {
    /// Overall progress `0.0..=1.0`.
    pub fraction: f32,
    /// Human stage text, e.g. `Compiling xai-grok-pager (12 packages so far)`.
    pub detail: String,
}

/// Pipeline stage weights (overall job, not wall-clock). Cargo compile is most
/// of the bar; strip / install / verify / leaders are short fixed segments.
pub mod rebuild_progress_weights {
    pub const RESOLVE: f32 = 0.02;
    pub const INSTALL_START: f32 = 0.05;
    /// Start of the cargo compile segment (just after install recipe starts).
    pub const CARGO_START: f32 = 0.05;
    /// End of cargo (after `Finished` / last artifact).
    pub const CARGO_END: f32 = 0.88;
    pub const STRIP: f32 = 0.91;
    pub const INSTALL_BIN: f32 = 0.95;
    pub const VERIFY: f32 = 0.97;
    pub const LEADERS: f32 = 0.99;
    pub const DONE: f32 = 1.0;
}

/// Clamp a progress fraction into `0.0..=1.0`.
pub fn clamp_rebuild_fraction(value: f32) -> f32 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

/// Pure: overall fraction for a cargo sub-progress `0.0..=1.0` inside the
/// cargo weight window.
pub fn overall_fraction_in_cargo(cargo_sub: f32) -> f32 {
    use rebuild_progress_weights::{CARGO_END, CARGO_START};
    let sub = clamp_rebuild_fraction(cargo_sub);
    clamp_rebuild_fraction(CARGO_START + (CARGO_END - CARGO_START) * sub)
}

/// Pure: map compiled crate count → cargo sub-fraction (`0.0..=1.0`).
///
/// Real crate counts drive the bar. Until cargo reports `Finished`, the bar
/// approaches but does not hit the end of the cargo segment (`0.98` max).
/// Soft denominator grows with `compiled` so early packages still move the
/// bar without inventing a time-based fake.
pub fn cargo_sub_fraction(compiled: usize, finished: bool) -> f32 {
    if finished {
        return 1.0;
    }
    if compiled == 0 {
        return 0.0;
    }
    // Soft room ahead: assumes a handful more packages may still compile.
    let denom = (compiled + 8) as f32;
    (compiled as f32 / denom).min(0.98)
}

/// Parse a human cargo `Compiling <name> ...` line → crate name.
pub fn parse_compiling_crate(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t.strip_prefix("Compiling ")?;
    let name = rest.split_whitespace().next()?;
    if name.is_empty() {
        return None;
    }
    Some(name)
}

/// Parse a cargo `--message-format=json` line for a compiler-artifact package
/// name (best-effort, no full serde model required).
pub fn parse_cargo_json_artifact_package(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('{') || !t.contains("\"compiler-artifact\"") {
        return None;
    }
    // Prefer target.name, fall back to package_id leaf.
    if let Some(name) = json_string_field(t, "\"name\"") {
        // target block usually appears after "target":{..."name":"pkg"
        // Heuristic: last "name" inside the line that is not a file name is ok
        // for progress; prefer the one after "target".
        if let Some(idx) = t.find("\"target\"") {
            if let Some(name_after) = json_string_field(&t[idx..], "\"name\"") {
                return Some(name_after);
            }
        }
        return Some(name);
    }
    None
}

/// Whether a cargo json line is `build-finished`.
pub fn is_cargo_json_build_finished(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('{') && t.contains("\"build-finished\"")
}

fn json_string_field(blob: &str, key: &str) -> Option<String> {
    let start = blob.find(key)? + key.len();
    let after = blob.get(start..)?;
    let colon = after.find(':')?;
    let mut rest = after[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    rest = &rest[1..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
            continue;
        }
        if c == '"' {
            break;
        }
        out.push(c);
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Format a single-line CLI progress bar: `[████░░░░]  42%  detail`.
pub fn format_rebuild_cli_progress(fraction: f32, detail: &str, bar_width: usize) -> String {
    let fraction = clamp_rebuild_fraction(fraction);
    let bar_width = bar_width.clamp(4, 64);
    let filled = ((fraction * bar_width as f32).round() as usize).min(bar_width);
    let mut bar = String::with_capacity(bar_width + 2);
    bar.push('[');
    for i in 0..bar_width {
        bar.push(if i < filled { '█' } else { '░' });
    }
    bar.push(']');
    let pct = (fraction * 100.0).round() as u32;
    let detail = detail.trim();
    if detail.is_empty() {
        format!("{bar}  {pct:>3}%")
    } else {
        format!("{bar}  {pct:>3}%  {detail}")
    }
}

/// Pure ASCII/block bar string (no percent/detail) for render unit tests.
pub fn rebuild_progress_bar_chars(fraction: f32, width: usize) -> String {
    let fraction = clamp_rebuild_fraction(fraction);
    let width = width.max(1);
    let filled = ((fraction * width as f32).round() as usize).min(width);
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

/// Tracks overall rebuild progress from stage markers and cargo lines.
///
/// All public mutators keep `fraction` monotonic (never decreases).
#[derive(Debug, Clone)]
pub struct RebuildProgressEngine {
    fraction: f32,
    detail: String,
    cargo_compiled: usize,
    cargo_finished: bool,
    seen_crates: std::collections::BTreeSet<String>,
}

impl Default for RebuildProgressEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RebuildProgressEngine {
    pub fn new() -> Self {
        Self {
            fraction: 0.0,
            detail: "Starting rebuild".into(),
            cargo_compiled: 0,
            cargo_finished: false,
            seen_crates: std::collections::BTreeSet::new(),
        }
    }

    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn event(&self) -> RebuildProgressEvent {
        RebuildProgressEvent {
            fraction: self.fraction,
            detail: self.detail.clone(),
        }
    }

    /// Raise overall fraction to at least `target` (monotonic).
    pub fn advance_to(&mut self, target: f32) {
        let target = clamp_rebuild_fraction(target);
        if target > self.fraction {
            self.fraction = target;
        }
    }

    pub fn set_detail(&mut self, detail: impl Into<String>) {
        let mut d = detail.into();
        if d.chars().count() > REBUILD_PROGRESS_LINE_MAX_CHARS {
            d = d
                .chars()
                .take(REBUILD_PROGRESS_LINE_MAX_CHARS.saturating_sub(3))
                .collect::<String>()
                + "...";
        }
        self.detail = d;
    }

    pub fn mark_resolve(&mut self, source: &Path) {
        self.advance_to(rebuild_progress_weights::RESOLVE);
        self.set_detail(format!("Resolving source tree ({})", source.display()));
    }

    pub fn mark_install_start(&mut self, backend_label: &str) {
        self.advance_to(rebuild_progress_weights::INSTALL_START);
        self.set_detail(format!("Starting {backend_label}"));
    }

    pub fn mark_cargo_start(&mut self) {
        self.advance_to(rebuild_progress_weights::CARGO_START);
        self.cargo_finished = false;
        self.set_detail("Compiling (cargo build)");
    }

    pub fn mark_strip(&mut self) {
        self.advance_to(rebuild_progress_weights::STRIP);
        self.set_detail("Stripping unneeded symbols");
    }

    pub fn mark_install_bin(&mut self) {
        self.advance_to(rebuild_progress_weights::INSTALL_BIN);
        self.set_detail("Installing to cargo bin");
    }

    pub fn mark_verify(&mut self) {
        self.advance_to(rebuild_progress_weights::VERIFY);
        self.set_detail("Verifying installed binary");
    }

    pub fn mark_leaders(&mut self) {
        self.advance_to(rebuild_progress_weights::LEADERS);
        self.set_detail("Soft-relaunching leaders");
    }

    pub fn mark_done(&mut self) {
        self.advance_to(rebuild_progress_weights::DONE);
        self.set_detail("Rebuild complete");
    }

    /// Note a compiled crate (human `Compiling` or cargo json artifact).
    pub fn note_compiled_crate(&mut self, name: &str) {
        if self.seen_crates.insert(name.to_string()) {
            self.cargo_compiled = self.seen_crates.len();
        }
        let sub = cargo_sub_fraction(self.cargo_compiled, self.cargo_finished);
        self.advance_to(overall_fraction_in_cargo(sub));
        self.set_detail(format!(
            "Compiling {name} ({} package{})",
            self.cargo_compiled,
            if self.cargo_compiled == 1 { "" } else { "s" }
        ));
    }

    pub fn note_cargo_finished(&mut self) {
        self.cargo_finished = true;
        self.advance_to(rebuild_progress_weights::CARGO_END);
        self.set_detail(format!(
            "Finished cargo build ({} package{})",
            self.cargo_compiled,
            if self.cargo_compiled == 1 { "" } else { "s" }
        ));
    }

    /// Ingest one sanitized stage/cargo line and update progress when matched.
    /// Returns `true` when the engine produced a meaningful update.
    pub fn ingest_line(&mut self, line: &str) -> bool {
        let t = line.trim();
        if t.is_empty() {
            return false;
        }

        // justfile / our own markers
        if t.starts_with("==>") {
            let lower = t.to_ascii_lowercase();
            if lower.contains("cargo build") {
                self.mark_cargo_start();
                return true;
            }
            if lower.contains("strip") {
                self.mark_strip();
                return true;
            }
            if lower.contains("install") && !lower.contains("just install") {
                self.mark_install_bin();
                return true;
            }
            if lower.contains("just install") {
                self.mark_install_start("just install");
                return true;
            }
            if lower.contains("verify") {
                self.mark_verify();
                return true;
            }
            // Generic stage echo
            self.set_detail(t.to_string());
            return true;
        }

        if let Some(name) = parse_compiling_crate(t) {
            self.note_compiled_crate(name);
            return true;
        }

        if t.starts_with("Finished ") {
            self.note_cargo_finished();
            return true;
        }

        if is_cargo_json_build_finished(t) {
            self.note_cargo_finished();
            return true;
        }

        if let Some(name) = parse_cargo_json_artifact_package(t) {
            self.note_compiled_crate(&name);
            return true;
        }

        if t.starts_with("error") || t.starts_with("Error") {
            self.set_detail(t.to_string());
            return true;
        }

        if t.starts_with("Installing ") || t.starts_with("    Building") {
            self.set_detail(t.to_string());
            return true;
        }

        false
    }
}

/// Strip ANSI CSI/OSC sequences from a byte slice (best-effort, no new dep).
pub fn strip_ansi_bytes(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == 0x1b {
            i += 1;
            if i >= input.len() {
                break;
            }
            match input[i] {
                b'[' => {
                    // CSI: ESC [ ... final byte in 0x40..=0x7E
                    i += 1;
                    while i < input.len() {
                        let b = input[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&b) {
                            break;
                        }
                    }
                }
                b']' => {
                    // OSC: ESC ] ... BEL or ST (ESC \)
                    i += 1;
                    while i < input.len() {
                        if input[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if input[i] == 0x1b && i + 1 < input.len() && input[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    // Other ESC sequences: skip ESC + next byte
                    i += 1;
                }
            }
            continue;
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

/// Sanitize a raw child-output fragment into a single stable-height progress line.
///
/// - Strips ANSI escapes
/// - Takes the last segment after `\r` (cargo progress bar rewrites)
/// - Collapses multi-line to the last non-empty line
/// - Trims whitespace; returns `None` when empty
/// - Truncates to [`REBUILD_PROGRESS_LINE_MAX_CHARS`]
pub fn sanitize_rebuild_progress_line(raw: &str) -> Option<String> {
    let stripped = strip_ansi_bytes(raw.as_bytes());
    let text = String::from_utf8_lossy(&stripped);
    // Cargo progress bars rewrite the same line with `\r`.
    let after_cr = text.rsplit('\r').next().unwrap_or(&text);
    let line = after_cr
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())?;
    if line.is_empty() {
        return None;
    }
    let mut s = line.to_string();
    // Drop leftover C0 controls that would break a one-line toast.
    s.retain(|c| c == '\t' || !c.is_control());
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut out = s.to_string();
    if out.chars().count() > REBUILD_PROGRESS_LINE_MAX_CHARS {
        out = out
            .chars()
            .take(REBUILD_PROGRESS_LINE_MAX_CHARS.saturating_sub(3))
            .collect::<String>()
            + "...";
    }
    Some(out)
}

/// Contract: a line safe for single-line TUI status (no newline, CR, or ESC).
pub fn is_stable_height_progress_line(line: &str) -> bool {
    !line.is_empty()
        && !line.contains('\n')
        && !line.contains('\r')
        && !line.contains('\x1b')
        && line.chars().count() <= REBUILD_PROGRESS_LINE_MAX_CHARS + 1
}

/// Which sanitized lines are worth feeding into the progress engine.
pub fn is_rebuild_progress_stage_line(sanitized: &str) -> bool {
    let t = sanitized.trim();
    t.starts_with("==>")
        || t.starts_with("Compiling ")
        || t.starts_with("Finished ")
        || t.starts_with("error")
        || t.starts_with("Error")
        || t.starts_with("Installing ")
        || t.starts_with("    Building")
        || (t.starts_with('{')
            && (t.contains("\"compiler-artifact\"") || t.contains("\"build-finished\"")))
}

/// Apply env so cargo/just do not emit TTY progress bars or colors into pipes.
fn apply_quiet_tool_env(cmd: &mut Command) {
    cmd.env("CARGO_TERM_COLOR", "never");
    cmd.env("CARGO_TERM_PROGRESS_WHEN", "never");
    cmd.env("NO_COLOR", "1");
    cmd.env("TERM", "dumb");
    cmd.env_remove("FORCE_COLOR");
    cmd.env_remove("CLICOLOR_FORCE");
}

/// Run a command with **captured** stdio (never inherit). Streams sanitized
/// stage lines into `engine` and emits [`RebuildProgressEvent`] via
/// `on_progress`. Returns combined output text for failure diagnostics.
fn run_command_captured(
    mut cmd: Command,
    label: &str,
    engine: &mut RebuildProgressEngine,
    on_progress: &mut dyn FnMut(RebuildProgressEvent),
) -> Result<(std::process::ExitStatus, String)> {
    use std::sync::Arc;

    use xai_grok_tools::util::{ProcessGroup, detach_std_command, global_process_scope};

    debug_assert_eq!(
        install_stdio_policy(),
        InstallStdioPolicy::Capture,
        "install must never inherit parent TTY"
    );
    apply_quiet_tool_env(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Own process group so session kill_all can reap the install tree.
    detach_std_command(&mut cmd);

    #[allow(clippy::disallowed_methods)] // enrolled into ProcessScope below
    let mut child = cmd.spawn().with_context(|| format!("spawn `{label}`"))?;
    let group = ProcessGroup::new()
        .and_then(|mut group| {
            group.attach_std(&child)?;
            Ok(Arc::new(group))
        })
        .with_context(|| format!("enroll process group for `{label}`"))?;
    if !global_process_scope().register(&group) {
        let _ = group.kill();
        let _ = child.wait();
        bail!("process scope closed; `{label}` aborted");
    }

    let stdout = child
        .stdout
        .take()
        .context("child stdout missing after piped setup")?;
    let stderr = child
        .stderr
        .take()
        .context("child stderr missing after piped setup")?;

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let tx_err = tx.clone();
    let t_out = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = tx.send(buf.clone());
                }
                Err(_) => break,
            }
        }
    });
    let t_err = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = tx_err.send(buf.clone());
                }
                Err(_) => break,
            }
        }
    });
    // Channel closes when both reader threads drop their senders.

    let mut combined = String::new();
    while let Ok(chunk) = rx.recv() {
        let raw = String::from_utf8_lossy(&chunk);
        combined.push_str(&raw);
        // Cargo `--message-format=json` lines are huge; parse before sanitize
        // truncation so package names stay intact. Detail strings the engine
        // emits stay short for the TUI.
        let raw_trim = raw.trim();
        if raw_trim.starts_with('{')
            && (raw_trim.contains("\"compiler-artifact\"")
                || raw_trim.contains("\"build-finished\""))
        {
            if engine.ingest_line(raw_trim) {
                on_progress(engine.event());
            }
            continue;
        }
        if let Some(line) = sanitize_rebuild_progress_line(&raw)
            && is_rebuild_progress_stage_line(&line)
            && is_stable_height_progress_line(&line)
            && engine.ingest_line(&line)
        {
            on_progress(engine.event());
        }
    }
    let _ = t_out.join();
    let _ = t_err.join();

    let status = child
        .wait()
        .with_context(|| format!("wait for `{label}`"))?;
    drop(group); // drop strong handle after reap so Weak cannot killpg a reused PID
    Ok((status, combined))
}

fn tail_for_error(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// Run install for `source_root` (blocking). Prefer `just install`; fall back
/// to fixed cargo argv matching the justfile recipe.
///
/// **Never inherits** parent stdio. Use [`run_install_with_progress`] for
/// structured progress; this wrapper discards mid-build progress.
pub fn run_install(source_root: &Path) -> Result<(InstallBackend, PathBuf)> {
    run_install_with_progress(source_root, &mut |_| {})
}

/// Like [`run_install`], but invokes `on_progress` with weighted
/// [`RebuildProgressEvent`] samples (fraction + human detail).
pub fn run_install_with_progress(
    source_root: &Path,
    on_progress: &mut dyn FnMut(RebuildProgressEvent),
) -> Result<(InstallBackend, PathBuf)> {
    let install_path = default_install_path();
    let mut engine = RebuildProgressEngine::new();

    if just_available() {
        engine.mark_install_start("just install");
        on_progress(engine.event());
        let mut cmd = Command::new("just");
        cmd.arg("install").current_dir(source_root);
        let (status, output) = run_command_captured(cmd, "just install", &mut engine, on_progress)?;
        if !status.success() {
            let tail = tail_for_error(&output, 40);
            bail!(
                "`just install` failed with status {status} in {}\n{tail}",
                source_root.display()
            );
        }
        // just install already stripped + installed; nudge bar to install end.
        engine.mark_verify();
        on_progress(engine.event());
        return Ok((InstallBackend::JustInstall, install_path));
    }

    // Match justfile install recipe (no wild linker; release; locked).
    // JSON message format gives real compiler-artifact counts off the TTY.
    engine.mark_cargo_start();
    on_progress(engine.event());
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "--release",
        "-p",
        PAGER_BIN_PACKAGE,
        "--locked",
        "--message-format=json",
        "--config",
        "target.x86_64-unknown-linux-gnu.rustflags=[\"-C\",\"force-unwind-tables=yes\"]",
        "--config",
        "target.aarch64-unknown-linux-gnu.rustflags=[\"-C\",\"force-unwind-tables=yes\"]",
    ])
    .current_dir(source_root);
    let (status, output) = run_command_captured(
        cmd,
        &format!("cargo build --release -p {PAGER_BIN_PACKAGE}"),
        &mut engine,
        on_progress,
    )?;
    if !status.success() {
        let tail = tail_for_error(&output, 40);
        bail!(
            "cargo build --release -p {PAGER_BIN_PACKAGE} failed with status {status} in {}\n{tail}",
            source_root.display()
        );
    }
    engine.note_cargo_finished();
    on_progress(engine.event());
    let built = source_root.join("target/release/grok-oss");
    if !built.is_file() {
        bail!(
            "expected release binary at {} after cargo build",
            built.display()
        );
    }
    // Best-effort strip (same as just install); ignore strip failures on odd hosts.
    // Capture stdio so strip never paints the TUI either.
    engine.mark_strip();
    on_progress(engine.event());
    let _ = Command::new("strip")
        .args(["--strip-unneeded"])
        .arg(&built)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Some(parent) = install_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create install dir {}", parent.display()))?;
    }
    engine.mark_install_bin();
    on_progress(engine.event());
    std::fs::copy(&built, &install_path)
        .with_context(|| format!("install {} → {}", built.display(), install_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&install_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&install_path, perms)?;
    }
    engine.mark_verify();
    on_progress(engine.event());
    Ok((InstallBackend::CargoFixedArgv, install_path))
}

/// Run `binary --version` and parse an identity string (`0.1.100 (sha)`).
///
/// The parenthetical SHA is a git object id, not a SHA-1 digest of the
/// binary bytes.
pub fn verify_installed_identity(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", binary.display()))?;
    if !output.status.success() {
        bail!(
            "{} --version failed with status {}",
            binary.display(),
            output.status
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_version_output(&text).ok_or_else(|| {
        anyhow::anyhow!(
            "could not parse identity from `{} --version` output: {text:?}",
            binary.display()
        )
    })
}

/// Extract `0.1.100 (sha)` from lines like `grok-oss 0.1.100 (sha)`.
pub fn parse_version_output(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    // Prefer "name version (sha)" → strip leading product name tokens until a semver-ish token.
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for i in 0..tokens.len() {
        let candidate = tokens[i..].join(" ");
        if leader::parse_binary_identity(&candidate).is_some() {
            // Prefer full "version (sha)" when present.
            if candidate.contains('(') {
                return Some(candidate);
            }
            // Keep looking for a form with SHA; fall back later.
        }
    }
    // Fall back: last two tokens as "version (sha)" or single version token.
    if tokens.len() >= 2 {
        let last_two = format!("{} {}", tokens[tokens.len() - 2], tokens[tokens.len() - 1]);
        if leader::parse_binary_identity(&last_two).is_some() {
            return Some(last_two);
        }
        if leader::parse_binary_identity(tokens[tokens.len() - 1]).is_some() {
            return Some(tokens[tokens.len() - 1].to_string());
        }
    } else if let Some(t) = tokens.first()
        && leader::parse_binary_identity(t).is_some()
    {
        return Some((*t).to_string());
    }
    None
}

/// Whether `/rebuild` may replace the live fleet after install/verify.
///
/// Failed `just install` / `--version` verify must not SIGUSR1 peers or
/// soft-signal leaders. Process-wide replace is success-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildFleetPlan {
    /// Soft-signal reachable leaders (`RelaunchForUpdate`).
    pub signal_leaders: bool,
    /// Write `rebuild_relaunch_request.json` and `SIGUSR1` other live TUIs.
    pub write_request_and_signal_peers: bool,
}

impl RebuildFleetPlan {
    /// Plan after the install recipe (and optional `--version` verify).
    pub fn after_install(install_succeeded: bool) -> Self {
        if install_succeeded {
            Self {
                signal_leaders: true,
                write_request_and_signal_peers: true,
            }
        } else {
            Self {
                signal_leaders: false,
                write_request_and_signal_peers: false,
            }
        }
    }

    /// True when leaders and peer TUIs should be asked to pick up the new binary.
    pub fn should_replace_fleet(self) -> bool {
        self.signal_leaders && self.write_request_and_signal_peers
    }
}

/// Pure helper for tests: build-fail path must not signal leaders or peers.
///
/// Returns `Err` without marking signals when install fails.
pub fn orchestrate_order_on_install_result(
    install_ok: bool,
    leader_signal_called: &mut bool,
    peer_signal_called: &mut bool,
) -> Result<()> {
    let plan = RebuildFleetPlan::after_install(install_ok);
    if !plan.should_replace_fleet() {
        bail!("install failed; not signaling leaders or peers");
    }
    *leader_signal_called = plan.signal_leaders;
    *peer_signal_called = plan.write_request_and_signal_peers;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cooperative peer relaunch (all active TUIs, not only the invoker)
// ---------------------------------------------------------------------------

/// Path of the rebuild-relaunch request under a Grok home root.
pub fn rebuild_relaunch_request_path(grok_home: &Path) -> PathBuf {
    grok_home.join(REBUILD_RELAUNCH_REQUEST_FILENAME)
}

/// Unix epoch seconds (best-effort; 0 if the clock is broken).
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a fresh request for peers to re-exec onto `installed_exe`.
pub fn make_rebuild_relaunch_request(
    installed_exe: PathBuf,
    installed_identity: impl Into<String>,
    now_secs: u64,
) -> RebuildRelaunchRequest {
    RebuildRelaunchRequest {
        installed_exe,
        installed_identity: installed_identity.into(),
        requested_at_unix_secs: now_secs,
    }
}

/// Whether a peer running `self_identity` should re-exec for `request`.
///
/// Accepts when the request is fresh and **either**:
/// - the running identity is older than the installed one (same rules as
///   leader relaunch: same package + different git SHA counts as older), or
/// - the process is still on a replaced/deleted binary image (common after
///   `just install` overwrites `~/.cargo/bin/grok-oss` while the TUI runs).
///
/// Does **not** check that `installed_exe` exists; callers that will `exec`
/// must verify the path first.
pub fn should_peer_relaunch_for_request(
    self_identity: &str,
    request: &RebuildRelaunchRequest,
    now_secs: u64,
) -> bool {
    should_peer_relaunch_for_request_with_current_exe(
        self_identity,
        request,
        now_secs,
        std::env::current_exe().ok().as_deref(),
    )
}

/// Whether a cooperative request is fresh enough to act on (identity/path
/// gates not applied). Used when this process received rebuild `SIGUSR1`.
pub fn peer_rebuild_request_is_actionable(request: &RebuildRelaunchRequest, now_secs: u64) -> bool {
    if request.installed_identity.trim().is_empty() {
        return false;
    }
    let age = now_secs.saturating_sub(request.requested_at_unix_secs);
    age <= REBUILD_RELAUNCH_REQUEST_MAX_AGE_SECS
}

/// Injectable form of [`should_peer_relaunch_for_request`] for tests.
pub fn should_peer_relaunch_for_request_with_current_exe(
    self_identity: &str,
    request: &RebuildRelaunchRequest,
    now_secs: u64,
    current_exe: Option<&Path>,
) -> bool {
    if !peer_rebuild_request_is_actionable(request, now_secs) {
        return false;
    }
    if leader::leader_is_older_than(self_identity, &request.installed_identity) {
        return true;
    }
    // Same compile-time identity (or unknown SHA) but still on a replaced
    // binary: Linux `/proc/self/exe` keeps the deleted inode after install.
    running_exe_needs_relaunch_onto(current_exe, &request.installed_exe)
}

/// True when this process should re-exec onto `installed_exe` because the
/// running image is gone/replaced (deleted inode) or is a different path.
pub fn running_exe_needs_relaunch_onto(current_exe: Option<&Path>, installed_exe: &Path) -> bool {
    let Some(current) = current_exe else {
        return false;
    };
    let current_s = current.to_string_lossy();
    // Linux marks replaced binaries: `…/grok-oss (deleted)`.
    if current_s.contains("(deleted)") {
        return true;
    }
    // Different path (dev binary vs cargo-bin install) while a fresh request
    // is outstanding: still pick up the installed product binary.
    let cur = dunce::canonicalize(current).unwrap_or_else(|_| current.to_path_buf());
    let inst = dunce::canonicalize(installed_exe).unwrap_or_else(|_| installed_exe.to_path_buf());
    cur != inst
}

/// Pure: PID set rebuild should SIGUSR1 after the composite `(pid, session_id)`
/// registry key.
///
/// Walks every row, then dedupes with a `BTreeSet` of PIDs. Two windows on the
/// same `session_id` are two rows and both PIDs stay. Duplicate rows for one
/// PID collapse to one. Skips the invoker, dead PIDs, and non-grok processes.
/// Does not send a signal.
pub fn collect_rebuild_signal_pids(
    sessions: &[(u32, String, bool /* alive */, bool /* is_grok */)],
    except_pid: Option<u32>,
) -> std::collections::BTreeSet<u32> {
    let mut pids = std::collections::BTreeSet::new();
    for (pid, _session_id, alive, is_grok) in sessions {
        if except_pid == Some(*pid) {
            continue;
        }
        if !*alive || !*is_grok {
            continue;
        }
        pids.insert(*pid);
    }
    pids
}

/// Pure: which active-session PIDs should receive the cooperative relaunch
/// signal. Excludes `except_pid` (the invoker, which re-execs itself), dead
/// PIDs, and non-product processes (recycled PID safety). Dedupes by PID
/// after the composite key, in first-seen order.
pub fn peer_pids_to_signal_for_relaunch(
    sessions: &[(u32, String, bool /* alive */, bool /* is_grok */)],
    except_pid: Option<u32>,
) -> Vec<(u32, String)> {
    let targets = collect_rebuild_signal_pids(sessions, except_pid);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (pid, session_id, _alive, _is_grok) in sessions {
        if targets.contains(pid) && seen.insert(*pid) {
            out.push((*pid, session_id.clone()));
        }
    }
    out
}

/// Write the cooperative request under the default Grok home.
pub fn write_rebuild_relaunch_request(request: &RebuildRelaunchRequest) -> std::io::Result<()> {
    write_rebuild_relaunch_request_in(&xai_grok_shell::util::grok_home::grok_home(), request)
}

/// Write the cooperative request under an injectable root (tests).
pub fn write_rebuild_relaunch_request_in(
    grok_home: &Path,
    request: &RebuildRelaunchRequest,
) -> std::io::Result<()> {
    std::fs::create_dir_all(grok_home)?;
    let path = rebuild_relaunch_request_path(grok_home);
    let tmp = grok_home.join(format!("{REBUILD_RELAUNCH_REQUEST_FILENAME}.tmp"));
    let json = serde_json::to_string_pretty(request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Read the cooperative request from the default Grok home.
pub fn read_rebuild_relaunch_request() -> Option<RebuildRelaunchRequest> {
    read_rebuild_relaunch_request_in(&xai_grok_shell::util::grok_home::grok_home())
}

/// Read the cooperative request from an injectable root (tests).
pub fn read_rebuild_relaunch_request_in(grok_home: &Path) -> Option<RebuildRelaunchRequest> {
    let path = rebuild_relaunch_request_path(grok_home);
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Soft-signal every other live product TUI in `active_sessions` to re-exec.
///
/// Writes the request first, then delivers `SIGUSR1` (best-effort). The
/// invoker (`except_pid`) re-execs via `/rebuild` itself and is not signaled.
pub fn signal_active_sessions_to_relaunch(
    installed_exe: &Path,
    installed_identity: &str,
    except_pid: Option<u32>,
) -> Vec<PeerRelaunchOutcome> {
    let request = make_rebuild_relaunch_request(
        installed_exe.to_path_buf(),
        installed_identity,
        now_unix_secs(),
    );
    if let Err(e) = write_rebuild_relaunch_request(&request) {
        tracing::warn!(error = %e, "failed to write rebuild relaunch request");
    }

    let sessions = active_sessions::list().unwrap_or_default();
    let classified: Vec<(u32, String, bool, bool)> = sessions
        .iter()
        .map(|s| {
            let pid = s.pid;
            (
                pid,
                s.session_id.0.to_string(),
                active_sessions::is_pid_alive(pid),
                xai_grok_shell::util::is_grok_process(pid),
            )
        })
        .collect();
    let targets = collect_rebuild_signal_pids(&classified, except_pid);

    let mut outcomes = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (pid, session_id, alive, is_grok) in classified {
        if !seen.insert(pid) {
            continue;
        }
        if targets.contains(&pid) {
            match xai_grok_shell::util::signal_process_rebuild_relaunch(pid) {
                Ok(()) => outcomes.push(PeerRelaunchOutcome::Signaled { pid, session_id }),
                Err(e) => outcomes.push(PeerRelaunchOutcome::Skipped {
                    pid,
                    session_id,
                    reason: format!("signal failed: {e}"),
                }),
            }
            continue;
        }
        if except_pid == Some(pid) {
            outcomes.push(PeerRelaunchOutcome::Skipped {
                pid,
                session_id,
                reason: "invoking process (self re-exec)".into(),
            });
            continue;
        }
        if !alive {
            outcomes.push(PeerRelaunchOutcome::Skipped {
                pid,
                session_id,
                reason: "pid not alive".into(),
            });
            continue;
        }
        if !is_grok {
            outcomes.push(PeerRelaunchOutcome::Skipped {
                pid,
                session_id,
                reason: "not a grok product process".into(),
            });
        }
    }
    outcomes
}

/// Full rebuild + leader signal + peer TUI signal + live session inventory.
///
/// On build/install failure, returns `Err` **before** any leader signal.
/// Mid-build progress is discarded; use [`rebuild_and_relaunch_with_progress`]
/// for weighted progress events.
pub async fn rebuild_and_relaunch(start_dir: &Path) -> Result<RebuildReport> {
    rebuild_and_relaunch_with_progress(start_dir, |_| {}).await
}

/// Like [`rebuild_and_relaunch`], with a progress callback for weighted
/// [`RebuildProgressEvent`] samples (TUI progress bar or CLI bar line).
/// Install-stage events may arrive from a blocking worker thread (relayed
/// through a channel); leader/done events run on the async task.
pub async fn rebuild_and_relaunch_with_progress<F>(
    start_dir: &Path,
    mut on_progress: F,
) -> Result<RebuildReport>
where
    F: FnMut(RebuildProgressEvent) + Send + 'static,
{
    let source_root = resolve_source_root(start_dir)?;

    // Resolve stage before the blocking install so the bar moves immediately.
    {
        let mut engine = RebuildProgressEngine::new();
        engine.mark_resolve(&source_root);
        on_progress(engine.event());
    }

    let (progress_tx, mut progress_rx) =
        tokio::sync::mpsc::unbounded_channel::<RebuildProgressEvent>();
    let root = source_root.clone();
    let install_task = tokio::task::spawn_blocking(move || {
        run_install_with_progress(&root, &mut |ev| {
            let _ = progress_tx.send(ev);
        })
    });
    let drain_task = async {
        while let Some(ev) = progress_rx.recv().await {
            on_progress(ev);
        }
    };
    let (install_join, ()) = tokio::join!(install_task, drain_task);
    let (backend, installed_path) = match install_join.context("install task join")? {
        Ok(v) => v,
        Err(e) => {
            // Named contract: failed install must not replace the live fleet.
            debug_assert!(!RebuildFleetPlan::after_install(false).should_replace_fleet());
            return Err(e);
        }
    };

    // `--version` verify is a hard gate. Swallowing it used to SIGUSR1 peers
    // onto a binary that cannot even print its identity (ENXIO / TUI start).
    let installed_identity = match verify_installed_identity(&installed_path) {
        Ok(id) => id,
        Err(e) => {
            debug_assert!(!RebuildFleetPlan::after_install(false).should_replace_fleet());
            return Err(e.context(
                "installed binary failed `--version` verify; not signaling peers or leaders",
            ));
        }
    };

    let plan = RebuildFleetPlan::after_install(true);
    debug_assert!(plan.should_replace_fleet());

    {
        let mut engine = RebuildProgressEngine::new();
        engine.advance_to(rebuild_progress_weights::VERIFY);
        engine.mark_leaders();
        on_progress(engine.event());
    }

    // Optional hygiene: drop dead PIDs from the registry before inventory.
    let _ = active_sessions::collect_crashed();

    let leader_outcomes = if plan.signal_leaders {
        leader::signal_leaders_to_relaunch(&installed_identity).await
    } else {
        Vec::new()
    };

    // Cooperative peer TUI relaunch: every other live product window, not only
    // the invoker. Writes request + SIGUSR1; peers re-exec with the same session.
    // Only after a successful install + verify.
    let peer_outcomes = if plan.write_request_and_signal_peers {
        signal_active_sessions_to_relaunch(
            &installed_path,
            &installed_identity,
            Some(std::process::id()),
        )
    } else {
        Vec::new()
    };

    let live_sessions = active_sessions::list()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| active_sessions::is_pid_alive(s.pid))
        .collect::<Vec<_>>();

    {
        let mut engine = RebuildProgressEngine::new();
        engine.mark_done();
        on_progress(engine.event());
    }

    let summary_lines = format_rebuild_summary(
        &source_root,
        &installed_path,
        &installed_identity,
        backend,
        &leader_outcomes,
        &peer_outcomes,
        &live_sessions,
    );

    Ok(RebuildReport {
        source_root,
        installed_path,
        installed_identity,
        install_backend: backend,
        leader_outcomes,
        peer_outcomes,
        live_sessions,
        summary_lines,
    })
}

/// Human-readable rebuild report lines.
pub fn format_rebuild_summary(
    source_root: &Path,
    installed_path: &Path,
    installed_identity: &str,
    backend: InstallBackend,
    leader_outcomes: &[LeaderRelaunchOutcome],
    peer_outcomes: &[PeerRelaunchOutcome],
    live_sessions: &[ActiveSession],
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("Rebuild complete.".to_string());
    lines.push(format!("  Source:    {}", source_root.display()));
    lines.push(format!("  Backend:   {}", backend.label()));
    lines.push(format!("  Installed: {}", installed_path.display()));
    lines.push(format!("  Identity:  {installed_identity}"));

    let relaunched = leader_outcomes
        .iter()
        .filter(|o| matches!(o, LeaderRelaunchOutcome::Relaunching { .. }))
        .count();
    let declined = leader_outcomes
        .iter()
        .filter(|o| matches!(o, LeaderRelaunchOutcome::Declined { .. }))
        .count();
    let skipped = leader_outcomes
        .iter()
        .filter(|o| matches!(o, LeaderRelaunchOutcome::Skipped { .. }))
        .count();
    lines.push(format!(
        "  Leaders:   {relaunched} relaunching, {declined} declined, {skipped} skipped"
    ));
    for o in leader_outcomes {
        match o {
            LeaderRelaunchOutcome::Relaunching {
                from_version,
                to_version,
                pid,
            } => lines.push(format!(
                "    ↻ pid {} {from_version} → {to_version}",
                pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
            )),
            LeaderRelaunchOutcome::Declined { reason, pid } => lines.push(format!(
                "    · declined pid {}: {reason}",
                pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
            )),
            LeaderRelaunchOutcome::Skipped { reason, pid } => lines.push(format!(
                "    · skipped pid {}: {reason}",
                pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
            )),
        }
    }

    let peers_signaled = peer_outcomes
        .iter()
        .filter(|o| matches!(o, PeerRelaunchOutcome::Signaled { .. }))
        .count();
    let peers_skipped = peer_outcomes
        .iter()
        .filter(|o| matches!(o, PeerRelaunchOutcome::Skipped { .. }))
        .count();
    lines.push(format!(
        "  Peer TUIs: {peers_signaled} signaled to re-exec, {peers_skipped} skipped"
    ));
    for o in peer_outcomes {
        match o {
            PeerRelaunchOutcome::Signaled { pid, session_id } => lines.push(format!(
                "    ↻ pid {pid} session {session_id} (cooperative re-exec)"
            )),
            PeerRelaunchOutcome::Skipped {
                pid,
                session_id,
                reason,
            } => lines.push(format!(
                "    · skipped pid {pid} session {session_id}: {reason}"
            )),
        }
    }

    if live_sessions.is_empty() {
        lines.push("  Live sessions: none registered (or all dead PIDs cleaned).".into());
    } else {
        lines.push(format!(
            "  Live sessions at report time ({}): peers were asked to re-exec onto the new binary; this process re-execs when invoked via /rebuild.",
            live_sessions.len()
        ));
        for s in live_sessions {
            lines.push(format!(
                "    · pid {} session {} cwd {}",
                s.pid, s.session_id.0, s.cwd
            ));
        }
    }
    lines.push(
        "All active product windows on this host should pick up the new binary (leaders drain; peer TUIs re-exec; /rebuild invoker re-execs)."
            .into(),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolve_source_root_walks_up_to_markers() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("repo");
        fs::create_dir_all(root.join("crates/codegen/xai-grok-pager-bin")).unwrap();
        fs::write(root.join("justfile"), "install:\n").unwrap();
        fs::write(
            root.join("crates/codegen/xai-grok-pager-bin/Cargo.toml"),
            "[package]\nname=\"xai-grok-pager-bin\"\n",
        )
        .unwrap();
        let nested = root.join("crates/codegen/xai-grok-pager/src");
        fs::create_dir_all(&nested).unwrap();
        let found = resolve_source_root(&nested).unwrap();
        assert_eq!(found, dunce::canonicalize(&root).unwrap());
    }

    #[test]
    fn resolve_source_root_fails_without_markers() {
        let tmp = TempDir::new().unwrap();
        let err = resolve_source_root(tmp.path()).unwrap_err().to_string();
        assert!(
            err.contains("Could not find a Grok OSS source tree"),
            "{err}"
        );
    }

    #[test]
    fn parse_version_output_extracts_identity() {
        assert_eq!(
            parse_version_output("grok-oss 0.1.100 (abc123def456)\n").as_deref(),
            Some("0.1.100 (abc123def456)")
        );
        assert_eq!(
            parse_version_output("0.1.100 (abc123)\n").as_deref(),
            Some("0.1.100 (abc123)")
        );
        assert_eq!(
            parse_version_output("grok-oss 0.1.100\n").as_deref(),
            Some("0.1.100")
        );
    }

    #[test]
    fn build_fail_does_not_signal_leaders() {
        let mut signaled = false;
        let mut peers = false;
        let err =
            orchestrate_order_on_install_result(false, &mut signaled, &mut peers).unwrap_err();
        assert!(!signaled);
        assert!(!peers);
        assert!(err.to_string().contains("not signaling"));
        orchestrate_order_on_install_result(true, &mut signaled, &mut peers).unwrap();
        assert!(signaled);
        assert!(peers);
    }

    /// Contract: a failed `just install` / verify must not write the
    /// cooperative request or SIGUSR1 peers. Fleet replace is success-only.
    #[test]
    fn failed_install_must_not_replace_or_signal_peers() {
        let plan = RebuildFleetPlan::after_install(false);
        assert!(
            !plan.should_replace_fleet(),
            "failed install must not replace the live fleet"
        );
        assert!(!plan.signal_leaders);
        assert!(!plan.write_request_and_signal_peers);

        let ok = RebuildFleetPlan::after_install(true);
        assert!(ok.should_replace_fleet());
        assert!(ok.signal_leaders);
        assert!(ok.write_request_and_signal_peers);
    }

    /// Contract: product install must capture child stdio (never inherit TTY).
    /// Under the old code path, `.status()` inherited and cargo painted the
    /// alt-screen mid-`/rebuild`.
    #[test]
    fn install_stdio_policy_is_always_capture() {
        assert_eq!(install_stdio_policy(), InstallStdioPolicy::Capture);
    }

    /// Contract: raw cargo progress (ANSI + CR rewrites) becomes a single
    /// stable-height line without ESC/CR/LF.
    #[test]
    fn sanitize_rebuild_progress_strips_ansi_and_carriage_returns() {
        let raw = "\x1b[1m\x1b[32m   Compiling\x1b[0m xai-grok-pager-bin\r\x1b[1m\x1b[32m   Compiling\x1b[0m xai-grok-pager v0.1\n";
        let line = sanitize_rebuild_progress_line(raw).expect("line");
        assert!(
            is_stable_height_progress_line(&line),
            "must be stable-height: {line:?}"
        );
        assert!(!line.contains('\x1b'), "{line:?}");
        assert!(!line.contains('\r'), "{line:?}");
        assert!(!line.contains('\n'), "{line:?}");
        assert!(line.contains("Compiling"), "{line:?}");
    }

    #[test]
    fn sanitize_rebuild_progress_takes_last_line_of_multiline() {
        let raw = "first\n==> cargo build --release -p xai-grok-pager-bin (no wild linker)\n";
        let line = sanitize_rebuild_progress_line(raw).expect("line");
        assert!(line.starts_with("==>"), "{line:?}");
        assert!(is_stable_height_progress_line(&line));
    }

    #[test]
    fn sanitize_rebuild_progress_empty_is_none() {
        assert_eq!(sanitize_rebuild_progress_line("   \n\r\n"), None);
        assert_eq!(sanitize_rebuild_progress_line(""), None);
    }

    #[test]
    fn sanitize_rebuild_progress_truncates_long_lines() {
        let long = "x".repeat(REBUILD_PROGRESS_LINE_MAX_CHARS + 50);
        let line = sanitize_rebuild_progress_line(&long).expect("line");
        assert!(line.chars().count() <= REBUILD_PROGRESS_LINE_MAX_CHARS);
        assert!(is_stable_height_progress_line(&line));
    }

    #[test]
    fn stage_filter_keeps_just_and_cargo_markers() {
        assert!(is_rebuild_progress_stage_line(
            "==> cargo build --release -p xai-grok-pager-bin (no wild linker)"
        ));
        assert!(is_rebuild_progress_stage_line(
            "Compiling xai-grok-pager v0.1.0"
        ));
        assert!(is_rebuild_progress_stage_line("Finished `release` profile"));
        assert!(!is_rebuild_progress_stage_line(
            "    Checking something v1.0.0"
        ));
    }

    /// Contract: every line emitted via the progress path must pass the
    /// stable-height check (what the TUI toast may display).
    #[test]
    fn progress_callback_path_only_forwards_stable_stage_lines() {
        let samples = [
            "\x1b[32m==> strip unneeded symbols\x1b[0m\n",
            "noise only\n",
            "\r\x1b[KCompiling foo\n",
            "error: could not compile `xai-grok-pager`\n",
        ];
        let mut forwarded = Vec::new();
        for raw in samples {
            if let Some(line) = sanitize_rebuild_progress_line(raw)
                && is_rebuild_progress_stage_line(&line)
                && is_stable_height_progress_line(&line)
            {
                forwarded.push(line);
            }
        }
        assert_eq!(forwarded.len(), 3, "{forwarded:?}");
        for line in &forwarded {
            assert!(is_stable_height_progress_line(line), "{line:?}");
        }
        assert!(forwarded[0].starts_with("==>"));
        assert!(forwarded[1].starts_with("Compiling"));
        assert!(forwarded[2].starts_with("error"));
    }

    /// Contract: fraction is clamped to 0..=1 for non-finite and out-of-range.
    #[test]
    fn rebuild_fraction_clamped_0_to_1() {
        assert_eq!(clamp_rebuild_fraction(-1.0), 0.0);
        assert_eq!(clamp_rebuild_fraction(0.0), 0.0);
        assert_eq!(clamp_rebuild_fraction(0.5), 0.5);
        assert_eq!(clamp_rebuild_fraction(1.0), 1.0);
        assert_eq!(clamp_rebuild_fraction(2.0), 1.0);
        assert_eq!(clamp_rebuild_fraction(f32::NAN), 0.0);
        assert_eq!(clamp_rebuild_fraction(f32::INFINITY), 1.0);
    }

    /// Contract: overall progress is monotonic across pipeline stages and
    /// cargo compile counts (no backwards jumps).
    #[test]
    fn rebuild_progress_engine_is_monotonic_across_stages() {
        let mut eng = RebuildProgressEngine::new();
        let mut last = eng.fraction();
        let mut assert_mono = |eng: &mut RebuildProgressEngine| {
            assert!(
                eng.fraction() + f32::EPSILON >= last,
                "fraction went backwards: {} → {} ({})",
                last,
                eng.fraction(),
                eng.detail()
            );
            last = eng.fraction();
        };
        eng.mark_resolve(Path::new("/tmp/repo"));
        assert_mono(&mut eng);
        eng.mark_install_start("just install");
        assert_mono(&mut eng);
        eng.mark_cargo_start();
        assert_mono(&mut eng);
        eng.note_compiled_crate("foo");
        assert_mono(&mut eng);
        eng.note_compiled_crate("bar");
        assert_mono(&mut eng);
        eng.note_compiled_crate("baz");
        assert_mono(&mut eng);
        eng.note_cargo_finished();
        assert_mono(&mut eng);
        eng.mark_strip();
        assert_mono(&mut eng);
        eng.mark_install_bin();
        assert_mono(&mut eng);
        eng.mark_verify();
        assert_mono(&mut eng);
        eng.mark_leaders();
        assert_mono(&mut eng);
        eng.mark_done();
        assert_mono(&mut eng);
        assert!((eng.fraction() - 1.0).abs() < 1e-5, "{}", eng.fraction());
    }

    /// Contract: human Compiling lines and cargo JSON artifacts advance the
    /// cargo segment with real package counts in the detail string.
    #[test]
    fn cargo_artifact_messages_drive_detail_and_fraction() {
        let mut eng = RebuildProgressEngine::new();
        eng.mark_cargo_start();
        let start = eng.fraction();

        assert!(eng.ingest_line("Compiling xai-grok-pager v0.1.0"));
        assert!(eng.detail().contains("xai-grok-pager"), "{}", eng.detail());
        assert!(eng.detail().contains("1 package"), "{}", eng.detail());
        assert!(eng.fraction() > start);

        let mid = eng.fraction();
        let json = r#"{"reason":"compiler-artifact","package_id":"xai-grok-update 0.1.0","target":{"name":"xai-grok-update","kind":["lib"]}}"#;
        assert!(eng.ingest_line(json));
        assert!(eng.detail().contains("xai-grok-update"), "{}", eng.detail());
        assert!(eng.fraction() >= mid);

        let before_fin = eng.fraction();
        assert!(eng.ingest_line(r#"{"reason":"build-finished","success":true}"#));
        assert!(eng.fraction() >= before_fin);
        assert!(
            (eng.fraction() - rebuild_progress_weights::CARGO_END).abs() < 1e-5
                || eng.fraction() >= rebuild_progress_weights::CARGO_END,
            "finished should reach cargo end: {}",
            eng.fraction()
        );
    }

    #[test]
    fn parse_compiling_crate_extracts_name() {
        assert_eq!(
            parse_compiling_crate("Compiling xai-grok-pager v0.1.0"),
            Some("xai-grok-pager")
        );
        assert_eq!(parse_compiling_crate("Finished `release`"), None);
    }

    #[test]
    fn cli_progress_bar_includes_blocks_percent_and_detail() {
        let line = format_rebuild_cli_progress(0.5, "Compiling foo (3 packages)", 10);
        assert!(line.contains('█'), "{line}");
        assert!(line.contains('░'), "{line}");
        assert!(line.contains("50%"), "{line}");
        assert!(line.contains("Compiling foo"), "{line}");
        assert!(line.starts_with('['), "{line}");
    }

    #[test]
    fn rebuild_progress_bar_chars_reflects_fraction() {
        assert_eq!(rebuild_progress_bar_chars(0.0, 4), "░░░░");
        assert_eq!(rebuild_progress_bar_chars(1.0, 4), "████");
        assert_eq!(rebuild_progress_bar_chars(0.5, 4), "██░░");
    }

    #[test]
    fn cargo_sub_fraction_uses_counts_not_time() {
        assert_eq!(cargo_sub_fraction(0, false), 0.0);
        assert_eq!(cargo_sub_fraction(10, true), 1.0);
        let a = cargo_sub_fraction(5, false);
        let b = cargo_sub_fraction(20, false);
        assert!(a > 0.0 && a < 1.0);
        assert!(b > a);
        assert!(b < 1.0);
    }

    /// Contract: after register identity is `(pid, session_id)`, two windows
    /// on the same conversation are two rows. Rebuild must consider both
    /// PIDs and dedupe by PID, not by session_id. Self, dead, and non-grok
    /// rows are skipped. This helper never sends SIGUSR1.
    #[test]
    fn rebuild_signals_each_pid_after_composite_key() {
        let session_id = "shared-conversation";
        let self_pid = 111;
        let peer_a = 222;
        let peer_b = 333;
        let dead_same_session = 444;
        let non_grok = 555;
        let sessions = vec![
            (self_pid, session_id.into(), true, true),
            (peer_a, session_id.into(), true, true),
            (peer_b, session_id.into(), true, true),
            (dead_same_session, session_id.into(), false, true),
            (non_grok, session_id.into(), true, false),
            (peer_a, session_id.into(), true, true),
        ];
        let targets = collect_rebuild_signal_pids(&sessions, Some(self_pid));
        assert!(
            targets.contains(&peer_a),
            "first window on the shared session must be signaled"
        );
        assert!(
            targets.contains(&peer_b),
            "second window on the same session_id must also be signaled; dedupe is by PID"
        );
        assert_eq!(
            targets.len(),
            2,
            "self, dead, non-grok, and a duplicate pid must not add extra targets: {targets:?}"
        );
        assert!(!targets.contains(&self_pid));
        assert!(!targets.contains(&dead_same_session));
        assert!(!targets.contains(&non_grok));
    }

    /// Contract: rebuild must schedule restart of **all** other live product
    /// sessions, not only the invoker. Pure PID filter excludes self / dead /
    /// non-grok.
    #[test]
    fn peer_pids_to_signal_excludes_self_dead_and_non_grok() {
        let sessions = vec![
            (100, "sess-self".into(), true, true),
            (200, "sess-peer".into(), true, true),
            (300, "sess-dead".into(), false, true),
            (400, "sess-other".into(), true, false),
            (500, "sess-peer-2".into(), true, true),
        ];
        let targets = peer_pids_to_signal_for_relaunch(&sessions, Some(100));
        assert_eq!(
            targets,
            vec![(200, "sess-peer".into()), (500, "sess-peer-2".into()),]
        );
    }

    /// Contract: same package + different git SHA is a rebuild peers must accept.
    /// The parenthetical tokens are fake git object ids, not SHA-1 download hashes.
    #[test]
    fn peer_relaunch_accepts_same_semver_different_sha() {
        let req = make_rebuild_relaunch_request(
            PathBuf::from("/tmp/grok-oss-new"),
            "0.2.120 (abc999)",
            1_000,
        );
        assert!(should_peer_relaunch_for_request_with_current_exe(
            "0.2.120 (oldgitsha)",
            &req,
            1_000,
            Some(Path::new("/tmp/grok-oss-new")),
        ));
    }

    /// Contract: equal identity + same live path must not thrash re-exec loops.
    #[test]
    fn peer_relaunch_declines_equal_identity_on_same_path() {
        let tmp = TempDir::new().unwrap();
        let exe = tmp.path().join("grok-oss");
        fs::write(&exe, b"stub").unwrap();
        let req = make_rebuild_relaunch_request(exe.clone(), "0.2.120 (abc999)", 1_000);
        assert!(!should_peer_relaunch_for_request_with_current_exe(
            "0.2.120 (abc999)",
            &req,
            1_000,
            Some(exe.as_path()),
        ));
    }

    /// Contract: after install replaces the binary, Linux shows `(deleted)` on
    /// `/proc/self/exe` — peers must re-exec even when compile-time SHA is equal
    /// or unknown (pager crate often has no `GROK_GIT_SHA`).
    #[test]
    fn peer_relaunch_accepts_deleted_inode_even_when_identity_equal() {
        let req = make_rebuild_relaunch_request(
            PathBuf::from("/home/me/.cargo/bin/grok-oss"),
            "0.2.120 (abc999)",
            1_000,
        );
        let deleted = PathBuf::from("/home/me/.cargo/bin/grok-oss (deleted)");
        assert!(should_peer_relaunch_for_request_with_current_exe(
            "0.2.120 (abc999)",
            &req,
            1_000,
            Some(deleted.as_path()),
        ));
        assert!(running_exe_needs_relaunch_onto(
            Some(deleted.as_path()),
            Path::new("/home/me/.cargo/bin/grok-oss")
        ));
    }

    /// Contract: stale request (older than 15 minutes) is ignored.
    #[test]
    fn peer_relaunch_declines_stale_request() {
        let req = make_rebuild_relaunch_request(
            PathBuf::from("/tmp/grok-oss-new"),
            "0.2.120 (abc999)",
            1_000,
        );
        let now = 1_000 + REBUILD_RELAUNCH_REQUEST_MAX_AGE_SECS + 1;
        assert!(!should_peer_relaunch_for_request_with_current_exe(
            "0.2.120 (oldgitsha)",
            &req,
            now,
            Some(Path::new("/tmp/grok-oss-old")),
        ));
        assert!(!peer_rebuild_request_is_actionable(&req, now));
    }

    #[test]
    fn peer_rebuild_request_is_actionable_when_fresh() {
        let req = make_rebuild_relaunch_request(
            PathBuf::from("/tmp/grok-oss-new"),
            "0.2.120 (abc999)",
            1_000,
        );
        assert!(peer_rebuild_request_is_actionable(&req, 1_000));
        assert!(peer_rebuild_request_is_actionable(
            &req,
            1_000 + REBUILD_RELAUNCH_REQUEST_MAX_AGE_SECS
        ));
        assert!(!peer_rebuild_request_is_actionable(
            &make_rebuild_relaunch_request(PathBuf::from("/x"), "", 1_000),
            1_000
        ));
    }

    #[test]
    fn rebuild_relaunch_request_round_trips_on_disk() {
        let tmp = TempDir::new().unwrap();
        let req = make_rebuild_relaunch_request(
            PathBuf::from("/home/me/.cargo/bin/grok-oss"),
            "0.2.120 (deadbeef)",
            42,
        );
        write_rebuild_relaunch_request_in(tmp.path(), &req).unwrap();
        let loaded = read_rebuild_relaunch_request_in(tmp.path()).expect("request");
        assert_eq!(loaded, req);
    }

    /// Contract: summary reports peer TUI signal outcomes (not only leaders).
    #[test]
    fn format_rebuild_summary_includes_peer_signals() {
        let peers = vec![
            PeerRelaunchOutcome::Signaled {
                pid: 222,
                session_id: "s-peer".into(),
            },
            PeerRelaunchOutcome::Skipped {
                pid: 111,
                session_id: "s-self".into(),
                reason: "invoking process (self re-exec)".into(),
            },
        ];
        let lines = format_rebuild_summary(
            Path::new("/src"),
            Path::new("/bin/grok-oss"),
            "0.2.120 (abc)",
            InstallBackend::JustInstall,
            &[],
            &peers,
            &[],
        );
        let joined = lines.join("\n");
        assert!(
            joined.contains("Peer TUIs: 1 signaled to re-exec"),
            "{joined}"
        );
        assert!(joined.contains("pid 222"), "{joined}");
        assert!(joined.contains("All active product windows"), "{joined}");
        assert!(
            !joined.contains("may still need reattach"),
            "old single-process wording must not remain: {joined}"
        );
    }
}
