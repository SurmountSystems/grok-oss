//! Cursor session discovery (CLI chats SQLite + desktop state.vscdb) and inert read.
//!
//! Host `session_reader.py` parity for common list/show cases. Binary/protobuf
//! blobs are unavailable (warning); never fabricate transcript content.

use super::mtime_millis;
use super::safe::{
    ReaderError, decode_jsonish, finalize_result, iso_from_millis, json_preview, one_line,
    open_sqlite_readonly, paths_equal, safe_text, sort_and_dedupe, table_columns,
    timestamp_to_millis, turn, within,
};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("uuid re")
});

const CURSOR_SKIPPED_ROLES: &[&str] = &[
    "system",
    "developer",
    "instruction",
    "instructions",
    "preamble",
];

/// Override root for tests (`GROK_SESSION_READER_CURSOR_ROOT`); else `~/.cursor`.
pub fn cursor_root() -> PathBuf {
    if let Ok(configured) = std::env::var("GROK_SESSION_READER_CURSOR_ROOT") {
        return PathBuf::from(configured);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
}

pub fn cursor_workspace_hash(cwd: &str) -> String {
    format!("{:x}", md5::compute(cwd.as_bytes()))
}

fn cursor_desktop_paths() -> Vec<PathBuf> {
    // Test override: single DB under cursor root.
    let root = cursor_root();
    let mut candidates = vec![
        root.join("desktop-state.vscdb"), // test convenience
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"),
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/Cursor/User/globalStorage/state.vscdb"),
    ];
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("Cursor/User/globalStorage/state.vscdb"));
    }
    if let Ok(override_db) = std::env::var("GROK_SESSION_READER_CURSOR_DESKTOP_DB") {
        candidates.insert(0, PathBuf::from(override_db));
    }
    let mut out = Vec::new();
    for path in candidates {
        if !out.iter().any(|p: &PathBuf| p == &path) {
            out.push(path);
        }
    }
    out
}

fn merge_cursor_metadata(
    target: &mut serde_json::Map<String, serde_json::Value>,
    value: &serde_json::Value,
) {
    let Some(obj) = value.as_object() else {
        return;
    };
    let pairs = [
        ("title", &["title", "name"][..]),
        ("cwd", &["cwd", "workspacePath"][..]),
        ("source_repo_root_path", &["sourceRepoRootPath"][..]),
    ];
    for (target_key, source_keys) in pairs {
        if target
            .get(target_key)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        for key in source_keys {
            if let Some(serde_json::Value::String(s)) = obj.get(*key) {
                if !s.is_empty() {
                    target.insert(target_key.to_owned(), serde_json::json!(s));
                    break;
                }
            }
        }
    }
    if target
        .get("updated_at_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        == 0
    {
        for key in ["updatedAtMs", "lastUpdatedAt", "updated_at_ms"] {
            if let Some(v) = obj.get(key) {
                if let Some(ms) = timestamp_to_millis(v) {
                    target.insert("updated_at_ms".into(), serde_json::json!(ms));
                    break;
                }
            }
        }
    }
    if target
        .get("cwd")
        .and_then(|v| v.as_str())
        .is_none_or(|s| s.is_empty())
    {
        if let Some(workspace) = obj.get("workspaceIdentifier").and_then(|w| w.as_object()) {
            if let Some(uri) = workspace.get("uri").and_then(|u| u.as_object()) {
                if let Some(s) = uri
                    .get("fsPath")
                    .or_else(|| uri.get("path"))
                    .and_then(|v| v.as_str())
                {
                    target.insert("cwd".into(), serde_json::json!(s));
                }
            }
            if target
                .get("cwd")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.is_empty())
            {
                if let Some(s) = workspace.get("fsPath").and_then(|v| v.as_str()) {
                    target.insert("cwd".into(), serde_json::json!(s));
                }
            }
        }
    }
}

