//! Named schedule path onto the existing composer prompt queue.
//!
//! First-arg `queue` or `later`, or `/queue <slash>`, holds the command
//! instead of invoking it this turn. Not a second queue.

use super::command::CommandResult;
use agent_client_protocol as acp;

/// True when `token` is the named hold path (`queue` or `later`).
pub fn is_schedule_token(token: &str) -> bool {
    token.eq_ignore_ascii_case("queue") || token.eq_ignore_ascii_case("later")
}

/// Split a command's args into (hold this turn, remaining args).
pub fn split_schedule_token(args: &str) -> (bool, &str) {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return (false, "");
    }
    match trimmed.split_once(char::is_whitespace) {
        Some((first, rest)) if is_schedule_token(first) => (true, rest.trim()),
        None if is_schedule_token(trimmed) => (true, ""),
        _ => (false, trimmed),
    }
}

fn first_slash_token(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

/// `/compact` and `/compaction` share the compact drain path.
pub fn is_compact_slash(text: &str) -> bool {
    matches!(first_slash_token(text), "/compact" | "/compaction")
}

pub fn is_plan_slash(text: &str) -> bool {
    first_slash_token(text) == "/plan"
}

pub fn compact_command_text(rest: &str) -> String {
    if rest.is_empty() {
        "/compact".to_string()
    } else {
        format!("/compact {rest}")
    }
}

pub fn plan_command_text(rest: &str) -> String {
    if rest.is_empty() {
        "/plan".to_string()
    } else {
        format!("/plan {rest}")
    }
}

/// Description after `/plan`, if any.
pub fn plan_description_from_command(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("/plan").unwrap_or("").trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn slash_name_and_rest(args: &str) -> Option<(&str, &str)> {
    let trimmed = args.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.split_once(char::is_whitespace) {
        Some((name, rest)) => Some((name, rest.trim())),
        None => Some((trimmed, "")),
    }
}

/// Hold a compact/plan command row (`QueueEntryKind::Command`).
pub fn queue_later_command(text: String) -> CommandResult {
    CommandResult::QueueLater {
        text,
        as_command: true,
        wire_blocks: None,
        display_as_skill: false,
    }
}

/// Hold a skill-inject row on the same composer prompt queue.
pub fn queue_later_skill(
    display_text: String,
    prompt_blocks: Vec<acp::ContentBlock>,
) -> CommandResult {
    CommandResult::QueueLater {
        text: display_text,
        as_command: false,
        wire_blocks: Some(prompt_blocks),
        display_as_skill: true,
    }
}

/// Parse `/queue` args into a hold, or `None` when args are empty (list the queue).
pub fn parse_queue_hold_args(args: &str) -> Result<Option<QueueHold>, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Some((name, rest)) = slash_name_and_rest(trimmed) else {
        return Err("Can queue /compaction, /plan, /reports, or /finish.".to_string());
    };
    match name {
        "compact" | "compaction" => Ok(Some(QueueHold::Command(compact_command_text(rest)))),
        "plan" => Ok(Some(QueueHold::Command(plan_command_text(rest)))),
        "reports" => Ok(Some(QueueHold::Reports(rest.to_string()))),
        "finish" => Ok(Some(QueueHold::Finish(rest.to_string()))),
        other => Err(format!(
            "Can queue /compaction, /plan, /reports, or /finish. Got /{other}."
        )),
    }
}

/// A slash the operator asked `/queue` to hold.
pub enum QueueHold {
    Command(String),
    Reports(String),
    Finish(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_schedule_token_detects_queue_and_later() {
        assert_eq!(split_schedule_token("queue"), (true, ""));
        assert_eq!(split_schedule_token("later keep auth"), (true, "keep auth"));
        assert_eq!(split_schedule_token("keep auth"), (false, "keep auth"));
        assert_eq!(split_schedule_token(""), (false, ""));
    }
}
