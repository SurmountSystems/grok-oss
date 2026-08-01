//! Codex session discovery (SQLite state + rollout files) and inert read.
//!
//! Host `session_reader.py` parity for common cases. Compressed `.jsonl.zst`
//! rollouts fail closed with a clear install-zstd message (no silent skip of
//! schema). Unknown top-level record types are skipped (inert).

use super::mtime_millis;
use super::safe::{
    ReaderError, finalize_result, iso_from_millis, json_preview, one_line, open_sqlite_readonly,
    paths_equal, safe_text, sort_and_dedupe, table_columns, timestamp_to_millis, turn, within,
};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("uuid re")
});

static CODEX_ROLLOUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^rollout-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.jsonl(?:\.zst)?$",
    )
    .expect("codex rollout re")
});

const CODEX_SAFE_TOP_LEVEL: &[&str] = &["session_meta", "response_item", "compacted", "event_msg"];
const CODEX_IGNORED_TOP_LEVEL: &[&str] = &[
    "turn_context",
    "world_state",
    "inter_agent_communication",
    "inter_agent_communication_metadata",
];

pub fn codex_home() -> PathBuf {
    if let Ok(configured) = std::env::var("CODEX_HOME") {
        PathBuf::from(configured)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex")
    }
}

pub fn codex_id_from_path(path: &Path) -> String {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if let Some(caps) = CODEX_ROLLOUT_RE.captures(name) {
        return caps
            .get(1)
            .map(|m| m.as_str().to_owned())
            .unwrap_or_default();
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .trim_end_matches(".jsonl")
        .to_owned()
}

pub fn is_codex_rollout_name(name: &str) -> bool {
    CODEX_ROLLOUT_RE.is_match(name)
}

fn read_plain_jsonl(path: &Path) -> Result<(Vec<serde_json::Value>, usize), ReaderError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ReaderError::msg(format!("failed to read session {}: {e}", path.display())))?;
    let mut records = Vec::new();
    let mut malformed = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(serde_json::Value::Object(map)) => records.push(serde_json::Value::Object(map)),
            Ok(_) | Err(_) => malformed += 1,
        }
    }
    Ok((records, malformed))
}

fn read_codex_jsonl(path: &Path) -> Result<(Vec<serde_json::Value>, usize), ReaderError> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.ends_with(".jsonl.zst") {
        return Err(ReaderError::msg(format!(
            "zstd is required to read compressed Codex rollout {}; install zstd and decompress, or use a plain .jsonl rollout",
            path.display()
        )));
    }
    read_plain_jsonl(path)
}

fn content_blocks(content: &serde_json::Value) -> Vec<&serde_json::Value> {
    match content {
        serde_json::Value::Array(items) => items.iter().filter(|i| i.is_object()).collect(),
        serde_json::Value::Object(_) => vec![content],
        _ => vec![],
    }
}

fn is_generated_meta_text(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<') && t.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase())
        || t.to_ascii_lowercase()
            .starts_with("[request interrupted by user")
}

fn codex_message_text(item: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    let content = item.get("content").unwrap_or(&serde_json::Value::Null);
    for block in content_blocks(content) {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if matches!(block_type, "reasoning" | "thinking" | "encrypted_content") {
            continue;
        }
        if matches!(block_type, "input_text" | "output_text" | "text") {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                if text.trim().is_empty() || is_generated_meta_text(text) {
                    continue;
                }
                parts.push(safe_text(text));
            }
        }
    }
    parts.join("\n")
}

