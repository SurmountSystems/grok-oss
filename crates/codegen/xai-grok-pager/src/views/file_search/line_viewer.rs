//! Line viewer popup for selecting line ranges from a file.
//!
//! A centered modal overlay showing syntax-highlighted file content with line numbers.
//! Backed by [`ListPaneState`] for navigation, visual selection, and search.
//! Used to build `@foo/bar.rs:10-12` line references.
//!
//! ## Lifecycle
//!
//! 1. Opened via `:` in dropdown, `Ctrl-L` on element, or `<left>:` after element.
//! 2. User navigates with j/k, searches with `/`, selects range with `v`.
//! 3. **Enter** confirms: the element is updated with the line range and the undo group is closed.
//! 4. **Esc** cancels: the undo group is cancelled, reverting to the pre-viewer state.

use std::ops::Range;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{StatefulWidget, Widget};
use syntect::easy::HighlightLines;

use crate::render::scrollbar::SCROLLBAR_TOTAL_COLS;
use crate::render::wrapping::word_wrap_line;
use crate::scrollback::blocks::markdown_content::MarkdownContent;
use crate::scrollback::blocks::mermaid_content::{MermaidDisplay, mermaid_display};
use crate::scrollback::render::DiagramAffordancePlacement;
use crate::syntax::get_syntect;
use crate::theme::Theme;
use crate::views::list_pane::{
    ListItem, ListPane, ListPaneConfig, ListPaneState, ListPaneStyle, WrapMode,
};

use xai_ratatui_textarea::ElementId;

/// Stable ids for mermaid affordance rows (above source lines and comments).
const MERMAID_AFFORDANCE_ID_BASE: u64 = 2_000_000;

/// A single source line for the line viewer.
#[derive(Clone)]
pub struct SourceLine {
    /// 1-based line number (for display and `@file:N-M` references).
    line_number: usize,
    /// Unique item ID for ListPane selection tracking.
    /// In normal mode this equals `line_number`; in markdown mode, source lines can repeat (table borders) so we use a monotonic counter instead.
    item_id: u64,
    /// Styled content (syntax highlighted). Used in normal mode.
    content: Line<'static>,
    /// Prefix: right-aligned line number (dim, the default).
    prefix: Line<'static>,
    /// Prefix for visual selection range (medium brightness).
    prefix_in_selection: Line<'static>,
    /// Prefix for the cursor line (brightest).
    prefix_cursor: Line<'static>,
    /// Blank prefix for continuation lines in multi-line items.
    prefix_blank: Line<'static>,
    /// Plain text for search matching.
    plain_text: String,
    /// Rendered markdown lines (empty in normal mode).
    rendered_lines: Vec<Line<'static>>,
    /// Per-rendered-line background color (for code blocks).
    rendered_bgs: Vec<Option<Color>>,
    /// Whether this source line has a comment attached (for highlight).
    pub commented: bool,
}

impl SourceLine {
    fn new(
        line_number: usize,
        content: Line<'static>,
        plain_text: String,
        max_digits: usize,
    ) -> Self {
        let (prefix, prefix_in_selection, prefix_cursor, prefix_blank) =
            build_prefixes(line_number, max_digits);
        Self {
            line_number,
            item_id: line_number as u64,
            content,
            prefix,
            prefix_in_selection,
            prefix_cursor,
            prefix_blank,
            plain_text,
            rendered_lines: Vec::new(),
            rendered_bgs: Vec::new(),
            commented: false,
        }
    }

    fn new_markdown(
        item_id: u64,
        line_number: usize,
        rendered_lines: Vec<Line<'static>>,
        rendered_bgs: Vec<Option<Color>>,
        plain_text: String,
        max_digits: usize,
    ) -> Self {
        let (prefix, prefix_in_selection, prefix_cursor, prefix_blank) =
            build_prefixes(line_number, max_digits);
        Self {
            line_number,
            item_id,
            content: Line::default(),
            prefix,
            prefix_in_selection,
            prefix_cursor,
            prefix_blank,
            plain_text,
            rendered_lines,
            rendered_bgs,
            commented: false,
        }
    }

    fn is_markdown(&self) -> bool {
        !self.rendered_lines.is_empty()
    }
}

fn build_prefixes(
    line_number: usize,
    max_digits: usize,
) -> (Line<'static>, Line<'static>, Line<'static>, Line<'static>) {
    let theme = Theme::current();
    let num_str = format!("{:>width$} ", line_number, width = max_digits);
    let blank_str = " ".repeat(num_str.len());
    let prefix = Line::from(Span::styled(
        num_str.clone(),
        Style::default().fg(theme.gray_dim),
    ));
    let prefix_in_selection = Line::from(Span::styled(
        num_str.clone(),
        Style::default().fg(theme.gray),
    ));
    let prefix_cursor = Line::from(Span::styled(
        num_str,
        Style::default().fg(theme.text_secondary),
    ));
    let prefix_blank = Line::from(Span::styled(blank_str, Style::default().fg(theme.gray_dim)));
    (prefix, prefix_in_selection, prefix_cursor, prefix_blank)
}

impl ListItem for SourceLine {
    fn content(&self) -> &Line<'_> {
        if self.is_markdown() {
            // Empty content triggers custom render().
            static EMPTY: std::sync::LazyLock<Line<'static>> =
                std::sync::LazyLock::new(Line::default);
            &EMPTY
        } else {
            &self.content
        }
    }

    fn prefix(&self) -> Option<Line<'_>> {
        if self.commented {
            Some(self.prefix_in_selection.clone())
        } else {
            Some(self.prefix.clone())
        }
    }

    fn prefix_in_selection(&self) -> Option<Line<'_>> {
        Some(self.prefix_in_selection.clone())
    }

    fn prefix_cursor(&self) -> Option<Line<'_>> {
        Some(self.prefix_cursor.clone())
    }

    fn stable_id(&self) -> u64 {
        self.item_id
    }

    fn search_text(&self) -> &str {
        &self.plain_text
    }

    fn copy_text(&self) -> String {
        if self.is_markdown() {
            self.plain_text.clone()
        } else {
            self.content
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if !self.is_markdown() || width == 0 {
            return 1;
        }
        let prefix_w = self
            .prefix()
            .map(|p| crate::views::list_pane::line_display_width(&p))
            .unwrap_or(0);
        let text_w = (width as usize).saturating_sub(prefix_w).max(1);
        let mut total: u16 = 0;
        for line in &self.rendered_lines {
            let wrapped = word_wrap_line(line, text_w);
            total += (wrapped.len() as u16).max(1);
        }
        total.max(1)
    }

    fn goto_line_number(&self) -> Option<usize> {
        Some(self.line_number)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _selected: bool, _focused: bool) {
        if !self.is_markdown() || area.height == 0 || area.width == 0 {
            return;
        }
        let theme = Theme::current();
        let prefix_w = self
            .prefix()
            .map(|p| crate::views::list_pane::line_display_width(&p))
            .unwrap_or(0) as u16;
        let text_x = area.x + prefix_w;
        let text_w = area.width.saturating_sub(prefix_w);

        // Apply commented background tint to the full item area.
        if self.commented {
            buf.set_style(area, Style::default().bg(theme.bg_visual));
        }

        // Render prefix on the first visual line (brighter when commented).
        let pfx = if self.commented {
            &self.prefix_in_selection
        } else {
            &self.prefix
        };
        buf.set_line(area.x, area.y, pfx, prefix_w);

        let mut y = area.y;
        let mut is_first_visual = true;
        for (i, line) in self.rendered_lines.iter().enumerate() {
            let bg = self.rendered_bgs.get(i).copied().flatten();
            let wrapped = word_wrap_line(line, text_w as usize);
            let visual_lines = if wrapped.is_empty() {
                vec![Line::default()]
            } else {
                wrapped
            };
            for wline in &visual_lines {
                if y >= area.y + area.height {
                    break;
                }
                if let Some(bg) = bg {
                    let bg_rect = Rect {
                        x: text_x,
                        y,
                        width: text_w,
                        height: 1,
                    };
                    buf.set_style(bg_rect, Style::default().bg(bg));
                }
                if !is_first_visual {
                    buf.set_line(area.x, y, &self.prefix_blank, prefix_w);
                }
                buf.set_line(text_x, y, wline, text_w);
                y += 1;
                is_first_visual = false;
            }
        }
    }
}

/// An inline review comment displayed between source lines.
pub struct CommentLine {
    pub comment_id: u64,
    item_id: u64,
    line_label: String,
    text: String,
    prefix: Line<'static>,
}

impl CommentLine {
    pub fn new(
        comment_id: u64,
        item_id: u64,
        line_range: &std::ops::Range<usize>,
        text: String,
        max_digits: usize,
    ) -> Self {
        let theme = Theme::current();
        let pad = " ".repeat(max_digits);
        let prefix = Line::from(Span::styled(
            format!("{pad} "),
            Style::default().fg(theme.accent_plan),
        ));
        let line_label = if line_range.len() == 1 {
            format!("L{}", line_range.start)
        } else {
            format!("L{}-{}", line_range.start, line_range.end - 1)
        };
        Self {
            comment_id,
            item_id,
            line_label,
            text,
            prefix,
        }
    }
}

impl ListItem for CommentLine {
    fn content(&self) -> &Line<'_> {
        static EMPTY: std::sync::LazyLock<Line<'static>> = std::sync::LazyLock::new(Line::default);
        &EMPTY
    }

    fn prefix(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn prefix_in_selection(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn prefix_cursor(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn stable_id(&self) -> u64 {
        self.item_id
    }

    fn search_text(&self) -> &str {
        &self.text
    }

    fn copy_text(&self) -> String {
        self.text.clone()
    }

    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 1;
        }
        let prefix_w =
            crate::views::list_pane::line_display_width(&self.prefix().unwrap_or_default());
        let text_w = (width as usize).saturating_sub(prefix_w).max(1);
        self.text
            .split('\n')
            .enumerate()
            .map(|(i, text_line)| {
                let display = if i == 0 {
                    format!(
                        "{} {} {}",
                        crate::glyphs::filled_dot(),
                        self.line_label,
                        text_line
                    )
                } else {
                    format!("  {text_line}")
                };
                word_wrap_line(&Line::from(display), text_w).len() as u16
            })
            .sum::<u16>()
            .max(1)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _selected: bool, _focused: bool) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let theme = Theme::current();
        let prefix_w =
            crate::views::list_pane::line_display_width(&self.prefix().unwrap_or_default()) as u16;
        let text_x = area.x + prefix_w;
        let text_w = area.width.saturating_sub(prefix_w);

        buf.set_line(area.x, area.y, &self.prefix, prefix_w);

        let mut y = area.y;
        for (i, text_line) in self.text.split('\n').enumerate() {
            if y >= area.y + area.height {
                break;
            }
            let line = if i == 0 {
                let bullet = Span::styled(
                    format!("{} ", crate::glyphs::filled_dot()),
                    Style::default().fg(theme.accent_plan),
                );
                let label = Span::styled(
                    format!("{} ", self.line_label),
                    Style::default().fg(theme.gray),
                );
                let body = Span::styled(
                    text_line.to_owned(),
                    Style::default().fg(theme.text_primary),
                );
                Line::from(vec![bullet, label, body])
            } else {
                let indent = Span::styled("  ", Style::default().fg(theme.accent_plan));
                let body = Span::styled(
                    text_line.to_owned(),
                    Style::default().fg(theme.text_primary),
                );
                Line::from(vec![indent, body])
            };
            let wrapped = word_wrap_line(&line, text_w as usize);
            for wline in &wrapped {
                if y >= area.y + area.height {
                    break;
                }
                buf.set_line(text_x, y, wline, text_w);
                y += 1;
            }
        }
    }
}

/// Blank reserved row under a Mermaid diagram; buttons are painted by the
/// draw loop (same pattern as scrollback).
pub struct MermaidAffordanceLine {
    item_id: u64,
    /// Fence body: data for Open / Copy path / Copy source.
    pub source: String,
    prefix: Line<'static>,
}

impl MermaidAffordanceLine {
    fn new(item_id: u64, source: String, max_digits: usize) -> Self {
        let prefix = Line::from(Span::styled(
            " ".repeat(max_digits + 1),
            Style::default().fg(Theme::current().gray_dim),
        ));
        Self {
            item_id,
            source,
            prefix,
        }
    }

    fn prefix_width(&self) -> u16 {
        crate::views::list_pane::line_display_width(&self.prefix) as u16
    }
}

impl ListItem for MermaidAffordanceLine {
    fn content(&self) -> &Line<'_> {
        static EMPTY: std::sync::LazyLock<Line<'static>> = std::sync::LazyLock::new(Line::default);
        &EMPTY
    }

    fn prefix(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn prefix_in_selection(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn prefix_cursor(&self) -> Option<Line<'_>> {
        Some(self.prefix.clone())
    }

    fn stable_id(&self) -> u64 {
        self.item_id
    }

    fn is_selectable(&self) -> bool {
        false
    }

    fn search_text(&self) -> &str {
        ""
    }

    fn copy_text(&self) -> String {
        String::new()
    }

    fn desired_height(&self, _width: u16) -> u16 {
        1
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _selected: bool, _focused: bool) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        // Blank prefix only; write via cell_mut so out-of-bounds coords cannot panic (Buffer::set_line indexes and panics on OOB)
        let prefix_w = self.prefix_width().min(area.width);
        let style = self
            .prefix
            .spans
            .first()
            .map(|s| s.style)
            .unwrap_or_default();
        for dx in 0..prefix_w {
            let Some(cell) = buf.cell_mut((area.x.saturating_add(dx), area.y)) else {
                break;
            };
            cell.set_char(' ');
            cell.set_style(style);
        }
    }
}