fn cursor_cli_metadata(session_dir: &Path) -> serde_json::Map<String, serde_json::Value> {
    let mut metadata = serde_json::Map::from_iter([
        ("title".into(), serde_json::Value::Null),
        ("cwd".into(), serde_json::Value::Null),
        ("updated_at_ms".into(), serde_json::json!(0)),
        ("source_repo_root_path".into(), serde_json::Value::Null),
    ]);
    let meta_path = session_dir.join("meta.json");
    if meta_path.is_file() && !meta_path.is_symlink() {
        if let Ok(text) = std::fs::read_to_string(&meta_path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                merge_cursor_metadata(&mut metadata, &value);
            }
        }
    }
    let store_path = session_dir.join("store.db");
    if store_path.is_file() && !store_path.is_symlink() {
        if let Ok(conn) = open_sqlite_readonly(&store_path) {
            let columns = table_columns(&conn, "meta");
            if columns.contains("key") && columns.contains("value") {
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT key, value FROM meta ORDER BY CASE key \
                     WHEN '0' THEN 0 WHEN 'metadata' THEN 1 WHEN 'updatedAtMs' THEN 2 \
                     WHEN 'title' THEN 3 WHEN 'name' THEN 4 WHEN 'cwd' THEN 5 ELSE 6 END, key",
                ) {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Vec<u8>>(1)
                                .or_else(|_| row.get::<_, String>(1).map(|s| s.into_bytes()))?,
                        ))
                    }) {
                        for row in rows.flatten() {
                            let (key, raw) = row;
                            if let Some(value) = decode_jsonish(&raw) {
                                merge_cursor_metadata(&mut metadata, &value);
                                if matches!(key.as_str(), "title" | "name" | "cwd" | "updatedAtMs")
                                {
                                    merge_cursor_metadata(
                                        &mut metadata,
                                        &serde_json::json!({ key: value }),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let updated = metadata
        .get("updated_at_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if updated == 0 {
        let ms = mtime_millis(&meta_path)
            .into_iter()
            .chain(mtime_millis(&store_path))
            .chain(mtime_millis(session_dir))
            .max()
            .unwrap_or(0);
        metadata.insert("updated_at_ms".into(), serde_json::json!(ms));
    }
    metadata
}

fn discover_cursor_cli(cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let workspace = cursor_root().join("chats").join(cursor_workspace_hash(cwd));
    if !workspace.is_dir() || workspace.is_symlink() {
        return vec![];
    }
    let Ok(rd) = std::fs::read_dir(&workspace) else {
        return vec![];
    };
    let mut children: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    children.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));

    let mut sessions = Vec::new();
    for child in children {
        let name = child.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !UUID_RE.is_match(name) || !child.is_dir() || child.is_symlink() {
            continue;
        }
        let metadata = cursor_cli_metadata(&child);
        let stored_cwd = metadata.get("cwd").and_then(|v| v.as_str());
        if let Some(sc) = stored_cwd {
            if !paths_equal(sc, cwd) {
                continue;
            }
        }
        let updated = metadata
            .get("updated_at_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if !within(updated, within_min) {
            continue;
        }
        let store = child.join("store.db");
        let meta = child.join("meta.json");
        let path = if store.is_file() { store } else { meta };
        if !path.is_file() {
            continue;
        }
        sessions.push(serde_json::json!({
            "tool": "cursor",
            "source": "cursor-cli",
            "session_id": name,
            "path": path.to_string_lossy(),
            "title": metadata.get("title").and_then(|t| t.as_str()).filter(|s| !s.is_empty()).unwrap_or("(untitled)"),
            "cwd": stored_cwd.unwrap_or(cwd),
            "branch": null,
            "updated_at_ms": updated,
            "updated_at": iso_from_millis(Some(updated)),
            "source_repo_root_path": metadata.get("source_repo_root_path"),
        }));
    }
    sessions
}

fn discover_cursor_desktop(cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let mut sessions = Vec::new();
    for path in cursor_desktop_paths() {
        if !path.is_file() || path.is_symlink() {
            continue;
        }
        let Ok(conn) = open_sqlite_readonly(&path) else {
            continue;
        };
        let columns = table_columns(&conn, "composerHeaders");
        let required = [
            "composerId",
            "lastUpdatedAt",
            "isArchived",
            "isSubagent",
            "value",
        ];
        if !required.iter().all(|c| columns.contains(*c)) {
            continue;
        }
        let order = if columns.contains("recency") {
            "recency"
        } else {
            "lastUpdatedAt"
        };
        let sql = format!(
            "SELECT composerId, lastUpdatedAt, value FROM composerHeaders \
             WHERE COALESCE(isArchived, 0) = 0 AND COALESCE(isSubagent, 0) = 0 \
             ORDER BY {order} DESC, composerId ASC"
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, rusqlite::types::Value>(1)?,
                row.get::<_, Vec<u8>>(2)
                    .or_else(|_| row.get::<_, String>(2).map(|s| s.into_bytes()))?,
            ))
        }) else {
            continue;
        };
        for row in rows.flatten() {
            let (session_id, raw_updated, raw_value) = row;
            let updated_json = match raw_updated {
                rusqlite::types::Value::Integer(i) => serde_json::json!(i),
                rusqlite::types::Value::Real(f) => serde_json::json!(f),
                rusqlite::types::Value::Text(s) => serde_json::json!(s),
                _ => serde_json::Value::Null,
            };
            let value = decode_jsonish(&raw_value).unwrap_or(serde_json::Value::Null);
            let mut metadata = serde_json::Map::from_iter([
                ("title".into(), serde_json::Value::Null),
                ("cwd".into(), serde_json::Value::Null),
                (
                    "updated_at_ms".into(),
                    serde_json::json!(timestamp_to_millis(&updated_json).unwrap_or(0)),
                ),
                ("source_repo_root_path".into(), serde_json::Value::Null),
            ]);
            merge_cursor_metadata(&mut metadata, &value);
            let Some(meta_cwd) = metadata.get("cwd").and_then(|c| c.as_str()) else {
                continue;
            };
            if !paths_equal(meta_cwd, cwd) {
                continue;
            }
            let updated = metadata
                .get("updated_at_ms")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if !within(updated, within_min) {
                continue;
            }
            let branch = value
                .get("gitBranch")
                .and_then(|b| b.as_str())
                .map(|s| s.to_owned());
            sessions.push(serde_json::json!({
                "tool": "cursor",
                "source": "cursor-desktop",
                "session_id": session_id,
                "path": path.to_string_lossy(),
                "title": metadata.get("title").and_then(|t| t.as_str()).filter(|s| !s.is_empty()).unwrap_or("(untitled)"),
                "cwd": meta_cwd,
                "branch": branch,
                "updated_at_ms": updated,
                "updated_at": iso_from_millis(Some(updated)),
                "source_repo_root_path": metadata.get("source_repo_root_path"),
            }));
        }
    }
    sessions
}