fn render_codex_item(
    item: &serde_json::Value,
    max_tool_chars: usize,
) -> (Option<serde_json::Value>, bool) {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match item_type {
        "message" => {
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "user" && role != "assistant" {
                return (None, matches!(role, "system" | "developer"));
            }
            let text = codex_message_text(item);
            if text.is_empty() {
                return (None, false);
            }
            (Some(turn(role, &text, vec![], vec![])), false)
        }
        "function_call" => (
            Some(turn(
                "assistant",
                "",
                vec![serde_json::json!({
                    "id": item.get("call_id").or_else(|| item.get("id")),
                    "name": safe_text(item.get("name").and_then(|v| v.as_str()).unwrap_or("function")),
                    "input": json_preview(item.get("arguments").unwrap_or(&serde_json::Value::Null), max_tool_chars),
                    "inert": true,
                })],
                vec![],
            )),
            false,
        ),
        "local_shell_call" => (
            Some(turn(
                "assistant",
                "",
                vec![serde_json::json!({
                    "id": item.get("call_id").or_else(|| item.get("id")),
                    "name": "local_shell",
                    "input": json_preview(item.get("action").unwrap_or(&serde_json::Value::Null), max_tool_chars),
                    "inert": true,
                })],
                vec![],
            )),
            false,
        ),
        "custom_tool_call" => (
            Some(turn(
                "assistant",
                "",
                vec![serde_json::json!({
                    "id": item.get("call_id").or_else(|| item.get("id")),
                    "name": safe_text(item.get("name").and_then(|v| v.as_str()).unwrap_or("custom_tool")),
                    "input": json_preview(item.get("input").unwrap_or(&serde_json::Value::Null), max_tool_chars),
                    "inert": true,
                })],
                vec![],
            )),
            false,
        ),
        "function_call_output" | "custom_tool_call_output" => {
            let mut output = item
                .get("output")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if let Some(obj) = output.as_object() {
                if let Some(body) = obj.get("body").or_else(|| obj.get("text")) {
                    output = body.clone();
                }
            }
            (
                Some(turn(
                    "tool",
                    "",
                    vec![],
                    vec![serde_json::json!({
                        "tool_use_id": item.get("call_id").or_else(|| item.get("id")),
                        "content": json_preview(&output, max_tool_chars),
                        "is_error": item.get("success") == Some(&serde_json::Value::Bool(false)),
                        "unavailable": false,
                        "inert": true,
                    })],
                )),
                false,
            )
        }
        "reasoning"
        | "world_state"
        | "environment_context"
        | "user_instructions"
        | "computer_initialize_state" => (None, true),
        _ => (None, true),
    }
}

fn drop_last_user_turns(turns: &mut Vec<serde_json::Value>, number: i64) {
    if number <= 0 {
        return;
    }
    let positions: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(|(_, t)| t.get("role").and_then(|r| r.as_str()) == Some("user"))
        .map(|(i, _)| i)
        .collect();
    if positions.is_empty() {
        return;
    }
    let cut_idx = positions
        .len()
        .saturating_sub(number as usize)
        .min(positions.len().saturating_sub(1));
    let cut = positions[cut_idx];
    turns.truncate(cut);
}

