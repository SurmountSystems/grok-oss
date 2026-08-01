//! Safe text helpers for untrusted foreign-session history.

use chrono::{TimeZone, Utc};
use std::fmt;
use std::path::Path;

/// Operator-facing reader error (host `ReaderError` parity).
#[derive(Debug)]
pub struct ReaderError {
    message: String,
}

impl ReaderError {
    pub fn msg(m: impl Into<String>) -> Self {
        Self { message: m.into() }
    }
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReaderError {}

/// Result of an in-process handler (stdout/stderr/exit like a process).
#[derive(Debug, Clone)]
pub struct HandlerResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Collapse control chars / NULs so untrusted transcript text cannot smuggle
/// terminal sequences into agent context. Fail-safe: replace, never panic.
pub fn safe_text(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c == '\n' || c == '\t' || c == '\r' {
                c
            } else if c.is_control() || c == '\u{0}' {
                '�'
            } else {
                c
            }
        })
        .collect()
}

/// Single-line preview of untrusted text (host `_one_line` shape).
pub fn one_line(value: &str, limit: usize) -> String {
    let collapsed: String = value
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let safe = safe_text(&collapsed);
    if safe.chars().count() <= limit {
        safe
    } else {
        let truncated: String = safe.chars().take(limit.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Compact JSON preview of untrusted tool payloads (host `_json_preview`).
pub fn json_preview(value: &serde_json::Value, limit: usize) -> String {
    match value {
        serde_json::Value::String(s) => one_line(s, limit),
        other => one_line(&other.to_string(), limit),
    }
}

/// Host `_timestamp_to_millis` parity (seconds vs millis heuristic).
pub fn timestamp_to_millis(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Bool(_) => None,
        serde_json::Value::Number(n) => {
            let number = n.as_i64().or_else(|| n.as_f64().map(|f| f as i64))?;
            Some(if number.abs() < 1_000_000_000_000 {
                number * 1000
            } else {
                number
            })
        }
        serde_json::Value::String(s) if !s.is_empty() => {
            let candidate = s.replace('Z', "+00:00");
            chrono::DateTime::parse_from_rfc3339(&candidate)
                .or_else(|_| chrono::DateTime::parse_from_str(&candidate, "%Y-%m-%dT%H:%M:%S%.f%z"))
                .ok()
                .map(|dt| dt.timestamp_millis())
                .or_else(|| {
                    // Naive ISO without offset → UTC
                    chrono::NaiveDateTime::parse_from_str(
                        s.trim_end_matches('Z'),
                        "%Y-%m-%dT%H:%M:%S%.f",
                    )
                    .or_else(|_| {
                        chrono::NaiveDateTime::parse_from_str(
                            s.trim_end_matches('Z'),
                            "%Y-%m-%dT%H:%M:%S",
                        )
                    })
                    .ok()
                    .map(|ndt| Utc.from_utc_datetime(&ndt).timestamp_millis())
                })
        }
        _ => None,
    }
}

/// Host `_iso_from_millis`.
pub fn iso_from_millis(ms: Option<i64>) -> Option<String> {
    let ms = ms?;
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.to_rfc3339())
}

/// Host `_within`.
pub fn within(updated_at_ms: i64, within_min: i64) -> bool {
    if within_min <= 0 {
        return true;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let delta = now.saturating_sub(updated_at_ms);
    delta >= 0 && delta <= within_min * 60_000
}

/// Normalize path comparison (host `os.path.normpath` spirit).
pub fn paths_equal(a: &str, b: &str) -> bool {
    let pa = Path::new(a);
    let pb = Path::new(b);
    if pa == pb {
        return true;
    }
    // Component-wise equality ignores trailing slashes differences somewhat.
    pa.components().eq(pb.components())
}

/// Open SQLite database read-only; fail closed with [`ReaderError`].
pub fn open_sqlite_readonly(path: &Path) -> Result<rusqlite::Connection, ReaderError> {
    use rusqlite::OpenFlags;
    if path.is_symlink() {
        return Err(ReaderError::msg(format!(
            "refusing to open symlink SQLite store {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(ReaderError::msg(format!(
            "SQLite store not found: {}",
            path.display()
        )));
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = rusqlite::Connection::open_with_flags(path, flags).map_err(|e| {
        ReaderError::msg(format!(
            "failed to open SQLite store {}: {e}",
            path.display()
        ))
    })?;
    conn.execute_batch("PRAGMA query_only = ON;").map_err(|e| {
        ReaderError::msg(format!(
            "failed to set query_only on {}: {e}",
            path.display()
        ))
    })?;
    Ok(conn)
}

/// Table column names via PRAGMA table_info (empty on error).
pub fn table_columns(
    conn: &rusqlite::Connection,
    table: &str,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    // table name is internal allowlist only — never user SQL concatenation of free text.
    let sql = format!("PRAGMA table_info(\"{table}\")");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return out;
    };
    for name in rows.flatten() {
        out.insert(name);
    }
    out
}

/// Decode Cursor/Codex blob values that may be JSON text, hex-JSON, or bytes.
pub fn decode_jsonish(raw: &[u8]) -> Option<serde_json::Value> {
    let text = match std::str::from_utf8(raw) {
        Ok(s) => s.to_owned(),
        Err(_) => return None,
    };
    decode_jsonish_str(&text)
}

/// Decode from string form (host `_decode_jsonish`).
pub fn decode_jsonish_str(text: &str) -> Option<serde_json::Value> {
    let stripped = text.trim();
    if stripped.is_empty() {
        return None;
    }
    if stripped.len().is_multiple_of(2) && stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(bytes) = hex_decode(stripped) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&decoded) {
                    return Some(v);
                }
            }
        }
    }
    serde_json::from_str(stripped).ok()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if !s.len().is_multiple_of(2) {
        return Err(());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, ()> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(()),
    }
}