/// Discover Cursor CLI + desktop sessions for cwd.
pub fn discover_cursor(cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let mut sessions = discover_cursor_cli(cwd, within_min);
    sessions.extend(discover_cursor_desktop(cwd, within_min));
    sort_and_dedupe(sessions)
}

fn content_blocks(content: &serde_json::Value) -> Vec<&serde_json::Value> {
    match content {
        serde_json::Value::Array(items) => items.iter().filter(|i| i.is_object()).collect(),
        serde_json::Value::Object(_) => vec![content],
        serde_json::Value::String(s) => {
            // String content is handled by caller via content_text.
            let _ = s;
            vec![]
        }
        _ => vec![],
    }
}

fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(t) = item.as_str() {
                    parts.push(t.to_owned());
                } else if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(t.to_owned());
                }
            }
            parts.join("\n")
        }
        serde_json::Value::Object(obj) => {
            for key in ["text", "output", "content"] {
                if let Some(serde_json::Value::String(s)) = obj.get(key) {
                    return s.clone();
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn is_generated_meta_text(text: &str) -> bool {
    let t = text.trim_start();
    (t.starts_with('<') && t.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase()))
        || t.to_ascii_lowercase()
            .starts_with("[request interrupted by user")
}

fn cursor_user_text(text: &str) -> Option<String> {
    static USER_QUERY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<user_query>\s*(.*?)\s*</user_query>").expect("user_query re")
    });
    let matches: Vec<_> = USER_QUERY_RE
        .captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .filter(|s| !s.trim().is_empty())
        .map(safe_text)
        .collect();
    if !matches.is_empty() {
        return Some(matches.join("\n"));
    }
    let stripped = text.trim_start();
    for blocked in [
        "<environment_context",
        "<user_instructions",
        "<system_reminder",
        "<manually_attached_skills",
        "<timestamp",
    ] {
        if stripped.starts_with(blocked) {
            return None;
        }
    }
    Some(safe_text(text))
}

