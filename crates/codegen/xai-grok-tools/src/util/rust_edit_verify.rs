//! Classify structured-edit Rust paths, resolve the owning crate from the
//! nearest Cargo.toml `[package]` name, and run rustfmt, clippy, and tests
//! after a structured edit.
//!
//! rustfmt runs immediately after each successful `.rs` write. File-level
//! clippy-driver and heuristic tests run on each pending `.rs` path when
//! the tool batch flushes. Tests must not clippy this workspace; they use
//! a tiny temp fixture or a spy runner.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Env: skip the whole edit-verify pipeline when set to `"1"`.
pub const ENV_SKIP_EDIT_VERIFY: &str = "GROK_SKIP_EDIT_VERIFY";

/// Whether a structured edit should enter the rustfmt / clippy / test pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditVerifyDecision {
    /// Path is a workspace Rust file that should be formatted and linted.
    Verify,
    /// Path is out of scope; see the skip reason.
    Skip(EditVerifySkipReason),
}

/// Why [`classify_edit_path`] skipped verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditVerifySkipReason {
    /// Not a `.rs` file (markdown, toml, and other docs stay quiet).
    NotRust,
    /// Path is under a `third_party` directory.
    ThirdParty,
    /// Exact session plan file (`is_plan_file_write`).
    SessionPlanFile,
    /// `GROK_SKIP_EDIT_VERIFY=1`.
    KillSwitch,
}

/// Heuristic tests for one edited file. Never a workspace cargo test run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTestPlan {
    /// Run this package-scoped cargo test argv.
    Run { argv: Vec<String> },
    /// Skip tests and say why (for example the file has no local tests).
    Skip { reason: String },
}

/// Classify a written path. `session_plan_file` is the exact plan-mode path
/// when the session has one; compare it before the `.rs` suffix.
pub fn classify_edit_path(path: &Path, session_plan_file: Option<&Path>) -> EditVerifyDecision {
    if skip_edit_verify_enabled() {
        return EditVerifyDecision::Skip(EditVerifySkipReason::KillSwitch);
    }
    if session_plan_file.is_some_and(|plan| path == plan) {
        return EditVerifyDecision::Skip(EditVerifySkipReason::SessionPlanFile);
    }
    if path_has_component(path, "third_party") {
        return EditVerifyDecision::Skip(EditVerifySkipReason::ThirdParty);
    }
    if path.extension().is_none_or(|ext| ext != "rs") {
        return EditVerifyDecision::Skip(EditVerifySkipReason::NotRust);
    }
    EditVerifyDecision::Verify
}

/// Walk parents for the nearest `Cargo.toml` with a `[package]` `name`.
/// A workspace-only manifest and `[workspace.package]` do not count.
/// Does not shell `cargo metadata`.
pub fn package_name_from_path(path: &Path) -> Option<String> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        if let Some(name) = package_name_from_manifest(&manifest) {
            return Some(name);
        }
    }
    None
}

/// `rustfmt --edition 2024 --config-path rustfmt.toml <abs.rs>…`
pub fn rustfmt_argv(files: &[PathBuf]) -> Vec<String> {
    let mut argv = vec![
        "rustfmt".to_string(),
        "--edition".to_string(),
        "2024".to_string(),
        "--config-path".to_string(),
        "rustfmt.toml".to_string(),
    ];
    argv.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    argv
}

/// `clippy-driver --edition 2024 … <abs.rs>`. The edited path is in argv.
/// Never `cargo clippy -p <crate> --lib`.
pub fn clippy_argv(package: &str, files: &[PathBuf]) -> Vec<String> {
    let mut argv = vec![
        "clippy-driver".to_string(),
        "--edition".to_string(),
        "2024".to_string(),
        "--crate-name".to_string(),
        rustc_crate_name(package, files),
    ];
    if files.len() == 1 && files.iter().any(|f| bin_name_from_path(f).is_some()) {
        argv.extend(["--crate-type".to_string(), "bin".to_string()]);
    } else if files.len() == 1 && files.iter().any(|f| path_needs_clippy_tests(f)) {
        argv.push("--test".to_string());
    } else {
        argv.extend(["--crate-type".to_string(), "lib".to_string()]);
    }
    argv.extend([
        "--emit".to_string(),
        "metadata".to_string(),
        "-D".to_string(),
        "warnings".to_string(),
    ]);
    argv.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    argv
}

/// Tests that `file` owns. Integration files under `tests/` run
/// `cargo test -p <package> --test <stem>`. A `src/` module infers
/// `cargo test -p <package> --lib <module>`. Crate-root `lib.rs` /
/// `main.rs` skip when there is no cheap filter.
pub fn test_plan_for_file(package: &str, file: &Path) -> FileTestPlan {
    if let Some(stem) = integration_test_stem(file) {
        return FileTestPlan::Run {
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                package.to_string(),
                "--test".to_string(),
                stem,
            ],
        };
    }
    if let Some(bin) = bin_name_from_path(file) {
        return FileTestPlan::Run {
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                package.to_string(),
                "--bin".to_string(),
                bin,
            ],
        };
    }
    if let Some(filter) = module_filter_from_src_path(file) {
        return FileTestPlan::Run {
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                package.to_string(),
                "--lib".to_string(),
                filter,
            ],
        };
    }
    FileTestPlan::Skip {
        reason: "no local tests".to_string(),
    }
}