/// Source line, review comment, or Mermaid affordance row.
pub enum PlanViewerItem {
    Source(Box<SourceLine>),
    Comment(CommentLine),
    MermaidAffordance(MermaidAffordanceLine),
}

impl PlanViewerItem {
    /// The 1-based source line number, if this is a source line.
    pub fn line_number(&self) -> Option<usize> {
        match self {
            Self::Source(s) => Some(s.line_number),
            Self::Comment(_) | Self::MermaidAffordance(_) => None,
        }
    }

    /// The comment ID, if this is a comment item.
    pub fn comment_id(&self) -> Option<u64> {
        match self {
            Self::Source(_) | Self::MermaidAffordance(_) => None,
            Self::Comment(c) => Some(c.comment_id),
        }
    }
}

impl ListItem for PlanViewerItem {
    fn content(&self) -> &Line<'_> {
        match self {
            Self::Source(s) => s.content(),
            Self::Comment(c) => c.content(),
            Self::MermaidAffordance(m) => m.content(),
        }
    }

    fn prefix(&self) -> Option<Line<'_>> {
        match self {
            Self::Source(s) => s.prefix(),
            Self::Comment(c) => c.prefix(),
            Self::MermaidAffordance(m) => m.prefix(),
        }
    }

    fn prefix_in_selection(&self) -> Option<Line<'_>> {
        match self {
            Self::Source(s) => s.prefix_in_selection(),
            Self::Comment(c) => c.prefix_in_selection(),
            Self::MermaidAffordance(m) => m.prefix_in_selection(),
        }
    }

    fn prefix_cursor(&self) -> Option<Line<'_>> {
        match self {
            Self::Source(s) => s.prefix_cursor(),
            Self::Comment(c) => c.prefix_cursor(),
            Self::MermaidAffordance(m) => m.prefix_cursor(),
        }
    }

    fn stable_id(&self) -> u64 {
        match self {
            Self::Source(s) => s.stable_id(),
            Self::Comment(c) => c.stable_id(),
            Self::MermaidAffordance(m) => m.stable_id(),
        }
    }

    fn is_selectable(&self) -> bool {
        match self {
            Self::Source(_) | Self::Comment(_) => true,
            Self::MermaidAffordance(m) => m.is_selectable(),
        }
    }

    fn search_text(&self) -> &str {
        match self {
            Self::Source(s) => s.search_text(),
            Self::Comment(c) => c.search_text(),
            Self::MermaidAffordance(m) => m.search_text(),
        }
    }

    fn copy_text(&self) -> String {
        match self {
            Self::Source(s) => s.copy_text(),
            Self::Comment(c) => c.copy_text(),
            Self::MermaidAffordance(m) => m.copy_text(),
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        match self {
            Self::Source(s) => s.desired_height(width),
            Self::Comment(c) => c.desired_height(width),
            Self::MermaidAffordance(m) => m.desired_height(width),
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer, selected: bool, focused: bool) {
        match self {
            Self::Source(s) => s.render(area, buf, selected, focused),
            Self::Comment(c) => c.render(area, buf, selected, focused),
            Self::MermaidAffordance(m) => m.render(area, buf, selected, focused),
        }
    }

    fn goto_line_number(&self) -> Option<usize> {
        match self {
            Self::Source(s) => Some(s.line_number),
            Self::Comment(_) | Self::MermaidAffordance(_) => None,
        }
    }
}

/// What kind of content the line viewer is showing. Replaces string-based type sniffing
/// (`title_override == Some("plan.md")`) with a typed enum. Plan-specific behavior (commenting,
/// approval, double-click, shortcuts) dispatches via `match` rather than string comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineViewerKind {
    /// Normal file preview opened from an `@file` reference.
    #[default]
    FilePreview,
    /// Plan document preview (plan.md): supports commenting, approval buttons, send-feedback, and double-click-to-comment.
    PlanPreview,
}

/// Explicit idle CTA recorded in grok_oss.db. None means no recorded row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordedPlanChoice {
    Approve,
    Comment,
    Revise,
    Exit,
}

/// CTA marked for Enter-submit. This is live selection, not a leftover
/// grok-oss.db recorded-choice glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedPlanCta {
    Approve,
    Comment,
    Clarify,
    Revise,
    Exit,
}

fn selected_cta_marks_index(
    selected: Option<SelectedPlanCta>,
    index: usize,
    comment_flow: bool,
) -> bool {
    match (selected, index) {
        (Some(SelectedPlanCta::Approve), 0) => true,
        (Some(SelectedPlanCta::Comment), 1) if !comment_flow => true,
        (Some(SelectedPlanCta::Clarify), 1) if comment_flow => true,
        (Some(SelectedPlanCta::Revise), 2) => true,
        (Some(SelectedPlanCta::Exit), 3) => true,
        _ => false,
    }
}

/// One pad cell on each side of an idle plan action word.
///
/// Operator: "plan search works very well, but the buttons are mushed too
/// close together. Make them a little bigger and spaced out better."
pub(crate) const PLAN_APPROVAL_ACTION_PAD: &str = " ";
/// Pipe between padded actions. With the pad cells, each side of the pipe
/// has two spaces.
pub(crate) const PLAN_APPROVAL_ACTION_BETWEEN: &str = " | ";

/// Wide idle row. Approve, comment, revise, exit:
/// ` approve  |  comment  |  revise  |  exit `
pub(crate) fn plan_approval_spaced_action_row(labels: [&str; 4]) -> String {
    let padded: Vec<String> = labels
        .iter()
        .map(|word| format!("{PLAN_APPROVAL_ACTION_PAD}{word}{PLAN_APPROVAL_ACTION_PAD}"))
        .collect();
    padded.join(PLAN_APPROVAL_ACTION_BETWEEN)
}

#[derive(Default)]
pub struct PlanViewerExtras {
    pub send_button_area: Option<Rect>,
    pub send_hovered: bool,
    pub questions_button_area: Option<Rect>,
    pub questions_hovered: bool,
    pub show_action_buttons: bool,
    pub feedback_active: bool,
    /// Latest explicit Approve / Comment / Revise / Exit for this plan.
    pub recorded_choice: Option<RecordedPlanChoice>,
    /// Live marked CTA. Enter submits this. A grok-oss.db recorded row
    /// does not paint this mark.
    pub selected_cta: Option<SelectedPlanCta>,
    pub approve_button_area: Option<Rect>,
    pub approve_hovered: bool,
    /// Unused Notes hit target (CTA removed; kept so hover/mouse stay typed).
    pub approve_notes_button_area: Option<Rect>,
    pub approve_notes_hovered: bool,
    pub comment_button_area: Option<Rect>,
    pub comment_hovered: bool,
    /// Comment-flow footer paints Clarify. Idle present (Preview or a
    /// focused Revise/Approve box) stays Comment, not Clarify.
    pub comment_flow_active: bool,
    pub abandon_button_area: Option<Rect>,
    pub abandon_hovered: bool,
    pub copy_button_area: Option<Rect>,
    pub copy_hovered: bool,
    /// Magnifying-glass hit target immediately left of copy.
    pub search_button_area: Option<Rect>,
    pub search_hovered: bool,
    pub last_click_at: Option<std::time::Instant>,
    pub gutter_drag_start: Option<usize>,
    pub gutter_drag_end: Option<usize>,
    pub gutter_hovered_line: Option<usize>,
    pub active_commenting_range: Option<std::ops::Range<usize>>,
    pub comment_close_areas: Vec<(u64, Rect)>,
    pub hovered_comment_id: Option<u64>,
    pub close_button_hovered: bool,
}

/// Double-click detection threshold in milliseconds.
pub const DOUBLE_CLICK_MS: u128 = 400;

/// State for the line viewer popup.
pub struct LineViewerState {
    /// What kind of content is being viewed (file vs plan).
    pub kind: LineViewerKind,
    /// The file being viewed.
    pub path: PathBuf,
    /// Viewer items (source lines interleaved with comment annotations).
    pub lines: Vec<PlanViewerItem>,
    /// Raw source lines kept for rebuilding items when comments change.
    source_lines: Vec<SourceLine>,
    /// ListPane state for navigation + visual selection.
    pub list_state: ListPaneState,
    /// The element ID being modified (if editing an existing element).
    pub element_id: Option<ElementId>,
    /// Whether we're inside an undo group (to close/cancel on exit).
    pub in_undo_group: bool,
    /// Cached inner popup area from last render (for mouse hit-testing).
    /// Excludes the divider and footer rows in plan modes, so it matches the area the ListPane was rendered into.
    /// Used for routing list events to `ListPaneState::handle_mouse_event`.
    pub last_popup_area: Option<Rect>,
    /// Cached full modal area (inside the border, including the footer) from the last render.
    /// Used by the click-outside-modal check so clicks landing on the divider or empty space between footer buttons do not close the modal.
    pub last_modal_area: Option<Rect>,
    /// Cached close button rect from last render (for mouse hit-testing).
    pub close_button_area: Option<Rect>,
    /// Whether the close button is hovered (for highlight).
    pub close_hovered: bool,
    /// Cached fullscreen toggle button rect from last render.
    pub fullscreen_button_area: Option<Rect>,
    /// Whether the fullscreen button is hovered.
    pub fullscreen_hovered: bool,
    /// Plan-specific state. `Some` only when `kind == PlanPreview`.
    /// Keeps plan-only fields (buttons, approval, double-click) out of the generic viewer.
    pub plan: Option<PlanViewerExtras>,
    /// Initial scroll range (0-based indices), consumed on the first prepare_layout to center the range in the viewport.
    initial_scroll_range: Option<Range<usize>>,
    /// Optional title override, shown in the title bar instead of the (potentially long) file path. Used for plan previews.
    pub title_override: Option<String>,
    /// Raw markdown content for rebuilding when the display width changes.
    /// `None` for non-markdown viewers.
    markdown_content: Option<String>,
    /// The last `max_table_width` used to build markdown lines.
    /// Compared against the current content width in `prepare_layout` to trigger a rebuild when the viewer is resized.
    last_table_width: Option<usize>,
    /// Last popup width passed to `rebuild_markdown_for_width`.
    /// Same-width paints must not walk the plan body again.
    last_rebuild_width: Option<u16>,
    /// How many times a new width was offered to the markdown rebuild.
    markdown_width_probes: u32,
    /// How many times the markdown body was actually re-parsed.
    markdown_rebuilds: u32,
    /// Copy of comments last applied via `rebuild_with_comments`, so that a width-triggered rebuild can re-interleave them automatically.
    last_comments: Vec<crate::views::plan_approval_view::PlanComment>,
    /// `(source_lines index to follow, diagram source)` for affordance rows.
    mermaid_after: Vec<(usize, String)>,
    /// When `true`, the viewer uses the full overlay area instead of the 75% centered popup.
    /// Toggled by Ctrl+F.
    pub fullscreen: bool,
}

impl LineViewerState {
    /// Open a file and create the viewer state.
    ///
    /// Returns `None` if the file can't be read.
    pub fn open(path: &Path, element_id: Option<ElementId>) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let source_lines = build_source_lines(path, &content);
        let lines = source_lines
            .iter()
            .cloned()
            .map(|s| PlanViewerItem::Source(Box::new(s)))
            .collect();

        let config = ListPaneConfig {
            follow_enabled: false,
            wrap_toggle_enabled: false,
            search_enabled: true,
            copy_enabled: true,
            show_selection_when_unfocused: true,
            visual_select_enabled: true,
            filter_enabled: false,
            goto_line_enabled: true,
        };
        let list_state = ListPaneState::new_with_config(WrapMode::Wrap, false, config);