fn render_cursor_role_value(
    value: &serde_json::Value,
    max_tool_chars: usize,
) -> (Vec<serde_json::Value>, bool) {
    let Some(obj) = value.as_object() else {
        return (vec![], false);
    };
    let value_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if matches!(value_type, "thinking" | "reasoning" | "redacted_thinking") {
        return (vec![], true);
    }
    if let Some(role) = obj.get("role").and_then(|r| r.as_str()) {
        let normalized = role.to_ascii_lowercase();
        if CURSOR_SKIPPED_ROLES.contains(&normalized.as_str()) {
            return (vec![], true);
        }
        if !matches!(normalized.as_str(), "user" | "assistant" | "tool") {
            return (vec![], true);
        }
        let content = if let Some(msg) = obj.get("message").and_then(|m| m.as_object()) {
            msg.get("content")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            obj.get("content")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        };
        let mut texts = Vec::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for block in content_blocks(&content) {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(
                block_type,
                "thinking" | "reasoning" | "redacted_thinking" | "signature"
            ) {
                continue;
            }
            if matches!(block_type, "text" | "input_text" | "output_text") {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    let rendered = if normalized == "user" {
                        cursor_user_text(text)
                    } else if is_generated_meta_text(text) {
                        None
                    } else {
                        Some(safe_text(text))
                    };
                    if let Some(r) = rendered {
                        if !r.is_empty() {
                            texts.push(r);
                        }
                    }
                }
            } else if matches!(block_type, "tool_use" | "tool_call") {
                calls.push(serde_json::json!({
                    "id": block.get("id").or_else(|| block.get("call_id")),
                    "name": safe_text(block.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")),
                    "input": json_preview(
                        block.get("input").or_else(|| block.get("arguments")).unwrap_or(&serde_json::Value::Null),
                        max_tool_chars
                    ),
                    "inert": true,
                }));
            } else if matches!(block_type, "tool_result" | "tool_output") {
                results.push(serde_json::json!({
                    "tool_use_id": block.get("tool_use_id").or_else(|| block.get("call_id")),
                    "content": one_line(&content_text(block.get("content").unwrap_or(&serde_json::Value::Null)), max_tool_chars),
                    "is_error": block.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
                    "unavailable": false,
                    "inert": true,
                }));
            }
        }
        if let Some(top_calls) = obj.get("tool_calls").and_then(|c| c.as_array()) {
            for call in top_calls {
                let Some(call_obj) = call.as_object() else {
                    continue;
                };
                let function = call_obj
                    .get("function")
                    .and_then(|f| f.as_object())
                    .unwrap_or(call_obj);
                calls.push(serde_json::json!({
                    "id": call_obj.get("id").or_else(|| function.get("call_id")),
                    "name": safe_text(function.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")),
                    "input": json_preview(
                        function.get("arguments").or_else(|| function.get("input")).unwrap_or(&serde_json::Value::Null),
                        max_tool_chars
                    ),
                    "inert": true,
                }));
            }
        }
        if normalized == "tool" && results.is_empty() {
            results.push(serde_json::json!({
                "tool_use_id": obj.get("tool_call_id").or_else(|| obj.get("call_id")),
                "content": one_line(&content_text(&content), max_tool_chars),
                "is_error": obj.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
                "unavailable": false,
                "inert": true,
            }));
            texts.clear();
        }
        // Also handle plain string content without typed blocks.
        if texts.is_empty() {
            if let serde_json::Value::String(s) = &content {
                let rendered = if normalized == "user" {
                    cursor_user_text(s)
                } else if is_generated_meta_text(s) {
                    None
                } else {
                    Some(safe_text(s))
                };
                if let Some(r) = rendered {
                    if !r.is_empty() {
                        texts.push(r);
                    }
                }
            }
        }
        let text = texts
            .into_iter()
            .filter(|p| !p.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() || !calls.is_empty() || !results.is_empty() {
            return (vec![turn(&normalized, &text, calls, results)], false);
        }
        return (vec![], false);
    }
    let mut turns = Vec::new();
    let mut skipped = false;
    for key in ["messages", "turns", "conversation", "bubbles"] {
        if let Some(list) = obj.get(key).and_then(|v| v.as_array()) {
            for item in list {
                let (item_turns, item_skipped) = render_cursor_role_value(item, max_tool_chars);
                turns.extend(item_turns);
                skipped |= item_skipped;
            }
            return (turns, skipped);
        }
    }
    (vec![], false)
}

fn read_cursor_values(
    rows: impl IntoIterator<Item = (String, Vec<u8>)>,
    max_tool_chars: usize,
    warnings: &mut Vec<serde_json::Value>,
) -> (Vec<serde_json::Value>, Option<String>) {
    let mut turns = Vec::new();
    let mut source_root = None;
    let mut unavailable = 0usize;
    let mut unsafe_count = 0usize;
    let mut row_count = 0usize;
    for (_key, raw) in rows {
        row_count += 1;
        let Some(value) = decode_jsonish(&raw) else {
            unavailable += 1;
            continue;
        };
        if source_root.is_none() {
            source_root = find_nested_string(&value, "sourceRepoRootPath", 0);
        }
        let (value_turns, skipped) = render_cursor_role_value(&value, max_tool_chars);
        turns.extend(value_turns);
        if skipped {
            unsafe_count += 1;
        }
    }
    if unavailable > 0 {
        warnings.push(serde_json::json!({
            "code": "binary_content_unavailable",
            "message": format!(
                "{unavailable} Cursor blob(s) were binary, protobuf, or non-JSON and are unavailable."
            ),
        }));
    }
    if unsafe_count > 0 {
        warnings.push(serde_json::json!({
            "code": "unsafe_records_skipped",
            "message": format!(
                "Skipped {unsafe_count} Cursor system, preamble, instruction, or reasoning payload(s)."
            ),
        }));
    }
    if row_count > 0 && turns.is_empty() {
        warnings.push(serde_json::json!({
            "code": "transcript_content_unavailable",
            "message": "No role-tagged UTF-8 JSON turns were recoverable; binary/protobuf content was not fabricated.",
        }));
    }
    (turns, source_root)
}

fn find_nested_string(value: &serde_json::Value, key: &str, depth: usize) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(s)) = map.get(key) {
                return Some(s.clone());
            }
            for nested in map.values() {
                if let Some(found) = find_nested_string(nested, key, depth + 1) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for nested in items {
                if let Some(found) = find_nested_string(nested, key, depth + 1) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn ordered_cursor_transcript(session_id: &str) -> Option<PathBuf> {
    let projects = cursor_root().join("projects");
    if !projects.is_dir() {
        return None;
    }
    let pattern_end = format!("agent-transcripts/{session_id}/{session_id}.jsonl");
    let mut candidates = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&projects) {
        for entry in rd.flatten() {
            let path = entry
                .path()
                .join("agent-transcripts")
                .join(session_id)
                .join(format!("{session_id}.jsonl"));
            if path.is_file() && !path.is_symlink() {
                candidates.push(path);
            }
        }
    }
    // Prefer newest mtime
    candidates.sort_by_key(|p| std::cmp::Reverse(mtime_millis(p).unwrap_or(0)));
    let _ = pattern_end;
    candidates.into_iter().next()
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
            Ok(v) => records.push(v),
            Err(_) => malformed += 1,
        }
    }
    Ok((records, malformed))
}

