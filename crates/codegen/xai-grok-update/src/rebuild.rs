//! Rebuild Grok OSS from a local source tree and soft-relaunch live leaders.
//!
//! This is the product path for `/rebuild` and `grok-oss rebuild`. It does
//! **not** use the SpaceXAI auto-updater channel. Install default is
//! `just install` → `${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, bail};

use crate::format_build_id;
use xai_grok_shell::active_sessions::{self, ActiveSession};
use xai_grok_shell::leader::{self, LeaderRelaunchOutcome};

/// Package that produces the `grok-oss` binary.
const PAGER_BIN_PACKAGE: &str = "xai-grok-pager-bin";

/// Relative path markers that identify this workspace root.
const JUSTFILE_NAME: &str = "justfile";
const PAGER_BIN_MANIFEST: &str = "crates/codegen/xai-grok-pager-bin/Cargo.toml";

/// Summary of one rebuild + relaunch attempt (for CLI, slash scrollback, tests).
#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub source_root: PathBuf,
    pub installed_path: PathBuf,
    /// Full identity when known, e.g. `0.1.100 (abc123)`.
    pub installed_identity: String,
    pub install_backend: InstallBackend,
    pub leader_outcomes: Vec<LeaderRelaunchOutcome>,
    /// Alive active_sessions rows after optional crash hygiene.
    pub live_sessions: Vec<ActiveSession>,
    /// Lines suitable for operator scrollback / stdout.
    pub summary_lines: Vec<String>,
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
    debug_assert_eq!(
        install_stdio_policy(),
        InstallStdioPolicy::Capture,
        "install must never inherit parent TTY"
    );
    apply_quiet_tool_env(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| format!("spawn `{label}`"))?;

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

/// Pure helper for tests: build-fail path must not signal leaders.
///
/// Returns `Err` without calling `signal` when install fails.
pub fn orchestrate_order_on_install_result(
    install_ok: bool,
    signal_called: &mut bool,
) -> Result<()> {
    if !install_ok {
        bail!("install failed; not signaling leaders");
    }
    *signal_called = true;
    Ok(())
}

/// Full rebuild + leader signal + live session inventory.
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
    let (backend, installed_path) = install_join.context("install task join")??;

    let installed_identity = verify_installed_identity(&installed_path).unwrap_or_else(|_| {
        // Fallback identity from this process when --version parse fails.
        format_build_id(
            env!("CARGO_PKG_VERSION"),
            option_env!("GROK_GIT_SHA").unwrap_or("unknown"),
        )
    });

    {
        let mut engine = RebuildProgressEngine::new();
        engine.advance_to(rebuild_progress_weights::VERIFY);
        engine.mark_leaders();
        on_progress(engine.event());
    }

    // Optional hygiene: drop dead PIDs from the registry before inventory.
    let _ = active_sessions::collect_crashed();

    let leader_outcomes = leader::signal_leaders_to_relaunch(&installed_identity).await;

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
        &live_sessions,
    );

    Ok(RebuildReport {
        source_root,
        installed_path,
        installed_identity,
        install_backend: backend,
        leader_outcomes,
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

    if live_sessions.is_empty() {
        lines.push("  Live sessions: none registered (or all dead PIDs cleaned).".into());
    } else {
        lines.push(format!(
            "  Live sessions ({}): standalone TUIs may still need reattach if they were not leaders and did not self-exec:",
            live_sessions.len()
        ));
        for s in live_sessions {
            lines.push(format!(
                "    · pid {} session {} cwd {}",
                s.pid, s.session_id.0, s.cwd
            ));
            lines.push(format!(
                "      reattach: grok-oss --resume {}",
                s.session_id.0
            ));
        }
    }
    lines.push(
        "This process (when invoked via /rebuild) re-execs onto the new binary with the same session when possible."
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
        let err = orchestrate_order_on_install_result(false, &mut signaled).unwrap_err();
        assert!(!signaled);
        assert!(err.to_string().contains("not signaling"));
        orchestrate_order_on_install_result(true, &mut signaled).unwrap();
        assert!(signaled);
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
}