/// Result of one rustfmt child.
#[derive(Debug, Clone)]
pub struct RustfmtRunResult {
    /// rustfmt exited 0 and files on disk should be read back.
    pub ok: bool,
}

/// Result of one timed cargo clippy or cargo test child.
#[derive(Debug, Clone)]
pub struct CargoRunResult {
    /// Process exit code, or `None` when the child was killed after timeout.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl CargoRunResult {
    fn success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// Spawn rustfmt / cargo for edit verify. Tests inject a spy.
pub trait EditVerifyCommandRunner: Send + Sync {
    fn run_rustfmt(&self, files: &[PathBuf]) -> RustfmtRunResult;
    fn run_cargo(&self, argv: &[String], cwd: &Path) -> CargoRunResult;
}

/// Real rustfmt + cargo children with a timeout. Honors `CARGO_TARGET_DIR`
/// and `TMPDIR` already in the environment. Does not invent a second target dir.
pub struct DefaultEditVerifyCommandRunner;

impl EditVerifyCommandRunner for DefaultEditVerifyCommandRunner {
    fn run_rustfmt(&self, files: &[PathBuf]) -> RustfmtRunResult {
        if files.is_empty() {
            return RustfmtRunResult { ok: true };
        }
        // File-level rustfmt: edition 2024 and the nearest rustfmt.toml.
        // [`rustfmt_argv`] documents the same program; the runner resolves
        // `--config-path` to an existing file when one is on disk.
        let mut cmd = std::process::Command::new("rustfmt");
        cmd.arg("--edition").arg("2024");
        if let Some(cfg) = resolve_rustfmt_toml(&files[0]) {
            cmd.arg("--config-path").arg(cfg);
        }
        cmd.args(files);
        let ran = run_command_with_timeout(cmd, Duration::from_secs(30));
        RustfmtRunResult { ok: ran.success() }
    }

    fn run_cargo(&self, argv: &[String], cwd: &Path) -> CargoRunResult {
        if argv.is_empty() {
            return CargoRunResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "empty cargo argv".to_string(),
                timed_out: false,
            };
        }
        let mut cmd = std::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]).current_dir(cwd);
        let timeout = if argv.first().is_some_and(|a| a == "clippy-driver")
            || argv.iter().any(|a| a == "clippy")
        {
            Duration::from_secs(180)
        } else {
            Duration::from_secs(120)
        };
        run_command_with_timeout(cmd, timeout)
    }
}

thread_local! {
    static FORMAT_HOOK_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

static PENDING_VERIFY_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static FORMAT_HOOK_ENTRIES: AtomicU32 = AtomicU32::new(0);
static TEST_RUNNER: OnceLock<Mutex<Option<Arc<dyn EditVerifyCommandRunner>>>> = OnceLock::new();
static VERIFY_RUNTIME_LOCK: Mutex<()> = Mutex::new(());

/// Hold this across format/flush tests so the spy runner and path queue
/// do not leak between parallel cases.
pub fn lock_edit_verify_runtime() -> std::sync::MutexGuard<'static, ()> {
    VERIFY_RUNTIME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn test_runner_slot() -> &'static Mutex<Option<Arc<dyn EditVerifyCommandRunner>>> {
    TEST_RUNNER.get_or_init(|| Mutex::new(None))
}

fn current_runner() -> Arc<dyn EditVerifyCommandRunner> {
    if let Ok(guard) = test_runner_slot().lock()
        && let Some(runner) = guard.as_ref()
    {
        return Arc::clone(runner);
    }
    Arc::new(DefaultEditVerifyCommandRunner)
}

/// Install a spy runner. Tests must call [`clear_test_command_runner`].
pub fn set_test_command_runner(runner: Arc<dyn EditVerifyCommandRunner>) {
    if let Ok(mut guard) = test_runner_slot().lock() {
        *guard = Some(runner);
    }
}

/// Drop the spy runner so later tests use the real rustfmt / cargo children.
pub fn clear_test_command_runner() {
    if let Ok(mut guard) = test_runner_slot().lock() {
        *guard = None;
    }
}

/// How many times the format hook was entered at depth 0 (re-entrancy probe).
pub fn format_hook_entry_count() -> u32 {
    FORMAT_HOOK_ENTRIES.load(Ordering::SeqCst)
}

/// Reset the format-hook entry counter.
pub fn reset_format_hook_entry_count() {
    FORMAT_HOOK_ENTRIES.store(0, Ordering::SeqCst);
}

/// Drain paths recorded by format-on-write. The tool-batch flush calls this.
pub fn take_pending_verify_paths() -> Vec<PathBuf> {
    PENDING_VERIFY_PATHS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect()
}

