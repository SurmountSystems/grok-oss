//! Renders `TodoItem`s from `xai-grok-tools` in a `ListPane`.
//!
//! Wraps the canonical `TodoItem` type with a `ListItem` implementation that provides status-icon prefixes and styled content.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use xai_grok_shell::tools::{TodoItem, TodoStatus};

use super::list_pane::ListItem;

#[derive(Debug, Clone, Copy)]
pub struct TodoStatusStyle {
    pub icon_fg: Color,
    pub text_style: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct TodoPaneStyle {
    pub pending: TodoStatusStyle,
    pub in_progress: TodoStatusStyle,
    pub completed: TodoStatusStyle,
    pub cancelled: TodoStatusStyle,
    /// Style for `meta.kind` tags (`[work]`, `[phase]`, …).
    ///
    /// Muted chrome: pure blue. Not cyan (`gray_dim` / `accent_system` system
    /// info), not human green, not agent magenta. Blue can be hard to see on
    /// black; that is intentional for secondary kind chrome.
    pub kind_badge: Style,
}

impl Default for TodoPaneStyle {
    fn default() -> Self {
        // The theme quantizes colors for terminal compatibility
        let theme = crate::theme::Theme::current();
        // Kind tags: pure DOGE blue (not theme cyan meta slots). See `kind_badge`.
        let kind_badge_fg = Color::Rgb(0, 0, 255);

        Self {
            pending: TodoStatusStyle {
                icon_fg: theme.text_primary,
                text_style: Style::default().fg(theme.text_primary),
            },
            in_progress: TodoStatusStyle {
                icon_fg: theme.warning,
                text_style: Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            },
            completed: TodoStatusStyle {
                icon_fg: theme.accent_success,
                text_style: Style::default().fg(theme.gray_bright),
            },
            cancelled: TodoStatusStyle {
                icon_fg: theme.accent_error,
                text_style: Style::default()
                    .fg(theme.gray_bright)
                    .add_modifier(Modifier::CROSSED_OUT),
            },
            kind_badge: Style::default().fg(kind_badge_fg),
        }
    }
}

/// A `TodoItem` wrapped for display in a `ListPane`.
///
/// Caches the styled `Line` for `content()` and generates a status-icon `prefix()` per frame.
#[derive(Debug, Clone)]
pub struct TodoListEntry {
    /// Unique ID (index in the todo list, or a stable ID from the model).
    pub id: u64,
    pub item: TodoItem,
    styled: Line<'static>,
    status_style: TodoStatusStyle,
}

impl TodoListEntry {
    pub fn new(id: u64, item: TodoItem, style: &TodoPaneStyle) -> Self {
        let status_style = match item.status {
            TodoStatus::Pending => style.pending,
            TodoStatus::InProgress => style.in_progress,
            TodoStatus::Completed => style.completed,
            TodoStatus::Cancelled => style.cancelled,
        };
        // Light level badge from `meta.kind` when present (residual|phase|work|child).
        // Colour: pure blue muted chrome (`style.kind_badge`), not cyan system/info.
        let kind = item
            .meta
            .as_ref()
            .and_then(|m| m.get("kind"))
            .and_then(|v| v.as_str())
            .filter(|k| !k.is_empty());
        let styled = if let Some(kind) = kind {
            Line::from(vec![
                Span::styled(format!("[{kind}] "), style.kind_badge),
                Span::styled(item.content.clone(), status_style.text_style),
            ])
        } else {
            Line::from(Span::styled(item.content.clone(), status_style.text_style))
        };
        Self {
            id,
            item,
            styled,
            status_style,
        }
    }

    fn icon(&self) -> &'static str {
        match self.item.status {
            TodoStatus::Pending => "□",
            TodoStatus::InProgress => "▶",
            TodoStatus::Completed => crate::glyphs::check_mark(),
            TodoStatus::Cancelled => crate::glyphs::ballot_x(),
        }
    }
}

impl ListItem for TodoListEntry {
    fn content(&self) -> &Line<'_> {
        &self.styled
    }

    fn prefix(&self) -> Option<Line<'_>> {
        let icon = self.icon();
        Some(Line::from(vec![
            Span::styled(icon, Style::default().fg(self.status_style.icon_fg)),
            Span::raw(" "),
        ]))
    }

    fn stable_id(&self) -> u64 {
        self.id
    }

    fn search_text(&self) -> &str {
        &self.item.content
    }
}

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

use crate::appearance::LayoutConfig;
use crate::theme::ThemeKind;

use super::list_pane::{ListPane, ListPaneConfig, ListPaneState, ListPaneStyle, WrapMode};
use super::overlay::OverlayState;