        Some(Self {
            kind: LineViewerKind::FilePreview,
            path: path.to_path_buf(),
            lines,
            source_lines,
            list_state,
            element_id,
            in_undo_group: false,
            last_popup_area: None,
            last_modal_area: None,
            close_button_area: None,
            close_hovered: false,
            fullscreen_button_area: None,
            fullscreen_hovered: false,
            plan: None,
            initial_scroll_range: None,
            title_override: None,
            markdown_content: None,
            last_table_width: None,
            last_rebuild_width: None,
            markdown_width_probes: 0,
            markdown_rebuilds: 0,
            last_comments: Vec::new(),
            mermaid_after: Vec::new(),
            fullscreen: false,
        })
    }

    /// Open a file and create the viewer with markdown rendering. Same as `open()` but renders the
    /// content as rich markdown instead of raw syntax-highlighted text. Source-line-based navigation is
    /// preserved.
    pub fn open_markdown(path: &Path, element_id: Option<ElementId>) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::open_markdown_content(path.to_path_buf(), content, element_id)
    }

    pub fn open_markdown_content(
        path: impl Into<PathBuf>,
        content: String,
        element_id: Option<ElementId>,
    ) -> Option<Self> {
        if content.trim().is_empty() {
            return None;
        }
        let path = path.into();

        // Defer the actual markdown render to the first `prepare_layout` call, which knows the display width
        // `rebuild_markdown_for_width` will build source_lines with the correct `max_table_width`
        let source_lines = Vec::new();
        let lines = Vec::new();

        let config = ListPaneConfig {
            follow_enabled: false,
            wrap_toggle_enabled: false,
            search_enabled: true,
            copy_enabled: true,
            show_selection_when_unfocused: true,
            visual_select_enabled: true,
            filter_enabled: false,
            goto_line_enabled: true,
        };
        let list_state = ListPaneState::new_with_config(WrapMode::Wrap, false, config);

        Some(Self {
            kind: LineViewerKind::FilePreview,
            path,
            lines,
            source_lines,
            list_state,
            element_id,
            in_undo_group: false,
            last_popup_area: None,
            last_modal_area: None,
            close_button_area: None,
            close_hovered: false,
            fullscreen_button_area: None,
            fullscreen_hovered: false,
            plan: None,
            initial_scroll_range: None,
            title_override: None,
            markdown_content: Some(content),
            last_table_width: None,
            last_rebuild_width: None,
            markdown_width_probes: 0,
            markdown_rebuilds: 0,
            last_comments: Vec::new(),
            mermaid_after: Vec::new(),
            fullscreen: false,
        })
    }

    /// Set initial selection and scroll to a line range (1-based).
    ///
    /// Enters visual mode with the range pre-selected and scrolls so the range is visible (centered if possible).
    pub fn set_initial_selection(&mut self, range: Range<usize>) {
        // Convert 1-based line numbers to 0-based ListPane indices.
        let start_idx = range.start.saturating_sub(1);
        let end_idx = range.end.saturating_sub(1).min(self.lines.len());

        if start_idx < self.lines.len() {
            // Select the start line.
            let Some(start_id) = self.lines.get(start_idx).map(|line| line.stable_id()) else {
                return;
            };
            self.list_state.select_by_id(start_id);

            // Enter visual mode and extend to end line.
            if end_idx > start_idx + 1 {
                self.list_state.enter_visual_mode(&self.lines);
                // Move selection to the end of the range.
                let end_line_idx = (end_idx - 1).min(self.lines.len() - 1);
                if let Some(end_id) = self.lines.get(end_line_idx).map(|line| line.stable_id()) {
                    self.list_state.select_by_id(end_id);
                }
            }

            // Store the range for scroll centering on first render.
            self.initial_scroll_range = Some(start_idx..end_idx);
        }
    }

    /// Rebuild markdown items when the available content width changes. Recomputes `max_table_width`
    /// from the ListPane's content width (total width minus line-number prefix). Markdown then
    /// re-renders with constrained tables so box-drawing borders aren't word-wrapped.
    fn rebuild_markdown_for_width(&mut self, width: u16) {
        let Some(ref content) = self.markdown_content else {
            return;
        };
        // Same-width paints (every keystroke) must not walk the plan body.
        if self.last_rebuild_width == Some(width) {
            return;
        }

        self.markdown_width_probes = self.markdown_width_probes.saturating_add(1);

        let prefix_width = digit_count(source_line_count(content).max(1)) + 1;
        let scrollbar_width = SCROLLBAR_TOTAL_COLS as usize; // gap and track
        let content_width = (width as usize)
            .saturating_sub(prefix_width)
            .saturating_sub(scrollbar_width);

        if self.last_table_width == Some(content_width) {
            self.last_rebuild_width = Some(width);
            return;
        }
        self.last_table_width = Some(content_width);
        self.last_rebuild_width = Some(width);
        self.markdown_rebuilds = self.markdown_rebuilds.saturating_add(1);

        let built = build_markdown_lines(content, Some(content_width));
        self.source_lines = built.source_lines;
        self.mermaid_after = built.mermaid_after;

        if self.last_comments.is_empty() && self.mermaid_after.is_empty() {
            self.lines = self
                .source_lines
                .iter()
                .cloned()
                .map(|s| PlanViewerItem::Source(Box::new(s)))
                .collect();
        } else {
            let comments = self.last_comments.clone();
            self.interleave_comments(&comments);
        }
    }

    /// Screen rects for visible Mermaid affordance rows (for paint + hit-testing).
    pub fn diagram_affordance_placements(
        &self,
        content_area: Rect,
    ) -> Vec<DiagramAffordancePlacement> {
        if content_area.width == 0 || content_area.height == 0 {
            return Vec::new();
        }

        let scroll = self.list_state.scroll_offset();
        let layout = self.list_state.layout();
        let visible = self.list_state.visible_range();
        if visible.is_empty() {
            return Vec::new();
        }
        let first_vi = visible.start;
        let skip_first = self.list_state.first_item_skip_rows();
        let mut placements = Vec::new();

        for vi in visible {
            let pi = self.list_state.to_physical(vi);
            let Some(PlanViewerItem::MermaidAffordance(m)) = self.lines.get(pi) else {
                continue;
            };
            let item_h = layout.item_height(vi);
            let skip = if vi == first_vi { skip_first } else { 0 };
            if skip >= item_h {
                continue;
            }
            // Align with list-pane layout: first visible item may be top-clipped.
            let screen_y_offset = layout
                .virtual_y(vi)
                .saturating_sub(scroll)
                .saturating_add(skip as usize);
            if screen_y_offset >= content_area.height as usize {
                continue;
            }
            let prefix_w = m.prefix_width();
            let text_w = content_area
                .width
                .saturating_sub(prefix_w)
                .saturating_sub(SCROLLBAR_TOTAL_COLS);
            if text_w == 0 {
                continue;
            }
            placements.push(DiagramAffordancePlacement {
                screen_rect: Rect {
                    x: content_area.x.saturating_add(prefix_w),
                    y: content_area.y.saturating_add(screen_y_offset as u16),
                    width: text_w,
                    height: 1,
                },
                source: m.source.clone(),
            });
        }
        placements
    }

    #[cfg(test)]
    pub(crate) fn markdown_content_for_test(&self) -> Option<&str> {
        self.markdown_content.as_deref()
    }

    /// Returns the raw markdown content for feedback formatting.
    pub fn markdown_content_for_feedback(&self) -> Option<String> {
        self.markdown_content.clone()
    }

    /// Mutable access to plan-specific extras, initializing if needed.
    pub fn plan_mut(&mut self) -> &mut PlanViewerExtras {
        self.plan.get_or_insert_with(PlanViewerExtras::default)
    }

    /// Read-only access to plan-specific extras.
    pub fn plan_ref(&self) -> Option<&PlanViewerExtras> {
        self.plan.as_ref()
    }

    pub fn feedback_active(&self) -> bool {
        self.plan.as_ref().is_some_and(|p| p.feedback_active)
    }

    /// Soft plan review: right-side pane, not the 75% centered overlay.
    pub fn is_soft_plan_side_pane(&self) -> bool {
        self.kind == LineViewerKind::PlanPreview && !self.fullscreen
    }

    /// Width reserved on the right for a soft plan pane.
    pub fn soft_plan_pane_width(full_width: u16) -> u16 {
        let half = full_width / 2;
        let min = 24.min(full_width);
        let leave_left = 16.min(full_width.saturating_sub(min));
        half.max(min).min(full_width.saturating_sub(leave_left))
    }

    /// Columns of soft-plan title and body kept inside the frame.
    /// The left border was covering the heading, so `Proposed plan.` painted as `sed plan.`
    const SOFT_PLAN_TEXT_INSET: u16 = 5;

    /// Whether the plan modal should render the action-button footer.
    /// True for plan-approval and casual plan preview (not plain file preview).
    pub fn show_footer(&self) -> bool {
        self.plan
            .as_ref()
            .is_some_and(|p| p.feedback_active || p.show_action_buttons)
    }

    pub fn source_line_at_screen_row(&self, row: u16, content_area: Rect) -> Option<usize> {
        self.item_at_screen_row(row, content_area)?.line_number()
    }

    pub fn comment_id_at_screen_row(&self, row: u16, content_area: Rect) -> Option<u64> {
        self.item_at_screen_row(row, content_area)?.comment_id()
    }

    fn item_at_screen_row(&self, row: u16, content_area: Rect) -> Option<&PlanViewerItem> {
        if row < content_area.y || row >= content_area.y + content_area.height {
            return None;
        }
        let ry = (row - content_area.y) as usize;
        let vy = self.list_state.scroll_offset() + ry;
        let vi = self.list_state.layout().item_at_y(vy)?;
        let pi = self.list_state.to_physical(vi);
        self.lines.get(pi)
    }

    pub fn selected_comment_id(&self) -> Option<u64> {
        let vi = self.list_state.selected_index()?;
        let pi = self.list_state.to_physical(vi);
        self.lines.get(pi)?.comment_id()
    }

    /// The comment whose `[✗]` button, as drawn in the last render, contains the given screen position.
    pub fn comment_close_button_at(&self, col: u16, row: u16) -> Option<u64> {
        self.plan_ref()?
            .comment_close_areas
            .iter()
            .find(|(_, area)| area.contains((col, row).into()))
            .map(|&(id, _)| id)
    }

    /// Prepare the layout for rendering (must be called each frame).
    pub fn prepare_layout(&mut self, width: u16, height: u16) {
        self.rebuild_markdown_for_width(width);
        self.list_state.prepare_layout(&self.lines, width, height);

        // On the first render with an initial range, center the range in the viewport
        // Consumed once so subsequent navigation is normal
        if let Some(range) = self.initial_scroll_range.take() {
            let vp = height as usize;
            let total = self.lines.len();
            let pad = 3usize; // inner padding (lines of context above/below)

            if total <= vp {
                // Entire file fits, so no scrolling is needed
            } else {
                let range_len = range.end.saturating_sub(range.start);
                let offset = if range_len + pad * 2 <= vp {
                    // Range fits with padding: center it
                    let center = range.start + range_len / 2;
                    center.saturating_sub(vp / 2)
                } else {
                    // Range larger than viewport: put the start near the top with padding
                    range.start.saturating_sub(pad)
                };
                // Clamp to valid range.
                let max_offset = total.saturating_sub(vp);
                self.list_state.set_scroll_offset(offset.min(max_offset));
            }
        }
    }

    /// Rebuild `self.lines` from source lines and comments. Comments are inserted after the last source
    /// line in their range. Item IDs for comments use a high base offset to avoid colliding with source
    /// line IDs.
    pub fn rebuild_with_comments(
        &mut self,
        comments: &[crate::views::plan_approval_view::PlanComment],
    ) {
        self.last_comments = comments.to_vec();
        self.interleave_comments(comments);
        // Item count/content changed: force the list pane to recompute wrapping heights on the next render frame
        self.list_state.invalidate_layout();
    }

    /// Interleave source lines with Mermaid affordance rows and comments without updating `last_comments`.
    ///
    /// `mermaid_after` is document-ordered; affordances sit under the diagram art, before any comments on the same source line.
    fn interleave_comments(&mut self, comments: &[crate::views::plan_approval_view::PlanComment]) {
        let max_digits = digit_count(
            self.source_lines
                .last()
                .map(|s| s.line_number)
                .unwrap_or(1)
                .max(1),
        );

        let mut sorted: Vec<_> = comments.iter().collect();
        sorted.sort_by_key(|c| c.line_range.end);

        let mut commented_lines = std::collections::HashSet::new();
        for c in comments {
            for ln in c.line_range.clone() {
                commented_lines.insert(ln);
            }
        }

        let mut items: Vec<PlanViewerItem> = Vec::new();
        let mut comment_idx = 0;
        let comment_id_base: u64 = 1_000_000;
        let mut mermaid_i = 0usize;

        for (src_idx, src) in self.source_lines.iter().enumerate() {
            let ln = src.line_number;
            let mut src = src.clone();
            src.commented = commented_lines.contains(&ln);
            items.push(PlanViewerItem::Source(Box::new(src)));

            while mermaid_i < self.mermaid_after.len()
                && self
                    .mermaid_after
                    .get(mermaid_i)
                    .is_some_and(|m| m.0 == src_idx)
            {
                let Some(diagram) = self.mermaid_after.get(mermaid_i).map(|m| m.1.clone()) else {
                    break;
                };
                items.push(PlanViewerItem::MermaidAffordance(
                    MermaidAffordanceLine::new(
                        MERMAID_AFFORDANCE_ID_BASE + mermaid_i as u64,
                        diagram,
                        max_digits,
                    ),
                ));
                mermaid_i += 1;
            }

            while comment_idx < sorted.len()
                && sorted
                    .get(comment_idx)
                    .is_some_and(|c| c.line_range.end == ln + 1)
            {
                let Some(c) = sorted.get(comment_idx).copied() else {
                    break;
                };
                let item_id = comment_id_base + c.id;
                items.push(PlanViewerItem::Comment(CommentLine::new(
                    c.id,
                    item_id,
                    &c.line_range,
                    c.text.clone(),
                    max_digits,
                )));
                comment_idx += 1;
            }
        }

        for c in sorted.get(comment_idx..).unwrap_or(&[]) {
            let item_id = comment_id_base + c.id;
            items.push(PlanViewerItem::Comment(CommentLine::new(
                c.id,
                item_id,
                &c.line_range,
                c.text.clone(),
                max_digits,
            )));
        }

        self.lines = items;
    }

    /// Get the selected line range (1-based, inclusive).
    ///
    /// Returns a single line if no visual selection, or the visual range.
    pub fn selected_line_range(&self) -> Option<Range<usize>> {
        if let Some(range) = self.list_state.multi_range() {
            // Scan forward from start to find first source line.
            let start = (range.start..range.end).find_map(|i| self.lines.get(i)?.line_number())?;
            // Scan backward from end to find last source line.
            let end = (range.start..range.end)
                .rev()
                .find_map(|i| self.lines.get(i)?.line_number())?;
            Some(start..end + 1)
        } else {
            let idx = self.list_state.selected_index()?;
            let ln = self.lines.get(idx)?.line_number()?;
            Some(ln..ln + 1)
        }
    }

    /// Format the line range as a suffix string (e.g., `:10` or `:10-12`).
    pub fn line_range_suffix(&self) -> Option<String> {
        let range = self.selected_line_range()?;
        if range.len() == 1 {
            Some(format!(":{}", range.start))
        } else {
            Some(format!(":{}-{}", range.start, range.end - 1))
        }
    }

    /// ListPane style matching the pager's visual language.
    pub fn list_pane_style() -> ListPaneStyle {
        let theme = Theme::current();
        ListPaneStyle {
            input_bar_bg: theme.bg_dark,
            input_bar_prompt_fg: theme.command,
            scrollbar_bg: theme.bg_dark,
            scrollbar_fg: theme.gray_dim,
            uniform_visual_bg: true,
            ..ListPaneStyle::default()
        }
    }
}

