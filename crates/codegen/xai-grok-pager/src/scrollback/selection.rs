//! Selection box rendering for v3 pager.
//!
//! The `SelectionBox` is computed by components (like ScrollbackPane) and rendered
//! by the frame, allowing selection boxes to span component boundaries.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::render::osc8::LinkOverlay;
use crate::scrollback::text_selection::ResolvedSelectionModel;
use crate::theme::Theme;

/// Box drawing characters for selection border.
mod border_chars {
    pub const TOP_LEFT: char = '┌';
    pub const TOP_RIGHT: char = '┐';
    pub const BOTTOM_LEFT: char = '└';
    pub const BOTTOM_RIGHT: char = '┘';
    pub const VERTICAL: char = '│';
    /// Dashed vertical - used on edge rows when clipped to indicate continuation.
    pub const VERTICAL_DASHED: char = '┆';
}

/// A selection box that can be drawn around a selected block.
///
/// The box consists of:
/// - Side borders (│) on the left and right edges of `inner_area`
/// - Top corners (┌┐) one row above `inner_area` (if `!top_clipped`)
/// - Bottom corners (└┘) one row below `inner_area` (if `!bottom_clipped`)
///
/// This struct is returned by components (like ScrollbackPane) and rendered
/// by the frame, allowing selection boxes to span component boundaries.
#[derive(Debug, Clone)]
pub struct SelectionBox {
    /// The inner area surrounded by the selection border.
    pub inner_area: Rect,
    /// True if the block has rows clipped at top (scrolled out of view).
    pub top_clipped: bool,
    /// True if the block has rows clipped at bottom.
    pub bottom_clipped: bool,
    /// Style for the border (typically just fg color).
    pub style: Style,
    /// Whether to render a close control replacing the top-right corner.
    pub closable: bool,
    /// Whether the close control is currently hovered.
    pub close_hovered: bool,
    /// Optional close label; `None` uses default `✗`.
    pub close_label: Option<&'static str>,
    /// Optional action label left of close (e.g. todo pane clear-finished `[−]`).
    pub action_label: Option<&'static str>,
    /// Whether the action control is currently hovered.
    pub action_hovered: bool,
    /// When false, the action still paints in a reserved slot (dim) but is not
    /// a live click target. Keeps chrome geometry stable as finished counts change.
    pub action_enabled: bool,
}

/// Output from render that needs post-processing.
///
/// Render returns this instead of mutating state, keeping render pure.
/// The caller is responsible for rendering these elements after the main pass.
///
/// # Example
/// ```ignore
/// let output = pane.render_with_scratch(area, buf, &state, &mut scratch);
///
/// // Post-render pass
/// if let Some(sel) = output.selection_box {
///     sel.render(buf);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct RenderOutput {
    /// Selection box to render around the selected entry.
    /// Rendered after main content so it can span component boundaries.
    pub selection_box: Option<SelectionBox>,
    /// Scroll info for scrollbar rendering.
    /// Viewport uses this to render the scrollbar at the correct position.
    pub scroll_info: Option<ScrollInfo>,
    /// Screen area of the individual selected entry (within a group).
    /// Used by agent_view to position inline buttons on the correct row.
    pub selected_entry_area: Option<Rect>,
    /// Per-frame resolved selection metadata for visible content.
    pub selection_model: ResolvedSelectionModel,
    /// OSC 8 link overlay for post-flush emission.
    pub link_overlay: LinkOverlay,
    /// Inline media to render via post-flush escape sequences.
    pub inline_media: Vec<crate::scrollback::render::InlineMediaPlacement>,
    /// Mermaid diagram affordance rows to paint + register click hit-rects for.
    pub diagram_affordances: Vec<crate::scrollback::render::DiagramAffordancePlacement>,
    /// Screen row (relative to the scrollback area top) of the sticky
    /// header's gap row, when this frame drew a pinned header. The ▲
    /// response-top indicator renders here; publishing the row the pane
    /// actually used keeps the indicator from re-deriving (and possibly
    /// disagreeing with) the frame's layout.
    pub sticky_gap_row: Option<u16>,
}

/// Scroll information for scrollbar rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollInfo {
    /// Current scroll offset (lines from top). `usize`: tall sessions exceed
    /// `u16::MAX`.
    pub scroll_offset: usize,
    /// Visible viewport height (lines). Stays `u16` (a terminal is never that tall).
    pub viewport_height: u16,
    /// Total content height (lines). `usize` for the same reason as `scroll_offset`.
    pub total_height: usize,
}