fn cursor_cli_store_rows(conn: &rusqlite::Connection) -> Vec<(String, Vec<u8>)> {
    let columns = table_columns(conn, "blobs");
    let key_column = ["id", "key", "hash"]
        .iter()
        .find(|n| columns.contains(**n))
        .copied();
    let value_column = ["data", "value", "blob"]
        .iter()
        .find(|n| columns.contains(**n))
        .copied();
    let (Some(key_column), Some(value_column)) = (key_column, value_column) else {
        return vec![];
    };
    let sql =
        format!("SELECT \"{key_column}\", \"{value_column}\" FROM blobs ORDER BY \"{key_column}\"");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return vec![];
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let key: String = row.get(0).unwrap_or_default();
        let val: Vec<u8> = row
            .get::<_, Vec<u8>>(1)
            .or_else(|_| row.get::<_, String>(1).map(|s| s.into_bytes()))
            .unwrap_or_default();
        Ok((key, val))
    }) else {
        return vec![];
    };
    rows.flatten().collect()
}

fn cursor_desktop_rows(conn: &rusqlite::Connection, session_id: &str) -> Vec<(String, Vec<u8>)> {
    let columns = table_columns(conn, "cursorDiskKV");
    if !columns.contains("key") || !columns.contains("value") {
        return vec![];
    }
    let Ok(mut stmt) = conn
        .prepare("SELECT key, value FROM cursorDiskKV WHERE key = ? OR key LIKE ? ORDER BY key")
    else {
        return vec![];
    };
    let like = format!("bubbleId:{session_id}:%");
    let key_exact = format!("composerData:{session_id}");
    let Ok(rows) = stmt.query_map(rusqlite::params![key_exact, like], |row| {
        let key: String = row.get(0)?;
        let val: Vec<u8> = row
            .get::<_, Vec<u8>>(1)
            .or_else(|_| row.get::<_, String>(1).map(|s| s.into_bytes()))?;
        Ok((key, val))
    }) else {
        return vec![];
    };
    rows.flatten().collect()
}