/// Build syntax-highlighted source lines from file content.
fn build_source_lines(path: &Path, content: &str) -> Vec<SourceLine> {
    let syntect = get_syntect();
    let mut highlighter = syntect.highlight_lines_by_file_path(path);

    // Split preserving all lines including trailing empty ones.
    // `split('\n')` keeps trailing empty strings unlike `.lines()`.
    let raw_lines: Vec<&str> = content.split('\n').collect();
    // If the file ends with a newline, remove the trailing empty split artifact.
    let line_count = if content.ends_with('\n') && raw_lines.last() == Some(&"") {
        raw_lines.len() - 1
    } else {
        raw_lines.len()
    };
    let max_digits = digit_count(line_count.max(1));

    raw_lines
        .iter()
        .take(line_count)
        .enumerate()
        .map(|(i, text)| {
            let line_number = i + 1;
            // Feed every line, blank ones included, through the highlighter so its parse state stays in sync
            // Skipping blanks corrupts constructs that span multiple lines (block comments, multi-line strings)
            let styled_line = match highlighter.as_mut() {
                Some(hl) => highlight_to_ratatui_line(hl, text, &syntect.syntax_set),
                None if text.is_empty() => Line::from(" ".to_owned()),
                None => Line::from((*text).to_owned()),
            };
            SourceLine::new(line_number, styled_line, (*text).to_owned(), max_digits)
        })
        .collect()
}

/// Count source lines the way `str::split('\n')` does, not counting a final trailing newline.
fn source_line_count(content: &str) -> usize {
    let count = content.split('\n').count();
    if content.ends_with('\n') {
        count - 1
    } else {
        count
    }
}

struct BuiltMarkdownLines {
    source_lines: Vec<SourceLine>,
    /// Document-ordered `(source_lines index to follow, diagram source)`.
    mermaid_after: Vec<(usize, String)>,
}

/// Build markdown-rendered source lines from file content.
fn build_markdown_lines(content: &str, max_table_width: Option<usize>) -> BuiltMarkdownLines {
    let md = MarkdownContent::new_source_faithful(content, max_table_width);
    let pre_wrap = md.pre_wrap_lines();
    let source_map = md.line_source_map();
    let mermaid = md.mermaid_content();
    let mermaid_ranges = md.mermaid_block_ranges();

    // Background colors come from each line's style (set by the renderer for code blocks etc.)
    // pre_wrap_lines() returns owned Lines that carry their style including bg
    let line_bgs: Vec<Option<Color>> = pre_wrap.iter().map(|line| line.style.bg).collect();

    // Split source text into raw lines for plain_text / search.
    let raw_lines: Vec<&str> = content.split('\n').collect();
    let slc = source_line_count(content);
    let max_digits = digit_count(slc.max(1));

    // Group by source line; track which group each pre-wrap line lands in.
    let mut groups: Vec<(usize, Vec<Line<'static>>, Vec<Option<Color>>)> = Vec::new();
    let mut prewrap_to_group: Vec<usize> = Vec::with_capacity(pre_wrap.len());
    for (rendered_idx, rendered_line) in pre_wrap.into_iter().enumerate() {
        let src_line = source_map.get(rendered_idx).copied().unwrap_or(0);
        let bg = line_bgs.get(rendered_idx).copied().flatten();
        if let Some(last) = groups.last_mut()
            && last.0 == src_line
        {
            last.1.push(rendered_line);
            last.2.push(bg);
            prewrap_to_group.push(groups.len() - 1);
            continue;
        }
        groups.push((src_line, vec![rendered_line], vec![bg]));
        prewrap_to_group.push(groups.len() - 1);
    }

    // Maps group index to source_lines index after blank-line injection
    let mut group_to_source_idx: Vec<usize> = Vec::with_capacity(groups.len());
    let mut source_lines = Vec::new();
    let mut next_item_id = 0u64;
    let mut next_blank_src = 0usize;

    for (src_line_0based, rendered_lines, rendered_bgs) in groups {
        for blank_src in next_blank_src..src_line_0based.min(slc) {
            if raw_lines
                .get(blank_src)
                .is_some_and(|line| line.trim().is_empty())
            {
                source_lines.push(SourceLine::new_markdown(
                    next_item_id,
                    blank_src + 1,
                    vec![Line::default()],
                    vec![None],
                    raw_lines.get(blank_src).unwrap_or(&"").to_string(),
                    max_digits,
                ));
                next_item_id += 1;
            }
        }

        group_to_source_idx.push(source_lines.len());
        source_lines.push(SourceLine::new_markdown(
            next_item_id,
            src_line_0based + 1,
            rendered_lines,
            rendered_bgs,
            raw_lines.get(src_line_0based).unwrap_or(&"").to_string(),
            max_digits,
        ));
        next_item_id += 1;
        next_blank_src = next_blank_src.max(src_line_0based.saturating_add(1));
    }

    for blank_src in next_blank_src..slc {
        if raw_lines
            .get(blank_src)
            .is_some_and(|line| line.trim().is_empty())
        {
            source_lines.push(SourceLine::new_markdown(
                next_item_id,
                blank_src + 1,
                vec![Line::default()],
                vec![None],
                raw_lines.get(blank_src).unwrap_or(&"").to_string(),
                max_digits,
            ));
            next_item_id += 1;
        }
    }

    let show_affordances = mermaid_display(crate::appearance::cache::load_render_mermaid())
        == MermaidDisplay::Affordances;
    let mut mermaid_after = Vec::new();
    if show_affordances {
        for (i, range) in mermaid_ranges.iter().enumerate() {
            if range.is_empty() {
                continue;
            }
            let Some(&group_idx) = prewrap_to_group.get(range.end - 1) else {
                continue;
            };
            let Some(&src_idx) = group_to_source_idx.get(group_idx) else {
                continue;
            };
            let Some(source) = mermaid.source(i) else {
                continue;
            };
            mermaid_after.push((src_idx, source.to_owned()));
        }
    }

    BuiltMarkdownLines {
        source_lines,
        mermaid_after,
    }
}

/// Convert syntect highlighting output to a ratatui Line.
fn highlight_to_ratatui_line(
    hl: &mut HighlightLines<'_>,
    text: &str,
    syntax_set: &syntect::parsing::SyntaxSet,
) -> Line<'static> {
    // syntect needs the trailing newline to recognize line-spanning constructs.
    // Feed it, then strip the newline back out of the rendered spans.
    let with_newline = format!("{text}\n");
    let highlighted = match hl.highlight_line(&with_newline, syntax_set) {
        Ok(h) => h,
        Err(_) if text.is_empty() => return Line::from(" ".to_owned()),
        Err(_) => return Line::from(text.to_owned()),
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (style, segment) in highlighted {
        let mut piece = segment.to_owned();
        while piece.ends_with('\n') || piece.ends_with('\r') {
            piece.pop();
        }
        if piece.is_empty() {
            continue;
        }
        // Shared path: polarity-safe under the terminal-native lock, else normal theme quantize (see xai_grok_pager_render::syntax)
        let fg = crate::syntax::syntect_rgb_to_fg(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        );
        spans.push(Span::styled(piece, Style::default().fg(fg)));
    }

    if spans.is_empty() {
        return Line::from(" ".to_owned());
    }
    Line::from(spans)
}

/// Count digits in a number (for line number padding).
fn digit_count(n: usize) -> usize {
    if n == 0 {
        1
    } else {
        ((n as f64).log10().floor() as usize) + 1
    }
}

/// Band for the active commenting / gutter-drag line range: a subtle 15% `accent_plan` tint over the canvas on RGB themes.
/// Profile palettes (terminal theme, Reset canvas) cannot express a dim yellow tint, so the band is the solid named `accent_plan` with forced Black text — readable on both polarities.
fn commenting_band(theme: &Theme) -> (Color, Option<Color>) {
    match crate::render::color::blend_color(theme.bg_base, theme.accent_plan, 0.15) {
        Some(tint) => (tint, None),
        None => (theme.accent_plan, Some(Color::Black)),
    }
}

/// Build a single review-footer shortcut button styled to match the shortcut hints in `modal_window::render_modal_shortcuts`.
/// The style is a bold key in the primary text color and a dim label, with a hover-highlighted background.
#[cfg(test)]
fn build_shortcut_button<'a>(
    key: char,
    rest: &str,
    hovered: bool,
    theme: &crate::theme::Theme,
) -> Vec<Span<'a>> {
    let bg = if hovered {
        theme.bg_highlight
    } else {
        theme.bg_base
    };
    let mut key_style = Style::default()
        .fg(theme.text_primary)
        .bg(bg)
        .add_modifier(Modifier::BOLD);
    let mut label_style = Style::default().fg(theme.gray).bg(bg);
    // Terminal theme (Reset band slots): the bg_highlight hover underlay is
    // invisible — reverse video carries the cue, as in render_modal_shortcuts.
    if hovered && theme.is_bandless() {
        key_style = key_style.add_modifier(Modifier::REVERSED);
        label_style = label_style.add_modifier(Modifier::REVERSED);
    }
    vec![
        Span::styled(key.to_string(), key_style),
        Span::styled(format!(" {rest}"), label_style),
    ]
}

/// One empty cell between plan-header bracket controls.
const PLAN_HEADER_CONTROL_GAP: u16 = 1;

/// Soft plan side-pane frame.
///
/// A real muted hairline when [`Theme::panel_border_fg`] is visible and is not
/// a bright white stroke. On DOGE that hairline is the black canvas, so the
/// frame glyphs stay and are not a white or neon-cyan rectangle.
pub(crate) fn soft_plan_frame_fg(theme: &Theme) -> Color {
    let hairline = theme.panel_border_fg();
    if !is_bright_white_stroke(theme, hairline)
        && hairline != theme.bg_base
        && hairline != theme.bg_light
    {
        hairline
    } else {
        theme.bg_base
    }
}

fn force_soft_plan_frame(buf: &mut Buffer, area: Rect, fg: Color, bg: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().fg(fg).bg(bg);
    let right = area.x + area.width - 1;
    let bottom = area.y + area.height - 1;
    for x in area.x..area.x + area.width {
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_style(style);
        }
        if bottom != area.y
            && let Some(cell) = buf.cell_mut((x, bottom))
        {
            cell.set_style(style);
        }
    }
    for y in area.y..area.y + area.height {
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_style(style);
        }
        if right != area.x
            && let Some(cell) = buf.cell_mut((right, y))
        {
            cell.set_style(style);
        }
    }
}

fn is_bright_white_stroke(theme: &Theme, color: Color) -> bool {
    color == theme.prompt_border
        || color == theme.prompt_border_active
        || color == theme.selection_border
        || color == theme.text_primary
        || matches!(color, Color::White | Color::Rgb(255, 255, 255))
}

fn plan_header_control_style(theme: &Theme, hovered: bool) -> Style {
    if hovered {
        Style::default()
            .fg(theme.text_primary)
            .bg(theme.bg_base)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.gray).bg(theme.bg_base)
    }
}

/// Plan title bar, right to left: `[✗]` `[↗]` `[⧉]` `[⌕]`.
///
/// Each label is 3 columns. One cell sits between them. Close is omitted
/// while plan review is not user-closeable.
fn paint_plan_header_controls(
    buf: &mut Buffer,
    popup_area: Rect,
    viewer: &mut LineViewerState,
    theme: &Theme,
) {
    viewer.close_button_area = None;
    viewer.fullscreen_button_area = None;
    if let Some(plan) = viewer.plan.as_mut() {
        plan.copy_button_area = None;
        plan.search_button_area = None;
    }

    let mut end = popup_area
        .x
        .saturating_add(popup_area.width)
        .saturating_sub(2);
    let left_limit = popup_area.x.saturating_add(1);
    let y = popup_area.y;

    let mut place = |label: &'static str, hovered: bool| -> Option<Rect> {
        const W: u16 = 3;
        if end < left_limit.saturating_add(W - 1) {
            return None;
        }
        let x = end.saturating_sub(W - 1);
        if x < left_limit {
            return None;
        }
        buf.set_span(
            x,
            y,
            &Span::styled(label, plan_header_control_style(theme, hovered)),
            W,
        );
        end = x.saturating_sub(1).saturating_sub(PLAN_HEADER_CONTROL_GAP);
        Some(Rect::new(x, y, W, 1))
    };

    if !viewer.feedback_active() {
        let hovered = viewer.close_hovered;
        viewer.close_button_area = place(crate::glyphs::ballot_x_button(), hovered);
    }
    let fs_hovered = viewer.fullscreen_hovered;
    viewer.fullscreen_button_area = place(crate::glyphs::enlarge_button(), fs_hovered);
    let copy_hovered = viewer.plan_ref().is_some_and(|p| p.copy_hovered);
    let search_hovered = viewer.plan_ref().is_some_and(|p| p.search_hovered);
    let copy_area = place(crate::glyphs::copy_button(), copy_hovered);
    let search_area = place(crate::glyphs::search_button(), search_hovered);
    if let Some(plan) = viewer.plan.as_mut() {
        plan.copy_button_area = copy_area;
        plan.search_button_area = search_area;
    }
}