impl RenderOutput {
    /// Create empty render output.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create render output with a selection box.
    pub fn with_selection_box(selection_box: SelectionBox) -> Self {
        Self {
            selection_box: Some(selection_box),
            ..Self::default()
        }
    }

    /// Add scroll info to the output.
    pub fn with_scroll_info(mut self, scroll_info: ScrollInfo) -> Self {
        self.scroll_info = Some(scroll_info);
        self
    }
}

impl SelectionBox {
    /// Create a new selection box with the given inner area and style.
    pub fn new(inner_area: Rect, style: Style) -> Self {
        Self {
            inner_area,
            top_clipped: false,
            bottom_clipped: false,
            style,
            closable: false,
            close_hovered: false,
            close_label: None,
            action_label: None,
            action_hovered: false,
            action_enabled: true,
        }
    }

    /// Set whether the top is clipped (no top corners).
    pub fn with_top_clipped(mut self, clipped: bool) -> Self {
        self.top_clipped = clipped;
        self
    }

    /// Set whether the bottom is clipped (no bottom corners).
    pub fn with_bottom_clipped(mut self, clipped: bool) -> Self {
        self.bottom_clipped = clipped;
        self
    }

    /// Enable a close control replacing the top-right corner `┐` (default: `✗`).
    ///
    /// Normal state: same color as the border. Hovered: bright white.
    pub fn with_closable(mut self, closable: bool, hovered: bool) -> Self {
        self.closable = closable;
        self.close_hovered = hovered;
        self
    }

    /// Set close control label (`Some` implies closable).
    pub fn with_close_label(mut self, label: Option<&'static str>) -> Self {
        self.close_label = label;
        if label.is_some() {
            self.closable = true;
        }
        self
    }

    /// Optional chrome action left of close (e.g. clear-finished `[−]`).
    ///
    /// Geometry is independent of focus; product clear-finished supplies a
    /// label when the todo board is open with finished rows. Defaults to
    /// enabled (live click).
    pub fn with_action_label(mut self, label: Option<&'static str>, hovered: bool) -> Self {
        self.action_label = label;
        self.action_hovered = hovered;
        self
    }

    /// When false, label still occupies its reserved slot (dim paint) but is not
    /// interactive. Prefer over dropping the label so chrome does not jump.
    pub fn with_action_enabled(mut self, enabled: bool) -> Self {
        self.action_enabled = enabled;
        self
    }

    /// Hit-test rect for the close control, if it would be rendered.
    ///
    /// Pure computation — does not touch the buffer. Use for mouse hit-testing.
    /// Returns `None` if not closable, top is clipped, or no room.
    pub fn close_button_rect(&self) -> Option<Rect> {
        if !self.closable || self.top_clipped || self.inner_area.y == 0 {
            return None;
        }
        let label_w = self
            .close_label
            .map(|s| s.chars().count() as u16)
            .unwrap_or(1)
            .max(1);
        let right_x = self.inner_area.x + self.inner_area.width.saturating_sub(1);
        let x = right_x.saturating_sub(label_w.saturating_sub(1));
        Some(Rect {
            x,
            y: self.inner_area.y - 1,
            width: label_w,
            height: 1,
        })
    }

    /// Layout rect for the optional action control left of close.
    ///
    /// One space gap between action label and close. **Always reserves** a
    /// close-slot width (default ✗ = 1 cell) even when close is not painted, so
    /// the action does not jump left/right when focus toggles the close control.
    /// `None` when no label, top clipped, or not enough width.
    ///
    /// Geometry is independent of [`Self::action_enabled`]; callers register a
    /// mouse hit only when enabled.
    pub fn action_button_rect(&self) -> Option<Rect> {
        let label = self.action_label?;
        if self.top_clipped || self.inner_area.y == 0 {
            return None;
        }
        let label_w = (label.chars().count() as u16).max(1);
        let y = self.inner_area.y - 1;
        // Stable placement: leave room for close even when unfocused/not closable.
        let close_w = self
            .close_button_rect()
            .map(|r| r.width)
            .unwrap_or(1)
            .max(1);
        let need = label_w.saturating_add(1).saturating_add(close_w);
        if need > self.inner_area.width {
            return None;
        }
        let right_x = self.inner_area.x + self.inner_area.width.saturating_sub(1);
        // close occupies [right_x - close_w + 1, right_x] when present.
        let close_x = right_x.saturating_sub(close_w.saturating_sub(1));
        let x = close_x.saturating_sub(1 + label_w);
        if x < self.inner_area.x {
            return None;
        }
        Some(Rect {
            x,
            y,
            width: label_w,
            height: 1,
        })
    }

