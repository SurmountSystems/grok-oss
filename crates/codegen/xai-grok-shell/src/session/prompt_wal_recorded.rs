//! Match WAL operator text against committed Human turns.
//!
//! Chat history is JSON-parsed user text. Do not substring-search the raw
//! JSONL file for decoded WAL bodies (escaped quotes miss; assistant lines
//! false-hit). `/goal <rest>` matches `A goal has been set: <rest>` in the
//! user turn, including a system-reminder that sits before `<user_query>`.

const USER_QUERY_OPEN: &str = "<user_query>";
const USER_QUERY_CLOSE: &str = "</user_query>";
const GOAL_SET_PREFIX: &str = "A goal has been set: ";

/// Inner text of `<user_query>` when present. Otherwise the trimmed line.
pub fn unwrap_user_query(text: &str) -> &str {
    let t = text.trim();
    let Some(start) = t.find(USER_QUERY_OPEN) else {
        return t;
    };
    let inner = &t[start + USER_QUERY_OPEN.len()..];
    let Some(end) = inner.find(USER_QUERY_CLOSE) else {
        return inner.trim();
    };
    inner[..end].trim()
}

fn json_line_is_user(value: &serde_json::Value) -> bool {
    let role = value.get("role").and_then(|r| r.as_str());
    let ty = value.get("type").and_then(|r| r.as_str());
    role == Some("user") || ty == Some("user")
}

fn json_content_text(content: &serde_json::Value) -> Option<String> {
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for part in arr {
            if let Some(s) = part.as_str() {
                parts.push(s.to_string());
                continue;
            }
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            }
        }
        if parts.is_empty() {
            return None;
        }
        return Some(parts.join("\n"));
    }
    content
        .get("text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

/// User-turn bodies from `chat_history.jsonl`. JSON-parsed, not a raw file
/// substring of decoded WAL text.
pub fn user_texts_from_chat_history_jsonl(blob: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in blob.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !json_line_is_user(&value) {
            continue;
        }
        let Some(text) = json_content_text(&value["content"]) else {
            continue;
        };
        if !text.trim().is_empty() {
            out.push(text);
        }
    }
    out
}

/// Objective after a `/goal` send. `None` when this is not `/goal <rest>`.
fn slash_goal_objective(text: &str) -> Option<&str> {
    let t = text.trim();
    let rest = t.strip_prefix("/goal")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest)
}

/// Objective from `A goal has been set: <rest>`.
///
/// Prefer the `<user_query>` body when that body holds the sentence. A
/// system-reminder before the tag must not hide it, and a same-line
/// `</user_query>` must not stick to the objective. When the sentence is
/// only in the reminder, search the whole turn.
fn recorded_goal_objective(text: &str) -> Option<&str> {
    let text = text.trim();
    let unwrapped = unwrap_user_query(text);
    let search = if unwrapped.contains(GOAL_SET_PREFIX) {
        unwrapped
    } else {
        text
    };
    let idx = search.find(GOAL_SET_PREFIX)?;
    let after = &search[idx + GOAL_SET_PREFIX.len()..];
    let line = after.lines().next().unwrap_or(after).trim();
    let line = line.split(USER_QUERY_CLOSE).next().unwrap_or(line).trim();
    if line.is_empty() {
        return None;
    }
    Some(line)
}

/// Whether WAL/queue `needle` is the same Human turn as `recorded`.
pub fn operator_text_matches_recorded(needle: &str, recorded: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    let recorded_trim = recorded.trim();
    if recorded_trim == needle {
        return true;
    }
    let unwrapped = unwrap_user_query(recorded);
    if unwrapped == needle {
        return true;
    }
    slash_goal_objective(needle)
        .zip(recorded_goal_objective(recorded))
        .is_some_and(|(goal_rest, recorded_obj)| goal_rest == recorded_obj)
}

/// WAL sends (and interject/queue) missing from chat/prompt/queue.
/// Restore those as pending Human turns. Plan notes and rebuild flush
/// have their own draft/queue restore paths.
pub fn wal_sends_missing_from_history(
    records: &[crate::session::prompt_wal::PromptWalRecord],
    prompt_history: &[String],
    queue_texts: &[String],
    chat_history_blob: Option<&str>,
) -> Vec<crate::session::prompt_wal::PromptWalRecord> {
    use crate::session::prompt_wal::PromptWalKind;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for rec in records {
        match rec.kind {
            PromptWalKind::Send | PromptWalKind::Interject | PromptWalKind::Queue => {}
            PromptWalKind::PlanNotes | PromptWalKind::RebuildFlush => continue,
        }
        let key = rec.text.trim().to_string();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        if operator_text_already_recorded(&rec.text, prompt_history, queue_texts, chat_history_blob)
        {
            continue;
        }
        out.push(rec.clone());
    }
    out
}