/// Render the line viewer popup. In normal mode, draws a 75% centered panel with dimmed background
/// (modifiers reset). In fullscreen mode (`viewer.fullscreen`), fills the entire overlay area
/// without dimming. Renders the ListPane inside the panel with syntax-highlighted lines.
pub fn render_line_viewer(
    buf: &mut Buffer,
    full_area: Rect,
    viewer: &mut LineViewerState,
    cwd: &Path,
    theme: &Theme,
    comment_count: usize,
) {
    // Compute the popup area. In enlarge (fullscreen) mode it nearly fills the overlay, leaving 1 row
    // of top and 2 cols of side padding so it doesn't crowd the edges. The caller already excludes the
    // prompt and turn_status from `full_area`. In normal mode it sits in a 75% centered popup.
    let (popup_area, should_dim) = if viewer.fullscreen {
        const TOP_PAD: u16 = 1;
        const SIDE_PAD: u16 = 2;
        let pad_w = SIDE_PAD.saturating_mul(2);
        let popup_x = full_area.x + SIDE_PAD.min(full_area.width);
        let popup_y = full_area.y + TOP_PAD.min(full_area.height);
        let popup_w = full_area.width.saturating_sub(pad_w);
        let popup_h = full_area.height.saturating_sub(TOP_PAD);
        (Rect::new(popup_x, popup_y, popup_w, popup_h), false)
    } else if viewer.is_soft_plan_side_pane() {
        let pane_w = LineViewerState::soft_plan_pane_width(full_area.width);
        let popup_x = full_area.x + full_area.width.saturating_sub(pane_w);
        (
            Rect::new(popup_x, full_area.y, pane_w, full_area.height),
            false,
        )
    } else {
        let popup_width = (full_area.width as f32 * 0.75) as u16;
        let popup_height = (full_area.height as f32 * 0.75) as u16;
        let popup_x = full_area.x + (full_area.width.saturating_sub(popup_width)) / 2;
        let popup_y = full_area.y + (full_area.height.saturating_sub(popup_height)) / 2;
        (Rect::new(popup_x, popup_y, popup_width, popup_height), true)
    };

    // Plan modes (both review and casual) reserve 2 extra rows inside the frame for the divider and action-button footer
    // They therefore need a slightly taller minimum than ordinary file previews
    let min_height: u16 = if viewer.show_footer() { 7 } else { 5 };
    if popup_area.width < 10 || popup_area.height < min_height {
        viewer.last_popup_area = None;
        viewer.last_modal_area = None;
        return;
    }

    // 1. Dim the entire screen behind the popup (skip when fullscreen).
    if should_dim {
        dim_area(buf, full_area, theme.bg_base, 0.5);
    }

    // 2. Clear the popup area.
    ratatui::widgets::Clear.render(popup_area, buf);
    buf.set_style(
        popup_area,
        Style::default().fg(theme.text_primary).bg(theme.bg_base),
    );

    // 3. Draw border. The soft plan side pane uses the muted frame, not
    // gray_dim (neon cyan on DOGE) and not the white prompt stroke.
    let frame_fg = if viewer.is_soft_plan_side_pane() {
        soft_plan_frame_fg(theme)
    } else {
        theme.gray_dim
    };
    let border = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(frame_fg))
        .style(Style::default().bg(theme.bg_base));
    let inner = border.inner(popup_area);
    border.render(popup_area, buf);
    if viewer.is_soft_plan_side_pane() {
        // Block style merge can leave the popup's primary-white fg on the
        // stroke. Force the muted frame after the glyphs are in place.
        force_soft_plan_frame(buf, popup_area, frame_fg, theme.bg_base);
    }

    // Plan modes reserve 2 rows at the bottom of `inner` for the divider and action-button row
    // (rendered in step 8 below).
    let footer_rows: u16 = if viewer.show_footer() { 2 } else { 0 };
    let mut content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(footer_rows),
    };
    // Soft plan text sits inside the frame so the left border cannot cover
    // the first characters, and the body wraps in the width that remains.
    if viewer.is_soft_plan_side_pane() {
        let inset = LineViewerState::SOFT_PLAN_TEXT_INSET.min(content_area.width.saturating_sub(8));
        content_area.x = content_area.x.saturating_add(inset);
        content_area.width = content_area.width.saturating_sub(inset);
    }

    // 4. Prepare layout (resolves selection index for title + rendering).
    // Subtract the scrollbar before measuring so a long plan line wraps and
    // scrolls instead of being clipped on one row.
    let layout_width = content_area
        .width
        .saturating_sub(SCROLLBAR_TOTAL_COLS)
        .max(1);
    viewer.prepare_layout(layout_width, content_area.height);

    // 5. Title bar: styled file path and line range.
    //    Only show the line range when visual selection is active
    //    When title_override is set (e.g. plan preview), use that instead of the full file path to avoid overflow.
    if inner.width > 4 {
        let rel_path = viewer.path.strip_prefix(cwd).unwrap_or(&viewer.path);
        let rel_path_str = viewer
            .title_override
            .as_deref()
            .unwrap_or_else(|| rel_path.to_str().unwrap_or(""))
            .to_string();
        let line_range = if viewer.list_state.visual_mode {
            viewer.line_range_suffix().map(|s| {
                // Strip the leading ':'; styled_file_ref adds its own
                s.strip_prefix(':').unwrap_or(&s).to_owned()
            })
        } else {
            None
        };

        let mut title = super::styled_file_ref(
            &rel_path_str,
            line_range.as_deref(),
            theme,
            false, // no @ prefix in viewer title
        );
        // Add bg to all spans (title sits on the border).
        for span in &mut title.spans {
            span.style = span.style.bg(theme.bg_base);
        }

        // Wrap with `─ ... ─` decorations to match other modals (see modal_window.rs:341-346) and left-align flush with the top-left corner.
        // Soft plan uses the same frame color so the top rule is not a cyan stroke.
        let deco = Style::default().fg(frame_fg).bg(theme.bg_base);
        title.spans.insert(0, Span::styled("\u{2500} ", deco));
        title.spans.push(Span::styled(" \u{2500}", deco));

        let title_width = title.width() as u16;
        let title_inset = if viewer.is_soft_plan_side_pane() {
            LineViewerState::SOFT_PLAN_TEXT_INSET
        } else {
            0
        };
        let title_x = popup_area.x.saturating_add(1).saturating_add(title_inset);
        let max_title_width = popup_area
            .width
            .saturating_sub(2)
            .saturating_sub(title_inset);
        buf.set_line(
            title_x,
            popup_area.y,
            &title,
            title_width.min(max_title_width),
        );
    }

    // Action buttons on the top border, right-aligned. Plan preview uses
    // four equal bracket controls with a one-cell gap. Other viewers keep
    // the close and enlarge pair. Close is omitted in plan-review
    // (feedback) mode because the modal is not user-closeable in that state.
    if viewer.kind == LineViewerKind::PlanPreview {
        paint_plan_header_controls(buf, popup_area, viewer, theme);
    } else {
        if let Some(plan) = viewer.plan.as_mut() {
            plan.copy_button_area = None;
            plan.search_button_area = None;
        }
        let mut right_edge = popup_area.x + popup_area.width - 1;

        if !viewer.feedback_active() {
            let close_text = crate::glyphs::ballot_x(); // ✗ (ASCII on legacy ConHost)
            // Label is `[✗] ` (trailing space, no leading space)
            // The fullscreen button's label has no trailing space when the close is visible, so the two buttons abut flush as `[↗][✗]`
            // They tuck under the top-right corner with one space inside the frame on each side: ` [↗][✗] `
            let close_w: u16 = 4; // "[✗] "
            if popup_area.width > close_w + 2 {
                let close_x = right_edge - close_w;
                let close_style = if viewer.close_hovered {
                    Style::default()
                        .fg(theme.text_primary)
                        .bg(theme.bg_base)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.gray).bg(theme.bg_base)
                };
                let close_span = Span::styled(format!("[{close_text}] "), close_style);
                buf.set_span(close_x, popup_area.y, &close_span, close_w);
                viewer.close_button_area = Some(Rect::new(close_x, popup_area.y, close_w, 1));
                right_edge = close_x;
            } else {
                viewer.close_button_area = None;
            }
        } else {
            viewer.close_button_area = None;
        }

        // Fullscreen toggle button. The icon stays constant regardless of current state: the button is a
        // toggle, not a status indicator. When the close is hidden (plan-review mode) the fullscreen keeps
        // its trailing space so it doesn't crowd the corner `╮`.
        let fs_icon = crate::glyphs::enlarge(); // ↗ (ASCII on legacy ConHost)
        let close_visible = viewer.close_button_area.is_some();
        let (fs_label, fs_w): (String, u16) = if close_visible {
            (format!(" [{fs_icon}]"), 4)
        } else {
            (format!(" [{fs_icon}] "), 5)
        };
        if right_edge > popup_area.x + fs_w + 2 {
            let fs_x = right_edge - fs_w;
            let fs_style = if viewer.fullscreen_hovered {
                Style::default()
                    .fg(theme.text_primary)
                    .bg(theme.bg_base)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.gray).bg(theme.bg_base)
            };
            let fs_span = Span::styled(fs_label, fs_style);
            buf.set_span(fs_x, popup_area.y, &fs_span, fs_w);
            viewer.fullscreen_button_area = Some(Rect::new(fs_x, popup_area.y, fs_w, 1));
        } else {
            viewer.fullscreen_button_area = None;
        }
    }

    // The legacy top-border "send" button is gone; both plan-approval and casual modes now render the send action in the modal footer
    // Clear stale hit-rects so mouse handlers don't act on positions from a previous render
    if let Some(plan) = viewer.plan.as_mut() {
        plan.send_button_area = None;
        plan.questions_button_area = None;
        plan.approve_button_area = None;
        plan.approve_notes_button_area = None;
        plan.comment_button_area = None;
        plan.abandon_button_area = None;
    }

    // Render ListPane.
    let style = LineViewerState::list_pane_style();

    let pane = ListPane::new(&viewer.lines).focused(true).style(style);
    StatefulWidget::render(pane, content_area, buf, &mut viewer.list_state);

    // Cache the list-rendered area (for ListPane mouse dispatch)
    // Also cache the full modal area inside the border, for the click-outside check that decides whether to close the modal
    viewer.last_popup_area = Some(content_area);
    viewer.last_modal_area = Some(inner);

    // 7a. Per-comment `[✗]` delete buttons.
    render_comment_close_buttons(buf, content_area, viewer, theme);

    // 7b. Line range highlight: active drag or commenting range.
    if let Some(plan) = viewer.plan_ref() {
        let highlight_range =
            if let (Some(start), Some(end)) = (plan.gutter_drag_start, plan.gutter_drag_end) {
                if start != end {
                    Some((start.min(end), start.max(end)))
                } else {
                    None
                }
            } else {
                plan.active_commenting_range
                    .as_ref()
                    .map(|r| (r.start, r.end.saturating_sub(1)))
            };
        if let Some((lo, hi)) = highlight_range {
            let (blend_bg, fg_override) = commenting_band(theme);
            // Stop the highlight one column before the scrollbar
            // The gap and track stay readable instead of being tinted by the comment-range overlay
            let highlight_width = content_area.width.saturating_sub(SCROLLBAR_TOTAL_COLS);
            for row in content_area.y..content_area.y + content_area.height {
                if let Some(ln) = viewer.source_line_at_screen_row(row, content_area)
                    && ln >= lo
                    && ln <= hi
                {
                    let row_rect = Rect::new(content_area.x, row, highlight_width, 1);
                    buf.set_style(row_rect, Style::default().bg(blend_bg));
                    if let Some(fg) = fg_override {
                        crate::render::color::force_area_fg(buf, row_rect, fg);
                    }
                }
            }
        }
    }

    // Action buttons inside the modal footer (centered), for both plan-approval and casual
    // plan-preview modes.
    if viewer.show_footer() && inner.height >= 2 {
        let div_y = inner.y + inner.height - 2;
        let div_style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
        let line: String = std::iter::repeat_n('\u{2500}', inner.width as usize).collect();
        buf.set_string(inner.x, div_y, &line, div_style);

        let bottom_y = inner.y + inner.height - 1;

        let is_plan_preview = viewer.kind == LineViewerKind::PlanPreview;
        if is_plan_preview {
            // Clickable CTAs. Letter keys type, so labels have no a/c/s/q
            // prefixes. Narrow docks drop separators, then drop the badge.
            // Idle: Comment. After Comment, Clarify. Copy stays on the title bar.
            use unicode_width::UnicodeWidthStr;
            let comment_flow = viewer.plan_ref().is_some_and(|p| p.comment_flow_active);
            let labels: [&str; 4] = if comment_flow {
                ["approve", "clarify", "revise", "exit"]
            } else {
                ["approve", "comment", "revise", "exit"]
            };
            let hovers = [
                viewer.plan_ref().is_some_and(|p| p.approve_hovered),
                if comment_flow {
                    viewer.plan_ref().is_some_and(|p| p.questions_hovered)
                } else {
                    viewer.plan_ref().is_some_and(|p| p.comment_hovered)
                },
                viewer.plan_ref().is_some_and(|p| p.send_hovered),
                viewer.plan_ref().is_some_and(|p| p.abandon_hovered),
            ];
            let selected = viewer.plan_ref().and_then(|p| p.selected_cta);
            let pad = PLAN_APPROVAL_ACTION_PAD;
            let pad_w = pad.width() as u16;
            let mut between = PLAN_APPROVAL_ACTION_BETWEEN;
            let mut between_w = between.width() as u16;
            let word_widths = [
                labels[0].width() as u16,
                labels[1].width() as u16,
                labels[2].width() as u16,
                labels[3].width() as u16,
            ];
            let marked = [
                selected_cta_marks_index(selected, 0, comment_flow),
                selected_cta_marks_index(selected, 1, comment_flow),
                selected_cta_marks_index(selected, 2, comment_flow),
                selected_cta_marks_index(selected, 3, comment_flow),
            ];
            let choice_dot = format!(" {}", crate::glyphs::filled_dot());
            let choice_dot_w = choice_dot.width() as u16;
            let mut badge_text = if comment_count > 0 {
                format!(" {comment_count} {}", crate::glyphs::filled_dot())
            } else {
                String::new()
            };
            let mut badge_w = badge_text.width() as u16;
            let row_width = |between_w: u16, badge_w: u16| -> u16 {
                let mut w = 0u16;
                for i in 0..4 {
                    w = w
                        .saturating_add(pad_w)
                        .saturating_add(word_widths.get(i).copied().expect("index out of bounds"))
                        .saturating_add(pad_w);
                    if marked.get(i).copied().expect("index out of bounds") {
                        w = w.saturating_add(choice_dot_w);
                    }
                    if i == 1 {
                        w = w.saturating_add(badge_w);
                    }
                    if i < 3 {
                        w = w.saturating_add(between_w);
                    }
                }
                w
            };
            if row_width(between_w, badge_w) > inner.width {
                between = "";
                between_w = 0;
            }
            if row_width(between_w, badge_w) > inner.width {
                badge_text.clear();
                badge_w = 0;
            }
            let total_w = row_width(between_w, badge_w);
            let mut x = inner.x + inner.width.saturating_sub(total_w) / 2;
            let sep_style = Style::default().fg(theme.gray_dim).bg(theme.bg_base);
            let badge_style = Style::default().fg(theme.accent_plan).bg(theme.bg_base);
            let choice_dot_style = Style::default().fg(theme.text_primary).bg(theme.bg_base);
            let mut areas: [Option<Rect>; 4] = [None; 4];
            for i in 0..4 {
                let start = x;
                let style = if hovers.get(i).copied().expect("index out of bounds")
                    || marked.get(i).copied().expect("index out of bounds")
                {
                    Style::default()
                        .fg(theme.text_primary)
                        .bg(theme.bg_base)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary).bg(theme.bg_base)
                };
                if pad_w > 0 {
                    buf.set_string(x, bottom_y, pad, style);
                    x = x.saturating_add(pad_w);
                }
                buf.set_string(
                    x,
                    bottom_y,
                    labels.get(i).copied().expect("index out of bounds"),
                    style,
                );
                x = x.saturating_add(word_widths.get(i).copied().expect("index out of bounds"));
                if marked.get(i).copied().expect("index out of bounds") {
                    buf.set_string(x, bottom_y, &choice_dot, choice_dot_style);
                    x = x.saturating_add(choice_dot_w);
                }
                if i == 1 && badge_w > 0 {
                    buf.set_string(x, bottom_y, &badge_text, badge_style);
                    x = x.saturating_add(badge_w);
                }
                if pad_w > 0 {
                    buf.set_string(x, bottom_y, pad, style);
                    x = x.saturating_add(pad_w);
                }
                *areas.get_mut(i).expect("index out of bounds") =
                    Some(Rect::new(start, bottom_y, x.saturating_sub(start), 1));
                if i < 3 && between_w > 0 {
                    buf.set_string(x, bottom_y, between, sep_style);
                    x = x.saturating_add(between_w);
                }
            }
            let plan = viewer.plan_mut();
            plan.approve_button_area = areas[0];
            if comment_flow {
                plan.questions_button_area = areas[1];
                plan.comment_button_area = None;
            } else {
                plan.comment_button_area = areas[1];
                plan.questions_button_area = None;
            }
            plan.send_button_area = areas[2];
            plan.abandon_button_area = areas[3];
            plan.approve_notes_button_area = None;
        } else if let Some(plan) = viewer.plan.as_mut() {
            plan.approve_button_area = None;
            plan.comment_button_area = None;
            plan.questions_button_area = None;
            plan.send_button_area = None;
            plan.abandon_button_area = None;
            plan.approve_notes_button_area = None;
        }
    }
}