    /// Style for the optional action label (todo clear-finished icon).
    ///
    /// Quiet idle: theme `gray` when enabled (not always-on neon
    /// `accent_user` green). Stronger on hover (`text_primary`). Disabled
    /// uses dimmer `gray_dim`. Never agent magenta.
    fn action_paint_style(&self) -> Style {
        let theme = Theme::current();
        if !self.action_enabled {
            Style::default().fg(theme.gray_dim)
        } else if self.action_hovered {
            Style::default().fg(theme.text_primary)
        } else {
            // Enabled idle: quiet gray chrome, not pure human-green CTA glow.
            Style::default().fg(theme.gray)
        }
    }

    /// Paint only the optional action label (no rails, corners, or close).
    ///
    /// Generic path for an action without focus chrome. Product clear-finished
    /// uses this when the todo board is open but unfocused (finished rows only).
    pub fn render_action_only(&self, buf: &mut Buffer) {
        self.paint_action_label(buf);
    }

    fn paint_action_label(&self, buf: &mut Buffer) {
        if self.top_clipped || self.inner_area.y == 0 {
            return;
        }
        if let Some(action_rect) = self.action_button_rect()
            && let Some(label) = self.action_label
        {
            use crate::render::SafeBuf;
            buf.set_string_safe(
                action_rect.x,
                action_rect.y,
                label,
                self.action_paint_style(),
            );
        }
    }

    /// Render the selection box to the buffer.
    ///
    /// Draws:
    /// - Side borders (│) on left and right edges of inner_area
    /// - Dashed borders (┆) on edge rows when clipped, to indicate continuation
    /// - Top corners (┌┐) at inner_area.y - 1 if !top_clipped and y > 0
    /// - Bottom corners (└┘) at inner_area.y + height if !bottom_clipped
    /// - Close button (✗) left of ┐ if enabled
    pub fn render(&self, buf: &mut Buffer) {
        let area = self.inner_area;
        if area.width == 0 || area.height == 0 {
            return;
        }

        let left_x = area.x;
        let right_x = area.x + area.width.saturating_sub(1);
        let y_top = area.y;
        let y_bottom = area.y + area.height.saturating_sub(1);

        // Draw side borders
        for y in y_top..=y_bottom {
            let is_first_row = y == y_top;
            let is_last_row = y == y_bottom;
            let use_dashed =
                (is_first_row && self.top_clipped) || (is_last_row && self.bottom_clipped);

            let vert_char = if use_dashed {
                border_chars::VERTICAL_DASHED
            } else {
                border_chars::VERTICAL
            };

            if let Some(cell) = buf.cell_mut((left_x, y)) {
                cell.set_char(vert_char).set_style(self.style);
            }
            if let Some(cell) = buf.cell_mut((right_x, y)) {
                cell.set_char(vert_char).set_style(self.style);
            }
        }

        // Draw top corners (if not clipped and there's room)
        if !self.top_clipped && y_top > 0 {
            let corner_y = y_top - 1;
            if let Some(cell) = buf.cell_mut((left_x, corner_y)) {
                cell.set_char(border_chars::TOP_LEFT).set_style(self.style);
            }
            // Close control replaces ┐, or draw normal corner
            if let Some(close_rect) = self.close_button_rect() {
                let style = if self.close_hovered {
                    Style::default().fg(Theme::current().text_primary)
                } else {
                    self.style
                };
                if let Some(label) = self.close_label {
                    use crate::render::SafeBuf;
                    buf.set_string_safe(close_rect.x, close_rect.y, label, style);
                } else if let Some(cell) = buf.cell_mut((close_rect.x, close_rect.y)) {
                    cell.set_symbol(crate::glyphs::ballot_x()).set_style(style);
                }
            } else if let Some(cell) = buf.cell_mut((right_x, corner_y)) {
                cell.set_char(border_chars::TOP_RIGHT).set_style(self.style);
            }
            // Optional action left of close (todo clear-finished [−] when open + finished).
            self.paint_action_label(buf);
        }

        // Draw bottom corners (if not clipped)
        if !self.bottom_clipped {
            let corner_y = y_bottom + 1;
            if let Some(cell) = buf.cell_mut((left_x, corner_y)) {
                cell.set_char(border_chars::BOTTOM_LEFT)
                    .set_style(self.style);
            }
            if let Some(cell) = buf.cell_mut((right_x, corner_y)) {
                cell.set_char(border_chars::BOTTOM_RIGHT)
                    .set_style(self.style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_box_render() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection = SelectionBox::new(Rect::new(0, 2, 10, 4), Style::default());

        selection.render(&mut buf);

        // Check top corners at y=1 (inner_area.y - 1)
        assert_eq!(buf.cell((0, 1)).unwrap().symbol(), "┌");
        assert_eq!(buf.cell((9, 1)).unwrap().symbol(), "┐");

        // Check side borders at y=2..=5 (all solid, not clipped)
        for y in 2..=5 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "│");
            assert_eq!(buf.cell((9, y)).unwrap().symbol(), "│");
        }

        // Check bottom corners at y=6 (inner_area.y + height)
        assert_eq!(buf.cell((0, 6)).unwrap().symbol(), "└");
        assert_eq!(buf.cell((9, 6)).unwrap().symbol(), "┘");
    }

    #[test]
    fn test_selection_box_top_clipped() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection =
            SelectionBox::new(Rect::new(0, 2, 10, 4), Style::default()).with_top_clipped(true);

        selection.render(&mut buf);

        // Top corners should NOT be drawn
        assert_ne!(buf.cell((0, 1)).unwrap().symbol(), "┌");
        assert_ne!(buf.cell((9, 1)).unwrap().symbol(), "┐");

        // First row (y=2) should have DASHED borders
        assert_eq!(buf.cell((0, 2)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 2)).unwrap().symbol(), "┆");

