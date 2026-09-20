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
//!
//! Spawn `write_paths` on `task` / `spawn_subagent` is a **soft
//! assignment**. Other nested agents get a reminder that a sibling is
//! assigned those paths. Assignment does not exclusive-block spawn or
//! edits for the child's lifetime. The hard exclusive lock is only
//! [`try_acquire_write`] for one `search_replace` / `write` /
//! `apply_patch` call.
//!
//! [`try_acquire_read`] is a separate CoW snapshot read: ephemeral, many
//! concurrent readers, not the exclusive write lock. A snapshot does not
//! block a writer and is not blocked by a writer.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::types::resources::{OwnerSessionId, SharedResources};

/// Process-wide table: in-flight hard locks and spawn-time soft assignments.
struct WriteLockTable {
    /// Path currently inside an edit-tool call (hard exclusive write lock).
    /// At most one writer per path.
    held: HashMap<PathBuf, String>,
    /// Path assigned to live subagents via spawn `write_paths` (soft).
    /// Several holders may share a path. This does not block acquire.
    assigned: HashMap<PathBuf, HashSet<String>>,
    /// Pre-write CoW bytes published while a writer holds the path.
    /// Snapshot readers clone this Arc; they do not take `held`.
    published: HashMap<PathBuf, Arc<[u8]>>,
}

static TABLE: OnceLock<Mutex<WriteLockTable>> = OnceLock::new();

fn table() -> &'static Mutex<WriteLockTable> {
    TABLE.get_or_init(|| {
        Mutex::new(WriteLockTable {
            held: HashMap::new(),
            assigned: HashMap::new(),
            published: HashMap::new(),
        })
    })
}

fn unique_normalized_paths(paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Vec<PathBuf> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let key = normalize_lock_path(path.as_ref());
        if seen.insert(key.clone()) {
            unique.push(key);
        }
    }
    unique
}

fn holder_conflict<'a>(table: &'a WriteLockTable, key: &Path, holder: &str) -> Option<&'a str> {
    if let Some(existing) = table.held.get(key)
        && existing != holder
    {
        return Some(existing.as_str());
    }
    None
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
            table.published.remove(&self.path);
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
    if let Some(existing) = holder_conflict(&table, &key, holder) {
        return Err(PathHeldError {
            path: key,
            holder: existing.to_string(),
        });
    }
    if !table.published.contains_key(&key)
        && let Ok(bytes) = std::fs::read(&key)
    {
        table.published.insert(key.clone(), Arc::from(bytes));
    }
    table.held.insert(key.clone(), holder.to_string());
    Ok(PerPathWriteGuard {
        path: key,
        holder: holder.to_string(),
        released: false,
    })
}

/// Published pre-write CoW bytes while a writer holds `path`.
///
/// Snapshot readers clone this. Absence means read the filesystem.
pub fn published_cow_snapshot(path: &Path) -> Option<Arc<[u8]>> {
    let key = normalize_lock_path(path);
    lock_table().published.get(&key).cloned()
}

/// RAII CoW snapshot. Dropping it does not block or unblock writers.
#[derive(Debug, Clone)]
pub struct PerPathReadGuard {
    path: PathBuf,
    bytes: Arc<[u8]>,
}

impl PerPathReadGuard {
    /// Frozen bytes from the snapshot point in time.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Path this snapshot was taken for.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Copy the snapshot into an owned buffer.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes.to_vec()
    }
}

/// CoW snapshot read. Ephemeral. Many concurrent readers.
///
/// Does not take the exclusive write lock. Does not fail because a writer
/// holds the path: if a writer published a pre-write copy, that copy is
/// the snapshot; otherwise this reads the filesystem outside the table
/// mutex so the snapshot itself does not block writers.
pub fn try_acquire_read(path: &Path) -> io::Result<PerPathReadGuard> {
    let key = normalize_lock_path(path);
    if let Some(bytes) = published_cow_snapshot(&key) {
        return Ok(PerPathReadGuard { path: key, bytes });
    }
    let disk = std::fs::read(&key)?;
    if let Some(bytes) = published_cow_snapshot(&key) {
        return Ok(PerPathReadGuard { path: key, bytes });
    }
    Ok(PerPathReadGuard {
        path: key,
        bytes: Arc::from(disk),
    })
}

