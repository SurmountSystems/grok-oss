mod agent;
mod bg_task;
mod btw;
mod cancel_cause;
mod context_info;
pub mod markdown_content;
pub mod mermaid_content;
mod quote_bar;
mod session_event;
mod subagent;
mod system;
mod thinking;
pub mod tool;
mod user;
mod workflow;

pub use agent::AgentMessageBlock;
pub(crate) use bg_task::KILLED_SIGNAL;
pub use bg_task::{BgTaskBlock, BgTaskKind};
pub use btw::BtwBlock;
pub use cancel_cause::CancelledBy;
pub use context_info::ContextInfoBlock;
pub use session_event::{MemoryCaptureBlock, MemoryCommandKind, SessionEvent, SessionEventBlock};
pub use subagent::{SubagentBlock, SubagentBlockKind};
pub use system::SystemMessageBlock;
pub use thinking::ThinkingBlock;
pub use tool::{
    DiffLineOutput, DiffRenderConfig, DiscoveredTool, EditToolCallBlock, ExecuteToolCallBlock,
    IntegrationSearchToolCallBlock, LineRange, ListDirToolCallBlock, OtherToolCallBlock,
    ReadToolCallBlock, SearchFileMatch, SearchLineMatch, SearchToolCallBlock,
    SentMessagePresentation, SentMessageToolCallBlock, ToolCallBlock, UseToolCallBlock,
    discovered_tool_action, render_diff_hunk_highlighted, render_diff_hunks_highlighted,
};
pub use user::UserPromptBlock;

use unicode_width::UnicodeWidthStr;

use crate::scrollback::types::{BlockContext, BlockLine};

/// Always-visible bubble ⧉ when `[scrollback.display] bubble_copy_buttons` is on.
///
/// Records a hit column on the first line. Does not append spans and does
/// not insert a `BlockLine`: wrap columns, table detect, and selectable
/// line identity stay unchanged. `EntryRenderer` (and the sticky-header
/// path) paint `⧉` at that column, including into the timestamp gutter or
/// right pad when the first content line already fills the wrap width.
pub(crate) fn append_bubble_copy_button(lines: &mut [BlockLine], ctx: &BlockContext) {
    if !ctx.appearance.scrollback.display.bubble_copy_buttons {
        return;
    }
    if lines.is_empty() {
        return;
    }
    let icon_w = crate::glyphs::copy_icon().width();
    let used: usize = lines[0]
        .content
        .spans
        .iter()
        .map(|s| s.content.width())
        .sum();
    let col = if used + 1 + icon_w <= ctx.content_width() {
        used.saturating_add(1)
    } else {
        // Slack is gone: sit in the first pad / timestamp-gutter column so
        // the glyph does not overwrite the last wrap cell.
        ctx.content_width()
    };
    if let Ok(col) = u16::try_from(col) {
        lines[0].copy_button_col = Some(col);
    }
}
pub use workflow::{WorkflowBlock, WorkflowBlockPhase, WorkflowBlockStatus};