/// Inert Codex rollout read (host `read_codex_session`).
pub fn read_codex_session(
    path: &str,
    max_tool_chars: usize,
) -> Result<serde_json::Value, ReaderError> {
    let session_path = PathBuf::from(path);
    let (records, malformed) = read_codex_jsonl(&session_path)?;
    let mut warnings = Vec::new();
    if malformed > 0 {
        warnings.push(serde_json::json!({
            "code": "malformed_records_skipped",
            "message": format!("Skipped {malformed} malformed Codex rollout record(s)."),
        }));
    }

    let first_meta = records
        .iter()
        .find(|r| r.get("type").and_then(|t| t.as_str()) == Some("session_meta"))
        .and_then(|r| r.get("payload"))
        .filter(|p| p.is_object())
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let mut base_items: Vec<serde_json::Value> = Vec::new();
    let mut start_index = 0usize;
    for (index, record) in records.iter().enumerate() {
        if record.get("type").and_then(|t| t.as_str()) != Some("compacted") {
            continue;
        }
        if let Some(replacement) = record
            .get("payload")
            .and_then(|p| p.get("replacement_history"))
            .and_then(|h| h.as_array())
        {
            base_items = replacement.clone();
            start_index = index + 1;
        }
    }

    let mut turns = Vec::new();
    let mut unsafe_count = 0usize;
    for item in &base_items {
        let (turn_v, unsafe_item) = render_codex_item(item, max_tool_chars);
        if unsafe_item {
            unsafe_count += 1;
        }
        if let Some(t) = turn_v {
            turns.push(t);
        }
    }
    for record in &records[start_index..] {
        let record_type = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let payload = record.get("payload");
        match record_type {
            "response_item" => {
                if let Some(p) = payload {
                    let (turn_v, unsafe_item) = render_codex_item(p, max_tool_chars);
                    if unsafe_item {
                        unsafe_count += 1;
                    }
                    if let Some(t) = turn_v {
                        turns.push(t);
                    }
                }
            }
            "event_msg" => {
                if let Some(p) = payload {
                    if p.get("type").and_then(|t| t.as_str()) == Some("thread_rolled_back") {
                        let number = p.get("num_turns").and_then(|n| n.as_i64()).unwrap_or(0);
                        drop_last_user_turns(&mut turns, number);
                    }
                }
            }
            "session_meta" | "compacted" => {}
            other
                if CODEX_IGNORED_TOP_LEVEL.contains(&other)
                    || !CODEX_SAFE_TOP_LEVEL.contains(&other) =>
            {
                unsafe_count += 1;
            }
            _ => {}
        }
    }
    if unsafe_count > 0 {
        warnings.push(serde_json::json!({
            "code": "unsafe_records_skipped",
            "message": format!(
                "Skipped {unsafe_count} foreign instruction, reasoning, context, or unknown Codex item(s)."
            ),
        }));
    }

    let session_id = first_meta
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| codex_id_from_path(&session_path));
    let git = first_meta.get("git");
    let branch = git
        .and_then(|g| g.get("branch"))
        .and_then(|b| b.as_str())
        .or_else(|| first_meta.get("git_branch").and_then(|b| b.as_str()))
        .map(|s| s.to_owned());
    let timestamps: Vec<String> = records
        .iter()
        .filter_map(|r| {
            r.get("timestamp")
                .and_then(|t| t.as_str())
                .map(|s| s.to_owned())
        })
        .collect();
    let title = turns
        .iter()
        .find(|t| {
            t.get("role").and_then(|r| r.as_str()) == Some("user")
                && t.get("text")
                    .and_then(|x| x.as_str())
                    .is_some_and(|s| !s.is_empty())
        })
        .and_then(|t| t.get("text").and_then(|x| x.as_str()))
        .map(|t| one_line(t, 200));
    let source = first_meta
        .get("source")
        .and_then(|s| s.as_str())
        .filter(|s| *s == "cli" || *s == "vscode")
        .map(|s| format!("codex-{s}"))
        .unwrap_or_else(|| "codex".to_owned());
    let updated_at = timestamps
        .last()
        .cloned()
        .or_else(|| iso_from_millis(mtime_millis(&session_path)));

    let result = serde_json::json!({
        "tool": "codex",
        "source": source,
        "session_id": session_id,
        "path": session_path.to_string_lossy(),
        "title": title,
        "cwd": first_meta.get("cwd").and_then(|c| c.as_str()),
        "branch": branch,
        "created_at": timestamps.first(),
        "updated_at": updated_at,
        "source_repo_root_path": null,
        "turns": turns,
        "warnings": warnings,
    });
    Ok(finalize_result(result))
}