/// Drop queued paths without running clippy. Used by tests for isolation.
pub fn clear_pending_verify_paths() {
    PENDING_VERIFY_PATHS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

fn record_pending_verify_path(path: PathBuf) {
    PENDING_VERIFY_PATHS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(path);
}

/// rustfmt a just-written path if it is in scope. Returns the bytes that
/// belong in `FileWritten.content`. A nested call (rustfmt rewrite) is a no-op.
pub fn after_structured_rust_write(path: &Path, written_content: &str) -> String {
    after_structured_rust_write_with_plan(path, written_content, None)
}

/// Same as [`after_structured_rust_write`], with the session plan path so
/// `is_plan_file_write` wins even when that path ends in `.rs`.
pub fn after_structured_rust_write_with_plan(
    path: &Path,
    written_content: &str,
    session_plan_file: Option<&Path>,
) -> String {
    let out = after_structured_rust_writes_with_plan(
        &[(path.to_path_buf(), written_content.to_string())],
        session_plan_file,
    );
    out.into_iter()
        .next()
        .unwrap_or_else(|| written_content.to_string())
}

/// rustfmt several just-written paths in one rustfmt argv. Same order out.
pub fn after_structured_rust_writes(files: &[(PathBuf, String)]) -> Vec<String> {
    after_structured_rust_writes_with_plan(files, None)
}

/// rustfmt several just-written paths, skipping the session plan file.
pub fn after_structured_rust_writes_with_plan(
    files: &[(PathBuf, String)],
    session_plan_file: Option<&Path>,
) -> Vec<String> {
    let mut out: Vec<String> = files.iter().map(|(_, c)| c.clone()).collect();
    if files.is_empty() {
        return out;
    }
    let reentered = FORMAT_HOOK_DEPTH.with(|d| d.get() > 0);
    if reentered {
        return out;
    }
    let mut to_fmt: Vec<PathBuf> = Vec::new();
    let mut fmt_idx: Vec<usize> = Vec::new();
    for (i, (path, _)) in files.iter().enumerate() {
        if classify_edit_path(path, session_plan_file) != EditVerifyDecision::Verify {
            continue;
        }
        to_fmt.push(path.clone());
        fmt_idx.push(i);
        record_pending_verify_path(path.clone());
    }
    if to_fmt.is_empty() {
        return out;
    }
    FORMAT_HOOK_ENTRIES.fetch_add(1, Ordering::SeqCst);
    FORMAT_HOOK_DEPTH.with(|d| d.set(d.get().saturating_add(1)));
    let fmt_ok = current_runner().run_rustfmt(&to_fmt).ok;
    FORMAT_HOOK_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    if !fmt_ok {
        return out;
    }
    for (idx, path) in fmt_idx.into_iter().zip(to_fmt.iter()) {
        if let Ok(formatted) = std::fs::read_to_string(path) {
            out[idx] = formatted;
        }
    }
    out
}

/// Run one clippy per crate and the heuristic tests. Does not undo writes.
pub fn flush_batch_clippy_and_tests() -> String {
    flush_batch_clippy_and_tests_for(take_pending_verify_paths(), None)
}

/// Clippy + tests for an explicit path list (session plan path re-checked).
pub fn flush_batch_clippy_and_tests_for(
    paths: Vec<PathBuf>,
    session_plan_file: Option<&Path>,
) -> String {
    if skip_edit_verify_enabled() {
        return String::new();
    }
    let mut by_pkg: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut missing_package = 0usize;
    for path in paths {
        if classify_edit_path(&path, session_plan_file) != EditVerifyDecision::Verify {
            continue;
        }
        match package_name_from_path(&path) {
            Some(name) => by_pkg.entry(name).or_default().push(path),
            None => missing_package += 1,
        }
    }
    if by_pkg.is_empty() && missing_package == 0 {
        return String::new();
    }
    let runner = current_runner();
    let mut report = String::new();
    for (pkg, files) in by_pkg {
        if !report.is_empty() {
            report.push('\n');
        }
        report.push_str("## Edit verify (");
        report.push_str(&pkg);
        report.push_str(")\n");
        let cwd = cargo_invoke_cwd(&files[0]);
        for file in &files {
            let clippy = clippy_argv(&pkg, std::slice::from_ref(file));
            let clippy_res = runner.run_cargo(&clippy, &cwd);
            report.push_str(&format_cargo_section("clippy", &clippy, &clippy_res));
        }
        let mut ran_test = false;
        let mut skip_reason: Option<String> = None;
        let mut seen_test = HashSet::new();
        for file in &files {
            match test_plan_for_file(&pkg, file) {
                FileTestPlan::Run { argv } => {
                    if seen_test.insert(argv.clone()) {
                        ran_test = true;
                        let test_res = runner.run_cargo(&argv, &cwd);
                        report.push_str(&format_cargo_section("tests", &argv, &test_res));
                    }
                }
                FileTestPlan::Skip { reason } => {
                    if skip_reason.is_none() {
                        skip_reason = Some(reason);
                    }
                }
            }
        }
        if !ran_test {
            let reason = skip_reason.unwrap_or_else(|| "no local tests".to_string());
            report.push_str("tests: skipped (");
            report.push_str(&reason);
            report.push_str(")\n");
        }
    }
    if missing_package > 0 {
        if !report.is_empty() {
            report.push('\n');
        }
        report.push_str("edit verify: skipped clippy (no package manifest)\n");
    }
    report
}

fn format_cargo_section(label: &str, argv: &[String], result: &CargoRunResult) -> String {
    let cmd = argv.join(" ");
    let mut out = format!("{label}: `{cmd}`\n");
    if result.timed_out {
        out.push_str(&format!("{label}: timed out\n"));
    } else if result.success() {
        out.push_str(&format!("{label}: ok\n"));
    } else {
        let code = result
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        out.push_str(&format!("{label}: failed (exit {code})\n"));
        let combined = if result.stderr.trim().is_empty() {
            result.stdout.clone()
        } else {
            result.stderr.clone()
        };
        let excerpt = excerpt_child_output(&combined, 4000);
        if !excerpt.is_empty() {
            out.push_str(&excerpt);
            if !excerpt.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

fn excerpt_child_output(text: &str, max_bytes: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max_bytes {
        return trimmed.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n...(truncated)", &trimmed[..end])
}

/// Directory to invoke cargo from: workspace root if one exists, else the
/// package directory. Never shells `cargo metadata`.
fn cargo_invoke_cwd(path: &Path) -> PathBuf {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let mut package_dir = None;
    let mut workspace_dir = None;
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        if package_name_from_manifest(&manifest).is_some() && package_dir.is_none() {
            package_dir = Some(dir.to_path_buf());
        }
        if manifest_has_workspace_table(&manifest) {
            workspace_dir = Some(dir.to_path_buf());
        }
    }
    workspace_dir
        .or(package_dir)
        .unwrap_or_else(|| start.to_path_buf())
}

fn manifest_has_workspace_table(manifest: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return false;
    };
    text.lines().any(|raw| {
        let line = strip_toml_comment(raw).trim();
        line == "[workspace]"
    })
}

fn resolve_rustfmt_toml(file: &Path) -> Option<PathBuf> {
    let start = file.parent().unwrap_or(file);
    for dir in start.ancestors() {
        let candidate = dir.join("rustfmt.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn run_command_with_timeout(mut cmd: std::process::Command, timeout: Duration) -> CargoRunResult {
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::util::detach_std_command(&mut cmd);
    #[allow(clippy::disallowed_methods)] // enrolled into ProcessScope below
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CargoRunResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("failed to spawn: {e}"),
                timed_out: false,
            };
        }
    };
    let group = match crate::util::global_process_scope().enroll_std(&child) {
        Ok(g) => g,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return CargoRunResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("failed to enroll edit verify child: {e}"),
                timed_out: false,
            };
        }
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => CargoRunResult {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            timed_out: false,
        },
        Ok(Err(e)) => CargoRunResult {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: format!("wait failed: {e}"),
            timed_out: false,
        },
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let _ = group.kill();
            let _ = rx.recv_timeout(Duration::from_secs(2));
            CargoRunResult {
                exit_code: None,
                stdout: String::new(),
                stderr: "edit verify child timed out".to_string(),
                timed_out: true,
            }
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => CargoRunResult {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "edit verify waiter dropped".to_string(),
            timed_out: false,
        },
    }
}