        // Middle rows should have solid borders
        for y in 3..=5 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "│");
            assert_eq!(buf.cell((9, y)).unwrap().symbol(), "│");
        }

        // Bottom corners should be drawn
        assert_eq!(buf.cell((0, 6)).unwrap().symbol(), "└");
    }

    #[test]
    fn test_selection_box_bottom_clipped() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection =
            SelectionBox::new(Rect::new(0, 2, 10, 4), Style::default()).with_bottom_clipped(true);

        selection.render(&mut buf);

        // Top corners should be drawn
        assert_eq!(buf.cell((0, 1)).unwrap().symbol(), "┌");
        assert_eq!(buf.cell((9, 1)).unwrap().symbol(), "┐");

        // First rows should have solid borders
        for y in 2..=4 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "│");
            assert_eq!(buf.cell((9, y)).unwrap().symbol(), "│");
        }

        // Last row (y=5) should have DASHED borders
        assert_eq!(buf.cell((0, 5)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 5)).unwrap().symbol(), "┆");

        // Bottom corners should NOT be drawn
        assert_ne!(buf.cell((0, 6)).unwrap().symbol(), "└");
        assert_ne!(buf.cell((9, 6)).unwrap().symbol(), "┘");
    }

    #[test]
    fn test_selection_box_both_clipped() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection = SelectionBox::new(Rect::new(0, 2, 10, 4), Style::default())
            .with_top_clipped(true)
            .with_bottom_clipped(true);

        selection.render(&mut buf);

        // No corners should be drawn
        assert_ne!(buf.cell((0, 1)).unwrap().symbol(), "┌");
        assert_ne!(buf.cell((0, 6)).unwrap().symbol(), "└");

        // First row (y=2) should have DASHED borders
        assert_eq!(buf.cell((0, 2)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 2)).unwrap().symbol(), "┆");

        // Middle rows should have solid borders
        for y in 3..=4 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "│");
            assert_eq!(buf.cell((9, y)).unwrap().symbol(), "│");
        }

        // Last row (y=5) should have DASHED borders
        assert_eq!(buf.cell((0, 5)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 5)).unwrap().symbol(), "┆");
    }

    #[test]
    fn test_selection_box_single_row_both_clipped() {
        // Edge case: only 1 row visible, both ends clipped
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection = SelectionBox::new(Rect::new(0, 3, 10, 1), Style::default())
            .with_top_clipped(true)
            .with_bottom_clipped(true);

        selection.render(&mut buf);

        // The single row should have DASHED borders (first row = last row, both clipped)
        assert_eq!(buf.cell((0, 3)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 3)).unwrap().symbol(), "┆");

        // No corners
        assert_ne!(buf.cell((0, 2)).unwrap().symbol(), "┌");
        assert_ne!(buf.cell((0, 4)).unwrap().symbol(), "└");
    }

    #[test]
    fn test_selection_box_single_row_top_clipped_only() {
        // Edge case: only 1 row visible, only top clipped
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        let selection =
            SelectionBox::new(Rect::new(0, 3, 10, 1), Style::default()).with_top_clipped(true);

        selection.render(&mut buf);

        // The single row should have DASHED borders (it's first row and top_clipped)
        assert_eq!(buf.cell((0, 3)).unwrap().symbol(), "┆");
        assert_eq!(buf.cell((9, 3)).unwrap().symbol(), "┆");

        // Bottom corners should be drawn
        assert_eq!(buf.cell((0, 4)).unwrap().symbol(), "└");
        assert_eq!(buf.cell((9, 4)).unwrap().symbol(), "┘");
    }

    #[test]
    fn test_selection_box_at_top_edge() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));

        // Selection at y=0 (no room for top corners even if not clipped)
        let selection = SelectionBox::new(Rect::new(0, 0, 10, 4), Style::default());

        selection.render(&mut buf);

        // Side borders at y=0..=3 (all solid, not clipped)
        for y in 0..=3 {
            assert_eq!(buf.cell((0, y)).unwrap().symbol(), "│");
        }

        // Bottom corners at y=4
        assert_eq!(buf.cell((0, 4)).unwrap().symbol(), "└");
    }

    /// Compact chrome control for archiving finished board rows (`[−]`).
    fn clear_finished_chrome() -> &'static str {
        crate::glyphs::clear_finished_button()
    }

    #[test]
    fn action_button_sits_left_of_close_with_gap() {
        // Wide enough for [−] + gap + ✗
        let label = clear_finished_chrome();
        let sel = SelectionBox::new(Rect::new(0, 2, 40, 4), Style::default())
            .with_closable(true, false)
            .with_action_label(Some(label), false);
        let close = sel.close_button_rect().expect("close");
        let action = sel.action_button_rect().expect("action");
        assert_eq!(action.height, 1);
        assert_eq!(action.y, close.y);
        assert_eq!(action.width, label.chars().count() as u16);
        assert_eq!(action.width, 3, "clear-finished chrome is icon-width [−]");
        // Gap of one cell between action right edge and close left.
        assert_eq!(action.x + action.width + 1, close.x);
    }

    /// Named contract: without a painted close control, action still reserves
    /// a close slot so x matches the focused (closable) layout.
    #[test]
    fn action_button_without_close_reserves_close_slot() {
        let label = clear_finished_chrome();
        let open = SelectionBox::new(Rect::new(0, 2, 40, 4), Style::default())
            .with_action_label(Some(label), false);
        assert!(open.close_button_rect().is_none());
        let action = open.action_button_rect().expect("action without close");
        assert_eq!(action.width, label.chars().count() as u16);
        assert_eq!(action.y, 1);
        // Reserved: [action][gap][1-cell close slot] against right edge.
        let right_x = 40 - 1;
        assert_eq!(action.x + action.width + 1 + 1 - 1, right_x);
    }

    /// Named contract: action x is identical with and without closable close,
    /// so focusing the todo pane does not jump the clear-finished control.
    #[test]
    fn action_button_x_stable_with_or_without_close() {
        let area = Rect::new(0, 2, 40, 4);
        let label = clear_finished_chrome();
        let without_close =
            SelectionBox::new(area, Style::default()).with_action_label(Some(label), false);
        let with_close = SelectionBox::new(area, Style::default())
            .with_closable(true, false)
            .with_action_label(Some(label), false);
        let a = without_close.action_button_rect().expect("unfocused");
        let b = with_close.action_button_rect().expect("focused");
        assert_eq!(
            a.x, b.x,
            "clear-finished x must not jump when focus paints close"
        );
        assert_eq!(a.width, b.width);
        assert_eq!(a.y, b.y);
    }

    /// Named contract: enabled idle clear-finished is quiet gray,
    /// not always-on neon `accent_user` green, and never agent magenta.
    #[test]
    fn clear_finished_action_idle_is_quiet_not_neon_green_or_magenta() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = Theme::current();
        let magenta = theme.accent_running;
        let neon = theme.accent_user;
        assert_ne!(magenta, neon, "DOGE setup: magenta != human green");

        let label = clear_finished_chrome();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        let sel = SelectionBox::new(
            Rect::new(0, 2, 40, 4),
            // Border style deliberately agent magenta — action must not inherit it.
            Style::default().fg(magenta),
        )
        .with_action_label(Some(label), false)
        .with_action_enabled(true);
        sel.render_action_only(&mut buf);

        let action = sel.action_button_rect().expect("action");
        let cell = buf.cell((action.x, action.y)).expect("label cell");
        assert_eq!(
            cell.fg, theme.gray,
            "enabled idle clear-finished must be quiet gray, got {:?}",
            cell.fg
        );
        assert_ne!(
            cell.fg, neon,
            "enabled idle must not be always-on accent_user neon green"
        );
        assert_ne!(
            cell.fg, magenta,
            "clear-finished must not inherit agent magenta"
        );
        // Icon paints (bracketed minus), not the long "Clear finished" string.
        let mut painted = String::new();
        for x in action.x..action.x + action.width {
            if let Some(c) = buf.cell((x, action.y)) {
                painted.push_str(c.symbol());
            }
        }
        assert_eq!(painted, label, "must paint compact clear-finished icon");
        assert!(!painted.contains("Clear finished"));
        assert!(
            painted.contains('\u{2212}') || painted.contains('-'),
            "must use minus glyph, not empty-set, got {painted:?}"
        );
        assert!(
            !painted.contains('\u{2205}'),
            "empty-set was dogfood-rejected"
        );
    }

    /// Hover brightens clear-finished above idle gray.
    #[test]
    fn clear_finished_action_hover_is_stronger_than_idle() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = Theme::current();
        let label = clear_finished_chrome();

        let mut buf_idle = Buffer::empty(Rect::new(0, 0, 40, 8));
        let idle = SelectionBox::new(Rect::new(0, 2, 40, 4), Style::default())
            .with_action_label(Some(label), false)
            .with_action_enabled(true);
        idle.render_action_only(&mut buf_idle);
        let action = idle.action_button_rect().expect("action");
        let idle_fg = buf_idle.cell((action.x, action.y)).expect("idle").fg;

        let mut buf_hover = Buffer::empty(Rect::new(0, 0, 40, 8));
        let hover = SelectionBox::new(Rect::new(0, 2, 40, 4), Style::default())
            .with_action_label(Some(label), true)
            .with_action_enabled(true);
        hover.render_action_only(&mut buf_hover);
        let hover_fg = buf_hover.cell((action.x, action.y)).expect("hover").fg;

        assert_eq!(idle_fg, theme.gray);
        assert_eq!(hover_fg, theme.text_primary);
        assert_ne!(
            idle_fg, hover_fg,
            "hover must read stronger than quiet idle"
        );
        assert_ne!(hover_fg, theme.accent_running, "hover must not be magenta");
    }

    /// Named contract: disabled action still paints in the reserved slot (dim),
    /// same geometry as enabled, so zero finished rows do not collapse chrome.
    #[test]
    fn clear_finished_disabled_reserves_slot_and_paints_dim() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = Theme::current();
        let area = Rect::new(0, 2, 40, 4);
        let label = clear_finished_chrome();

        let enabled = SelectionBox::new(area, Style::default())
            .with_action_label(Some(label), false)
            .with_action_enabled(true);
        let disabled = SelectionBox::new(area, Style::default())
            .with_action_label(Some(label), false)
            .with_action_enabled(false);

        let e = enabled.action_button_rect().expect("enabled geom");
        let d = disabled.action_button_rect().expect("disabled geom");
        assert_eq!(e.x, d.x, "disabled must keep same x as enabled");
        assert_eq!(e.width, d.width);
        assert_eq!(e.width, 3, "icon-width reserved slot");

        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        disabled.render_action_only(&mut buf);
        let cell = buf.cell((d.x, d.y)).expect("label cell");
        assert_eq!(
            cell.fg, theme.gray_dim,
            "disabled clear-finished must paint gray_dim, got {:?}",
            cell.fg
        );
        assert_ne!(
            cell.fg, theme.accent_user,
            "disabled must not look like a live human-green CTA"
        );
        assert_ne!(
            cell.fg, theme.gray,
            "disabled must read dimmer than enabled idle gray"
        );
        // First glyph of the bracketed icon is still painted.
        assert_eq!(cell.symbol().chars().next(), Some('['));
    }
}
