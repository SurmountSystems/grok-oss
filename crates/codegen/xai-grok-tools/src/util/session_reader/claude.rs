//! Claude Code session discover + inert read (simplified host parity).
//!
//! Full parent-chain recovery / compact segments remain residual; this path
//! still treats every payload as inert text and fails closed on I/O errors.

use super::safe::{ReaderError, one_line, safe_text};
use super::{mtime_millis, slugify};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("uuid re")
});

fn claude_config_dir() -> PathBuf {
    if let Ok(configured) = std::env::var("CLAUDE_CONFIG_DIR") {
        PathBuf::from(configured)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude")
    }
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
            Ok(serde_json::Value::Object(map)) => {
                records.push(serde_json::Value::Object(map));
            }
            Ok(_) => malformed += 1,
            Err(_) => malformed += 1,
        }
    }
    Ok((records, malformed))
}

fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => safe_text(s),
        serde_json::Value::Array(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    parts.push(safe_text(t));
                } else if let Some(t) = block.as_str() {
                    parts.push(safe_text(t));
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

fn message_text(record: &serde_json::Value) -> String {
    let msg = record.get("message").unwrap_or(record);
    if let Some(c) = msg.get("content") {
        return content_text(c);
    }
    if let Some(t) = record.get("text").and_then(|v| v.as_str()) {
        return safe_text(t);
    }
    String::new()
}

/// Simplified inert Claude session read (no parent-chain reconstruction).
pub fn read_claude_session(
    path: &str,
    max_tool_chars: usize,
) -> Result<serde_json::Value, ReaderError> {
    let session_path = PathBuf::from(path);
    let (records, malformed) = read_plain_jsonl(&session_path)?;
    let mut warnings = Vec::new();
    if malformed > 0 {
        warnings.push(serde_json::json!({
            "code": "malformed_records_skipped",
            "message": format!("Skipped {malformed} malformed Claude transcript record(s)."),
        }));
    }

    let mut turns = Vec::new();
    let mut cwd: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut timestamps: Vec<String> = Vec::new();

    for record in &records {
        let ty = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(c) = record.get("cwd").and_then(|v| v.as_str()) {
            cwd = Some(c.to_owned());
        }
        if let Some(b) = record
            .get("gitBranch")
            .or_else(|| record.get("git_branch"))
            .and_then(|v| v.as_str())
        {
            branch = Some(b.to_owned());
        }
        if let Some(ts) = record.get("timestamp").and_then(|v| v.as_str()) {
            timestamps.push(ts.to_owned());
        }

        match ty {
            "user" | "assistant" => {
                let role = if ty == "user" { "user" } else { "assistant" };
                let text = message_text(record);
                // Also skim inert tool_use blocks as named calls (text only).
                let mut tool_calls = Vec::new();
                if let Some(content) = record
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                {
                    for block in content {
                        if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let input_preview = block
                                .get("input")
                                .map(|i| {
                                    let s = i.to_string();
                                    one_line(&s, max_tool_chars)
                                })
                                .unwrap_or_default();
                            tool_calls.push(serde_json::json!({
                                "name": name,
                                "input": input_preview,
                            }));
                        }
                    }
                }
                if text.is_empty() && tool_calls.is_empty() {
                    continue;
                }
                turns.push(serde_json::json!({
                    "role": role,
                    "text": text,
                    "tool_calls": tool_calls,
                    "tool_results": [],
                }));
            }
            // Known meta types: skip without interpreting payloads.
            "system"
            | "summary"
            | "custom-title"
            | "ai-title"
            | "content-replacement"
            | "progress"
            | "file-history-snapshot"
            | "attribution-snapshot"
            | "queue-operation"
            | "last-prompt"
            | "tag"
            | "agent-name"
            | "agent-color"
            | "agent-setting"
            | "mode"
            | "worktree-state"
            | "context-collapse-commit"
            | "context-collapse-snapshot" => {}
            _ if !ty.is_empty() => {
                // Unknown: skip payload (fail closed / do not interpret).
            }
            _ => {}
        }
    }

    let session_id = session_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_owned();
    let title = turns
        .iter()
        .find(|t| t.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|t| t.get("text").and_then(|x| x.as_str()))
        .map(|t| one_line(t, 200));

    let updated_at = timestamps.last().cloned().or_else(|| {
        mtime_millis(&session_path).map(|ms| {
            // ISO-ish fallback from millis (not full host parity; good enough).
            format!("{ms}")
        })
    });

    Ok(serde_json::json!({
        "tool": "claude",
        "source": "claude-code",
        "session_id": session_id,
        "path": session_path.to_string_lossy(),
        "title": title,
        "cwd": cwd,
        "branch": branch,
        "created_at": timestamps.first(),
        "updated_at": updated_at,
        "source_repo_root_path": null,
        "turns": turns,
        "warnings": warnings,
        "last_user_request": turns.iter().rev()
            .find(|t| t.get("role").and_then(|r| r.as_str()) == Some("user"))
            .and_then(|t| t.get("text").cloned()),
        "last_assistant_action": turns.iter().rev()
            .find(|t| t.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .and_then(|t| t.get("text").cloned()),
    }))
}

/// Discover Claude sessions for a cwd (simplified host `_discover_claude`).
pub fn discover_claude(cwd: &str, within_min: i64) -> Vec<serde_json::Value> {
    let projects = claude_config_dir().join("projects");
    if !projects.is_dir() {
        return vec![];
    }
    let expected = projects.join(slugify(cwd));
    let mut project_dirs = Vec::new();
    if expected.is_dir() && !expected.is_symlink() {
        project_dirs.push(expected.clone());
    }
    if let Ok(rd) = std::fs::read_dir(&projects) {
        let mut extras: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p != &expected && p.is_dir() && !p.is_symlink())
            .collect();
        extras.sort();
        project_dirs.extend(extras);
    }

    let mut sessions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    for project in project_dirs {
        let Ok(rd) = std::fs::read_dir(&project) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_symlink() || !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned();
            if !UUID_RE.is_match(&stem) || seen.contains(&stem) {
                continue;
            }
            let updated = mtime_millis(&path).unwrap_or(0);
            if within_min > 0 && now_ms.saturating_sub(updated) > within_min * 60_000 {
                continue;
            }
            let Ok(result) = read_claude_session(path.to_str().unwrap_or(""), 80) else {
                continue;
            };
            if let Some(rcwd) = result.get("cwd").and_then(|v| v.as_str()) {
                let a = std::path::Path::new(rcwd);
                let b = std::path::Path::new(cwd);
                if a != b {
                    // Normalize comparison loosely.
                    if a.components().collect::<Vec<_>>() != b.components().collect::<Vec<_>>() {
                        continue;
                    }
                }
            } else if project != expected {
                continue;
            }
            seen.insert(stem.clone());
            sessions.push(serde_json::json!({
                "tool": "claude",
                "source": "claude-code",
                "session_id": stem,
                "path": path.to_string_lossy(),
                "title": result.get("title").cloned().unwrap_or(serde_json::json!("(untitled)")),
                "cwd": result.get("cwd").cloned().unwrap_or(serde_json::json!(cwd)),
                "branch": result.get("branch"),
                "updated_at_ms": updated,
                "updated_at": result.get("updated_at"),
                "source_repo_root_path": null,
            }));
        }
    }

    sessions.sort_by(|a, b| {
        let am = a.get("updated_at_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let bm = b.get("updated_at_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        bm.cmp(&am)
    });
    sessions
}