/// Sort by recency + source priority; dedupe by session_id (host `_sort_and_dedupe`).
pub fn sort_and_dedupe(mut sessions: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    fn source_priority(source: &str) -> i32 {
        match source {
            "cursor-cli" | "claude-code" | "codex-cli" => 0,
            "cursor-desktop" | "codex-vscode" => 1,
            _ => 9,
        }
    }
    sessions.sort_by(|a, b| {
        let am = a.get("updated_at_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let bm = b.get("updated_at_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        bm.cmp(&am)
            .then_with(|| {
                let asrc = a.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let bsrc = b.get("source").and_then(|v| v.as_str()).unwrap_or("");
                source_priority(asrc).cmp(&source_priority(bsrc))
            })
            .then_with(|| {
                let aid = a.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                let bid = b.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                aid.cmp(bid)
            })
            .then_with(|| {
                let ap = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let bp = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
                ap.cmp(bp)
            })
    });
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for session in sessions {
        let id = session
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if !seen.insert(id) {
            continue;
        }
        deduped.push(session);
    }
    deduped
}

/// Host `_finalize_result` for show payloads.
pub fn finalize_result(mut result: serde_json::Value) -> serde_json::Value {
    let turns = result
        .get("turns")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let last_user = turns
        .iter()
        .rev()
        .find(|t| {
            t.get("role").and_then(|r| r.as_str()) == Some("user")
                && t.get("text")
                    .and_then(|x| x.as_str())
                    .is_some_and(|s| !s.is_empty())
        })
        .and_then(|t| t.get("text").and_then(|x| x.as_str()))
        .map(|t| one_line(t, 400));
    let last_assistant = turns.iter().rev().find_map(|t| {
        if t.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            return None;
        }
        if let Some(text) = t.get("text").and_then(|x| x.as_str()) {
            if !text.is_empty() {
                return Some(one_line(text, 400));
            }
        }
        if let Some(calls) = t.get("tool_calls").and_then(|c| c.as_array()) {
            if !calls.is_empty() {
                let names: Vec<&str> = calls
                    .iter()
                    .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                    .collect();
                return Some(format!(
                    "called inert foreign tool(s): {}",
                    names.join(", ")
                ));
            }
        }
        None
    });
    if let Some(obj) = result.as_object_mut() {
        // Pass &str keys directly — `.into()` hits ambiguous Into<String> with unicase.
        obj.entry("turns").or_insert_with(|| serde_json::json!([]));
        obj.entry("warnings")
            .or_insert_with(|| serde_json::json!([]));
        obj.insert("last_user_request".into(), serde_json::json!(last_user));
        obj.insert(
            "last_assistant_action".into(),
            serde_json::json!(last_assistant),
        );
        for field in [
            "title",
            "cwd",
            "branch",
            "created_at",
            "updated_at",
            "source_repo_root_path",
        ] {
            obj.entry(field).or_insert(serde_json::Value::Null);
        }
        if let Some(warnings) = obj.get_mut("warnings").and_then(|w| w.as_array_mut()) {
            warnings.sort_by(|a, b| {
                let ac = a.get("code").and_then(|c| c.as_str()).unwrap_or("");
                let bc = b.get("code").and_then(|c| c.as_str()).unwrap_or("");
                ac.cmp(bc).then_with(|| {
                    let am = a.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    let bm = b.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    am.cmp(bm)
                })
            });
        }
    }
    result
}

/// Build an inert turn object.
pub fn turn(
    role: &str,
    text: &str,
    tool_calls: Vec<serde_json::Value>,
    tool_results: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "role": role,
        "text": safe_text(text),
        "tool_calls": tool_calls,
        "tool_results": tool_results,
        "inert": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn safe_text_strips_controls() {
        let s = safe_text("hi\u{0}there\x1b[31mx");
        assert!(!s.contains('\u{0}'));
        assert!(!s.contains('\u{1b}'));
        assert!(s.contains("hi"));
        assert!(s.contains("there"));
    }

    #[test]
    fn one_line_collapses_ws() {
        assert_eq!(one_line("a\n\nb  c", 100), "a b c");
    }

    #[test]
    fn timestamp_millis_seconds_heuristic() {
        assert_eq!(
            timestamp_to_millis(&json!(1_700_000_000)),
            Some(1_700_000_000_000)
        );
        assert_eq!(
            timestamp_to_millis(&json!(1_700_000_000_000_i64)),
            Some(1_700_000_000_000)
        );
    }

    #[test]
    fn decode_jsonish_plain() {
        let v = decode_jsonish_str(r#"{"a":1}"#).unwrap();
        assert_eq!(v["a"], 1);
    }
}