fn codex_state_database(home: &Path) -> Option<PathBuf> {
    let rd = std::fs::read_dir(home).ok()?;
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if path.is_symlink() || !path.is_file() {
            continue;
        }
        if let Some(rest) = name.strip_prefix("state_") {
            if let Some(num) = rest.strip_suffix(".sqlite") {
                if let Ok(n) = num.parse::<u64>() {
                    if best.as_ref().map(|(bn, _)| n > *bn).unwrap_or(true) {
                        best = Some((n, path));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn existing_codex_rollout(home: &Path, raw_path: &str, session_id: &str) -> Option<PathBuf> {
    if raw_path.is_empty() {
        return None;
    }
    let mut path = PathBuf::from(raw_path);
    if path.starts_with("~") {
        if let Some(h) = dirs::home_dir() {
            if let Ok(rest) = path.strip_prefix("~") {
                path = h.join(rest);
            }
        }
    }
    if !path.is_absolute() {
        path = home.join(path);
    }
    let mut candidates = vec![path.clone()];
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n.ends_with(".jsonl") && !n.ends_with(".jsonl.zst"))
    {
        candidates.push(PathBuf::from(format!("{}.zst", path.display())));
    }
    for candidate in candidates {
        let name = candidate.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(caps) = CODEX_ROLLOUT_RE.captures(name) {
            let id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if id.eq_ignore_ascii_case(session_id) && candidate.is_file() && !candidate.is_symlink()
            {
                return Some(candidate);
            }
        }
    }
    None
}

fn discover_codex_database(
    home: &Path,
    database_path: &Path,
    cwd: &str,
    within_min: i64,
) -> Option<Vec<serde_json::Value>> {
    let conn = open_sqlite_readonly(database_path).ok()?;
    let columns = table_columns(&conn, "threads");
    let required = ["id", "rollout_path", "source", "cwd", "archived"];
    if !required.iter().all(|c| columns.contains(*c)) {
        return None;
    }
    let updated_column = if columns.contains("updated_at_ms") {
        "updated_at_ms"
    } else if columns.contains("updated_at") {
        "updated_at"
    } else {
        return None;
    };
    let title_col = if columns.contains("title") {
        "title"
    } else {
        "''"
    };
    let first_col = if columns.contains("first_user_message") {
        "first_user_message"
    } else {
        "''"
    };
    let branch_col = if columns.contains("git_branch") {
        "git_branch"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT id, rollout_path, {updated_column}, source, cwd, {title_col}, {first_col}, {branch_col} \
         FROM threads WHERE archived = 0 AND cwd = ? AND source IN ('cli', 'vscode') \
         ORDER BY {updated_column} DESC, id ASC"
    );
    let mut stmt = conn.prepare(&sql).ok()?;
    let rows = stmt
        .query_map(rusqlite::params![cwd], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, rusqlite::types::Value>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })
        .ok()?;

    let mut sessions = Vec::new();
    for row in rows.flatten() {
        let (session_id, raw_path, raw_updated, source, stored_cwd, raw_title, first_user, git) =
            row;
        if !UUID_RE.is_match(&session_id) {
            continue;
        }
        let Some(rollout) =
            existing_codex_rollout(home, raw_path.as_deref().unwrap_or(""), &session_id)
        else {
            continue;
        };
        let updated_json = match raw_updated {
            rusqlite::types::Value::Integer(i) => serde_json::json!(i),
            rusqlite::types::Value::Real(f) => serde_json::json!(f),
            rusqlite::types::Value::Text(s) => serde_json::json!(s),
            _ => serde_json::Value::Null,
        };
        let updated = timestamp_to_millis(&updated_json)
            .or_else(|| mtime_millis(&rollout))
            .unwrap_or(0);
        if !within(updated, within_min) {
            continue;
        }
        let title_value = raw_title.filter(|t| !t.trim().is_empty()).or(first_user);
        sessions.push(serde_json::json!({
            "tool": "codex",
            "source": format!("codex-{source}"),
            "session_id": session_id,
            "path": rollout.to_string_lossy(),
            "title": one_line(title_value.as_deref().unwrap_or(""), 200)
                .if_empty("(untitled)"),
            "cwd": stored_cwd,
            "branch": git,
            "updated_at_ms": updated,
            "updated_at": iso_from_millis(Some(updated)),
            "source_repo_root_path": null,
        }));
    }
    Some(sessions)
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
}

fn iter_codex_rollouts(home: &Path, include_archived: bool) -> Vec<PathBuf> {
    let names: &[&str] = if include_archived {
        &["sessions", "archived_sessions"]
    } else {
        &["sessions"]
    };
    let mut out = Vec::new();
    for name in names {
        let root = home.join(name);
        if !root.is_dir() || root.is_symlink() {
            continue;
        }
        walk_rollouts(&root, &mut out);
    }
    out
}

fn walk_rollouts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            walk_rollouts(&path, out);
        } else if path.is_file() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if CODEX_ROLLOUT_RE.is_match(name) {
                out.push(path);
            }
        }
    }
}

