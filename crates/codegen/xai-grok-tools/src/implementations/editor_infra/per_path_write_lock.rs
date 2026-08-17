//! Automatic per-path write lock for ACP edit tools.
//!
//! `search_replace`, `apply_patch`, `write`, OpenCode `edit`, and
//! `hashline_edit` take this lock as part of the tool call. The happy
//! path is silent: there is no lock argument on the tool schema and a
//! successful write does not mention the lock. A held path is a tool
//! error that names the holder and the file. The tool does not write,
//! does not wait, and does not show a human steal, skip, or wait menu.
//! Agents resolve the conflict by talking to each other.
//!
//! This is a fail-fast table, not the unused FIFO waiter in
//! [`super::file_operation_lock`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::types::resources::{OwnerSessionId, SharedResources};

/// Process-wide table: normalized path to the agent that is writing it.
struct WriteLockTable {
    held: HashMap<PathBuf, String>,
}

static TABLE: OnceLock<Mutex<WriteLockTable>> = OnceLock::new();

fn table() -> &'static Mutex<WriteLockTable> {
    TABLE.get_or_init(|| {
        Mutex::new(WriteLockTable {
            held: HashMap::new(),
        })
    })
}

fn lock_table() -> MutexGuard<'static, WriteLockTable> {
    table()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Normalize a path so relative and absolute forms of the same file collide.
pub fn normalize_lock_path(path: &Path) -> PathBuf {
    if let Ok(canon) = dunce::canonicalize(path) {
        return canon;
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    if let Some(parent) = absolute.parent()
        && let Ok(parent_canon) = dunce::canonicalize(parent)
    {
        return match absolute.file_name() {
            Some(name) => parent_canon.join(name),
            None => parent_canon,
        };
    }
    dunce::simplified(&absolute).to_path_buf()
}

/// Why a write could not take the per-path lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHeldError {
    pub path: PathBuf,
    pub holder: String,
}

impl PathHeldError {
    /// Model-facing error. Names the holder and the file. No steal, skip,
    /// or wait menu.
    pub fn message(&self) -> String {
        format!(
            "Cannot write {}: {} is already writing this file. \
             Tell that agent you need the file, or pick another path. Do not overwrite.",
            self.path.display(),
            self.holder
        )
    }

    pub fn into_tool_error(self, tool_id: &str) -> xai_tool_runtime::ToolError {
        xai_tool_runtime::ToolError::execution(
            xai_tool_protocol::ToolId::new(tool_id).expect("valid tool id"),
            self.message(),
        )
    }
}

/// RAII guard. The path is free again when this value is dropped.
#[derive(Debug)]
pub struct PerPathWriteGuard {
    path: PathBuf,
    holder: String,
    released: bool,
}

impl PerPathWriteGuard {
    fn release_in_place(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let mut table = lock_table();
        if table
            .held
            .get(&self.path)
            .is_some_and(|held| held == &self.holder)
        {
            table.held.remove(&self.path);
        }
    }
}

impl Drop for PerPathWriteGuard {
    fn drop(&mut self) {
        self.release_in_place();
    }
}

/// Try to take the write lock for one path. Fails immediately when held.
pub fn try_acquire_write(path: &Path, holder: &str) -> Result<PerPathWriteGuard, PathHeldError> {
    let key = normalize_lock_path(path);
    let mut table = lock_table();
    if let Some(existing) = table.held.get(&key) {
        return Err(PathHeldError {
            path: key,
            holder: existing.clone(),
        });
    }
    table.held.insert(key.clone(), holder.to_string());
    Ok(PerPathWriteGuard {
        path: key,
        holder: holder.to_string(),
        released: false,
    })
}

/// Try to take the write lock for every path. All-or-nothing.
pub fn try_acquire_writes(
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
    holder: &str,
) -> Result<Vec<PerPathWriteGuard>, PathHeldError> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let key = normalize_lock_path(path.as_ref());
        if seen.insert(key.clone()) {
            unique.push(key);
        }
    }
    let mut guards = Vec::with_capacity(unique.len());
    for key in unique {
        let guard = try_acquire_write(&key, holder)?;
        guards.push(guard);
    }
    Ok(guards)
}

/// Who should be named if this call holds the path.
pub async fn holder_label(
    ctx: &xai_tool_runtime::ToolCallContext,
    resources: &SharedResources,
) -> String {
    let from_resources = {
        let res = resources.lock().await;
        res.get::<OwnerSessionId>().map(|owner| owner.0.clone())
    };
    if let Some(id) = from_resources.filter(|id| !id.is_empty()) {
        return id;
    }
    if let Some(session) = ctx.extensions.get::<xai_tool_runtime::SessionContext>()
        && !session.0.is_empty()
    {
        return session.0.clone();
    }
    let call = ctx.call_id.as_str();
    if !call.is_empty() {
        return format!("tool call {call}");
    }
    "unknown agent".to_string()
}

/// Take the lock for one path using the calling agent's identity.
pub async fn acquire_for_tool(
    path: &Path,
    ctx: &xai_tool_runtime::ToolCallContext,
    resources: &SharedResources,
    tool_id: &str,
) -> Result<PerPathWriteGuard, xai_tool_runtime::ToolError> {
    let holder = holder_label(ctx, resources).await;
    try_acquire_write(path, &holder).map_err(|held| held.into_tool_error(tool_id))
}

/// Take the lock for every path in one tool call (apply_patch).
pub async fn acquire_paths_for_tool(
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ctx: &xai_tool_runtime::ToolCallContext,
    resources: &SharedResources,
    tool_id: &str,
) -> Result<Vec<PerPathWriteGuard>, xai_tool_runtime::ToolError> {
    let holder = holder_label(ctx, resources).await;
    try_acquire_writes(paths, &holder).map_err(|held| held.into_tool_error(tool_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_acquire_same_path_fails_and_names_holder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("held.txt");
        std::fs::write(&path, "x\n").unwrap();
        let first = try_acquire_write(&path, "explore-agent-a").unwrap();
        let err = try_acquire_write(&path, "explore-agent-b").unwrap_err();
        assert_eq!(err.holder, "explore-agent-a");
        let message = err.message();
        assert!(
            message.contains("explore-agent-a"),
            "error must name the holder: {message}"
        );
        assert!(
            message.contains("held.txt"),
            "error must name the file: {message}"
        );
        drop(first);
    }

    #[test]
    fn try_acquire_releases_on_drop() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("later.txt");
        std::fs::write(&path, "x\n").unwrap();
        let first = try_acquire_write(&path, "first").unwrap();
        drop(first);
        let second = try_acquire_write(&path, "second");
        assert!(second.is_ok(), "a later call must be able to take the path");
    }

    #[test]
    fn different_paths_can_be_held_together() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, "a\n").unwrap();
        std::fs::write(&b, "b\n").unwrap();
        let _ga = try_acquire_write(&a, "one").unwrap();
        let gb = try_acquire_write(&b, "two");
        assert!(gb.is_ok(), "a different path must not be blocked");
    }

    #[test]
    fn held_error_has_no_steal_skip_wait_menu() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("conflict.txt");
        std::fs::write(&path, "x\n").unwrap();
        let _first = try_acquire_write(&path, "holder-a").unwrap();
        let message = try_acquire_write(&path, "holder-b").unwrap_err().message();
        let lower = message.to_ascii_lowercase();
        assert!(!lower.contains("steal"), "{message}");
        assert!(!lower.contains("skip"), "{message}");
        assert!(!lower.contains("wait"), "{message}");
        assert!(
            !lower.contains("press ") && !lower.contains("[s]"),
            "error must not be a human choice list: {message}"
        );
    }
}
