//! Pure helpers for `/btw` side questions (single-shot and multi-turn follow-up).
//!
//! The session actor (`handle_side_question`) owns sampling and persistence;
//! this module builds the request items and session-id policy so multi-turn
//! behaviour is unit-testable without a live model.
//!
//! **History schema:** each turn is a separate `BtwEntry` line in
//! `btw_history.jsonl` sharing the same `btw_session_id` (ordered by
//! `asked_at`). No nested turns array — append-only multi-entry.

use crate::sampling::ConversationItem;

/// One completed Q/A turn in a btw thread (oldest first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtwPriorTurn {
    pub question: String,
    pub answer: String,
}

/// Resolve the btw conversation id: reuse a client-supplied id on follow-up,
/// otherwise mint a new `btw-{uuid}`.
pub fn resolve_btw_session_id(existing: Option<&str>) -> String {
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) => id.to_string(),
        None => format!("btw-{}", uuid::Uuid::new_v4()),
    }
}

/// Whether this request continues an existing btw thread.
pub fn is_follow_up(btw_session_id: Option<&str>, prior_turns: &[BtwPriorTurn]) -> bool {
    !prior_turns.is_empty() || btw_session_id.map(str::trim).is_some_and(|s| !s.is_empty())
}

/// System-reminder body for a side-question user message.
///
/// First turn: one-off constraints including "no follow-up turns".
/// Follow-up: same tool constraints, but acknowledges the side-thread and
/// prior turns already in the request.
pub fn side_question_reminder(tag: &str, follow_up: bool) -> String {
    if follow_up {
        format!(
            "<{tag}>This is a follow-up side question from the user in the same \
             lightweight btw thread. Answer this question directly in a single response.\n\n\
             IMPORTANT CONTEXT:\n\
             - You are a separate, lightweight agent answering side questions\n\
             - Prior turns of this btw thread appear as earlier user/assistant messages \
             after the main conversation snapshot\n\
             - The main agent is NOT interrupted - it continues working independently\n\
             - You share the conversation context but are a completely separate instance\n\
             - Do NOT reference being interrupted or what you were \"previously doing\"\n\n\
             CRITICAL CONSTRAINTS:\n\
             - You have NO tools available - you cannot read files, run commands, search, or take any actions\n\
             - You can ONLY provide information based on what you already know from the conversation \
             and prior turns in this btw thread\n\
             - NEVER say things like \"Let me try...\", \"I'll now...\", \"Let me check...\", or promise to take any action\n\
             - If you don't know the answer, say so - do not offer to look it up or investigate\n\n\
             Simply answer the question with the information you have.</{tag}>"
        )
    } else {
        format!(
            "<{tag}>This is a side question from the user. \
             You must answer this question directly in a single response.\n\n\
             IMPORTANT CONTEXT:\n\
             - You are a separate, lightweight agent spawned to answer this one question\n\
             - The main agent is NOT interrupted - it continues working independently in the background\n\
             - You share the conversation context but are a completely separate instance\n\
             - Do NOT reference being interrupted or what you were \"previously doing\" - that framing is incorrect\n\n\
             CRITICAL CONSTRAINTS:\n\
             - You have NO tools available - you cannot read files, run commands, search, or take any actions\n\
             - This is a one-off response - there will be no follow-up turns\n\
             - You can ONLY provide information based on what you already know from the conversation context\n\
             - NEVER say things like \"Let me try...\", \"I'll now...\", \"Let me check...\", or promise to take any action\n\
             - If you don't know the answer, say so - do not offer to look it up or investigate\n\n\
             Simply answer the question with the information you have.</{tag}>"
        )
    }
}

/// Truncate trailing incomplete assistant tool-call / tool-result runs so the
/// snapshot is valid for Anthropic Messages (and other strict APIs).
///
/// Also drops trailing [`ConversationItem::Reasoning`] left behind when a
/// mid-turn assistant (with in-flight tool calls) is popped — unpaired
/// reasoning would otherwise go out on the wire as an orphaned prefix.
pub fn truncate_incomplete_tool_run(items: &mut Vec<ConversationItem>) {
    while let Some(last) = items.last() {
        match last {
            ConversationItem::Assistant(a) if !a.tool_calls.is_empty() => {
                items.pop();
            }
            ConversationItem::ToolResult(_) => {
                items.pop();
            }
            ConversationItem::Reasoning(_) => {
                items.pop();
            }
            _ => break,
        }
    }
}