fn codex_rollout_head(path: &Path) -> Option<serde_json::Value> {
    let (records, _) = read_codex_jsonl(path).ok()?;
    for record in records.iter().take(10) {
        if record.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
            if let Some(payload) = record.get("payload") {
                if payload.is_object() {
                    return Some(payload.clone());
                }
            }
        }
    }
    None
}

fn discover_codex_files(home: &Path, cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let mut sessions = Vec::new();
    for path in iter_codex_rollouts(home, false) {
        let updated = mtime_millis(&path).unwrap_or(0);
        if !within(updated, within_min) {
            continue;
        }
        let Some(metadata) = codex_rollout_head(&path) else {
            continue;
        };
        let source = metadata
            .get("source")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if source != "cli" && source != "vscode" {
            continue;
        }
        let meta_cwd = metadata.get("cwd").and_then(|c| c.as_str()).unwrap_or("");
        if meta_cwd != cwd && !paths_equal(meta_cwd, cwd) {
            continue;
        }
        let session_id = metadata
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .unwrap_or_else(|| codex_id_from_path(&path));
        if !UUID_RE.is_match(&session_id) {
            continue;
        }
        let branch = metadata
            .get("git")
            .and_then(|g| g.get("branch"))
            .and_then(|b| b.as_str());
        sessions.push(serde_json::json!({
            "tool": "codex",
            "source": format!("codex-{source}"),
            "session_id": session_id,
            "path": path.to_string_lossy(),
            "title": "(untitled)",
            "cwd": cwd,
            "branch": branch,
            "updated_at_ms": updated,
            "updated_at": iso_from_millis(Some(updated)),
            "source_repo_root_path": null,
        }));
    }
    sessions
}

/// Discover Codex sessions for cwd (SQLite state preferred, file walk fallback).
pub fn discover_codex(cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let home = codex_home();
    if let Some(database_path) = codex_state_database(&home) {
        if let Some(sessions) = discover_codex_database(&home, &database_path, cwd, within_min) {
            return sort_and_dedupe(sessions);
        }
    }
    sort_and_dedupe(discover_codex_files(&home, cwd, within_min))
}