/// Try to take the write lock for every path. All-or-nothing.
pub fn try_acquire_writes(
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
    holder: &str,
) -> Result<Vec<PerPathWriteGuard>, PathHeldError> {
    let unique = unique_normalized_paths(paths);
    let mut guards = Vec::with_capacity(unique.len());
    for key in unique {
        let guard = try_acquire_write(&key, holder)?;
        guards.push(guard);
    }
    Ok(guards)
}

/// Soft-assign paths for a live subagent until [`release_holder`].
///
/// Overlapping assignment with another live subagent succeeds. It does
/// not exclusive-block that sibling's later edit. The same holder may
/// assign a path twice. Hard conflict is only an in-flight
/// [`try_acquire_write`] by someone else, and that is still only for
/// that one tool call.
pub fn try_reserve_writes(paths: impl IntoIterator<Item = impl AsRef<Path>>, holder: &str) {
    let unique = unique_normalized_paths(paths);
    let mut table = lock_table();
    for key in unique {
        table
            .assigned
            .entry(key)
            .or_default()
            .insert(holder.to_string());
    }
}

/// Drop every spawn-time path assignment for this holder.
pub fn release_holder(holder: &str) {
    let mut table = lock_table();
    for holders in table.assigned.values_mut() {
        holders.remove(holder);
    }
    table.assigned.retain(|_, holders| !holders.is_empty());
}