/// Build conversation items for a side-question request.
///
/// Starts from a parent-session snapshot (caller applies Messages-only
/// reasoning strip when needed), truncates incomplete tool runs (and any
/// reasoning orphaned by that trim), appends prior btw turns as user/assistant
/// pairs, then the new question wrapped in a system-reminder.
pub fn build_side_question_items(
    mut parent_items: Vec<ConversationItem>,
    prior_turns: &[BtwPriorTurn],
    question: &str,
    tag: &str,
) -> Vec<ConversationItem> {
    truncate_incomplete_tool_run(&mut parent_items);

    let follow_up = !prior_turns.is_empty();
    for turn in prior_turns {
        parent_items.push(ConversationItem::user(turn.question.clone()));
        parent_items.push(ConversationItem::assistant(turn.answer.clone()));
    }

    let reminder = side_question_reminder(tag, follow_up);
    let wrapped = format!("{reminder}\n\n{question}");
    parent_items.push(ConversationItem::user(wrapped));
    parent_items
}

/// Successful side-question reply returned to the ACP client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideQuestionResult {
    pub answer: String,
    pub btw_session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_btw_session_id_mints_when_absent() {
        let a = resolve_btw_session_id(None);
        let b = resolve_btw_session_id(Some(""));
        let c = resolve_btw_session_id(Some("   "));
        assert!(a.starts_with("btw-"), "got {a}");
        assert!(b.starts_with("btw-"), "got {b}");
        assert!(c.starts_with("btw-"), "got {c}");
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_btw_session_id_reuses_existing() {
        let id = "btw-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        assert_eq!(resolve_btw_session_id(Some(id)), id);
        assert_eq!(resolve_btw_session_id(Some("  btw-keep  ")), "btw-keep");
    }

    #[test]
    fn is_follow_up_from_prior_turns_or_session_id() {
        assert!(!is_follow_up(None, &[]));
        assert!(!is_follow_up(Some(""), &[]));
        assert!(is_follow_up(Some("btw-1"), &[]));
        assert!(is_follow_up(
            None,
            &[BtwPriorTurn {
                question: "q".into(),
                answer: "a".into(),
            }]
        ));
    }

    #[test]
    fn first_turn_reminder_says_no_follow_up() {
        let r = side_question_reminder("system-reminder", false);
        assert!(r.contains("no follow-up turns"), "{r}");
        assert!(!r.contains("follow-up side question"), "{r}");
    }

    #[test]
    fn follow_up_reminder_does_not_forbid_continuation() {
        let r = side_question_reminder("system-reminder", true);
        assert!(r.contains("follow-up side question"), "{r}");
        assert!(
            !r.contains("there will be no follow-up turns"),
            "continuation must not claim one-off only: {r}"
        );
        assert!(r.contains("Prior turns"), "{r}");
    }

    #[test]
    fn build_side_question_items_first_turn_is_snapshot_plus_one_user() {
        let parent = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("hello"),
            ConversationItem::assistant("hi"),
        ];
        let items = build_side_question_items(parent, &[], "what is X?", "system-reminder");
        assert_eq!(items.len(), 4);
        let text = items.last().expect("trailing").text_content();
        assert!(text.contains("no follow-up turns"), "{text}");
        assert!(text.contains("what is X?"), "{text}");
    }

    #[test]
    fn build_side_question_items_includes_prior_turns_before_new_question() {
        let parent = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("main work"),
            ConversationItem::assistant("working"),
        ];
        let prior = vec![
            BtwPriorTurn {
                question: "first q".into(),
                answer: "first a".into(),
            },
            BtwPriorTurn {
                question: "second q".into(),
                answer: "second a".into(),
            },
        ];
        let items = build_side_question_items(parent, &prior, "third q?", "system-reminder");
        // system + main user + main asst + 2*(user+asst) + new user = 8
        assert_eq!(items.len(), 8);

        // Prior turns as plain user/assistant (not reminder-wrapped).
        assert_eq!(items[3].text_content(), "first q");
        assert_eq!(items[4].text_content(), "first a");
        assert_eq!(items[5].text_content(), "second q");
        assert_eq!(items[6].text_content(), "second a");
        let text = items.last().expect("trailing").text_content();
        assert!(text.contains("third q?"), "{text}");
        assert!(text.contains("follow-up side question"), "{text}");
        assert!(!text.contains("there will be no follow-up turns"), "{text}");
    }

    #[test]
    fn build_side_question_items_truncates_orphan_tool_use() {
        use xai_grok_sampling_types::ToolCall;
        let parent = vec![
            ConversationItem::user("fix"),
            ConversationItem::assistant_tool_calls(vec![ToolCall {
                id: "call_x".into(),
                name: "read_file".into(),
                arguments: "{}".into(),
            }]),
        ];
        let items = build_side_question_items(parent, &[], "btw?", "system-reminder");
        // orphan tool asst removed → user + btw user
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], ConversationItem::User(_)));
        assert!(matches!(items[1], ConversationItem::User(_)));
    }

    #[test]
    fn side_question_reuses_btw_session_id_contract() {
        // Acceptance: first creates session; follow-up reuses same id.
        let first = resolve_btw_session_id(None);
        let second = resolve_btw_session_id(Some(&first));
        assert_eq!(first, second);
        assert!(first.starts_with("btw-"));
    }
}