/// Resolve a Codex session by native UUID when not in the cwd-filtered list.
pub fn find_codex_id(session_id: &str, cwd: &str) -> Option<serde_json::Value> {
    let home = codex_home();
    for path in iter_codex_rollouts(&home, true) {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(caps) = CODEX_ROLLOUT_RE.captures(name) {
            let id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if id.eq_ignore_ascii_case(session_id) {
                let updated = mtime_millis(&path).unwrap_or(0);
                return Some(serde_json::json!({
                    "tool": "codex",
                    "source": "codex",
                    "session_id": codex_id_from_path(&path),
                    "path": path.to_string_lossy(),
                    "title": null,
                    "cwd": cwd,
                    "updated_at_ms": updated,
                }));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn read_codex_jsonl_inert_user_assistant() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".jsonl").unwrap();
        // Use a valid rollout-ish name for path id extraction only; content drives turns.
        writeln!(
            tmp,
            r#"{{"type":"session_meta","timestamp":"2026-01-01T00:00:00Z","payload":{{"id":"11111111-1111-1111-1111-111111111111","cwd":"/proj","source":"cli"}}}}"#
        )
        .unwrap();
        writeln!(
            tmp,
            r#"{{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"hello codex"}}]}}}}"#
        )
        .unwrap();
        writeln!(
            tmp,
            r#"{{"type":"response_item","timestamp":"2026-01-01T00:00:02Z","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"hi there"}}]}}}}"#
        )
        .unwrap();
        let r = read_codex_session(tmp.path().to_str().unwrap(), 200).unwrap();
        assert_eq!(r["tool"], "codex");
        assert_eq!(r["session_id"], "11111111-1111-1111-1111-111111111111");
        let turns = r["turns"].as_array().unwrap();
        assert!(turns.len() >= 2);
        let joined = serde_json::to_string(&r).unwrap();
        assert!(joined.contains("hello codex"));
        assert!(joined.contains("hi there"));
    }

    #[test]
    fn discover_codex_from_sqlite_state() {
        let _lock = lock_env();
        let home = tempfile::tempdir().unwrap();
        let prev = std::env::var("CODEX_HOME").ok();
        unsafe { std::env::set_var("CODEX_HOME", home.path()) };

        let sessions_dir = home.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let sid = "22222222-2222-2222-2222-222222222222";
        let rollout_name = format!("rollout-2026-01-01T00-00-00-{sid}.jsonl");
        let rollout_path = sessions_dir.join(&rollout_name);
        std::fs::write(
            &rollout_path,
            format!(
                r#"{{"type":"session_meta","timestamp":"2026-01-01T00:00:00Z","payload":{{"id":"{sid}","cwd":"/work/proj","source":"cli"}}}}
{{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"from db"}}]}}}}
"#
            ),
        )
        .unwrap();

        let db_path = home.path().join("state_1.sqlite");
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE threads (
                    id TEXT PRIMARY KEY,
                    rollout_path TEXT,
                    updated_at_ms INTEGER,
                    source TEXT,
                    cwd TEXT,
                    archived INTEGER,
                    title TEXT,
                    first_user_message TEXT,
                    git_branch TEXT
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO threads (id, rollout_path, updated_at_ms, source, cwd, archived, title, first_user_message, git_branch)
                 VALUES (?1, ?2, ?3, 'cli', '/work/proj', 0, 'DB title', 'from db', 'main')",
                rusqlite::params![
                    sid,
                    rollout_path.to_string_lossy().as_ref(),
                    1_700_000_000_000_i64
                ],
            )
            .unwrap();
        }

        let found = discover_codex("/work/proj", 0);
        match prev {
            Some(v) => unsafe { std::env::set_var("CODEX_HOME", v) },
            None => unsafe { std::env::remove_var("CODEX_HOME") },
        }
        assert_eq!(found.len(), 1, "expected one session, got {found:?}");
        assert_eq!(found[0]["session_id"], sid);
        assert_eq!(found[0]["source"], "codex-cli");
        assert!(
            found[0]["title"]
                .as_str()
                .unwrap_or("")
                .contains("DB title")
                || found[0]["title"].as_str().unwrap_or("").contains("from db")
        );

        let shown = read_codex_session(found[0]["path"].as_str().unwrap(), 100).unwrap();
        assert!(
            shown["turns"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.get("text").and_then(|x| x.as_str()) == Some("from db"))
        );
    }

    #[test]
    fn discover_codex_empty_cwd_fail_closed() {
        let _lock = lock_env();
        let home = tempfile::tempdir().unwrap();
        let prev = std::env::var("CODEX_HOME").ok();
        unsafe { std::env::set_var("CODEX_HOME", home.path()) };
        let found = discover_codex("/no/such/project", 0);
        match prev {
            Some(v) => unsafe { std::env::set_var("CODEX_HOME", v) },
            None => unsafe { std::env::remove_var("CODEX_HOME") },
        }
        assert!(found.is_empty());
    }

    #[test]
    fn compressed_zst_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("rollout-2026-01-01T00-00-00-33333333-3333-3333-3333-333333333333.jsonl.zst");
        std::fs::write(&path, b"not-zstd").unwrap();
        let err = read_codex_session(path.to_str().unwrap(), 80).unwrap_err();
        assert!(
            err.to_string().contains("zstd"),
            "expected zstd error, got {err}"
        );
    }
}