/// Draws the `[✗]` delete button on the hovered or selected comment row.
/// The button rects are saved in `comment_close_areas` for mouse hit-testing.
fn render_comment_close_buttons(
    buf: &mut Buffer,
    content_area: Rect,
    viewer: &mut LineViewerState,
    theme: &Theme,
) {
    if viewer.kind != LineViewerKind::PlanPreview {
        return;
    }

    let hovered = viewer.plan_ref().and_then(|p| p.hovered_comment_id);
    let close_hovered = viewer.plan_ref().is_some_and(|p| p.close_button_hovered);
    let selected = viewer.selected_comment_id();

    let label = crate::glyphs::ballot_x_button();
    let label_w = label.chars().count() as u16;
    let x_right = content_area.x + content_area.width.saturating_sub(SCROLLBAR_TOTAL_COLS);
    let fits = x_right > content_area.x + label_w + 1;

    let mut close_areas: Vec<(u64, Rect)> = Vec::new();
    if fits && (hovered.is_some() || selected.is_some()) {
        let x = x_right - label_w - 1;

        // Only the first screen row of a wrapped comment gets the button
        let mut prev_row_id: Option<u64> = None;

        for row in content_area.y..content_area.y + content_area.height {
            let Some(cid) = viewer.comment_id_at_screen_row(row, content_area) else {
                prev_row_id = None;
                continue;
            };

            let first_row = prev_row_id != Some(cid);
            prev_row_id = Some(cid);

            if !first_row || (Some(cid) != hovered && Some(cid) != selected) {
                continue;
            }

            let style = if close_hovered && hovered == Some(cid) {
                Style::default().fg(theme.accent_error)
            } else {
                Style::default().fg(theme.gray)
            };

            buf.set_span(x, row, &Span::styled(label, style), label_w);
            close_areas.push((cid, Rect::new(x, row, label_w, 1)));
        }
    }

    viewer.plan_mut().comment_close_areas = close_areas;
}

pub use crate::render::color::dim_area;

#[cfg(test)]
mod tests {
    use super::*;

    /// RGB themes tint the commenting band (subtle blend, text keeps its fgs); the terminal theme
    /// cannot blend against the Reset canvas, so the band is the solid plan accent with forced Black
    /// text instead of an unreadable accent-behind-default-fg fallback.
    #[test]
    fn commenting_band_stays_readable_on_terminal_theme() {
        let theme = Theme::terminal();
        let (band, fg) = commenting_band(&theme);
        assert_eq!(band, theme.accent_plan);
        assert_eq!(fg, Some(Color::Black), "forced readable fg on the band");

        let theme = Theme::groknight();
        let (band, fg) = commenting_band(&theme);
        assert_ne!(band, theme.accent_plan, "subtle tint, not the raw accent");
        assert_eq!(fg, None, "RGB rows keep their own fgs");
    }

    /// The terminal theme's `bg_highlight` is Reset, so a hovered review-footer
    /// button must carry reverse video (like `render_modal_shortcuts`); RGB
    /// themes keep the plain `bg_highlight` underlay.
    #[test]
    fn hovered_shortcut_button_is_reversed_on_terminal_theme() {
        let theme = Theme::terminal();
        for (hovered, expect_reversed) in [(true, true), (false, false)] {
            let spans = build_shortcut_button('a', "approve", hovered, &theme);
            for span in &spans {
                assert_eq!(
                    span.style.add_modifier.contains(Modifier::REVERSED),
                    expect_reversed,
                    "hovered={hovered}"
                );
            }
        }

        let theme = Theme::groknight();
        let spans = build_shortcut_button('a', "approve", true, &theme);
        for span in &spans {
            assert!(!span.style.add_modifier.contains(Modifier::REVERSED));
            assert_eq!(span.style.bg, Some(theme.bg_highlight));
        }
    }

    #[test]
    fn open_markdown_content_uses_in_memory_content() {
        let viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\n- Do the thing".to_owned(),
            None,
        )
        .expect("markdown content should open");