/// Feeds the placeholder message shown when the pane is empty.
/// Counts ALL items regardless of the `show_done` filter.
#[derive(Debug, Clone, Copy, Default)]
pub struct TodoCounts {
    pub in_progress: usize,
    pub pending: usize,
    pub completed: usize,
    pub cancelled: usize,
    /// Sum of leaf sizes for completed leaves (points mode only).
    pub completed_points: usize,
    /// Sum of leaf sizes for non-cancelled sized leaves (points mode only).
    pub total_points: usize,
    /// True when at least one non-cancelled leaf has an explicit size.
    pub points_mode: bool,
}

impl TodoCounts {
    pub fn total(&self) -> usize {
        self.in_progress + self.pending + self.completed + self.cancelled
    }
}

fn empty_placeholder_message(todos_empty: bool, counts: TodoCounts) -> String {
    if todos_empty {
        return "No todo items.".into();
    }
    match (counts.completed, counts.cancelled) {
        (_, 0) => "All done.".into(),
        (0, c) => format!("{c} cancelled."),
        (d, c) => format!("{d} done. {c} cancelled."),
    }
}

/// Absolute maximum height (in lines) the todo pane will request.
const MAX_TODO_HEIGHT: u16 = 10;

/// Maximum fraction of total view height the todo pane may occupy.
const MAX_TODO_FRACTION: f32 = 0.15;

/// Owns the raw `TodoItem` data, the filtered `TodoListEntry` cache, `ListPaneState`, and style config.
/// `AgentView` holds a single `TodoPane` and delegates input and render to it.
pub struct TodoPane {
    /// Raw todo items from ACP Plan updates.
    todos: Vec<TodoItem>,
    /// Filtered and styled entries for `ListPane` rendering.
    /// Rebuilt from `todos` at the start of each `render()` call.
    entries: Vec<TodoListEntry>,
    /// List pane state (scroll, selection, search, layout cache).
    pub list_state: ListPaneState,
    /// Per-status visual style for icons and text.
    style: TodoPaneStyle,
    /// Visual style for the `ListPane` widget (selection bg, etc.).
    list_style: ListPaneStyle,
    /// Whether to show completed and cancelled items.
    show_done: bool,
    /// Shared visibility and focus state.
    pub overlay: OverlayState,
    /// Last theme kind seen; used to detect theme switches and restyle.
    last_theme: ThemeKind,
}

impl Default for TodoPane {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoPane {
    pub fn new() -> Self {
        let config = ListPaneConfig {
            follow_enabled: false,
            wrap_toggle_enabled: false,
            search_enabled: true,
            copy_enabled: true,
            show_selection_when_unfocused: false,
            visual_select_enabled: false,
            filter_enabled: true,
            goto_line_enabled: false,
        };
        let mut list_state = ListPaneState::new_with_config(WrapMode::NoWrap, false, config);
        list_state.set_clipboard_provider(Box::new(crate::clipboard::SystemClipboard));
        Self {
            todos: Vec::new(),
            entries: Vec::new(),
            list_state,
            style: TodoPaneStyle::default(),
            list_style: ListPaneStyle::default(),
            show_done: true,
            overlay: OverlayState::hidden(),
            last_theme: crate::theme::Theme::current_kind(),
        }
    }

    pub fn todos(&self) -> &[TodoItem] {
        &self.todos
    }

    /// Replace all todo items (called from ACP Plan handler).
    ///
    /// Does NOT auto-show the todo pane. Users toggle it with Ctrl-T.
    pub fn update_todos(&mut self, items: Vec<TodoItem>) {
        self.todos = items;
    }

    fn compute_counts(items: &[TodoItem]) -> TodoCounts {
        use std::collections::HashSet;

        // Same rule as the tool: any id used as someone's parentId is a parent.
        let parent_ids: HashSet<&str> = items
            .iter()
            .filter_map(|item| {
                item.meta
                    .as_ref()
                    .and_then(|m| m.get("parentId"))
                    .and_then(|v| v.as_str())
            })
            .collect();

        let mut c = TodoCounts::default();
        for item in items {
            match item.status {
                TodoStatus::InProgress => c.in_progress += 1,
                TodoStatus::Pending => c.pending += 1,
                TodoStatus::Completed => c.completed += 1,
                TodoStatus::Cancelled => c.cancelled += 1,
            }
            if matches!(item.status, TodoStatus::Cancelled) {
                continue;
            }
            // Parents never count toward points (even if size is still set).
            let item_id = item
                .meta
                .as_ref()
                .and_then(|m| m.get("id"))
                .and_then(|v| v.as_str());
            if item_id.is_some_and(|id| parent_ids.contains(id)) {
                continue;
            }
            if let Some(sz) = item.size {
                c.points_mode = true;
                c.total_points += usize::from(sz);
                if matches!(item.status, TodoStatus::Completed) {
                    c.completed_points += usize::from(sz);
                }
            }
        }
        c
    }