/// Inert Cursor session read from candidate metadata.
pub fn read_cursor_session(
    candidate: &serde_json::Value,
    max_tool_chars: usize,
) -> Result<serde_json::Value, ReaderError> {
    let mut warnings = Vec::new();
    let session_id = candidate
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let source = candidate
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("cursor")
        .to_owned();
    let path = PathBuf::from(candidate.get("path").and_then(|v| v.as_str()).unwrap_or(""));
    let mut metadata = serde_json::Map::from_iter([
        (
            "title".into(),
            candidate
                .get("title")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
        (
            "cwd".into(),
            candidate
                .get("cwd")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
        (
            "updated_at_ms".into(),
            serde_json::json!(
                candidate
                    .get("updated_at_ms")
                    .and_then(|v| v.as_i64())
                    .or_else(|| mtime_millis(&path))
                    .unwrap_or(0)
            ),
        ),
        (
            "source_repo_root_path".into(),
            candidate
                .get("source_repo_root_path")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
    ]);

    let turns;
    let selected_path;
    let mut source_root: Option<String> = None;

    let transcript = if source == "cursor-transcript"
        || path.extension().and_then(|e| e.to_str()) == Some("jsonl")
    {
        Some(path.clone())
    } else {
        ordered_cursor_transcript(&session_id)
    };

    if let Some(transcript_path) = transcript {
        let (records, malformed) = read_plain_jsonl(&transcript_path)?;
        if malformed > 0 {
            warnings.push(serde_json::json!({
                "code": "malformed_records_skipped",
                "message": format!("Skipped {malformed} malformed Cursor transcript record(s)."),
            }));
        }
        let row_pairs: Vec<(String, Vec<u8>)> = records
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), serde_json::to_vec(&v).unwrap_or_default()))
            .collect();
        let (t, root) = read_cursor_values(row_pairs, max_tool_chars, &mut warnings);
        turns = t;
        source_root = root;
        selected_path = transcript_path;
    } else if source == "cursor-desktop"
        || path.file_name().and_then(|s| s.to_str()) == Some("state.vscdb")
        || path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with(".vscdb"))
    {
        selected_path = path.clone();
        let conn = open_sqlite_readonly(&path)?;
        let rows = cursor_desktop_rows(&conn, &session_id);
        let (t, root) = read_cursor_values(rows, max_tool_chars, &mut warnings);
        turns = t;
        source_root = root;
        if let Ok(mut stmt) = conn.prepare(
            "SELECT lastUpdatedAt, value FROM composerHeaders WHERE composerId = ? ORDER BY lastUpdatedAt DESC LIMIT 1",
        ) {
            if let Ok(mut rows) = stmt.query(rusqlite::params![session_id]) {
                if let Ok(Some(row)) = rows.next() {
                    let raw_updated: rusqlite::types::Value =
                        row.get(0).unwrap_or(rusqlite::types::Value::Null);
                    let raw_value: Vec<u8> = row
                        .get::<_, Vec<u8>>(1)
                        .or_else(|_| row.get::<_, String>(1).map(|s| s.into_bytes()))
                        .unwrap_or_default();
                    let updated_json = match raw_updated {
                        rusqlite::types::Value::Integer(i) => serde_json::json!(i),
                        rusqlite::types::Value::Text(s) => serde_json::json!(s),
                        _ => serde_json::Value::Null,
                    };
                    if let Some(ms) = timestamp_to_millis(&updated_json) {
                        metadata.insert("updated_at_ms".into(), serde_json::json!(ms));
                    }
                    if let Some(value) = decode_jsonish(&raw_value) {
                        merge_cursor_metadata(&mut metadata, &value);
                    }
                }
            }
        }
    } else {
        let session_dir = if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n == "store.db" || n == "meta.json")
        {
            path.parent().unwrap_or(path.as_path()).to_path_buf()
        } else {
            path.clone()
        };
        let cli_meta = cursor_cli_metadata(&session_dir);
        for (k, v) in cli_meta {
            let empty = metadata
                .get(&k)
                .map(|cur| {
                    cur.is_null()
                        || cur.as_str().is_some_and(|s| s.is_empty())
                        || cur == &serde_json::json!(0)
                })
                .unwrap_or(true);
            if empty && !v.is_null() {
                metadata.insert(k, v);
            }
        }
        let store_path = session_dir.join("store.db");
        selected_path = if store_path.is_file() {
            store_path.clone()
        } else {
            path.clone()
        };
        if store_path.is_file() {
            let conn = open_sqlite_readonly(&store_path)?;
            let rows = cursor_cli_store_rows(&conn);
            let (t, root) = read_cursor_values(rows, max_tool_chars, &mut warnings);
            turns = t;
            source_root = root;
        } else {
            turns = vec![];
            warnings.push(serde_json::json!({
                "code": "transcript_content_unavailable",
                "message": "Cursor CLI store.db is absent; no transcript content was fabricated.",
            }));
        }
    }

    if source_root.is_none() {
        source_root = metadata
            .get("source_repo_root_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
    }
    let updated_ms = metadata
        .get("updated_at_ms")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            timestamp_to_millis(
                metadata
                    .get("updated_at_ms")
                    .unwrap_or(&serde_json::Value::Null),
            )
        });
    let mut title = metadata
        .get("title")
        .and_then(|t| t.as_str())
        .filter(|s| *s != "(untitled)")
        .map(|s| s.to_owned());
    if title.is_none() {
        title = turns
            .iter()
            .find(|t| {
                t.get("role").and_then(|r| r.as_str()) == Some("user")
                    && t.get("text")
                        .and_then(|x| x.as_str())
                        .is_some_and(|s| !s.is_empty())
            })
            .and_then(|t| t.get("text").and_then(|x| x.as_str()))
            .map(|t| one_line(t, 200));
    }

    let result = serde_json::json!({
        "tool": "cursor",
        "source": source,
        "session_id": if session_id.is_empty() {
            selected_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_owned()
        } else {
            session_id
        },
        "path": selected_path.to_string_lossy(),
        "title": title,
        "cwd": metadata.get("cwd"),
        "branch": candidate.get("branch"),
        "created_at": null,
        "updated_at": iso_from_millis(updated_ms),
        "source_repo_root_path": source_root,
        "turns": turns,
        "warnings": warnings,
    });
    Ok(finalize_result(result))
}