fn skip_edit_verify_enabled() -> bool {
    std::env::var(ENV_SKIP_EDIT_VERIFY).ok().as_deref() == Some("1")
}

fn path_has_component(path: &Path, name: &str) -> bool {
    path.components().any(|c| c.as_os_str() == name)
}

fn package_name_from_manifest(manifest: &Path) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let mut in_package_table = false;
    for raw in text.lines() {
        let line = strip_toml_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_package_table = line == "[package]";
            continue;
        }
        if !in_package_table {
            continue;
        }
        if let Some(name) = toml_quoted_name_value(line) {
            return Some(name);
        }
    }
    None
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut quote = '\0';
    for (i, ch) in line.char_indices() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote = ch;
            }
            c if in_quotes && c == quote => {
                in_quotes = false;
            }
            '#' if !in_quotes => return &line[..i],
            _ => {}
        }
    }
    line
}

fn toml_quoted_name_value(line: &str) -> Option<String> {
    let rest = line.strip_prefix("name")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = rest.get(1..)?;
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

/// `src/bin/<name>.rs` or `src/bin/<name>/main.rs` → `<name>`.
fn bin_name_from_path(path: &Path) -> Option<String> {
    let parts: Vec<_> = path.iter().collect();
    for i in 0..parts.len().saturating_sub(2) {
        if parts[i] != "src" || parts[i + 1] != "bin" {
            continue;
        }
        let after_bin = Path::new(parts[i + 2]);
        if after_bin.extension().is_some_and(|ext| ext == "rs") {
            return after_bin
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned());
        }
        if parts.get(i + 3).is_some_and(|p| *p == "main.rs") {
            return Some(parts[i + 2].to_string_lossy().into_owned());
        }
    }
    None
}

fn path_needs_clippy_tests(path: &Path) -> bool {
    let parts: Vec<_> = path.iter().collect();
    if parts.windows(2).any(|w| w[0] == "tests") {
        return true;
    }
    path.file_name().is_some_and(|name| name == "tests.rs")
}