        assert_eq!(viewer.path, PathBuf::from("plan.md"));
        assert_eq!(
            viewer.markdown_content.as_deref(),
            Some("# Plan\n\n- Do the thing")
        );
    }

    #[test]
    fn markdown_content_for_test_returns_in_memory_content() {
        let viewer =
            LineViewerState::open_markdown_content("plan.md", "# Latest Plan".to_owned(), None)
                .expect("markdown content should open");

        assert_eq!(viewer.markdown_content_for_test(), Some("# Latest Plan"));
    }

    #[test]
    fn open_markdown_content_rejects_blank_content() {
        assert!(
            LineViewerState::open_markdown_content("plan.md", "  \n".to_owned(), None).is_none()
        );
    }

    #[test]
    fn plan_preview_exposes_full_raw_markdown_for_copy() {
        let body = "# Plan\n\n- Do the thing\n- Then ship";
        let mut viewer = LineViewerState::open_markdown_content("plan.md", body.to_owned(), None)
            .expect("markdown content should open");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.prepare_layout(80, 20);

        assert_eq!(
            viewer.markdown_content_for_feedback().as_deref(),
            Some(body)
        );
    }

    /// Operator contract: typing/paint stay responsive. A 240k plan must
    /// not be scanned or re-parsed on every same-width keystroke paint.
    #[test]
    fn plan_overlay_repeat_prepare_at_same_width_does_not_rebuild_markdown() {
        let mut body = String::from("# Plan\n\n");
        body.push_str(&"step\n".repeat(48_000));
        assert!(
            body.len() >= 240_000,
            "fixture must be a large plan body, got {}",
            body.len()
        );
        let mut viewer = LineViewerState::open_markdown_content("plan.md", body, None)
            .expect("markdown content should open");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.prepare_layout(80, 24);
        let probes = viewer.markdown_width_probes;
        let rebuilds = viewer.markdown_rebuilds;
        assert!(
            probes >= 1 && rebuilds >= 1,
            "first layout must probe and rebuild, probes={probes} rebuilds={rebuilds}"
        );
        for _ in 0..20 {
            viewer.prepare_layout(80, 24);
        }
        assert_eq!(
            viewer.markdown_width_probes, probes,
            "same-width paints must not walk the 240k plan body every key"
        );
        assert_eq!(
            viewer.markdown_rebuilds, rebuilds,
            "same-width paints must not re-parse the 240k plan every key"
        );
        viewer.prepare_layout(120, 24);
        assert!(
            viewer.markdown_width_probes > probes,
            "a real width change must still rebuild"
        );
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn source_line(item: &PlanViewerItem) -> &SourceLine {
        match item {
            PlanViewerItem::Source(source) => source,
            _ => panic!("expected source line"),
        }
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        let area = *buf.area();
        let mut row = String::new();
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell((x, y)) {
                row.push_str(cell.symbol());
            }
        }
        row
    }

    /// Copy is a 3-column `[⧉]` one cell left of `[↗]`, not a footer CTA.
    fn assert_title_bar_copy_left_of_enlarge(buf: &Buffer, plan: &PlanViewerExtras, footer_y: u16) {
        let area = plan
            .copy_button_area
            .expect("copy must be a clickable hit target");
        assert_ne!(
            area.y, footer_y,
            "copy glyph must not sit on the Approve CTA row"
        );
        assert_eq!(area.width, 3, "copy is the same 3 columns as [↗] and [✗]");
        assert_eq!(area.height, 1, "copy stays one row on the title bar");
        let icon = crate::glyphs::copy_icon();
        assert_eq!(
            buf[(area.x, area.y)].symbol(),
            "[",
            "copy control opens with a square bracket"
        );
        assert_eq!(
            buf[(area.x.saturating_add(1), area.y)].symbol(),
            icon,
            "copy_button_area must cover the title-bar copy glyph"
        );
        assert_eq!(
            buf[(area.x.saturating_add(2), area.y)].symbol(),
            "]",
            "copy control closes with a square bracket"
        );
        let gap_x = area.x.saturating_add(area.width);
        let gap = buf[(gap_x, area.y)].symbol();
        assert_ne!(
            gap, "[",
            "one-cell gap between copy and [↗]; they must not glue together"
        );
        assert_ne!(
            gap, "]",
            "one-cell gap between copy and [↗]; they must not glue together"
        );
        let bracket_x = gap_x.saturating_add(1);
        assert_eq!(
            buf[(bracket_x, area.y)].symbol(),
            "[",
            "copy sits one cell left of [↗]"
        );
        assert_eq!(
            buf[(bracket_x.saturating_add(1), area.y)].symbol(),
            crate::glyphs::enlarge(),
            "copy sits one cell left of [↗]"
        );
    }

    /// Glass is a 3-column `[⌕]` one cell left of the bordered copy control.
    fn assert_title_bar_search_left_of_copy(buf: &Buffer, plan: &PlanViewerExtras) {
        let search = plan
            .search_button_area
            .expect("magnifying glass must be a clickable hit target");
        let copy = plan
            .copy_button_area
            .expect("copy must stay next to enlarge");
        assert_eq!(search.width, 3, "search is the same 3 columns as copy");
        assert_eq!(copy.width, 3, "copy is the same 3 columns as search");
        assert_eq!(
            search.width, copy.width,
            "search and copy are the same size"
        );
        assert_eq!(
            search.x + search.width + 1,
            copy.x,
            "one-cell gap between the bracketed glass and the bracketed copy"
        );
        assert_eq!(search.y, copy.y, "glass shares the title-bar row with copy");
        assert_eq!(
            buf[(search.x, search.y)].symbol(),
            "[",
            "search control opens with a square bracket"
        );
        assert_eq!(
            buf[(search.x.saturating_add(1), search.y)].symbol(),
            crate::glyphs::search_icon(),
            "search_button_area must cover the title-bar glass glyph"
        );
        assert_eq!(
            buf[(search.x.saturating_add(2), search.y)].symbol(),
            "]",
            "search control closes with a square bracket"
        );
    }

    #[test]
    fn build_markdown_lines_preserves_blank_source_lines() {
        let built = build_markdown_lines("# Plan\n\n- First\n\n- Second", Some(80));
        let numbered_rows: Vec<(usize, Vec<String>)> = built
            .source_lines
            .iter()
            .map(|line| {
                (
                    line.line_number,
                    line.rendered_lines.iter().map(line_text).collect(),
                )
            })
            .collect();

        assert_eq!(
            numbered_rows,
            vec![
                (1, vec!["Plan".to_owned()]),
                (2, vec![String::new()]),
                (3, vec!["• First".to_owned()]),
                (4, vec![String::new()]),
                (5, vec!["• Second".to_owned()]),
            ]
        );
    }

    #[test]
    fn markdown_source_blank_line_renders_as_numbered_empty_row() {
        let built = build_markdown_lines("# Plan\n\n- First", Some(80));
        let Some(blank) = built.source_lines.get(1) else {
            panic!("expected a blank source line");
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));

        blank.render(Rect::new(0, 0, 20, 1), &mut buf, false, true);

        assert_eq!(blank.line_number, 2);
        assert_eq!(row_text(&buf, 0), "2                   ");
    }

    #[test]
    fn mermaid_affordance_respects_render_setting() {
        use crate::appearance::{RenderMermaid, cache};

        const MD: &str = "# Plan\n\n```mermaid\nflowchart TD\n  A --> B\n```\n\nDone.\n";

        cache::set_render_mermaid(RenderMermaid::On);
        let built = build_markdown_lines(MD, Some(80));
        assert_eq!(built.mermaid_after.len(), 1);
        let Some(first) = built.mermaid_after.first() else {
            panic!("expected a mermaid affordance: {:?}", built.mermaid_after);
        };
        assert!(first.1.contains("A --> B"));
        assert!(first.0 < built.source_lines.len());

        let mut viewer =
            LineViewerState::open_markdown_content("plan.md", MD.to_owned(), None).unwrap();
        viewer.prepare_layout(100, 40);
        assert_eq!(
            viewer
                .lines
                .iter()
                .filter(|i| matches!(i, PlanViewerItem::MermaidAffordance(_)))
                .count(),
            1
        );
        let placements = viewer.diagram_affordance_placements(Rect::new(0, 0, 100, 40));
        assert_eq!(placements.len(), 1);
        let Some(placement) = placements.first() else {
            panic!("expected a placement: {placements:?}");
        };
        assert_eq!(placement.screen_rect.height, 1);
        assert!(placement.screen_rect.width > 0);

        cache::set_render_mermaid(RenderMermaid::Off);
        assert!(build_markdown_lines(MD, Some(80)).mermaid_after.is_empty());
        cache::set_render_mermaid(RenderMermaid::Auto);
    }

    #[test]
    fn markdown_viewer_selection_uses_source_line_numbers_with_blank_rows() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\n- First\n\n- Second".to_owned(),
            None,
        )
        .expect("markdown content should open");
        viewer.prepare_layout(80, 20);

        let ids_by_line: Vec<(usize, u64)> = viewer
            .lines
            .iter()
            .map(|item| {
                let source = source_line(item);
                (source.line_number, source.item_id)
            })
            .collect();
        assert_eq!(
            ids_by_line
                .iter()
                .map(|(line, _)| *line)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );

        let line_4_id = ids_by_line
            .iter()
            .find_map(|(line, id)| (*line == 4).then_some(*id))
            .expect("blank source line should have its own row");
        viewer.list_state.select_by_id(line_4_id);
        viewer.prepare_layout(80, 20);

        assert_eq!(viewer.selected_line_range(), Some(4..5));
        assert_eq!(viewer.line_range_suffix(), Some(":4".to_owned()));
        let Some(line) = viewer.lines.get(3) else {
            panic!("expected line 4");
        };
        assert_eq!(source_line(line).plain_text, "");
    }

    #[test]
    fn markdown_viewer_preserves_soft_break_collapsed_lines() {
        // Repro: consecutive non-blank lines (a poem) form one CommonMark paragraph
        // Source-faithful rendering must keep each line on its own numbered row instead of collapsing to one paragraph
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "Line one,\nLine two,\nLine three.".to_owned(),
            None,
        )
        .expect("markdown content should open");
        viewer.prepare_layout(80, 20);

        let rows: Vec<(usize, String)> = viewer
            .lines
            .iter()
            .map(|item| {
                let s = source_line(item);
                (
                    s.line_number,
                    s.rendered_lines.first().map(line_text).unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                (1, "Line one,".to_owned()),
                (2, "Line two,".to_owned()),
                (3, "Line three.".to_owned()),
            ]
        );
    }

    /// Soft park is a right-docked pane, not the 75% centered dimmed overlay.
    #[test]
    fn soft_park_plan_pane_covers_transcript_not_centered_overlay() {
        // Commenting round-trip: selecting all rows of a soft-break paragraph must map back to the full file line range
        // The agent then inspects the correct lines; this used to collapse to a single line number
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = false;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        // `Z` is transcript text. The pane must paint over the columns
        // behind it. `X` stays on the left, beside the pane.
        for x in 0..full.width {
            buf[(x, 12)].set_char('Z');
        }
        buf[(2, 12)].set_char('X');
        let left_bg_before = buf[(2, 12)].bg;
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let pane_w = LineViewerState::soft_plan_pane_width(full.width);
        let pane_x = full.x.saturating_add(full.width.saturating_sub(pane_w));
        assert!(
            pane_w > 0 && pane_x > 2 && pane_x < full.width,
            "soft park pane must overlap the transcript, not replace the whole frame; pane_x={pane_x} pane_w={pane_w}"
        );
        for x in pane_x..pane_x.saturating_add(pane_w) {
            let symbol = buf[(x, 12)].symbol().to_string();
            assert!(
                symbol != "Z",
                "plan pane must cover the transcript text behind it; column {x} still shows {symbol:?}. A narrowed transcript that leaves this text visible beside the pane is too weak."
            );
        }
        for x in 0..pane_x {
            let expected = if x == 2 { "X" } else { "Z" };
            assert_eq!(
                buf[(x, 12)].symbol(),
                expected,
                "text beside the pane stays put; the cover is the region behind the pane"
            );
        }

        let modal = viewer
            .last_modal_area
            .expect("soft park must paint a plan pane");
        assert!(
            modal.x >= pane_x && modal.x < pane_x.saturating_add(pane_w),
            "painted plan modal must sit on the transcript region the pane covers; modal={modal:?} pane_x={pane_x}"
        );
        assert!(
            viewer
                .plan_ref()
                .and_then(|plan| plan.search_button_area)
                .is_some(),
            "plan search must stay a hit target on the side panel"
        );
        let centered_75_x =
            full.x + (full.width.saturating_sub((full.width as f32 * 0.75) as u16)) / 2;
        assert!(
            modal.x >= full.width / 2,
            "soft park must sit on the right half, not a centered overlay; modal={modal:?}"
        );
        assert!(
            modal.x > centered_75_x + 8,
            "soft park must not be the 75% centered popup (that starts near x={centered_75_x}); modal={modal:?}"
        );
        assert!(
            modal.y <= full.y + 1,
            "soft park must not be vertically centered; modal={modal:?}"
        );
        assert_eq!(
            buf[(2, 12)].symbol(),
            "X",
            "left transcript columns must stay visible"
        );
        assert_eq!(
            buf[(2, 12)].bg,
            left_bg_before,
            "left transcript must not be dim_area-blended"
        );

        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                footer.to_ascii_lowercase().contains(needle),
                "right pane must keep the four idle CTAs; missing {needle} in {footer:?}"
            );
        }
        let lower = footer.to_ascii_lowercase();
        assert!(
            !lower.contains("notes") && !lower.contains("quit"),
            "right pane must not paint Notes or Quit; got {footer:?}"
        );
    }

    /// Named contract: idle plan-approval footer is four clickable CTAs
    /// (Approve / Comment / Revise / Exit), not the 1.0.3
    /// `request changes` + `c comment` placeholder row, and not Notes / Quit.
    #[test]
    fn plan_approval_footer_paints_five_cta_vocabulary() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        assert!(
            !footer.contains("request changes"),
            "approval footer must not use the 1.0.3 request-changes placeholder; got {footer:?}"
        );
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                footer.to_ascii_lowercase().contains(needle),
                "approval footer must name {needle}; got {footer:?}"
            );
        }
        let lower = footer.to_ascii_lowercase();
        assert!(
            !lower.contains("notes"),
            "approval footer must not paint Notes; got {footer:?}"
        );
        assert!(
            !lower.contains("quit"),
            "approval footer must not paint Quit; got {footer:?}"
        );
        assert!(
            !lower.contains("clarify"),
            "idle approval footer must not paint standalone Clarify; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.approve_button_area.is_some(),
            "Approve must be a clickable hit target"
        );
        assert!(
            plan.approve_notes_button_area.is_none(),
            "Notes must not be a clickable hit target"
        );
        assert!(
            plan.comment_button_area.is_some(),
            "Comment must be a clickable idle hit target"
        );
        assert!(
            plan.questions_button_area.is_none(),
            "Clarify is comment-flow only, not idle"
        );
        assert!(
            plan.send_button_area.is_some(),
            "Revise must be a clickable hit target"
        );
        assert!(
            plan.abandon_button_area.is_some(),
            "Exit must be a clickable hit target"
        );
    }

    /// Named contract (G1): footer last button is Exit, not Quit.
    #[test]
    fn plan_footer_exit_not_quit() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let lower = footer.to_ascii_lowercase();
        assert!(
            lower.contains("exit"),
            "approval footer must name Exit; got {footer:?}"
        );
        assert!(
            !lower.contains("quit"),
            "approval footer must not name Quit; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.abandon_button_area.is_some(),
            "Exit must be a clickable hit target"
        );
    }

    /// Named contract (G1): Notes (`A`) is removed from the footer.
    #[test]
    fn plan_footer_has_no_notes_button() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let lower = footer.to_ascii_lowercase();
        assert!(
            !lower.contains("notes"),
            "approval footer must not paint Notes; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.approve_notes_button_area.is_none(),
            "Notes must not be a clickable hit target"
        );
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "approval footer must name {needle}; got {footer:?}"
            );
        }
        assert!(
            !lower.contains("clarify"),
            "idle footer must not paint standalone Clarify; got {footer:?}"
        );
    }

    /// Idle present footer is Approve / Comment / Revise / Exit.
    /// Standalone Clarify is not an idle decision CTA.
    #[test]
    fn plan_approval_idle_footer_paints_comment_not_clarify() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;
        viewer.plan_mut().comment_flow_active = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let lower = footer.to_ascii_lowercase();
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "idle footer must name {needle}; got {footer:?}"
            );
        }
        assert!(
            !lower.contains("clarify"),
            "idle footer must not paint standalone Clarify; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.comment_button_area.is_some(),
            "Comment must be a clickable idle hit target"
        );
        assert!(
            plan.questions_button_area.is_none(),
            "Clarify must not be an idle hit target"
        );
        assert!(plan.approve_button_area.is_some());
        assert!(plan.send_button_area.is_some());
        assert!(plan.abandon_button_area.is_some());
    }

    /// After Comment (or focusing the plan prompt), footer is
    /// Approve / Clarify / Revise / Exit so the typed comment can ride
    /// with implement, read-only questions, or rewrite.
    #[test]
    fn plan_approval_comment_flow_footer_paints_clarify() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;
        viewer.plan_mut().comment_flow_active = true;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let lower = footer.to_ascii_lowercase();
        for needle in ["approve", "clarify", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "comment-flow footer must name {needle}; got {footer:?}"
            );
        }
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.questions_button_area.is_some(),
            "Clarify must be a clickable comment-flow hit target"
        );
        assert!(
            plan.comment_button_area.is_none(),
            "Comment is the entry; comment-flow replaces it with Clarify"
        );
    }

    /// `/view-plan` after Approve/Exit still paints the four idle present
    /// actions. Casual `c comment | y copy plan` is not the view-plan footer.
    #[test]
    fn view_plan_after_resolved_footer_paints_four_idle_ctas() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nAlready decided\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = false;
        viewer.plan_mut().show_action_buttons = true;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("view-plan footer needs a painted modal");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let lower = footer.to_ascii_lowercase();
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "/view-plan after resolved must name {needle}; got {footer:?}"
            );
        }
        assert!(
            !lower.contains("c comment") && !lower.contains("y copy plan"),
            "/view-plan after resolved must not stay casual comment+copy; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert!(
            plan.approve_button_area.is_some(),
            "Approve must stay a clickable hit target on view-plan"
        );
        assert!(plan.comment_button_area.is_some());
        assert!(plan.send_button_area.is_some());
        assert!(plan.abandon_button_area.is_some());
        assert_title_bar_copy_left_of_enlarge(&buf, plan, modal.y + modal.height.saturating_sub(1));
    }

    /// No selected CTA and no leftover recorded glyph means no choice dot.
    #[test]
    fn view_plan_no_recorded_choice_paints_no_choice_dot() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nNo choice yet\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;
        viewer.plan_mut().recorded_choice = None;
        viewer.plan_mut().selected_cta = None;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer.last_modal_area.expect("footer");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        let dot = crate::glyphs::filled_dot();
        assert!(
            !footer.contains(dot),
            "no selected CTA must not paint a choice dot; got {footer:?}"
        );
    }

    /// Operator: buttons marked when selected. A grok-oss.db recorded
    /// Approve row is not the live mark.
    #[test]
    fn recorded_plan_choice_is_not_the_live_selection_mark() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nApproved\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().recorded_choice = Some(RecordedPlanChoice::Approve);
        viewer.plan_mut().selected_cta = None;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer.last_modal_area.expect("footer");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        assert!(
            !label_has_choice_dot(&footer, "approve"),
            "a leftover recorded Approve must not mark the live CTA; got {footer:?}"
        );
    }

    /// Operator: selected idle CTA is visually marked.
    #[test]
    fn selected_idle_cta_is_visually_marked() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nSelect Revise\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().selected_cta = Some(SelectedPlanCta::Revise);

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer.last_modal_area.expect("footer");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        assert!(
            label_has_choice_dot(&footer, "revise"),
            "the selected idle CTA must be marked; got {footer:?}"
        );
        for other in ["approve", "comment", "exit"] {
            assert!(
                !label_has_choice_dot(&footer, other),
                "only the selected CTA is marked; {other} was; got {footer:?}"
            );
        }
    }

    /// Operator: plan approval pane has a clickable copy control.
    /// Copy is the title-bar glyph left of `[↗]`, not a fifth idle CTA.
    #[test]
    fn plan_approval_pane_has_a_clickable_copy_control() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nCopy me\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;
        viewer.plan_mut().comment_flow_active = true;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer.last_modal_area.expect("footer");
        let footer_y = modal.y + modal.height.saturating_sub(1);
        let footer = row_text(&buf, footer_y);
        let lower = footer.to_ascii_lowercase();
        assert!(
            !lower.contains("copy"),
            "comment overlay must not paint copy on the CTA row; got {footer:?}"
        );
        let plan = viewer.plan_ref().expect("plan extras");
        assert_title_bar_copy_left_of_enlarge(&buf, plan, footer_y);
        for needle in ["approve", "clarify", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "comment-flow CTAs must stay; missing {needle}; got {footer:?}"
            );
        }
    }

    /// Operator: the Approve / Comment / Revise / Exit row must not paint copy.
    #[test]
    fn plan_approval_cta_row_does_not_paint_copy() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nDo the thing\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer
            .last_modal_area
            .expect("approval footer needs a painted modal");
        let footer_y = modal.y + modal.height.saturating_sub(1);
        let footer = row_text(&buf, footer_y);
        let lower = footer.to_ascii_lowercase();
        assert!(
            !lower.contains("copy"),
            "CTA row must not paint copy; got {footer:?}"
        );
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "idle footer must name {needle}; got {footer:?}"
            );
        }
        let plan = viewer.plan_ref().expect("plan extras");
        assert_title_bar_copy_left_of_enlarge(&buf, plan, footer_y);
    }

    /// Comment-count badge (number + dot) is not the recorded-choice marker.
    #[test]
    fn view_plan_comment_count_badge_is_not_recorded_choice_dot() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nComments\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().recorded_choice = None;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 2);

        let modal = viewer.last_modal_area.expect("footer");
        let footer = row_text(&buf, modal.y + modal.height.saturating_sub(1));
        assert!(
            footer.contains('2'),
            "comment-count badge must still show the count; got {footer:?}"
        );
        assert!(
            !label_has_choice_dot(&footer, "approve"),
            "comment-count badge must not look like a recorded Approve; got {footer:?}"
        );
        assert!(
            !label_has_choice_dot(&footer, "revise"),
            "comment-count badge must not look like a recorded Revise; got {footer:?}"
        );
        assert!(
            !label_has_choice_dot(&footer, "exit"),
            "comment-count badge must not look like a recorded Exit; got {footer:?}"
        );
    }

    fn label_has_choice_dot(footer: &str, label: &str) -> bool {
        let lower = footer.to_ascii_lowercase();
        let Some(idx) = lower.find(label) else {
            return false;
        };
        let after = &footer[idx + label.len()..];
        let trimmed = after.trim_start();
        let dot = crate::glyphs::filled_dot();
        trimmed.starts_with(dot)
            && !after
                .chars()
                .take_while(|c| c.is_whitespace() || c.is_ascii_digit())
                .any(|c| c.is_ascii_digit())
    }

    #[test]
    fn markdown_viewer_comment_range_maps_full_soft_break_paragraph() {
        // Commenting round-trip: selecting all rows of a soft-break paragraph
        // must map back to the full file line range so the agent inspects the
        // correct lines. Pre-fix this collapsed to a single line number.
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "Line one,\nLine two,\nLine three.".to_owned(),
            None,
        )
        .expect("markdown content should open");
        viewer.prepare_layout(80, 20);
        viewer.set_initial_selection(1..4);
        viewer.prepare_layout(80, 20);

        assert_eq!(viewer.selected_line_range(), Some(1..4));
        assert_eq!(viewer.line_range_suffix(), Some(":1-3".to_owned()));
    }

    /// Glass is a bracketed control one cell left of copy. Copy stays one
    /// cell left of `[↗]`. Do not steal `assert_title_bar_copy_left_of_enlarge`.
    #[test]
    fn plan_preview_title_bar_search_glass_immediately_left_of_copy() {
        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nThen PLAN is next\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = true;
        viewer.plan_mut().feedback_active = true;
        viewer.plan_mut().show_action_buttons = false;

        let full = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(full);
        let theme = crate::theme::Theme::current();
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let modal = viewer.last_modal_area.expect("title bar needs a modal");
        let footer_y = modal.y + modal.height.saturating_sub(1);
        let plan = viewer.plan_ref().expect("plan extras");
        assert_title_bar_copy_left_of_enlarge(&buf, plan, footer_y);
        assert_title_bar_search_left_of_copy(&buf, plan);
    }

    /// The plan side panel frame is not a bright white stroke. Copy is a
    /// bordered control. Search and copy use the same bracket size and the
    /// same one-cell gap as enlarge and close.
    #[test]
    fn soft_plan_side_panel_uses_muted_frame_and_bracketed_header_controls() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = crate::theme::Theme::doge();

        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Proposed plan.\n\nKeep the inset and the wrapping body.\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.fullscreen = false;
        viewer.plan_mut().feedback_active = false;
        viewer.plan_mut().show_action_buttons = true;

        let full = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(full);
        render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

        let pane_w = LineViewerState::soft_plan_pane_width(full.width);
        let pane_x = full.width.saturating_sub(pane_w);
        let edge = &buf[(pane_x, 4)];
        let fg = edge.style().fg;
        assert!(
            edge.symbol() == "│" || edge.symbol() == "┃",
            "the side panel keeps a frame glyph; got {:?}",
            edge.symbol()
        );
        assert_eq!(
            fg,
            Some(theme.bg_base),
            "DOGE plan frame uses the canvas hairline, not a bright stroke"
        );
        assert_ne!(
            fg,
            Some(theme.prompt_border),
            "plan frame is not the white prompt border"
        );
        assert_ne!(fg, Some(theme.prompt_border_active));
        assert_ne!(fg, Some(theme.selection_border));
        assert_ne!(
            fg,
            Some(theme.text_primary),
            "plan frame is not primary white text"
        );
        assert_ne!(
            fg,
            Some(theme.gray_dim),
            "plan frame is not the neon cyan gray_dim stroke"
        );
        assert_ne!(fg, Some(Color::White));
        assert_ne!(fg, Some(Color::Rgb(255, 255, 255)));

        let right = &buf[(pane_x + pane_w - 1, 4)];
        assert_eq!(
            right.style().fg,
            Some(theme.bg_base),
            "the right edge uses the same muted frame"
        );
        assert!(
            right.symbol() == "│" || right.symbol() == "┃",
            "the right edge keeps a frame glyph; got {:?}",
            right.symbol()
        );

        let plan = viewer.plan_ref().expect("plan extras");
        let search = plan
            .search_button_area
            .expect("search stays a clickable header control");
        let copy = plan
            .copy_button_area
            .expect("copy stays a clickable header control");
        let enlarge = viewer
            .fullscreen_button_area
            .expect("enlarge stays on the title bar");
        let close = viewer
            .close_button_area
            .expect("close stays on the title bar");
        for area in [search, copy, enlarge, close] {
            assert_eq!(area.width, 3, "each header control is 3 columns");
            assert_eq!(area.height, 1, "each header control is one row");
            assert_eq!(
                buf[(area.x, area.y)].symbol(),
                "[",
                "header control opens with a square bracket"
            );
            assert_eq!(
                buf[(area.x + 2, area.y)].symbol(),
                "]",
                "header control closes with a square bracket"
            );
        }
        assert_eq!(
            buf[(copy.x + 1, copy.y)].symbol(),
            crate::glyphs::copy_icon(),
            "copy control has a border around the glyph"
        );
        assert_ne!(
            crate::glyphs::copy_button(),
            crate::glyphs::copy_icon(),
            "the bordered copy control is not the bare glyph"
        );
        assert!(
            crate::glyphs::copy_button().starts_with('[')
                && crate::glyphs::copy_button().ends_with(']'),
            "tool-card and plan-header copy is a bordered control, not flat text"
        );
        assert_eq!(
            buf[(search.x + 1, search.y)].symbol(),
            crate::glyphs::search_icon()
        );
        assert_eq!(
            buf[(enlarge.x + 1, enlarge.y)].symbol(),
            crate::glyphs::enlarge()
        );
        assert_eq!(
            buf[(close.x + 1, close.y)].symbol(),
            crate::glyphs::ballot_x()
        );
        assert_eq!(
            search.x + search.width + 1,
            copy.x,
            "one-cell gap between search and copy"
        );
        assert_eq!(
            copy.x + copy.width + 1,
            enlarge.x,
            "one-cell gap between copy and enlarge"
        );
        assert_eq!(
            enlarge.x + enlarge.width + 1,
            close.x,
            "one-cell gap between enlarge and close"
        );
        assert!(search.x + search.width < copy.x);
        assert!(copy.x + copy.width < enlarge.x);
        assert!(enlarge.x + enlarge.width < close.x);

        let modal = viewer.last_modal_area.expect("soft plan pane");
        let footer_y = modal.y + modal.height.saturating_sub(1);
        let footer = row_text(&buf, footer_y);
        let lower = footer.to_ascii_lowercase();
        for needle in ["approve", "comment", "revise", "exit"] {
            assert!(
                lower.contains(needle),
                "footer CTAs stay; missing {needle} in {footer:?}"
            );
        }
        assert_ne!(
            copy.y, footer_y,
            "copy stays on the title bar, not the CTA row"
        );
        assert_eq!(
            LineViewerState::SOFT_PLAN_TEXT_INSET,
            5,
            "plan text stays inset from the left frame"
        );
    }

    /// Query `plan` matches `Plan` and `PLAN`. Reuse LineViewerState search.
    /// n/N jump after Enter accepts the search.
    #[test]
    fn isolated_preview_search_query_plan_matches_plan_and_plan() {
        use crate::views::list_pane::InputBarMode;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut viewer = LineViewerState::open_markdown_content(
            "plan.md",
            "# Plan\n\nThen PLAN is next\n".to_owned(),
            None,
        )
        .expect("open plan");
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.prepare_layout(80, 24);
        viewer.list_state.open_search(&viewer.lines);
        assert_eq!(
            viewer.list_state.input_mode(),
            Some(InputBarMode::Search),
            "open_search must reuse :search, not a second engine"
        );
        for ch in ['p', 'l', 'a', 'n'] {
            viewer.list_state.handle_key_event(
                &KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &viewer.lines,
            );
        }
        assert_eq!(viewer.list_state.input_textarea().text(), "plan");
        viewer.list_state.handle_key_event(
            &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &viewer.lines,
        );
        assert!(
            viewer.list_state.input_mode().is_none(),
            "Enter accepts search and closes the input bar"
        );
        assert!(
            viewer.list_state.match_count() >= 2,
            "query plan must match Plan and PLAN; got {}",
            viewer.list_state.match_count()
        );
        let first = viewer.list_state.matcher().and_then(|m| m.current_match);
        viewer.list_state.handle_key_event(
            &KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &viewer.lines,
        );
        let second = viewer.list_state.matcher().and_then(|m| m.current_match);
        assert_ne!(first, second, "n jumps to the next hit");
        viewer.list_state.handle_key_event(
            &KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
            &viewer.lines,
        );
        let back = viewer.list_state.matcher().and_then(|m| m.current_match);
        assert_eq!(back, first, "N jumps to the previous hit");
    }
}