/// Whether `text` already exists as a Human turn in history or the pager queue.
///
/// Chat JSONL is parsed as user-turn text. A raw file `contains` of the
/// decoded WAL body is not a match.
pub fn operator_text_already_recorded(
    text: &str,
    prompt_history: &[String],
    queue_texts: &[String],
    chat_history_blob: Option<&str>,
) -> bool {
    let needle = text.trim();
    if needle.is_empty() {
        return true;
    }
    if prompt_history
        .iter()
        .any(|p| operator_text_matches_recorded(needle, p))
    {
        return true;
    }
    if queue_texts
        .iter()
        .any(|p| operator_text_matches_recorded(needle, p))
    {
        return true;
    }
    if let Some(blob) = chat_history_blob {
        for user_text in user_texts_from_chat_history_jsonl(blob) {
            if operator_text_matches_recorded(needle, &user_text) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract: WAL `/goal <rest>` is already recorded when history has
    /// `A goal has been set: <rest>` after unwrapping `<user_query>`.
    #[test]
    fn wal_goal_slash_is_already_recorded_when_history_has_a_goal_has_been_set() {
        let wal = "/goal run just check-remote";
        let blob = concat!(
            r#"{"type":"user","content":[{"type":"text","text":"<user_query>\n# /goal -- pursue an objective\n\nA goal has been set: run just check-remote\n\nStart now.\n</user_query>"}]}"#,
            "\n",
        );
        assert!(
            operator_text_already_recorded(wal, &[], &[], Some(blob)),
            "WAL /goal rest must match parsed user text A goal has been set: rest"
        );
    }

    /// Contract: WAL body with real quotes is already recorded when JSONL
    /// has escaped quotes.
    #[test]
    fn wal_body_with_real_quotes_is_already_recorded_when_jsonl_has_escaped_quotes() {
        let wal = r#"say "noted""#;
        let blob = concat!(
            r#"{"type":"user","content":[{"type":"text","text":"say \"noted\""}]}"#,
            "\n",
        );
        assert!(
            operator_text_already_recorded(wal, &[], &[], Some(blob)),
            "JSON parse must turn escaped quotes into the WAL body"
        );
        assert!(
            !blob.contains(wal),
            "fixture must keep escaped quotes so raw contains cannot pass"
        );
    }

    /// Contract: a WAL send whose body is truly absent from parsed user text
    /// still restores. Assistant JSONL that substring-contains the body is
    /// not a recorded Human turn.
    #[test]
    fn wal_send_whose_body_is_truly_absent_from_parsed_user_text_still_restores() {
        let wal = "operator send that never reached chat history";
        let blob = concat!(
            r#"{"type":"user","content":[{"type":"text","text":"some other turn"}]}"#,
            "\n",
            r#"{"type":"assistant","content":"please do not restore: operator send that never reached chat history"}"#,
            "\n",
        );
        assert!(
            !operator_text_already_recorded(wal, &[], &[], Some(blob)),
            "assistant substring must not count as a committed user turn"
        );
    }

    fn rec(
        kind: crate::session::prompt_wal::PromptWalKind,
        text: &str,
    ) -> crate::session::prompt_wal::PromptWalRecord {
        crate::session::prompt_wal::PromptWalRecord::new("sess-a", kind, text, Vec::new())
    }

    /// Same contracts through `wal_sends_missing_from_history`.
    #[test]
    fn wal_sends_missing_from_history_skips_goal_and_quoted_and_keeps_absent() {
        use crate::session::prompt_wal::PromptWalKind;
        let goal = rec(PromptWalKind::Send, "/goal run just check-remote");
        let quoted = rec(PromptWalKind::Send, r#"say "noted""#);
        let absent = rec(
            PromptWalKind::Send,
            "operator send that never reached chat history",
        );
        let blob = concat!(
            r#"{"type":"user","content":[{"type":"text","text":"<user_query>\nA goal has been set: run just check-remote\n</user_query>"}]}"#,
            "\n",
            r#"{"type":"user","content":[{"type":"text","text":"say \"noted\""}]}"#,
            "\n",
            r#"{"type":"assistant","content":"operator send that never reached chat history"}"#,
            "\n",
        );
        let missing = wal_sends_missing_from_history(
            &[goal.clone(), quoted.clone(), absent.clone()],
            &[],
            &[],
            Some(blob),
        );
        assert_eq!(
            missing.len(),
            1,
            "only the truly absent WAL send restores, got {:?}",
            missing.iter().map(|r| r.text.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(missing[0].text, absent.text);
    }

    /// Operator: "Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary."
    ///
    /// WAL send `/goal do the thing`. History is a system-reminder, then
    /// `<user_query>A goal has been set: do the thing</user_query>` on one
    /// line. This matcher does not read `canceled_turn_resume.json`. The
    /// slash must not be missing. A send that is truly absent still is.
    #[test]
    fn wal_goal_send_is_not_missing_when_reminder_precedes_user_query() {
        let history = "<system-reminder>\nThe session is continuing.\n</system-reminder>\n<user_query>A goal has been set: do the thing</user_query>";
        let goal = rec(
            crate::session::prompt_wal::PromptWalKind::Send,
            "/goal do the thing",
        );
        let absent = rec(
            crate::session::prompt_wal::PromptWalKind::Send,
            "operator send that never reached chat history",
        );
        let blob = concat!(
            r#"{"type":"user","content":[{"type":"text","text":"<system-reminder>\nThe session is continuing.\n</system-reminder>\n<user_query>A goal has been set: do the thing</user_query>"}]}"#,
            "\n",
        );
        assert!(
            operator_text_matches_recorded("/goal do the thing", history),
            "Operator: \"Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary.\" A system-reminder before <user_query> must not hide A goal has been set: do the thing"
        );
        let missing = wal_sends_missing_from_history(&[goal, absent.clone()], &[], &[], Some(blob));
        assert!(
            missing.iter().all(|r| r.text != "/goal do the thing"),
            "Operator: \"Stale prompts at start are still a problem sadly... And yes, what is running is the latest binary.\" WAL /goal do the thing must not be missing; missing={:?}",
            missing.iter().map(|r| r.text.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(missing.len(), 1, "the absent send still restores");
        assert_eq!(missing[0].text, absent.text);
    }
}
