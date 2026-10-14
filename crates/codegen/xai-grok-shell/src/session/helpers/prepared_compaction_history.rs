//! Prepares one cache-aligned, image-budgeted compaction request history.

use std::num::NonZeroU64;

use xai_chat_state::compaction_utils::ModelRequestHistory;
use xai_chat_state::image_budget::{
    ImageBudgetOutcome, apply_image_budget_with_limits, image_budget_limits,
};
use xai_grok_sampling_types::ConversationItem;

use super::session_compact::build_compaction_prompt;

/// The exact owned history sent by a compaction attempt and persisted in its artifact.
pub(crate) struct PreparedCompactionHistory {
    /// Items ending with the summarization prompt, after the one image-budget transformation.
    pub(crate) items: Vec<ConversationItem>,
    /// Exact accounting for the transformation applied to `items`.
    pub(crate) image_budget: ImageBudgetOutcome,
}

/// Raw direct-call history or history already fully transformed for sampling.
pub(crate) enum CompactionHistoryInput {
    Raw(Vec<ConversationItem>),
    Prepared(PreparedCompactionHistory),
}

impl From<Vec<ConversationItem>> for CompactionHistoryInput {
    fn from(items: Vec<ConversationItem>) -> Self {
        Self::Raw(items)
    }
}

impl From<PreparedCompactionHistory> for CompactionHistoryInput {
    fn from(history: PreparedCompactionHistory) -> Self {
        Self::Prepared(history)
    }
}

impl CompactionHistoryInput {
    /// Budget raw direct-call history once; preserve an already-prepared history verbatim.
    pub(crate) fn prepare(
        self,
        max_request_bytes: Option<NonZeroU64>,
        compaction_tool_tokens: u64,
    ) -> PreparedCompactionHistory {
        match self {
            Self::Raw(items) => prepare_items(items, max_request_bytes, compaction_tool_tokens),
            Self::Prepared(history) => history,
        }
    }
}

/// Append the summarization prompt, then apply the compaction request's image budget once.
pub(crate) fn build_compaction_chat_history(
    mut chat_history: Vec<ConversationItem>,
    user_context: Option<&str>,
    use_short_prompt: bool,
    max_request_bytes: Option<NonZeroU64>,
    compaction_tool_tokens: u64,
) -> PreparedCompactionHistory {
    let prompt = build_compaction_prompt(user_context, use_short_prompt);
    chat_history.push(ConversationItem::user(prompt));
    prepare_items(chat_history, max_request_bytes, compaction_tool_tokens)
}

fn prepare_items(
    items: Vec<ConversationItem>,
    max_request_bytes: Option<NonZeroU64>,
    compaction_tool_tokens: u64,
) -> PreparedCompactionHistory {
    // Project an agent message once, before the budget, so a later Prepared
    // replay does not prepend the label again.
    let items = ModelRequestHistory::from_raw(items).into_items();
    let (trigger_bytes, reclaim_target_bytes) =
        effective_image_budget_limits(max_request_bytes, compaction_tool_tokens);
    // Budget the pre-strip body so reserved tool headroom can lower the
    // trigger under a small image and record the eviction.
    let budgeted = apply_image_budget_with_limits(items, trigger_bytes, reclaim_target_bytes);
    let mut outcome = budgeted.outcome;
    // Compact HTTP is not the vision path. Drop data URLs after the budget
    // so the 47 MB trigger is not the only reason a URL stays out of the
    // request. Agent-message images stay: that projection is the label plus
    // the image part.
    // See [xAI image understanding](https://docs.x.ai/docs/guides/image-understanding)
    // (accessed: 2026-09-01).
    let mut items = strip_non_agent_images(budgeted.items);
    let _ = xai_chat_state::image_handles::repair_conversation_images_for_api(&mut items);
    outcome.inline_images = 0;
    PreparedCompactionHistory {
        items,
        image_budget: outcome,
    }
}

/// Replace inline images with `[image]` except on an agent-message user item.
fn strip_non_agent_images(conversation: Vec<ConversationItem>) -> Vec<ConversationItem> {
    use xai_grok_sampling_types::SyntheticReason;
    conversation
        .into_iter()
        .map(|item| match item {
            ConversationItem::User(user)
                if user.synthetic_reason == SyntheticReason::AgentMessage =>
            {
                ConversationItem::User(user)
            }
            other => {
                let mut stripped = xai_chat_state::compaction_utils::strip_images(vec![other]);
                stripped
                    .pop()
                    .expect("strip_images keeps each conversation item")
            }
        })
        .collect()
}

fn effective_image_budget_limits(
    max_request_bytes: Option<NonZeroU64>,
    compaction_tool_tokens: u64,
) -> (usize, usize) {
    let (trigger_bytes, reclaim_target_bytes) = image_budget_limits(max_request_bytes);
    // The existing tool estimate is bytes/4; invert that same heuristic here.
    // Saturation is conservative: an unrepresentable reserve leaves no image budget.
    let reserved_bytes =
        usize::try_from(xai_token_estimation::estimate_chars(compaction_tool_tokens))
            .unwrap_or(usize::MAX);
    (
        trigger_bytes.saturating_sub(reserved_bytes),
        reclaim_target_bytes.saturating_sub(reserved_bytes),
    )
}

#[cfg(test)]
#[path = "prepared_compaction_history_tests.rs"]
mod tests;
