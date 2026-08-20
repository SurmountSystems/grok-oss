//! Block implementations for v3 pager.
//!
//! Each block type represents a different kind of content in the scrollback.

mod agent;
mod bg_task;
mod btw;
mod context_info;
mod credit_limit;
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
pub use bg_task::{BgTaskBlock, BgTaskKind};
pub use btw::BtwBlock;
pub use context_info::ContextInfoBlock;
pub use credit_limit::{CreditLimitBlock, CreditLimitCardAction};
pub use session_event::{SessionEvent, SessionEventBlock};
pub use subagent::{SubagentBlock, SubagentBlockKind};
pub use system::SystemMessageBlock;
pub use thinking::ThinkingBlock;
pub use tool::{
    DiffLineOutput, DiffRenderConfig, DiscoveredTool, EditToolCallBlock, ExecuteToolCallBlock,
    IntegrationSearchToolCallBlock, LineRange, ListDirToolCallBlock, OtherToolCallBlock,
    ReadToolCallBlock, SearchFileMatch, SearchLineMatch, SearchToolCallBlock, ToolCallBlock,
    UseToolCallBlock, discovered_tool_action, render_diff_hunk_highlighted,
    render_diff_hunks_highlighted,
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

// Backwards compatibility alias
pub type EditBlock = EditToolCallBlock;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::AppearanceConfig;
    use crate::scrollback::types::{BlockContext, DisplayMode};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    fn bubble_ctx(width: u16) -> BlockContext {
        let mut appearance = AppearanceConfig::default();
        appearance.scrollback.display.bubble_copy_buttons = true;
        BlockContext {
            mode: DisplayMode::Expanded,
            is_running: false,
            width,
            raw: false,
            max_lines: None,
            appearance,
            is_selected: false,
            cwd: None,
        }
    }

    fn paints_copy_icon(line: &BlockLine) -> bool {
        if line.copy_button_col.is_none() {
            return false;
        }
        let icon_w = crate::glyphs::copy_icon().width() as u16;
        let width = icon_w.saturating_add(4).max(8);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
        line.paint_bubble_copy_button(&mut buf, 0, width, 0, Style::default());
        let col = width.saturating_sub(icon_w.max(1));
        buf.cell((col, 0))
            .is_some_and(|c| c.symbol() == crate::glyphs::copy_icon())
    }

    /// Direct helper: a first line that already uses the full content width
    /// must still mark a hit column without changing wrap geometry.
    #[test]
    fn append_bubble_copy_button_paints_when_first_line_fills_content_width() {
        let ctx = bubble_ctx(20);
        let filled = "X".repeat(ctx.content_width());
        let mut lines = vec![BlockLine::text(filled)];
        let used: usize = lines[0]
            .content
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum();
        assert_eq!(used, ctx.content_width());
        assert!(used + 1 + crate::glyphs::copy_icon().width() > ctx.content_width());
        append_bubble_copy_button(&mut lines, &ctx);
        assert_eq!(
            lines.len(),
            1,
            "bubble copy must not insert a chrome line into output().lines"
        );
        let after: usize = lines[0]
            .content
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum();
        assert_eq!(
            after, used,
            "bubble copy must not change first-line wrap width"
        );
        assert_eq!(
            lines[0].copy_button_col,
            Some(u16::try_from(ctx.content_width()).expect("content width fits u16")),
            "a full-width first line marks the first pad / timestamp-gutter column"
        );
        assert!(
            lines[0]
                .content
                .spans
                .iter()
                .all(|s| s.content.as_ref() != crate::glyphs::copy_icon()),
            "the copy icon is paint, not wrap content"
        );
        assert!(
            paints_copy_icon(&lines[0]),
            "a full-width first line must still paint the copy icon at the hit column"
        );
    }
}