    /// Current status counts (across ALL items, ignoring `show_done`).
    pub fn counts(&self) -> TodoCounts {
        Self::compute_counts(&self.todos)
    }

    /// Visible when the overlay is shown.
    /// When empty, the pane shows a placeholder message instead of the list.
    pub fn is_visible(&self) -> bool {
        self.overlay.visible
    }

    /// Called after an overlay state change hides the pane.
    /// Closes the input bar if it's open mid-typing, but keeps any accepted search or filter across hide and show.
    pub fn on_state_change(&mut self) {
        if !self.overlay.visible {
            self.list_state.close_input_bar();
        }
    }

    /// Whether completed and cancelled items are shown.
    pub fn show_done(&self) -> bool {
        self.show_done
    }

    /// Toggle visibility of completed and cancelled items.
    pub fn toggle_show_done(&mut self) {
        self.show_done = !self.show_done;
    }

    /// Desired height in lines for layout computation. Returns 0 when hidden (overlay not visible or no
    /// items). When visible but empty, returns 2 (for placeholder message). Otherwise: `min(10, 15% of
    /// view_height)` but at least 1.
    pub fn desired_height(&self, view_height: u16) -> u16 {
        if !self.overlay.visible {
            return 0;
        }
        let count = self.visible_count();
        if count == 0 {
            return 1;
        }
        let fraction_cap = (view_height as f32 * MAX_TODO_FRACTION).floor() as u16;
        let max = MAX_TODO_HEIGHT.min(fraction_cap).max(1);
        (count as u16).min(max).max(1)
    }

    /// Number of items that pass the `show_done` filter.
    fn visible_count(&self) -> usize {
        if self.show_done {
            self.todos.len()
        } else {
            self.todos
                .iter()
                .filter(|t| !matches!(t.status, TodoStatus::Completed | TodoStatus::Cancelled))
                .count()
        }
    }

    /// Rebuild `entries` from `todos`, filtered by `show_done`. Items preserve their original order
    /// from the agent (no reordering by status). IDs are based on the original index in `todos` so that
    /// `ListPaneState` can maintain selection across rebuilds.
    fn rebuild_entries(&mut self) {
        self.entries.clear();
        for (idx, item) in self.todos.iter().enumerate() {
            if !self.show_done
                && matches!(item.status, TodoStatus::Completed | TodoStatus::Cancelled)
            {
                continue;
            }
            self.entries
                .push(TodoListEntry::new(idx as u64, item.clone(), &self.style));
        }
    }

    /// Handle a key event when the todo pane is focused.
    ///
    /// Returns `true` if the event was consumed.
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        // 'h' toggles show_done (only when not typing in the search or filter bar)
        if key.code == KeyCode::Char('h') && self.list_state.input_mode().is_none() {
            self.toggle_show_done();
            self.rebuild_entries();
            return true;
        }
        // Don't route to ListPaneState when empty.
        if self.entries.is_empty() {
            return false;
        }
        self.list_state.handle_key_event(key, &self.entries)
    }

    pub fn handle_paste(&mut self, text: &str) -> bool {
        self.list_state.handle_paste(text, &self.entries)
    }

    /// Handle a mouse scroll event over the todo pane area. Caps scroll speed for small viewports. The
    /// app-level scroll accumulator can produce deltas of 3-5 lines, which would jump past most items
    /// in a 4-row pane.
    pub fn handle_scroll(&mut self, lines: i32, col: u16, row: u16) {
        let max = match self.list_state.viewport_height() {
            0..=5 => 1,
            6..=10 => 2,
            _ => lines.unsigned_abs() as i32,
        };
        let capped = lines.signum() * lines.abs().min(max);
        self.list_state
            .handle_scroll_event(capped, col, row, &self.entries);
    }

    /// Handle a mouse click or drag event over the todo pane area.
    ///
    /// Returns `true` if the event was consumed.
    pub fn handle_mouse(&mut self, kind: MouseEventKind, col: u16, row: u16, area: Rect) -> bool {
        self.list_state
            .handle_mouse_event(kind, col, row, area, &self.entries)
    }