/// Reminder text for other nested agents: which live L2s are assigned
/// which paths. `except_holder` is the current agent, so they do not
/// get a note about their own assignment.
pub fn format_soft_assignment_reminder(except_holder: Option<&str>) -> Option<String> {
    let table = lock_table();
    let mut by_holder: BTreeMap<&str, Vec<&Path>> = BTreeMap::new();
    for (path, holders) in &table.assigned {
        for holder in holders {
            if except_holder.is_some_and(|id| id == holder) {
                continue;
            }
            by_holder.entry(holder.as_str()).or_default().push(path);
        }
    }
    if by_holder.is_empty() {
        return None;
    }
    let mut lines = Vec::with_capacity(by_holder.len());
    for (holder, mut paths) in by_holder {
        paths.sort();
        let listed = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("L2 {holder} is assigned these paths: {listed}."));
    }
    Some(lines.join("\n"))
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

    #[test]
    fn sequential_writes_succeed_after_the_first_tool_call_returns_even_when_both_agents_were_assigned_the_same_write_paths()
     {
        // Operator: write locks must be hard at the tool-call level, not at
        // the agent/layer level. Two agents sequential writes to the same
        // file after first tool call returns must succeed.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("shared.txt");
        std::fs::write(&path, "x\n").unwrap();
        let first = format!("seq-first-{}", path.display());
        let second = format!("seq-second-{}", path.display());
        try_reserve_writes([&path], &first);
        try_reserve_writes([&path], &second);

        let first_call = try_acquire_write(&path, &first).unwrap();
        drop(first_call);
        let second_call = try_acquire_write(&path, &second);
        assert!(
            second_call.is_ok(),
            "after the first search_replace/write/apply_patch call returns, a sibling must be able to write the same file"
        );
        release_holder(&first);
        release_holder(&second);
    }

    #[test]
    fn concurrent_in_flight_writes_on_the_same_path_still_conflict() {
        // Operator: keep the two-agents-cannot-write-the-same-file-at-the-same-instant
        // contract. That is the hard lock. Concurrent overlapping in-flight
        // edits on the same path still fail.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("inflight.txt");
        std::fs::write(&path, "x\n").unwrap();
        let first = format!("hard-first-{}", path.display());
        let second = format!("hard-second-{}", path.display());
        try_reserve_writes([&path], &first);
        try_reserve_writes([&path], &second);
        let _first_call = try_acquire_write(&path, &first).unwrap();
        let err = try_acquire_write(&path, &second).unwrap_err();
        assert_eq!(err.holder, first);
        release_holder(&first);
        release_holder(&second);
    }

    #[test]
    fn spawn_write_paths_soft_assignment_does_not_block_a_sibling_and_the_reminder_is_observable() {
        // Operator: layer/L2 write_paths claims must be a soft lock. Other
        // agents get an automated reminder that a sibling is working on that
        // path. They must not be blocked for minutes (ACP claim for the whole
        // L2 lifetime).
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("claimed.txt");
        std::fs::write(&path, "x\n").unwrap();
        let first = format!("soft-first-{}", path.display());
        let second = format!("soft-second-{}", path.display());
        try_reserve_writes([&path], &first);
        try_reserve_writes([&path], &second);

        let note = format_soft_assignment_reminder(Some(&second))
            .expect("soft-lock reminder must be observable to the sibling");
        assert!(
            note.contains(&format!("L2 {first} is assigned these paths")),
            "reminder must name the assigned sibling: {note}"
        );
        assert!(
            note.contains("claimed.txt"),
            "reminder must name the file: {note}"
        );

        let write = try_acquire_write(&path, &second);
        assert!(
            write.is_ok(),
            "a sibling must not be exclusive-blocked for the assignee's lifetime"
        );
        drop(write);
        release_holder(&first);
        let leftover = format_soft_assignment_reminder(Some(&second)).unwrap_or_default();
        assert!(
            !leftover.contains(&first),
            "reminder must end when the assignee finishes: {leftover}"
        );
        release_holder(&second);
    }

    #[test]
    fn same_holder_can_write_a_path_they_reserved() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("own.txt");
        std::fs::write(&path, "x\n").unwrap();
        let holder = format!("same-holder-{}", path.display());
        try_reserve_writes([&path], &holder);
        let write = try_acquire_write(&path, &holder);
        assert!(
            write.is_ok(),
            "the agent that claimed the path must still be able to write it"
        );
        drop(write);
        release_holder(&holder);
    }

    #[test]
    fn cow_snapshot_read_is_ephemeral_many_readers_one_writer() {
        // Operator: "there is a read lock, which is a snapshot read (CoW),
        // and that is a separate thing from a write lock, and a read lock
        // is ephemeral and doesn't interfere with writers. There can be
        // only one write lock, there can be multiple readers."
        // A read lock is not blocked by a writer for the snapshot itself
        // (snapshot at a point in time). Readers must not take the exclusive
        // write lock. Soft write_paths assignment stays a writer reminder.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("cow.txt");
        std::fs::write(&path, "before\n").unwrap();

        let reader_a = try_acquire_read(&path).expect("first CoW snapshot read must succeed");
        let reader_b = try_acquire_read(&path).expect("many concurrent readers");
        assert_eq!(
            reader_a.as_bytes(),
            b"before\n",
            "CoW snapshot read must copy the file at acquire time"
        );
        assert_eq!(
            reader_b.as_bytes(),
            b"before\n",
            "a second reader must snapshot the same point in time"
        );

        let writer_a =
            try_acquire_write(&path, "writer-a").expect("a read lock must not block a writer");
        let writer_b_err = try_acquire_write(&path, "writer-b").unwrap_err();
        assert_eq!(
            writer_b_err.holder, "writer-a",
            "there can be only one write lock"
        );

        std::fs::write(&path, "after\n").unwrap();
        assert_eq!(
            reader_a.as_bytes(),
            b"before\n",
            "CoW snapshot read must stay frozen after a later write"
        );
        assert_eq!(
            reader_b.as_bytes(),
            b"before\n",
            "every live reader keeps its own frozen snapshot"
        );
        let reader_during_write =
            try_acquire_read(&path).expect("a snapshot read must not be blocked by a writer");
        assert_eq!(
            reader_during_write.as_bytes(),
            b"before\n",
            "CoW snapshot while a writer holds must be the pre-write point in time"
        );

        drop(writer_a);
        assert_eq!(
            reader_a.as_bytes(),
            b"before\n",
            "an ephemeral reader snapshot must outlive the writer drop"
        );
        let reader_after_commit =
            try_acquire_read(&path).expect("a new reader after writer drop sees committed disk");
        assert_eq!(
            reader_after_commit.as_bytes(),
            b"after\n",
            "after the exclusive write lock drops, a new snapshot read uses disk"
        );

        let assignee = format!("soft-{}", path.display());
        try_reserve_writes([&path], &assignee);
        let reader_soft = try_acquire_read(&path)
            .expect("readers must not take the exclusive write lock or fail spawn assignment");
        assert_eq!(reader_soft.as_bytes(), b"after\n");
        let note = format_soft_assignment_reminder(Some("other-agent"))
            .expect("soft assignment reminder stays for writers");
        assert!(
            note.contains(&format!("L2 {assignee} is assigned these paths")),
            "reminder must still name the writer assignment: {note}"
        );
        drop(reader_soft);
        let later_write = try_acquire_write(&path, "writer-after-readers");
        assert!(
            later_write.is_ok(),
            "live CoW readers must not exclusive-block a later writer"
        );
        drop(later_write);
        drop(reader_a);
        drop(reader_b);
        drop(reader_during_write);
        drop(reader_after_commit);
        release_holder(&assignee);
    }
}