fn integration_test_stem(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent.file_name()? != "tests" {
        return None;
    }
    if path.extension().is_none_or(|ext| ext != "rs") {
        return None;
    }
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

/// rustc crate names cannot contain `-`. Prefer the bin name when the
/// only input is `src/bin/<name>.rs`.
fn rustc_crate_name(package: &str, files: &[PathBuf]) -> String {
    if files.len() == 1
        && let Some(bin) = bin_name_from_path(&files[0])
    {
        return bin.replace('-', "_");
    }
    package.replace('-', "_")
}

/// `src/util/rust_edit_verify.rs` → `util::rust_edit_verify`.
/// Crate-root `src/lib.rs` and `src/main.rs` have no cheap filter.
fn module_filter_from_src_path(path: &Path) -> Option<String> {
    let parts: Vec<_> = path.iter().collect();
    let src_idx = parts.iter().position(|p| *p == "src")?;
    let after = parts.get(src_idx + 1..)?;
    if after.is_empty() || after[0] == "bin" {
        return None;
    }
    if after.len() == 1 {
        let name = after[0];
        if name == "lib.rs" || name == "main.rs" {
            return None;
        }
        return Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned());
    }
    let mut segs = Vec::new();
    for (i, part) in after.iter().enumerate() {
        if i + 1 == after.len() {
            if *part == "mod.rs" || *part == "lib.rs" || *part == "main.rs" {
                break;
            }
            if let Some(stem) = Path::new(part).file_stem() {
                segs.push(stem.to_string_lossy().into_owned());
            }
        } else {
            segs.push(part.to_string_lossy().into_owned());
        }
    }
    if segs.is_empty() {
        None
    } else {
        Some(segs.join("::"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CargoRunResult, EditVerifyCommandRunner, EditVerifyDecision, EditVerifySkipReason,
        FileTestPlan, RustfmtRunResult, after_structured_rust_write,
        after_structured_rust_write_with_plan, after_structured_rust_writes, classify_edit_path,
        clear_pending_verify_paths, clear_test_command_runner, clippy_argv,
        flush_batch_clippy_and_tests, format_hook_entry_count, lock_edit_verify_runtime,
        package_name_from_path, reset_format_hook_entry_count, rustfmt_argv,
        set_test_command_runner, test_plan_for_file,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Serialize tests that mutate `GROK_SKIP_EDIT_VERIFY`.
    static SKIP_ENV_LOCK: Mutex<()> = Mutex::new(());

    const SKIP_ENV: &str = "GROK_SKIP_EDIT_VERIFY";

    struct SkipEnvGuard {
        prev: Option<String>,
    }

    impl SkipEnvGuard {
        fn set(value: Option<&str>) -> Self {
            let prev = std::env::var(SKIP_ENV).ok();
            match value {
                Some(v) => unsafe { std::env::set_var(SKIP_ENV, v) },
                None => unsafe { std::env::remove_var(SKIP_ENV) },
            }
            Self { prev }
        }
    }

    impl Drop for SkipEnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(SKIP_ENV, v) },
                None => unsafe { std::env::remove_var(SKIP_ENV) },
            }
        }
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn write_package_manifest(dir: &Path, name: &str) {
        write(
            &dir.join("Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
        );
    }

    fn write_workspace_only_manifest(dir: &Path) {
        write(
            &dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        );
    }

    #[test]
    fn classify_rust_source_runs_verify() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let path = Path::new("/tmp/edit-verify-fixture/src/lib.rs");
        assert_eq!(
            classify_edit_path(path, None),
            EditVerifyDecision::Verify,
            "a workspace .rs file must enter the format and lint pipeline"
        );
    }

    #[test]
    fn classify_markdown_is_not_rust() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let path = Path::new("/tmp/edit-verify-fixture/README.md");
        assert_eq!(
            classify_edit_path(path, None),
            EditVerifyDecision::Skip(EditVerifySkipReason::NotRust)
        );
    }

    #[test]
    fn classify_toml_is_not_rust() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let path = Path::new("/tmp/edit-verify-fixture/Cargo.toml");
        assert_eq!(
            classify_edit_path(path, None),
            EditVerifyDecision::Skip(EditVerifySkipReason::NotRust)
        );
    }

    #[test]
    fn classify_third_party_rust_is_skipped() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let path = Path::new("/tmp/proj/third_party/syn/src/lib.rs");
        assert_eq!(
            classify_edit_path(path, None),
            EditVerifyDecision::Skip(EditVerifySkipReason::ThirdParty)
        );
    }

    #[test]
    fn classify_session_plan_file_is_skipped() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let plan = Path::new("/home/user/.grok/sessions/proj/abc/plan.md");
        let decision = classify_edit_path(plan, Some(plan));
        assert!(
            matches!(
                decision,
                EditVerifyDecision::Skip(
                    EditVerifySkipReason::SessionPlanFile | EditVerifySkipReason::NotRust
                )
            ),
            "session plan.md must skip the rust verify pipeline, got {decision:?}"
        );
    }

    #[test]
    fn classify_session_plan_path_skips_even_when_suffix_is_rs() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(None);
        let plan = Path::new("/tmp/session/plan.rs");
        assert_eq!(
            classify_edit_path(plan, Some(plan)),
            EditVerifyDecision::Skip(EditVerifySkipReason::SessionPlanFile),
            "is_plan_file_write is an exact path match and must win over the .rs suffix"
        );
    }

    #[test]
    fn classify_kill_switch_skips_verify() {
        let _lock = SKIP_ENV_LOCK.lock().unwrap();
        let _env = SkipEnvGuard::set(Some("1"));
        let path = Path::new("/tmp/edit-verify-fixture/src/lib.rs");
        assert_eq!(
            classify_edit_path(path, None),
            EditVerifyDecision::Skip(EditVerifySkipReason::KillSwitch),
            "GROK_SKIP_EDIT_VERIFY=1 must skip the whole pipeline"
        );
    }

    #[test]
    fn package_name_from_nearest_member_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_workspace_only_manifest(tmp.path());
        let member = tmp.path().join("member");
        write_package_manifest(&member, "fixture");
        let lib = member.join("src/lib.rs");
        write(&lib, "pub fn n() {}\n");
        assert_eq!(
            package_name_from_path(&lib).as_deref(),
            Some("fixture"),
            "walk parents and take the nearest Cargo.toml [package] name"
        );
    }

    #[test]
    fn package_name_skips_workspace_only_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_workspace_only_manifest(tmp.path());
        let stray = tmp.path().join("src/lib.rs");
        write(&stray, "pub fn n() {}\n");
        assert_eq!(
            package_name_from_path(&stray),
            None,
            "a workspace-only Cargo.toml is not a package"
        );
    }

    #[test]
    fn package_name_skips_workspace_package_table() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nname = \"workspace_name\"\nversion = \"0.1.0\"\n",
        );
        let stray = tmp.path().join("src/lib.rs");
        write(&stray, "pub fn n() {}\n");
        assert_eq!(
            package_name_from_path(&stray),
            None,
            "[workspace.package] name is not a [package] name"
        );
    }

    #[test]
    fn package_name_prefers_nearest_package_over_parent() {
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "outer");
        let inner = tmp.path().join("inner");
        write_package_manifest(&inner, "inner");
        let lib = inner.join("src/lib.rs");
        write(&lib, "pub fn n() {}\n");
        assert_eq!(package_name_from_path(&lib).as_deref(), Some("inner"));
    }

    #[test]
    fn package_name_none_without_package_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("src/lib.rs");
        write(&lib, "pub fn n() {}\n");
        assert_eq!(package_name_from_path(&lib), None);
    }

    fn assert_file_level_clippy_argv(argv: &[String], files: &[PathBuf]) {
        assert_eq!(
            argv.first().map(String::as_str),
            Some("clippy-driver"),
            "file-level lint is clippy-driver (rustc + clippy) on the path, not cargo clippy: {argv:?}"
        );
        assert!(
            !argv.iter().any(|a| a == "cargo"),
            "file-level clippy must not invoke cargo: {argv:?}"
        );
        assert!(
            !argv.iter().any(|a| a == "-p"),
            "file-level clippy must not pass cargo -p: {argv:?}"
        );
        assert!(
            !argv.iter().any(|a| a == "--lib"),
            "file-level clippy must not pass cargo --lib: {argv:?}"
        );
        assert!(
            !argv
                .iter()
                .any(|a| a == "--all-targets" || a == "--workspace" || a == "--locked"),
            "file-level clippy must not use crate-wide cargo selectors: {argv:?}"
        );
        for file in files {
            let path = file.to_string_lossy().into_owned();
            assert!(
                argv.iter().any(|a| a == &path),
                "edited path {path} must appear in clippy argv: {argv:?}"
            );
        }
        assert!(
            argv.windows(2)
                .any(|w| w[0] == "--edition" && w[1] == "2024"),
            "clippy-driver must pass --edition 2024: {argv:?}"
        );
        assert!(
            argv.windows(2).any(|w| w[0] == "-D" && w[1] == "warnings"),
            "clippy-driver must deny warnings: {argv:?}"
        );
    }

    #[test]
    fn rustfmt_argv_edition_2024_config_and_absolute_files() {
        let first = PathBuf::from("/tmp/edit-verify-fixture/src/lib.rs");
        let second = PathBuf::from("/tmp/edit-verify-fixture/src/foo.rs");
        assert!(first.is_absolute());
        assert!(second.is_absolute());
        let argv = rustfmt_argv(&[first.clone(), second.clone()]);
        assert_eq!(
            argv,
            vec![
                "rustfmt".to_string(),
                "--edition".to_string(),
                "2024".to_string(),
                "--config-path".to_string(),
                "rustfmt.toml".to_string(),
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ],
            "format argv must be rustfmt --edition 2024 --config-path rustfmt.toml <abs.rs> (operator 2026-08-15 file-level verify)"
        );
        assert!(
            !argv
                .iter()
                .any(|a| a == "cargo" || a == "fmt" || a == "-p" || a == "--all"),
            "file-level rustfmt must not be cargo fmt -p: {argv:?}"
        );
    }

    #[test]
    fn clippy_argv_lints_the_edited_file_not_crate_lib() {
        let lib = PathBuf::from("/tmp/edit-verify-fixture/src/lib.rs");
        let argv = clippy_argv("fixture", std::slice::from_ref(&lib));
        assert_file_level_clippy_argv(&argv, std::slice::from_ref(&lib));
    }

    #[test]
    fn clippy_argv_includes_bin_path_not_package_lib() {
        let bin = PathBuf::from("/tmp/edit-verify-fixture/src/bin/tool.rs");
        let argv = clippy_argv("fixture", std::slice::from_ref(&bin));
        assert_file_level_clippy_argv(&argv, std::slice::from_ref(&bin));
    }

    #[test]
    fn clippy_argv_includes_integration_test_path_not_package_lib() {
        let test_file = PathBuf::from("/tmp/edit-verify-fixture/tests/owns_this.rs");
        let argv = clippy_argv("fixture", std::slice::from_ref(&test_file));
        assert_file_level_clippy_argv(&argv, std::slice::from_ref(&test_file));
    }

    #[test]
    fn clippy_argv_is_file_level_not_package_lib() {
        let file = PathBuf::from(
            "/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/foo.rs",
        );
        let argv = clippy_argv("xai-grok-shell", std::slice::from_ref(&file));
        assert_file_level_clippy_argv(&argv, std::slice::from_ref(&file));
        assert!(
            !argv
                .windows(2)
                .any(|w| w[0] == "-p" && w[1] == "xai-grok-shell"),
            "must not be cargo clippy -p xai-grok-shell --lib: {argv:?}"
        );
    }

    #[test]
    fn test_plan_integration_file_runs_package_test_filter() {
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        let owns = tmp.path().join("tests/owns_this.rs");
        write(
            &owns,
            "#[test]\nfn owns_this_contract() {\n    assert!(true);\n}\n",
        );
        match test_plan_for_file("fixture", &owns) {
            FileTestPlan::Run { argv } => {
                assert_eq!(
                    argv,
                    vec![
                        "cargo".to_string(),
                        "test".to_string(),
                        "-p".to_string(),
                        "fixture".to_string(),
                        "--test".to_string(),
                        "owns_this".to_string(),
                    ],
                    "editing tests/owns_this.rs must run cargo test -p fixture --test owns_this"
                );
                assert!(
                    !argv.iter().any(|a| a == "--workspace"),
                    "test argv must stay package-scoped: {argv:?}"
                );
            }
            other => {
                panic!("editing tests/owns_this.rs must run a package test filter, got {other:?}")
            }
        }
    }

    #[test]
    fn test_plan_lib_without_cfg_test_skips_and_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        let lib = tmp.path().join("src/lib.rs");
        write(&lib, "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
        match test_plan_for_file("fixture", &lib) {
            FileTestPlan::Skip { reason } => {
                let lower = reason.to_ascii_lowercase();
                assert!(
                    lower.contains("no local tests") || lower.contains("no tests"),
                    "skip reason must say the lib file has no local tests: {reason}"
                );
            }
            other => {
                panic!("a lib file with no #[cfg(test)] must skip tests and say so, got {other:?}")
            }
        }
    }

    #[test]
    fn test_plan_src_module_uses_lib_filter_from_path() {
        let file = PathBuf::from("/tmp/edit-verify-fixture/src/util/rust_edit_verify.rs");
        match test_plan_for_file("fixture", &file) {
            FileTestPlan::Run { argv } => {
                assert_eq!(
                    argv,
                    vec![
                        "cargo".to_string(),
                        "test".to_string(),
                        "-p".to_string(),
                        "fixture".to_string(),
                        "--lib".to_string(),
                        "util::rust_edit_verify".to_string(),
                    ],
                    "a src module must use cargo test -p <crate> --lib <module>, not the whole lib"
                );
                assert!(
                    !argv.iter().any(|a| a == "--workspace"),
                    "test argv must stay package-scoped: {argv:?}"
                );
            }
            other => {
                panic!("src/util/rust_edit_verify.rs must infer a module filter, got {other:?}")
            }
        }
    }

    struct SpyRunner {
        rustfmt_calls: AtomicUsize,
        clippy_calls: AtomicUsize,
        test_calls: AtomicUsize,
        last_test_argv: Mutex<Vec<String>>,
        last_clippy_argv: Mutex<Vec<String>>,
        reenter_on_fmt: bool,
        clippy_fail_stderr: Mutex<Option<String>>,
    }

    impl SpyRunner {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                rustfmt_calls: AtomicUsize::new(0),
                clippy_calls: AtomicUsize::new(0),
                test_calls: AtomicUsize::new(0),
                last_test_argv: Mutex::new(Vec::new()),
                last_clippy_argv: Mutex::new(Vec::new()),
                reenter_on_fmt: false,
                clippy_fail_stderr: Mutex::new(None),
            })
        }

        fn reentering() -> Arc<Self> {
            Arc::new(Self {
                rustfmt_calls: AtomicUsize::new(0),
                clippy_calls: AtomicUsize::new(0),
                test_calls: AtomicUsize::new(0),
                last_test_argv: Mutex::new(Vec::new()),
                last_clippy_argv: Mutex::new(Vec::new()),
                reenter_on_fmt: true,
                clippy_fail_stderr: Mutex::new(None),
            })
        }
    }

    impl EditVerifyCommandRunner for SpyRunner {
        fn run_rustfmt(&self, files: &[PathBuf]) -> RustfmtRunResult {
            self.rustfmt_calls.fetch_add(1, Ordering::SeqCst);
            if self.reenter_on_fmt {
                for file in files {
                    let _ = after_structured_rust_write(file, "fn reenter() {}\n");
                }
            }
            RustfmtRunResult { ok: true }
        }

        fn run_cargo(&self, argv: &[String], _cwd: &Path) -> CargoRunResult {
            let is_clippy = argv.first().is_some_and(|a| a == "clippy-driver")
                || argv.iter().any(|a| a == "clippy");
            if is_clippy {
                self.clippy_calls.fetch_add(1, Ordering::SeqCst);
                *self.last_clippy_argv.lock().unwrap() = argv.to_vec();
                if let Some(stderr) = self.clippy_fail_stderr.lock().unwrap().clone() {
                    return CargoRunResult {
                        exit_code: Some(101),
                        stdout: String::new(),
                        stderr,
                        timed_out: false,
                    };
                }
            } else if argv.iter().any(|a| a == "test") {
                self.test_calls.fetch_add(1, Ordering::SeqCst);
                *self.last_test_argv.lock().unwrap() = argv.to_vec();
            }
            CargoRunResult {
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
            }
        }
    }

    struct RuntimeGuard {
        _env: SkipEnvGuard,
        _skip: std::sync::MutexGuard<'static, ()>,
        _runtime: std::sync::MutexGuard<'static, ()>,
    }

    impl RuntimeGuard {
        fn lock() -> Self {
            let skip = SKIP_ENV_LOCK.lock().unwrap();
            let runtime = lock_edit_verify_runtime();
            clear_test_command_runner();
            clear_pending_verify_paths();
            reset_format_hook_entry_count();
            let env = SkipEnvGuard::set(None);
            Self {
                _env: env,
                _skip: skip,
                _runtime: runtime,
            }
        }
    }

    impl Drop for RuntimeGuard {
        fn drop(&mut self) {
            clear_test_command_runner();
            clear_pending_verify_paths();
            reset_format_hook_entry_count();
        }
    }

    #[test]
    fn plan_file_write_skips_rustfmt_even_when_suffix_is_rs() {
        let _g = RuntimeGuard::lock();
        let spy = SpyRunner::new();
        set_test_command_runner(spy.clone());
        let plan = PathBuf::from("/tmp/session/plan.rs");
        let out = after_structured_rust_write_with_plan(&plan, "fn x(){}\n", Some(&plan));
        assert_eq!(out, "fn x(){}\n");
        assert_eq!(
            spy.rustfmt_calls.load(Ordering::SeqCst),
            0,
            "session plan file must not spawn rustfmt"
        );
        let report = flush_batch_clippy_and_tests();
        assert!(
            report.is_empty(),
            "session plan file must not queue clippy: {report}"
        );
    }

    #[test]
    fn several_rust_writes_run_file_level_clippy_per_file() {
        let _g = RuntimeGuard::lock();
        let spy = SpyRunner::new();
        set_test_command_runner(spy.clone());
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        write(&tmp.path().join("src/lib.rs"), "pub fn a() {}\n");
        write(&tmp.path().join("src/a.rs"), "pub fn b() {}\n");
        write(&tmp.path().join("src/b.rs"), "pub fn c() {}\n");
        let files = [
            (tmp.path().join("src/lib.rs"), "pub fn a() {}\n".to_string()),
            (tmp.path().join("src/a.rs"), "pub fn b() {}\n".to_string()),
            (tmp.path().join("src/b.rs"), "pub fn c() {}\n".to_string()),
        ];
        let _ = after_structured_rust_writes(&files);
        assert_eq!(
            spy.rustfmt_calls.load(Ordering::SeqCst),
            1,
            "several files in one flush use one rustfmt argv"
        );
        let report = flush_batch_clippy_and_tests();
        assert_eq!(
            spy.clippy_calls.load(Ordering::SeqCst),
            3,
            "file-level clippy is one clippy-driver per edited file. report={report}"
        );
        let argv = spy.last_clippy_argv.lock().unwrap().clone();
        assert_file_level_clippy_argv(&argv, &[tmp.path().join("src/b.rs")]);
        assert!(
            report.contains("clippy:") && report.contains("fixture"),
            "verify report names the crate: {report}"
        );
    }

    #[test]
    fn clippy_findings_appear_in_report_and_write_is_not_rolled_back() {
        let _g = RuntimeGuard::lock();
        let spy = SpyRunner::new();
        *spy.clippy_fail_stderr.lock().unwrap() = Some("unused variable: `dead`\n".to_string());
        set_test_command_runner(spy.clone());
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        let lib = tmp.path().join("src/lib.rs");
        let written = "pub fn n() { let dead = 1; }\n";
        write(&lib, written);
        let _ = after_structured_rust_write(&lib, written);
        let report = flush_batch_clippy_and_tests();
        assert!(
            report.contains("unused variable") || report.contains("dead"),
            "clippy findings must reach the verify report: {report}"
        );
        assert!(
            report.contains("failed"),
            "clippy failure must be reported without undoing the write: {report}"
        );
        let on_disk = fs::read_to_string(&lib).unwrap();
        assert_eq!(
            on_disk, written,
            "clippy failure must not roll back the write"
        );
    }

    #[test]
    fn flush_runs_package_test_for_integration_file_and_skips_lib_without_tests() {
        let _g = RuntimeGuard::lock();
        let spy = SpyRunner::new();
        set_test_command_runner(spy.clone());
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        let owns = tmp.path().join("tests/owns_this.rs");
        write(
            &owns,
            "#[test]\nfn owns_this_contract() {\n    assert!(true);\n}\n",
        );
        let _ = after_structured_rust_write(&owns, "#[test]\nfn owns_this_contract() {}\n");
        let report = flush_batch_clippy_and_tests();
        assert_eq!(spy.test_calls.load(Ordering::SeqCst), 1);
        let argv = spy.last_test_argv.lock().unwrap().clone();
        assert_eq!(
            argv,
            vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "fixture".to_string(),
                "--test".to_string(),
                "owns_this".to_string(),
            ],
            "editing tests/owns_this.rs must run cargo test -p fixture --test owns_this"
        );
        assert!(report.contains("tests:"), "{report}");

        let lib = tmp.path().join("src/lib.rs");
        write(&lib, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n");
        let _ = after_structured_rust_write(&lib, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n");
        let report = flush_batch_clippy_and_tests();
        assert!(
            report.to_ascii_lowercase().contains("no local tests"),
            "lib file with no #[cfg(test)] must skip tests and say so: {report}"
        );
    }

    #[test]
    fn rustfmt_rewrite_does_not_reenter_the_hook() {
        let _g = RuntimeGuard::lock();
        let spy = SpyRunner::reentering();
        set_test_command_runner(spy.clone());
        let tmp = tempfile::tempdir().unwrap();
        write_package_manifest(tmp.path(), "fixture");
        let lib = tmp.path().join("src/lib.rs");
        write(&lib, "fn  foo(){}\n");
        let _ = after_structured_rust_write(&lib, "fn  foo(){}\n");
        assert_eq!(
            spy.rustfmt_calls.load(Ordering::SeqCst),
            1,
            "rustfmt rewrite must not start a second rustfmt"
        );
        assert_eq!(
            format_hook_entry_count(),
            1,
            "rustfmt rewrite must not re-enter the format hook"
        );
    }
}