    /// Compute the inner content area with horizontal padding matching the scrollback's `HorizontalLayout`.
    /// The left pad is accent + block_pad_left; the right pad is block_pad_right.
    fn content_area(area: Rect, layout_cfg: &LayoutConfig) -> Rect {
        use crate::scrollback::layout::HorizontalLayout;
        let pad_left = HorizontalLayout::ACCENT + layout_cfg.block_pad_left;
        let pad_right = layout_cfg.block_pad_right;
        Rect {
            x: area.x + pad_left,
            y: area.y,
            width: area.width.saturating_sub(pad_left + pad_right),
            height: area.height,
        }
    }

    /// Render the todo pane into the given area. Rebuilds entries from `todos` each call, which is
    /// cheap at the typical count of under 20 items. Runs layout, then renders the `ListPane` widget in
    /// a padded inner area matching the scrollback's horizontal layout.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        layout_cfg: &LayoutConfig,
    ) {
        // Detect a theme switch and refresh styles before rebuilding entries
        let current_theme = crate::theme::Theme::current_kind();
        if current_theme != self.last_theme {
            self.last_theme = current_theme;
            self.style = TodoPaneStyle::default();
            self.list_style = ListPaneStyle::default();
        }

        self.rebuild_entries();
        let inner = Self::content_area(area, layout_cfg);
        if self.entries.is_empty() {
            if inner.height > 0 && inner.width > 0 {
                let msg = empty_placeholder_message(self.todos.is_empty(), self.counts());
                let theme = crate::theme::Theme::current();
                let span = ratatui::text::Span::styled(
                    msg,
                    ratatui::style::Style::default().fg(theme.gray_bright),
                );
                buf.set_span(inner.x, inner.y, &span, inner.width);
            }
            return;
        }
        self.list_state
            .prepare_layout(&self.entries, inner.width, inner.height);
        ListPane::new(&self.entries)
            .focused(focused)
            .style(self.list_style)
            .render(inner, buf, &mut self.list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_shell::tools::TodoPriority;

    fn counts(completed: usize, cancelled: usize) -> TodoCounts {
        TodoCounts {
            completed,
            cancelled,
            ..Default::default()
        }
    }

    #[test]
    fn compute_counts_points_mode_from_sizes() {
        let items = vec![
            TodoItem {
                content: "a".into(),
                priority: TodoPriority::default(),
                status: TodoStatus::Completed,
                meta: None,
                size: Some(2),
            },
            TodoItem {
                content: "b".into(),
                priority: TodoPriority::default(),
                status: TodoStatus::Pending,
                meta: None,
                size: Some(1),
            },
            TodoItem {
                content: "phase".into(),
                priority: TodoPriority::default(),
                status: TodoStatus::Pending,
                meta: Some(serde_json::json!({"kind": "phase"})),
                size: None,
            },
        ];
        let c = TodoPane::compute_counts(&items);
        assert!(c.points_mode);
        assert_eq!(c.completed_points, 2);
        assert_eq!(c.total_points, 3);
        assert_eq!(c.completed, 1);
        assert_eq!(c.pending, 2);
    }

    /// Sized item that later gains a child (zombie parent size) must not
    /// contribute badge points — only leaf sizes count (mirrors tool).
    #[test]
    fn compute_counts_excludes_parent_size_when_child_references_parent_id() {
        let items = vec![
            // Was a sized leaf; then gained a child. Size may still be set
            // (ACP / pre-clear state). Parent id is in meta.id for the graph.
            TodoItem {
                content: "Parent phase".into(),
                priority: TodoPriority::default(),
                status: TodoStatus::Completed,
                meta: Some(serde_json::json!({"id": "parent", "kind": "phase"})),
                size: Some(2),
            },
            TodoItem {
                content: "Child leaf".into(),
                priority: TodoPriority::default(),
                status: TodoStatus::Completed,
                meta: Some(serde_json::json!({"parentId": "parent", "id": "child"})),
                size: Some(1),
            },
        ];
        let c = TodoPane::compute_counts(&items);
        assert!(c.points_mode);
        // Parent size=2 ignored; only child size=1.
        assert_eq!(c.total_points, 1);
        assert_eq!(c.completed_points, 1);
        // Status counts still include both items.
        assert_eq!(c.completed, 2);
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn empty_todos_message() {
        assert_eq!(
            empty_placeholder_message(true, TodoCounts::default()),
            "No todo items."
        );
    }

    #[test]
    fn list_entry_shows_meta_kind_badge() {
        let style = TodoPaneStyle::default();
        let item = TodoItem {
            content: "Wire the API".into(),
            priority: TodoPriority::default(),
            status: TodoStatus::Pending,
            meta: Some(serde_json::json!({"kind": "phase", "namespace": "impl"})),
            size: None,
        };
        let entry = TodoListEntry::new(0, item, &style);
        let text = line_text(entry.content());
        assert!(
            text.starts_with("[phase] "),
            "expected kind badge prefix, got {text:?}"
        );
        assert!(text.contains("Wire the API"));
    }

    /// Kind tags (`[work]`, …) are muted chrome: pure blue, not cyan system/info
    /// (`gray_dim` / `accent_system`), not human green, not agent magenta.
    #[test]
    fn list_entry_kind_badge_is_blue_muted_not_cyan() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = crate::theme::Theme::current();
        let style = TodoPaneStyle::default();
        let item = TodoItem {
            content: "Implement the fix".into(),
            priority: TodoPriority::default(),
            status: TodoStatus::Pending,
            meta: Some(serde_json::json!({"kind": "work"})),
            size: None,
        };
        let entry = TodoListEntry::new(0, item, &style);
        let spans = &entry.content().spans;
        assert!(
            !spans.is_empty() && spans[0].content.as_ref().starts_with("[work]"),
            "expected [work] kind span, got {:?}",
            spans.iter().map(|s| s.content.as_ref()).collect::<Vec<_>>()
        );
        let fg = spans[0].style.fg;
        let pure_blue = Color::Rgb(0, 0, 255);
        let cyan = Color::Rgb(0, 255, 255);
        let green = Color::Rgb(0, 255, 0);
        let magenta = Color::Rgb(255, 0, 255);
        assert_eq!(
            fg,
            Some(pure_blue),
            "kind badge must be pure blue muted chrome, got {fg:?}"
        );
        // Explicit non-goals: cyan system/info, human green, agent magenta.
        assert_ne!(fg, Some(cyan), "kind badge must not be cyan system/info");
        assert_ne!(
            fg,
            Some(theme.gray_dim),
            "must not use gray_dim (cyan on DOGE)"
        );
        assert_ne!(
            fg,
            Some(theme.accent_system),
            "must not use accent_system (cyan on DOGE)"
        );
        assert_ne!(fg, Some(green), "kind badge must not be human green");
        assert_ne!(fg, Some(magenta), "kind badge must not be agent magenta");
        // Title text stays on the status text style (not the kind blue).
        assert_eq!(
            spans.get(1).and_then(|s| s.style.fg),
            Some(theme.text_primary),
            "content span must keep status text colour"
        );
    }

    #[test]
    fn list_entry_without_meta_kind_is_plain_content() {
        let style = TodoPaneStyle::default();
        let item = TodoItem {
            content: "Plain task".into(),
            priority: TodoPriority::default(),
            status: TodoStatus::Pending,
            meta: None,
            size: None,
        };
        let entry = TodoListEntry::new(0, item, &style);
        assert_eq!(line_text(entry.content()), "Plain task");
    }

    #[test]
    fn all_completed_is_all_done() {
        assert_eq!(empty_placeholder_message(false, counts(3, 0)), "All done.");
    }

    #[test]
    fn mixed_done_and_cancelled_summarizes_counts() {
        assert_eq!(
            empty_placeholder_message(false, counts(5, 1)),
            "5 done. 1 cancelled."
        );
    }

    #[test]
    fn only_cancelled() {
        assert_eq!(
            empty_placeholder_message(false, counts(0, 2)),
            "2 cancelled."
        );
    }

    fn sample_item(content: &str) -> TodoItem {
        TodoItem {
            content: content.into(),
            priority: TodoPriority::default(),
            status: TodoStatus::Pending,
            meta: None,
            size: None,
        }
    }

    #[test]
    fn first_plan_with_items_auto_opens_pane_once() {
        let mut pane = TodoPane::new();
        assert!(!pane.is_visible(), "starts hidden");
        assert_eq!(pane.counts().total(), 0);

        pane.update_todos(vec![sample_item("First ask")]);
        assert!(
            pane.is_visible(),
            "first 0→N Plan should auto-open the pane"
        );
        assert!(
            pane.badge_flash_active(),
            "badge should flash on first paint"
        );
        assert_eq!(pane.counts().total(), 1);

        // User closes; later Plan must not force-reopen.
        pane.overlay.hide();
        pane.update_todos(vec![sample_item("First ask"), sample_item("Second")]);
        assert!(
            !pane.is_visible(),
            "subsequent updates must respect user hide after first auto-open"
        );
        assert_eq!(pane.counts().total(), 2);
    }

    #[test]
    fn empty_plan_does_not_auto_open() {
        let mut pane = TodoPane::new();
        pane.update_todos(vec![]);
        assert!(!pane.is_visible());
        assert_eq!(pane.counts().total(), 0);
    }
}