/// Resolve Cursor session by native UUID when not in cwd-filtered discovery.
pub fn find_cursor_id(session_id: &str, cwd: &str) -> Option<serde_json::Value> {
    if let Some(transcript) = ordered_cursor_transcript(session_id) {
        let updated = mtime_millis(&transcript).unwrap_or(0);
        return Some(serde_json::json!({
            "tool": "cursor",
            "source": "cursor-transcript",
            "session_id": session_id,
            "path": transcript.to_string_lossy(),
            "title": null,
            "cwd": cwd,
            "updated_at_ms": updated,
        }));
    }
    let chats = cursor_root().join("chats");
    if chats.is_dir() {
        if let Ok(rd) = std::fs::read_dir(&chats) {
            let mut paths: Vec<PathBuf> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path().join(session_id).join("store.db"))
                .filter(|p| p.is_file())
                .collect();
            paths.sort();
            if let Some(path) = paths.into_iter().next() {
                let updated = mtime_millis(&path).unwrap_or(0);
                return Some(serde_json::json!({
                    "tool": "cursor",
                    "source": "cursor-cli",
                    "session_id": session_id,
                    "path": path.to_string_lossy(),
                    "title": null,
                    "cwd": cwd,
                    "updated_at_ms": updated,
                }));
            }
        }
    }
    for database_path in cursor_desktop_paths() {
        if !database_path.is_file() {
            continue;
        }
        let Ok(conn) = open_sqlite_readonly(&database_path) else {
            continue;
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT lastUpdatedAt, value FROM composerHeaders WHERE composerId = ? \
             AND COALESCE(isArchived, 0) = 0 AND COALESCE(isSubagent, 0) = 0 \
             ORDER BY lastUpdatedAt DESC LIMIT 1",
        ) else {
            continue;
        };
        let Ok(mut rows) = stmt.query(rusqlite::params![session_id]) else {
            continue;
        };
        if let Ok(Some(row)) = rows.next() {
            let raw_updated: rusqlite::types::Value =
                row.get(0).unwrap_or(rusqlite::types::Value::Null);
            let raw_value: Vec<u8> = row
                .get::<_, Vec<u8>>(1)
                .or_else(|_| row.get::<_, String>(1).map(|s| s.into_bytes()))
                .unwrap_or_default();
            let updated_json = match raw_updated {
                rusqlite::types::Value::Integer(i) => serde_json::json!(i),
                rusqlite::types::Value::Text(s) => serde_json::json!(s),
                _ => serde_json::Value::Null,
            };
            let value = decode_jsonish(&raw_value).unwrap_or(serde_json::Value::Null);
            let mut metadata = serde_json::Map::from_iter([
                ("title".into(), serde_json::Value::Null),
                ("cwd".into(), serde_json::json!(cwd)),
                (
                    "updated_at_ms".into(),
                    serde_json::json!(timestamp_to_millis(&updated_json).unwrap_or(0)),
                ),
                ("source_repo_root_path".into(), serde_json::Value::Null),
            ]);
            merge_cursor_metadata(&mut metadata, &value);
            return Some(serde_json::json!({
                "tool": "cursor",
                "source": "cursor-desktop",
                "session_id": session_id,
                "path": database_path.to_string_lossy(),
                "title": metadata.get("title"),
                "cwd": metadata.get("cwd"),
                "updated_at_ms": metadata.get("updated_at_ms"),
                "source_repo_root_path": metadata.get("source_repo_root_path"),
            }));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn workspace_hash_stable() {
        assert_eq!(
            cursor_workspace_hash("/tmp/proj"),
            cursor_workspace_hash("/tmp/proj")
        );
        assert_ne!(
            cursor_workspace_hash("/tmp/a"),
            cursor_workspace_hash("/tmp/b")
        );
    }

    #[test]
    fn discover_and_read_cursor_cli_store() {
        let _lock = lock_env();
        let root = tempfile::tempdir().unwrap();
        let prev = std::env::var("GROK_SESSION_READER_CURSOR_ROOT").ok();
        unsafe { std::env::set_var("GROK_SESSION_READER_CURSOR_ROOT", root.path()) };

        let cwd = "/work/cursor-proj";
        let sid = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let session_dir = root
            .path()
            .join("chats")
            .join(cursor_workspace_hash(cwd))
            .join(sid);
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("meta.json"),
            serde_json::json!({
                "title": "CLI chat",
                "cwd": cwd,
                "updatedAtMs": 1_700_000_000_000_i64
            })
            .to_string(),
        )
        .unwrap();
        let store = session_dir.join("store.db");
        {
            let conn = rusqlite::Connection::open(&store).unwrap();
            conn.execute_batch(
                "CREATE TABLE blobs (id TEXT PRIMARY KEY, data BLOB);
                 CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB);",
            )
            .unwrap();
            let blob = serde_json::json!({
                "role": "user",
                "content": [{"type": "text", "text": "hello cursor cli"}]
            })
            .to_string();
            conn.execute(
                "INSERT INTO blobs (id, data) VALUES ('1', ?1)",
                rusqlite::params![blob.as_bytes()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('cwd', ?1)",
                rusqlite::params![format!("\"{cwd}\"").as_bytes()],
            )
            .unwrap();
        }

        let found = discover_cursor(cwd, 0);
        assert_eq!(found.len(), 1, "got {found:?}");
        assert_eq!(found[0]["session_id"], sid);
        assert_eq!(found[0]["source"], "cursor-cli");

        let shown = read_cursor_session(&found[0], 200).unwrap();
        match prev {
            Some(v) => unsafe { std::env::set_var("GROK_SESSION_READER_CURSOR_ROOT", v) },
            None => unsafe { std::env::remove_var("GROK_SESSION_READER_CURSOR_ROOT") },
        }
        let joined = serde_json::to_string(&shown).unwrap();
        assert!(
            joined.contains("hello cursor cli"),
            "expected user text in {joined}"
        );
        assert_eq!(shown["tool"], "cursor");
    }

    #[test]
    fn discover_and_read_cursor_desktop_db() {
        let _lock = lock_env();
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("state.vscdb");
        let prev_root = std::env::var("GROK_SESSION_READER_CURSOR_ROOT").ok();
        let prev_db = std::env::var("GROK_SESSION_READER_CURSOR_DESKTOP_DB").ok();
        unsafe {
            std::env::set_var("GROK_SESSION_READER_CURSOR_ROOT", root.path());
            std::env::set_var("GROK_SESSION_READER_CURSOR_DESKTOP_DB", &db_path);
        }

        let cwd = "/work/desktop-proj";
        let sid = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE composerHeaders (
                    composerId TEXT,
                    lastUpdatedAt INTEGER,
                    isArchived INTEGER,
                    isSubagent INTEGER,
                    value BLOB,
                    recency INTEGER
                );
                CREATE TABLE cursorDiskKV (key TEXT PRIMARY KEY, value BLOB);",
            )
            .unwrap();
            let header = serde_json::json!({
                "title": "Desktop chat",
                "cwd": cwd,
            })
            .to_string();
            conn.execute(
                "INSERT INTO composerHeaders (composerId, lastUpdatedAt, isArchived, isSubagent, value, recency)
                 VALUES (?1, ?2, 0, 0, ?3, ?2)",
                rusqlite::params![sid, 1_700_000_000_000_i64, header.as_bytes()],
            )
            .unwrap();
            let bubble = serde_json::json!({
                "role": "user",
                "content": "hello desktop"
            })
            .to_string();
            conn.execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                rusqlite::params![format!("bubbleId:{sid}:1"), bubble.as_bytes()],
            )
            .unwrap();
        }

        let found = discover_cursor(cwd, 0);
        assert_eq!(found.len(), 1, "got {found:?}");
        assert_eq!(found[0]["source"], "cursor-desktop");
        let shown = read_cursor_session(&found[0], 200).unwrap();

        match prev_root {
            Some(v) => unsafe { std::env::set_var("GROK_SESSION_READER_CURSOR_ROOT", v) },
            None => unsafe { std::env::remove_var("GROK_SESSION_READER_CURSOR_ROOT") },
        }
        match prev_db {
            Some(v) => unsafe { std::env::set_var("GROK_SESSION_READER_CURSOR_DESKTOP_DB", v) },
            None => unsafe { std::env::remove_var("GROK_SESSION_READER_CURSOR_DESKTOP_DB") },
        }

        let joined = serde_json::to_string(&shown).unwrap();
        assert!(
            joined.contains("hello desktop"),
            "expected user text in {joined}"
        );
    }

    #[test]
    fn fail_closed_missing_store() {
        let candidate = serde_json::json!({
            "tool": "cursor",
            "source": "cursor-cli",
            "session_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
            "path": "/no/such/store.db",
            "cwd": "/tmp",
        });
        // Missing path: open fails when we try store; path parent doesn't exist.
        let err = read_cursor_session(&candidate, 80);
        // Either error or empty with warning — fail closed means no panic / no fake turns.
        match err {
            Ok(v) => {
                let turns = v["turns"].as_array().map(|a| a.len()).unwrap_or(0);
                assert_eq!(turns, 0);
            }
            Err(e) => assert!(
                e.to_string().contains("SQLite")
                    || e.to_string().contains("not found")
                    || e.to_string().contains("failed")
            ),
        }
    }
}
