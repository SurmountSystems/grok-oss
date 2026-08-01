//! Subcommand handlers: path / read / snapshot / update.

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;
use serde_json::json;

use super::intercept::{MemorySubcommand, UpdateStdinSource};
use super::markdown::{MemoryState, parse_memory_file, render_memory_file};
use super::merge::merge_run;
use super::workspace::memory_paths;
use super::{ExitCode, MemoryError};

const FILE_MODE: u32 = 0o600;
const LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const LOCK_POLL: Duration = Duration::from_millis(200);

/// Result of an in-process handler (stdout/stderr/exit like a process).
#[derive(Debug, Clone)]
pub struct HandlerResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl HandlerResult {
    fn ok(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: ExitCode::Success.as_i32(),
        }
    }

    fn from_err(e: MemoryError) -> Self {
        let code = e.exit_code().as_i32();
        let msg = match &e {
            MemoryError::Io(m) if !m.starts_with("I/O error") => {
                format!("memory.py: I/O error: {m}")
            }
            _ => e.stderr_line(),
        };
        Self {
            stdout: String::new(),
            stderr: msg,
            exit_code: code,
        }
    }
}

pub fn run_subcommand(
    sub: &MemorySubcommand,
    cwd: &Path,
    home: Option<&Path>,
    stdin: &str,
) -> HandlerResult {
    match run_inner(sub, cwd, home, stdin) {
        Ok(s) => HandlerResult::ok(s),
        Err(e) => HandlerResult::from_err(e),
    }
}

fn run_inner(
    sub: &MemorySubcommand,
    cwd: &Path,
    home: Option<&Path>,
    stdin: &str,
) -> Result<String, MemoryError> {
    match sub {
        MemorySubcommand::Path => {
            let paths = memory_paths(cwd, home, false)?;
            Ok(format!("{}\n", paths.file.display()))
        }
        MemorySubcommand::Read => {
            let paths = memory_paths(cwd, home, false)?;
            if paths.file.exists() {
                std::fs::read_to_string(&paths.file).map_err(|e| MemoryError::Io(e.to_string()))
            } else {
                Ok(String::new())
            }
        }
        MemorySubcommand::Snapshot => {
            let paths = memory_paths(cwd, home, false)?;
            let (state, exists) = if paths.file.exists() {
                let content = std::fs::read_to_string(&paths.file)
                    .map_err(|e| MemoryError::Io(e.to_string()))?;
                (parse_memory_file(&content), true)
            } else {
                (MemoryState::default(), false)
            };
            let payload = build_snapshot(&state, exists);
            let mut s = serde_json::to_string_pretty(&payload)
                .map_err(|e| MemoryError::Io(e.to_string()))?;
            s.push('\n');
            Ok(s)
        }
        MemorySubcommand::Update { stdin: _src } => {
            // stdin already resolved by execute_intercept
            cmd_update(cwd, home, stdin)
        }
    }
}

fn build_snapshot(state: &MemoryState, exists: bool) -> serde_json::Value {
    let mut common_issues = Vec::new();
    for (cat, entries) in &state.common_issues {
        for e in entries {
            common_issues.push(json!({
                "category": cat,
                "description": e.description,
                "count": e.count,
            }));
        }
    }
    let recent_runs: Vec<_> = state
        .recent_runs
        .iter()
        .map(|r| {
            json!({
                "date": r.date,
                "description": r.description,
                "body_lines": r.body_lines,
            })
        })
        .collect();
    json!({
        "common_issues": common_issues,
        "recent_runs": recent_runs,
        "exists": exists,
    })
}

fn cmd_update(cwd: &Path, home: Option<&Path>, raw: &str) -> Result<String, MemoryError> {
    if raw.trim().is_empty() {
        return Err(MemoryError::Spec(
            "update requires JSON spec on stdin".into(),
        ));
    }
    let spec: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| MemoryError::Spec(format!("invalid JSON on stdin: {e}")))?;
    let spec = spec
        .as_object()
        .ok_or_else(|| MemoryError::Spec("update spec must be a JSON object".into()))?;

    let paths = memory_paths(cwd, home, true)?;
    let existed_before = paths.file.exists();

    let _lock = acquire_lock(&paths.lock)?;

    let existing = if paths.file.exists() {
        std::fs::read_to_string(&paths.file).map_err(|e| MemoryError::Io(e.to_string()))?
    } else {
        String::new()
    };
    let mut state = parse_memory_file(&existing);
    let stats = merge_run(&mut state, spec)?;
    let new_content = render_memory_file(&state);
    atomic_write(&paths.file, &new_content)?;

    let total_categories = state
        .common_issues
        .values()
        .filter(|e| !e.is_empty())
        .count();
    let total_patterns: usize = state.common_issues.values().map(|e| e.len()).sum();
    let total_recent_runs = state.recent_runs.len();

    let payload = json!({
        "file": paths.file.to_string_lossy(),
        "existed_before": existed_before,
        "stats": stats,
        "total_categories": total_categories,
        "total_patterns": total_patterns,
        "total_recent_runs": total_recent_runs,
    });
    let mut s =
        serde_json::to_string_pretty(&payload).map_err(|e| MemoryError::Io(e.to_string()))?;
    s.push('\n');
    Ok(s)
}

struct LockGuard {
    file: std::fs::File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn acquire_lock(lock_path: &Path) -> Result<LockGuard, MemoryError> {
    // Ensure parent exists (update path already create_dir's base)
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MemoryError::Io(e.to_string()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| MemoryError::Io(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(lock_path, std::fs::Permissions::from_mode(FILE_MODE));
    }

    let deadline = Instant::now() + LOCK_TIMEOUT;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(LockGuard { file }),
            Err(_) => {
                if Instant::now() >= deadline {
                    return Err(MemoryError::LockTimeout(format!(
                        "could not acquire lock on {} within {}s",
                        lock_path.display(),
                        LOCK_TIMEOUT.as_secs()
                    )));
                }
                std::thread::sleep(LOCK_POLL);
            }
        }
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<(), MemoryError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| MemoryError::Io(e.to_string()))?;
    let mut tmp =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| MemoryError::Io(e.to_string()))?;
    tmp.write_all(content.as_bytes())
        .map_err(|e| MemoryError::Io(e.to_string()))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| MemoryError::Io(e.to_string()))?;
    tmp.persist(path)
        .map_err(|e| MemoryError::Io(e.error.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(FILE_MODE));
    }
    Ok(())
}

/// Resolve update stdin from intercept source (used by tests).
#[allow(dead_code)]
pub fn resolve_update_stdin(src: &UpdateStdinSource) -> Result<String, MemoryError> {
    match src {
        UpdateStdinSource::Empty => Ok(String::new()),
        UpdateStdinSource::Literal(s) => Ok(s.clone()),
        UpdateStdinSource::FromFile(p) => {
            let mut f = std::fs::File::open(p).map_err(|e| MemoryError::Io(e.to_string()))?;
            let mut s = String::new();
            f.read_to_string(&mut s)
                .map_err(|e| MemoryError::Io(e.to_string()))?;
            Ok(s)
        }
    }
}
