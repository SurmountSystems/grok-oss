//! UserPromptBlock - displays user input.

use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::render::wrapping::{RtOptions, word_wrap_line_with_joiners};
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{
    AccentStyle, BlockBackground, BlockContext, BlockLine, BlockOutput, DisplayMode, Selectable,
};

const USER_PROMPT_BODY_RANGE: u16 = 0;
/// Max visible lines when a user prompt is collapsed.
const COLLAPSED_MAX_LINES: usize = 3;
use crate::appearance::AppearanceConfig;
use crate::theme::Theme;

/// Drop invalid token ranges (replay meta is untrusted): out of bounds, not
/// on char boundaries, or empty. Survivors are sorted; overlaps with an
/// earlier kept range are dropped so span slicing never goes backwards.
fn sanitize_token_ranges(text: &str, mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.retain(|r| {
        r.start < r.end
            && r.end <= text.len()
            && text.is_char_boundary(r.start)
            && text.is_char_boundary(r.end)
    });
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut out: Vec<Range<usize>> = Vec::new();
    for r in ranges {
        if out.last().is_some_and(|prev| r.start < prev.end) {
            continue;
        }
        out.push(r);
    }
    out
}

/// Split one logical line into token/body spans by intersecting the line's
/// byte span in the block text with the (sanitized) token ranges.
fn token_styled_line(
    line_text: &str,
    line_start: usize,
    ranges: &[Range<usize>],
    token_style: Style,
    body_style: Style,
) -> Line<'static> {
    let line_end = line_start + line_text.len();
    let mut spans = Vec::new();
    let mut pos = line_start;
    for r in ranges {
        let start = r.start.clamp(line_start, line_end);
        let end = r.end.clamp(line_start, line_end);
        if start >= end {
            continue;
        }
        if start > pos {
            spans.push(Span::styled(
                line_text[pos - line_start..start - line_start].to_string(),
                body_style,
            ));
        }
        spans.push(Span::styled(
            line_text[start - line_start..end - line_start].to_string(),
            token_style,
        ));
        pos = end;
    }
    if pos < line_end {
        spans.push(Span::styled(
            line_text[pos - line_start..].to_string(),
            body_style,
        ));
    }
    Line::from(spans)
}

/// Block displaying a user's prompt.
#[derive(Debug, Clone)]
pub struct UserPromptBlock {
    /// The user's input text.
    pub text: String,
    /// Whether this was a bash command (! prefix).
    pub is_bash: bool,
    /// Whether this prompt was injected by the scheduler (cron/loop).
    pub is_cron: bool,
    /// Mid-turn interjection. Renders identically to a typed prompt but is
    /// excluded from shell prompt-index bookkeeping — the shell numbers only
    /// turn-starting prompts, so counting interjections would skew the
    /// positional prompt↔entry mapping used by rewind.
    pub is_interjection: bool,
    pub prompt_index: Option<usize>,
    /// Sanitized byte ranges into `text` rendered in the skill accent color
    /// (recognized `/command` tokens). Empty = plain prompt styling. This is
    /// the sole skill signal — a leading skill invocation is `[0..token_end]`.
    pub skill_token_ranges: Vec<Range<usize>>,
}

impl UserPromptBlock {
    /// Create a new user prompt block.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_bash: false,
            is_cron: false,
            is_interjection: false,
            prompt_index: None,
            skill_token_ranges: Vec::new(),
        }
    }

    /// Get the copyable text for this block.
    pub fn copy_text(&self) -> String {
        self.text.clone()
    }

    /// Create a new bash command prompt block.
    pub fn bash(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_bash: true,
            is_cron: false,
            is_interjection: false,
            prompt_index: None,
            skill_token_ranges: Vec::new(),
        }
    }

    /// Create a new skill invocation prompt block. The leading token on the
    /// first line (up to whitespace, or the whole line) gets the skill accent.
    pub fn skill(text: impl Into<String>) -> Self {
        let text = text.into();
        let first_line = text.lines().next().unwrap_or("");
        let token_end = first_line
            .find(char::is_whitespace)
            .unwrap_or(first_line.len());
        #[allow(clippy::single_range_in_vec_init)] // field is multi-range capable
        let skill_token_ranges = if token_end > 0 {
            vec![0..token_end]
        } else {
            Vec::new()
        };
        Self {
            text,
            is_bash: false,
            is_cron: false,
            is_interjection: false,
            prompt_index: None,
            skill_token_ranges,
        }
    }

    /// Create a plain user prompt with recognized mid-text slash tokens
    /// styled in the skill accent. Ranges are sanitized here — replay meta
    /// is untrusted, so invalid ranges are dropped rather than panicking.
    pub fn with_skill_tokens(text: impl Into<String>, ranges: Vec<Range<usize>>) -> Self {
        let text = text.into();
        let skill_token_ranges = sanitize_token_ranges(&text, ranges);
        Self {
            text,
            is_bash: false,
            is_cron: false,
            is_interjection: false,
            prompt_index: None,
            skill_token_ranges,
        }
    }

    /// Create a scheduled (cron) prompt block.
    pub fn cron(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_bash: false,
            is_cron: true,
            is_interjection: false,
            prompt_index: None,
            skill_token_ranges: Vec::new(),
        }
    }

    /// Create a mid-turn interjection prompt block (standard prompt
    /// rendering; never receives a shell prompt index).
    pub fn interjection(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_bash: false,
            is_cron: false,
            is_interjection: true,
            prompt_index: None,
            skill_token_ranges: Vec::new(),
        }
    }

    /// Elevated band behind user-prompt rows so turns are scannable in a long
    /// transcript. Pure band selection (no process-global reads) — unit-tested
    /// without toggling `terminal_native_lock`, which races concurrent
    /// `Theme::current()` tests that do not hold the theme test mutex.
    ///
    /// - Terminal-native / minimal: ANSI bright black as **background** — dark
    ///   profiles elevate off black, light profiles darken off white (bright-black
    ///   / `dark-ansi` slot). Semantic line bg survives
    ///   `flat_background` (unlike block-level `bg_light`, which minimal strips).
    ///   Selected prompts step up to silver (`Gray`) for a clearer focus cue.
    /// - RGB themes: `bg_light` (matches the fullscreen prompt band).
    fn prompt_band_color_for(
        theme: &Theme,
        is_selected: bool,
        terminal_native: bool,
    ) -> Option<ratatui::style::Color> {
        use ratatui::style::Color;
        if terminal_native {
            Some(if is_selected {
                Color::Gray
            } else {
                Color::DarkGray
            })
        } else {
            match theme.bg_light {
                Color::Reset => None,
                c => Some(c),
            }
        }
    }

    /// Prefix/body/skill styles. Bold only when `terminal_native` (minimal mode).
    /// Cyan pointer when `accent_user` is Reset so OSC 12 leaves the host cursor alone.
    fn prompt_styles(theme: &Theme, terminal_native: bool) -> (Style, Style, Style) {
        let prefix_color = match theme.accent_user {
            ratatui::style::Color::Reset => ratatui::style::Color::Cyan,
            c => c,
        };
        let mut prefix_style = theme.fg(prefix_color);
        let mut text_style = theme.fg(theme.text_primary);
        let mut skill_style = theme.fg(theme.accent_skill);
        if terminal_native {
            prefix_style = prefix_style.add_modifier(Modifier::BOLD);
            text_style = text_style.add_modifier(Modifier::BOLD);
            skill_style = skill_style.add_modifier(Modifier::BOLD);
        }
        (prefix_style, text_style, skill_style)
    }

    /// Wrap and style the prompt text, returning visual lines. When
    /// `max_lines` is set and content exceeds it, the last line is
    /// truncated with a " …" ellipsis.
    fn wrap_prompt_lines(
        &self,
        width: u16,
        max_lines: Option<usize>,
        show_prefix: bool,
        is_selected: bool,
    ) -> Vec<BlockLine> {
        let theme = Theme::current();
        // Minimal mode engages this lock; read it here instead of app state.
        let terminal_native = crate::theme::cache::terminal_native_locked();
        let (prefix_style, text_style, skill_style) = Self::prompt_styles(&theme, terminal_native);
        let band = Self::prompt_band_color_for(&theme, is_selected, terminal_native);
        // Semantic line bg (not a "panel") so it survives minimal's flat_background.
        let with_band = |line: BlockLine| -> BlockLine {
            match band {
                Some(c) => line.with_background(c),
                None => line,
            }
        };

        let prefix = if !show_prefix {
            ""
        } else if self.is_bash {
            "$ "
        } else if self.is_cron {
            "\u{21BB}  "
        } else {
            crate::glyphs::prompt_arrow()
        };
        let prefix_width = prefix.width();
        let has_visible_prefix = prefix_width > 0;
        let ellipsis = " \u{2026}";
        let ellipsis_width = 2;

        // Available width for text content (after prefix/indent)
        let base_content_width = (width as usize).saturating_sub(prefix_width).max(1);

        let mut all_lines: Vec<BlockLine> = Vec::new();
        let logical_lines: Vec<&str> = self.text.lines().collect();
        let total_logical = logical_lines.len();

        for (logical_idx, line_text) in logical_lines.iter().enumerate() {
            if line_text.is_empty() {
                // Empty line - just show prefix/indent
                let indent = " ".repeat(prefix_width);
                let line = if logical_idx == 0 {
                    Line::from(vec![Span::styled(prefix.to_string(), prefix_style)])
                } else {
                    Line::from(vec![Span::styled(indent, prefix_style)])
                };
                let block_line = with_band(BlockLine {
                    selectable: if has_visible_prefix {
                        Selectable::Spans(1..1) // empty content range past prefix
                    } else {
                        Selectable::All
                    },
                    selection_range: Some(USER_PROMPT_BODY_RANGE),
                    content: line,
                    ..Default::default()
                });
                all_lines.push(block_line);

                // Check if we hit max_lines
                if let Some(max) = max_lines
                    && all_lines.len() >= max
                {
                    // There's more content if we haven't processed all logical lines
                    let has_more = logical_idx + 1 < total_logical;
                    if has_more {
                        // Add ellipsis to last line
                        if let Some(last) = all_lines.last_mut() {
                            last.content
                                .spans
                                .push(Span::styled(ellipsis.to_string(), text_style));
                        }
                    }
                    return all_lines;
                }
                continue;
            }

            // Wrap this line's content.
            let content_line = if self.skill_token_ranges.is_empty() {
                Line::from(Span::styled(line_text.to_string(), text_style))
            } else {
                // `lines()` strips terminators, so recover this line's byte
                // offset from the subslice pointer into `self.text`.
                debug_assert!(
                    self.text
                        .as_bytes()
                        .as_ptr_range()
                        .contains(&line_text.as_ptr()),
                    "line_text must be a subslice of self.text"
                );
                let line_start = line_text.as_ptr() as usize - self.text.as_ptr() as usize;
                token_styled_line(
                    line_text,
                    line_start,
                    &self.skill_token_ranges,
                    skill_style,
                    text_style,
                )
            };
            let (wrapped, wrap_joiners) =
                word_wrap_line_with_joiners(&content_line, RtOptions::new(base_content_width));
            let wrapped_count = wrapped.len();

            for (wrap_idx, (wrapped_line, wrap_joiner)) in
                wrapped.into_iter().zip(wrap_joiners).enumerate()
            {
                let is_first_line = logical_idx == 0 && wrap_idx == 0;
                let indent: String = " ".repeat(prefix_width);
                let line_prefix = if is_first_line { prefix } else { &indent };

                // Check if this will be the last allowed line
                let will_be_last = max_lines.is_some_and(|max| all_lines.len() + 1 == max);

                // Check if there's more content after this line
                let has_more_wrapped = wrap_idx + 1 < wrapped_count;
                let has_more_logical = logical_idx + 1 < total_logical;
                let has_more = has_more_wrapped || has_more_logical;

                // First line of block or new logical line: hard break (None).
                // Continuation within same logical line: use wrap joiner.
                let joiner = if is_first_line || wrap_idx == 0 {
                    None
                } else {
                    wrap_joiner
                };

                if will_be_last && has_more {
                    // This is the last allowed line and there's more content.
                    // Re-wrap the current line's content with reduced width to
                    // make room for the ellipsis. Re-wrapping the styled line
                    // (not flattened text) keeps token spans teal here.
                    let reduced_width = base_content_width.saturating_sub(ellipsis_width);
                    let (re_wrapped_lines, _) =
                        word_wrap_line_with_joiners(&wrapped_line, RtOptions::new(reduced_width));

                    let final_content = re_wrapped_lines
                        .into_iter()
                        .next()
                        .unwrap_or_else(Line::default);

                    // Build final line with prefix, content, and ellipsis
                    let mut spans = vec![Span::styled(line_prefix.to_string(), prefix_style)];
                    spans.extend(final_content.spans.into_iter().map(|s| Span {
                        content: s.content.to_string().into(),
                        style: s.style,
                    }));
                    spans.push(Span::styled(ellipsis.to_string(), text_style));

                    let content_start = if has_visible_prefix { 1 } else { 0 };
                    let content_end = spans.len() - 1; // exclude ellipsis
                    let block_line = with_band(BlockLine {
                        content: Line::from(spans),
                        selectable: if content_start < content_end {
                            Selectable::Spans(content_start..content_end)
                        } else {
                            Selectable::Spans(content_start..content_start)
                        },
                        selection_range: Some(USER_PROMPT_BODY_RANGE),
                        joiner,
                        ..Default::default()
                    });
                    all_lines.push(block_line);
                    return all_lines;
                }

                // Normal line (not truncated)
                let mut spans = vec![Span::styled(line_prefix.to_string(), prefix_style)];
                spans.extend(wrapped_line.spans.into_iter().map(|s| Span {
                    content: s.content.to_string().into(),
                    style: s.style,
                }));
                let content_end = spans.len();
                let block_line = with_band(BlockLine {
                    content: Line::from(spans),
                    selectable: if has_visible_prefix {
                        Selectable::Spans(1..content_end)
                    } else {
                        Selectable::All
                    },
                    selection_range: Some(USER_PROMPT_BODY_RANGE),
                    joiner,
                    ..Default::default()
                });
                all_lines.push(block_line);

                // Check if we hit max_lines (for edge case where no more content)
                if let Some(max) = max_lines
                    && all_lines.len() >= max
                {
                    return all_lines;
                }
            }
        }

        // Handle empty text
        if all_lines.is_empty() {
            all_lines.push(with_band(BlockLine {
                content: Line::from(Span::styled(prefix.trim().to_string(), prefix_style)),
                selectable: Selectable::All,
                selection_range: Some(USER_PROMPT_BODY_RANGE),
                ..Default::default()
            }));
        }

        all_lines
    }
}

impl BlockContent for UserPromptBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        let max_lines = match ctx.mode {
            DisplayMode::Expanded => None,
            DisplayMode::Collapsed | DisplayMode::Truncated => Some(COLLAPSED_MAX_LINES),
        };

        let prompt_cfg = &ctx.appearance.scrollback.blocks.prompt;
        let compact = ctx.appearance.prompt.compact;
        let mut lines = self.wrap_prompt_lines(
            ctx.width,
            max_lines,
            prompt_cfg.show_prefix && !compact,
            ctx.is_selected,
        );
        super::append_bubble_copy_button(&mut lines, ctx);

        BlockOutput { lines }
    }

    fn accent(&self, _ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        // Match prompt pointer: Reset (terminal-native / NO_COLOR) → Cyan so
        // the rail stays visible without inventing a second token path.
        let color = match theme.accent_user {
            ratatui::style::Color::Reset => ratatui::style::Color::Cyan,
            c => c,
        };
        Some(AccentStyle::static_color(color))
    }

    fn accent_background(&self, _ctx: &BlockContext) -> bool {
        true // fill accent column with block bg so it matches content
    }

    fn background(&self, ctx: &BlockContext) -> BlockBackground {
        ctx.appearance.scrollback.blocks.prompt.bg
    }

    fn has_vpad_for(&self, appearance: &AppearanceConfig) -> bool {
        appearance.scrollback.blocks.prompt.vpad && !appearance.prompt.compact
    }

    fn has_raw_mode(&self) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        // Estimate visual line count to catch long single-line prompts that
        // wrap past the limit. Uses a conservative content width (terminal
        // width minus prefix/padding); at wider terminals we may slightly
        // over-report foldability, which is harmless.
        const MIN_CONTENT_WIDTH: usize = 60;
        let mut visual_lines = 0usize;
        for line in self.text.lines() {
            let w = line.width();
            visual_lines += if w == 0 {
                1
            } else {
                w.div_ceil(MIN_CONTENT_WIDTH)
            };
            if visual_lines > COLLAPSED_MAX_LINES {
                return true;
            }
        }
        false
    }

    fn default_display_mode(&self) -> DisplayMode {
        if self.is_foldable() {
            DisplayMode::Collapsed
        } else {
            DisplayMode::Expanded
        }
    }

    fn next_fold_mode(&self, current: DisplayMode, _is_running: bool) -> DisplayMode {
        match current {
            DisplayMode::Collapsed | DisplayMode::Truncated => DisplayMode::Expanded,
            DisplayMode::Expanded => DisplayMode::Collapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to get line text content (excluding styles)
    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn bubble_ctx(bubble_copy_buttons: bool) -> BlockContext {
        let mut appearance = AppearanceConfig::default();
        appearance.scrollback.display.bubble_copy_buttons = bubble_copy_buttons;
        BlockContext {
            mode: DisplayMode::Expanded,
            is_running: false,
            width: 80,
            raw: false,
            max_lines: None,
            appearance,
            is_selected: false,
            cwd: None,
        }
    }

    fn output_text(block: &UserPromptBlock, ctx: &BlockContext) -> String {
        block
            .output(ctx)
            .lines
            .iter()
            .flat_map(|l| l.content.spans.iter().map(|s| s.content.as_ref()))
            .collect()
    }

    fn paints_copy_icon(line: &BlockLine) -> bool {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        let Some(col) = line.copy_button_col else {
            return false;
        };
        let width = col.saturating_add(4).max(8);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
        line.paint_bubble_copy_button(&mut buf, 0, 0, Style::default());
        buf.cell((col, 0))
            .is_some_and(|c| c.symbol() == crate::glyphs::copy_icon())
    }

    #[test]
    fn bubble_copy_buttons_on_paints_copy_icon() {
        let block = UserPromptBlock::new("hello");
        let out = block.output(&bubble_ctx(true));
        assert!(
            out.lines.iter().any(|line| line.copy_button_col.is_some()),
            "bubble_copy_buttons on must mark a bubble-copy hit column on the user bubble"
        );
        let text = output_text(&block, &bubble_ctx(true));
        assert!(
            !text.contains(crate::glyphs::copy_icon()),
            "the copy icon is paint, not wrap content: {text:?}"
        );
        assert!(
            out.lines.iter().any(paints_copy_icon),
            "bubble_copy_buttons on must paint the copy icon on the user bubble"
        );
    }

    #[test]
    fn bubble_copy_buttons_off_omits_copy_icon() {
        let block = UserPromptBlock::new("hello");
        let out = block.output(&bubble_ctx(false));
        let text = output_text(&block, &bubble_ctx(false));
        assert!(
            !text.contains(crate::glyphs::copy_icon()),
            "bubble_copy_buttons off must not paint the copy icon: {text:?}"
        );
        assert!(
            out.lines.iter().all(|line| line.copy_button_col.is_none()),
            "bubble_copy_buttons off must not mark a bubble-copy hit column"
        );
    }

    /// A first line that already fills the content width must still paint
    /// the always-on copy glyph and mark a hit column. Omitting the icon
    /// when tight is not product behavior.
    #[test]
    fn bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width() {
        // One unbreakable word so wrap fills the first line. Hyphens would
        // wrap early and leave room for the icon (a false green).
        let block = UserPromptBlock::new(format!("WIDEFIRSTLINECOPYCONTRACT{}", "W".repeat(200)));
        let ctx = bubble_ctx(true);
        let wrapped = block.wrap_prompt_lines(ctx.width, None, true, false);
        let first_used: usize = wrapped
            .first()
            .map(|line| line.content.spans.iter().map(|s| s.content.width()).sum())
            .unwrap_or(0);
        assert!(
            first_used + 1 + crate::glyphs::copy_icon().width() > ctx.content_width(),
            "precondition: first line must be too wide for space plus icon under the old omit rule (used={first_used}, content_width={})",
            ctx.content_width()
        );
        let out = block.output(&ctx);
        assert_eq!(
            out.lines.len(),
            wrapped.len(),
            "bubble copy must not insert a chrome line into output().lines"
        );
        let after: usize = out
            .lines
            .first()
            .map(|line| line.content.spans.iter().map(|s| s.content.width()).sum())
            .unwrap_or(0);
        assert_eq!(
            after, first_used,
            "bubble copy must not change first-line wrap width"
        );
        assert!(
            out.lines.iter().any(|line| line.copy_button_col.is_some()),
            "a full-width first line must still mark a bubble-copy hit column"
        );
        assert!(
            out.lines.iter().all(|line| {
                line.content
                    .spans
                    .iter()
                    .all(|s| s.content.as_ref() != crate::glyphs::copy_icon())
            }),
            "the copy icon is paint, not wrap content"
        );
        assert!(
            out.lines.iter().any(paints_copy_icon),
            "a full-width first line must still paint the copy icon"
        );
    }

    #[test]
    fn test_short_prompt_no_truncation() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        let expected = format!("{}hello", crate::glyphs::prompt_arrow());

        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0].content), expected);
    }

    #[test]
    fn test_short_prompt_with_max_lines() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, Some(2), true, false);
        let expected = format!("{}hello", crate::glyphs::prompt_arrow());

        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0].content), expected);
        // No ellipsis because content fits
        assert!(!line_text(&lines[0].content).contains('\u{2026}'));
    }

    #[test]
    fn test_long_prompt_wraps() {
        let block = UserPromptBlock::new("this is a very long prompt that should wrap");
        let lines = block.wrap_prompt_lines(20, None, true, false);

        assert!(lines.len() > 1, "Should wrap to multiple lines");
        assert!(line_text(&lines[0].content).starts_with(crate::glyphs::prompt_arrow()));
        // Continuation lines have 2-space indent
        assert!(line_text(&lines[1].content).starts_with("  "));
    }

    #[test]
    fn test_truncation_adds_ellipsis() {
        let block =
            UserPromptBlock::new("this is a very long prompt that should wrap to many lines");
        let lines = block.wrap_prompt_lines(20, Some(2), true, false);

        assert_eq!(lines.len(), 2);
        // Last line should have ellipsis
        let last = line_text(&lines[1].content);
        assert!(
            last.ends_with(" \u{2026}"),
            "Last line should end with ellipsis: {:?}",
            last
        );
    }

    #[test]
    fn test_ellipsis_fits_within_width() {
        // Create a prompt that wraps to exactly fill lines
        let block = UserPromptBlock::new("aaaa bbbb cccc dddd eeee ffff");
        let width = 15; // Narrow width to force wrapping
        let lines = block.wrap_prompt_lines(width, Some(2), true, false);

        assert_eq!(lines.len(), 2);

        // Each line (including prefix and ellipsis) should fit within width
        for line in &lines {
            let text = line_text(&line.content);
            let char_count = text.chars().count();
            assert!(
                char_count <= width as usize,
                "Line exceeds width {}: {:?} ({})",
                width,
                text,
                char_count
            );
        }

        // Last line should have ellipsis
        assert!(line_text(&lines[1].content).ends_with(" \u{2026}"));
    }

    #[test]
    fn test_bash_prompt_prefix() {
        let block = UserPromptBlock::bash("ls -la");
        let lines = block.wrap_prompt_lines(80, None, true, false);

        assert_eq!(lines.len(), 1);
        assert!(line_text(&lines[0].content).starts_with("$ "));
    }

    #[test]
    fn skill_with_args_only_command_is_teal() {
        let block = UserPromptBlock::skill("/pr-workflow create a ticket for this");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);

        let theme = Theme::current();
        let spans = &lines[0].content.spans;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content.as_ref(), "/pr-workflow");
        assert_eq!(spans[1].style.fg, Some(theme.accent_skill));
        assert_eq!(spans[2].content.as_ref(), " create a ticket for this");
        assert_eq!(spans[2].style.fg, Some(theme.text_primary));
    }

    #[test]
    fn skill_without_args_all_teal() {
        let block = UserPromptBlock::skill("/pr-workflow");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);

        let theme = Theme::current();
        let spans = &lines[0].content.spans;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].content.as_ref(), "/pr-workflow");
        assert_eq!(spans[1].style.fg, Some(theme.accent_skill));
    }

    #[test]
    fn skill_multiline_only_first_token_teal() {
        let block = UserPromptBlock::skill("/foo bar\nbaz");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 2);

        let theme = Theme::current();
        let line0 = &lines[0].content.spans;
        assert_eq!(line0[1].content.as_ref(), "/foo");
        assert_eq!(line0[1].style.fg, Some(theme.accent_skill));
        assert_eq!(line0[2].content.as_ref(), " bar");
        assert_eq!(line0[2].style.fg, Some(theme.text_primary));

        let line1 = &lines[1].content.spans;
        assert_eq!(line1[1].content.as_ref(), "baz");
        assert_eq!(line1[1].style.fg, Some(theme.text_primary));
    }

    // --- Mid-text skill token styling (with_skill_tokens) ---

    #[test]
    fn mid_text_token_only_token_is_teal() {
        let text = "great /pr-workflow all good now";
        let block = UserPromptBlock::with_skill_tokens(text, vec![6..18]);
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);

        let theme = Theme::current();
        let spans = &lines[0].content.spans;
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[1].content.as_ref(), "great ");
        assert_eq!(spans[1].style.fg, Some(theme.text_primary));
        assert_eq!(spans[2].content.as_ref(), "/pr-workflow");
        assert_eq!(spans[2].style.fg, Some(theme.accent_skill));
        assert_eq!(spans[3].content.as_ref(), " all good now");
        assert_eq!(spans[3].style.fg, Some(theme.text_primary));
    }

    #[test]
    fn mid_text_multiple_tokens_each_teal() {
        let _pin = crate::theme::cache::pin_theme();
        let text = "run /commit then /review please";
        let block = UserPromptBlock::with_skill_tokens(text, vec![4..11, 17..24]);
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);

        let theme = Theme::current();
        let teal: Vec<&str> = lines[0]
            .content
            .spans
            .iter()
            .filter(|s| s.style.fg == Some(theme.accent_skill))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(teal, vec!["/commit", "/review"]);
    }

    #[test]
    fn mid_text_token_on_second_logical_line() {
        let _pin = crate::theme::cache::pin_theme();
        let text = "first line\nthen /model here";
        // "/model" starts after "first line\nthen " = 16 bytes.
        let block = UserPromptBlock::with_skill_tokens(text, vec![16..22]);
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 2);

        let theme = Theme::current();
        let line0 = &lines[0].content.spans;
        assert!(
            line0.iter().all(|s| s.style.fg != Some(theme.accent_skill)),
            "line 0 has no token"
        );
        let line1 = &lines[1].content.spans;
        assert_eq!(line1[1].content.as_ref(), "then ");
        assert_eq!(line1[1].style.fg, Some(theme.text_primary));
        assert_eq!(line1[2].content.as_ref(), "/model");
        assert_eq!(line1[2].style.fg, Some(theme.accent_skill));
        assert_eq!(line1[3].content.as_ref(), " here");
    }

    #[test]
    fn invalid_token_ranges_are_dropped() {
        let _pin = crate::theme::cache::pin_theme();
        let text = "héllo /model now"; // 'é' is 2 bytes: "/model" = 7..13
        let block = UserPromptBlock::with_skill_tokens(
            text,
            vec![
                2..3,   // not a char boundary (inside 'é')
                40..50, // out of bounds
                9..9,   // empty
                7..13,  // valid token
                10..15, // overlaps the kept 7..13
            ],
        );
        assert_eq!(block.skill_token_ranges, vec![7..13]);

        let lines = block.wrap_prompt_lines(80, None, true, false);
        let theme = Theme::current();
        let teal: Vec<&str> = lines[0]
            .content
            .spans
            .iter()
            .filter(|s| s.style.fg == Some(theme.accent_skill))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(teal, vec!["/model"]);
    }

    #[test]
    fn all_token_ranges_invalid_renders_plain() {
        let block = UserPromptBlock::with_skill_tokens("plain text", vec![100..200]);
        assert!(block.skill_token_ranges.is_empty());
        let lines = block.wrap_prompt_lines(80, None, true, false);
        let theme = Theme::current();
        assert_eq!(lines[0].content.spans[1].style.fg, Some(theme.text_primary));
    }

    // --- Token styling across soft-wrap and collapsed truncation ---

    /// Concatenated content of a line's skill-accent spans.
    fn teal_text(line: &Line, theme: &Theme) -> String {
        line.spans
            .iter()
            .filter(|s| s.style.fg == Some(theme.accent_skill))
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn collapsed_truncation_keeps_teal_on_straddling_token() {
        let _pin = crate::theme::cache::pin_theme();
        // "/pr-workflow" (bytes 8..20) is wider than the content width, so it
        // straddles the last visible row and the hidden continuation; the
        // truncating re-wrap must keep the visible head teal.
        let text = "one\ntwo\n/pr-workflow tail";
        let block = UserPromptBlock::with_skill_tokens(text, vec![8..20]);
        let lines = block.wrap_prompt_lines(8, Some(3), false, false);
        assert_eq!(lines.len(), 3);

        let theme = Theme::current();
        let last = &lines[2].content;
        assert!(line_text(last).ends_with(" \u{2026}"));
        let teal = teal_text(last, &theme);
        assert!(
            !teal.is_empty() && "/pr-workflow".starts_with(&teal),
            "visible head of the straddling token must stay teal, got {teal:?}"
        );
    }

    #[test]
    fn collapsed_truncation_keeps_teal_on_token_within_last_line() {
        let _pin = crate::theme::cache::pin_theme();
        // "/do-it" (bytes 8..14) fits fully on the truncated last line even at
        // the ellipsis-reduced width, so it must survive whole and teal.
        let text = "one\ntwo\n/do-it more words here";
        let block = UserPromptBlock::with_skill_tokens(text, vec![8..14]);
        let lines = block.wrap_prompt_lines(20, Some(3), false, false);
        assert_eq!(lines.len(), 3);

        let theme = Theme::current();
        let last = &lines[2].content;
        assert!(line_text(last).ends_with(" \u{2026}"));
        assert_eq!(teal_text(last, &theme), "/do-it");
        let body: String = last
            .spans
            .iter()
            .filter(|s| s.style.fg == Some(theme.text_primary))
            .map(|s| s.content.as_ref())
            .collect();
        assert!(body.contains("more"), "args stay body-styled, got {body:?}");
    }

    #[test]
    fn narrow_wrap_keeps_teal_on_both_rows_of_split_token() {
        let _pin = crate::theme::cache::pin_theme();
        // Expanded (no max_lines): the 12-wide token cannot fit at width 8, so
        // the wrapper splits it mid-token; every piece must stay teal.
        let text = "aa /pr-workflow zz";
        let block = UserPromptBlock::with_skill_tokens(text, vec![3..15]);
        let lines = block.wrap_prompt_lines(8, None, false, false);
        assert!(lines.len() >= 2);

        let theme = Theme::current();
        let teal_by_line: Vec<String> = lines
            .iter()
            .map(|l| teal_text(&l.content, &theme))
            .collect();
        let lines_with_teal = teal_by_line.iter().filter(|t| !t.is_empty()).count();
        assert!(
            lines_with_teal >= 2,
            "split token must stay teal on every row: {teal_by_line:?}"
        );
        assert_eq!(teal_by_line.concat(), "/pr-workflow");
    }

    #[test]
    fn test_multiline_input() {
        let block = UserPromptBlock::new("line one\nline two\nline three");
        let lines = block.wrap_prompt_lines(80, None, true, false);

        assert_eq!(lines.len(), 3);
        assert!(line_text(&lines[0].content).starts_with(crate::glyphs::prompt_arrow()));
        assert!(line_text(&lines[1].content).starts_with("  ")); // continuation indent
        assert!(line_text(&lines[2].content).starts_with("  "));
    }

    #[test]
    fn test_multiline_truncated() {
        let block = UserPromptBlock::new("line one\nline two\nline three");
        let lines = block.wrap_prompt_lines(80, Some(2), true, false);

        assert_eq!(lines.len(), 2);
        // Last line should have ellipsis since there's more content
        assert!(line_text(&lines[1].content).ends_with(" \u{2026}"));
    }

    #[test]
    fn test_exact_fit_no_ellipsis() {
        // If content fits exactly in max_lines, no ellipsis needed
        let block = UserPromptBlock::new("short");
        let lines = block.wrap_prompt_lines(80, Some(1), true, false);

        assert_eq!(lines.len(), 1);
        assert!(!line_text(&lines[0].content).contains('\u{2026}'));
    }

    #[test]
    fn test_selected_prompt_uses_accent_color() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, true);
        let expected = format!("{}hello", crate::glyphs::prompt_arrow());

        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0].content), expected);

        // Prefix always uses accent (or Cyan when accent_user is Reset for
        // terminal-native / NO_COLOR), never dim gray. Bold is minimal-only.
        let theme = Theme::current();
        let prefix_span = &lines[0].content.spans[0];
        let expected_fg = match theme.accent_user {
            ratatui::style::Color::Reset => Some(ratatui::style::Color::Cyan),
            c => Some(c),
        };
        assert_eq!(prefix_span.style.fg, expected_fg);
        assert!(!prefix_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_unselected_prompt_still_uses_accent_pointer() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);

        assert_eq!(lines.len(), 1);

        // Unselected no longer collapses onto gray_dim — same accent pointer
        // so user turns stay scannable in a long transcript.
        let theme = Theme::current();
        let prefix_span = &lines[0].content.spans[0];
        let expected_fg = match theme.accent_user {
            ratatui::style::Color::Reset => Some(ratatui::style::Color::Cyan),
            c => Some(c),
        };
        assert_eq!(prefix_span.style.fg, expected_fg);
        // Fullscreen (default test env): accent pointer, not bold.
        assert!(!prefix_span.style.add_modifier.contains(Modifier::BOLD));
        // Not the old unselected path (dim gray), unless the whole palette is
        // Reset (NO_COLOR / native grays), in which case Cyan still wins above.
        if !matches!(theme.gray_dim, ratatui::style::Color::Reset) {
            assert_ne!(prefix_span.style.fg, Some(theme.gray_dim));
        }
    }

    // --- Selection metadata tests ---

    #[test]
    fn test_prompt_lines_have_selection_range() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert!(
            lines
                .iter()
                .all(|l| l.selection_range == Some(USER_PROMPT_BODY_RANGE))
        );
    }

    #[test]
    fn test_prompt_prefix_excluded_from_selection() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);
        // Prefix is span 0, content starts at span 1
        match &lines[0].selectable {
            Selectable::Spans(range) => {
                assert_eq!(range.start, 1);
            }
            _ => panic!("Expected Selectable::Spans"),
        }
    }

    #[test]
    fn test_prompt_no_prefix_all_selectable() {
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, false, false);
        assert_eq!(lines.len(), 1);
        assert!(matches!(lines[0].selectable, Selectable::All));
    }

    #[test]
    fn test_prompt_wrapped_lines_have_joiners() {
        let block = UserPromptBlock::new("this is a long prompt that should wrap");
        let lines = block.wrap_prompt_lines(15, None, true, false);
        assert!(lines.len() > 1);
        // First line: no joiner
        assert!(lines[0].joiner.is_none());
        // Continuation lines within same logical line: should have joiner
        assert!(lines.iter().skip(1).any(|l| l.joiner.is_some()));
    }

    #[test]
    fn test_prompt_multiline_joiners() {
        let block = UserPromptBlock::new("line one\nline two");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 2);
        // First line: no joiner
        assert!(lines[0].joiner.is_none());
        // Second line: also no joiner (hard break between logical lines)
        assert!(lines[1].joiner.is_none());
    }

    #[test]
    fn test_cron_prompt_prefix() {
        let block = UserPromptBlock::cron("/pr-babysit check");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);
        let text = line_text(&lines[0].content);
        assert!(
            text.starts_with("\u{21BB}  "),
            "Cron prompt should start with \u{21BB}, got: {text:?}"
        );
        assert!(text.contains("/pr-babysit check"));
    }

    #[test]
    fn test_bash_prefix_excluded_from_selection() {
        let block = UserPromptBlock::bash("ls -la");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 1);
        match &lines[0].selectable {
            Selectable::Spans(range) => {
                assert_eq!(range.start, 1);
            }
            _ => panic!("Expected Selectable::Spans"),
        }
    }

    #[test]
    fn test_continuation_indent_excluded_from_selection() {
        let block = UserPromptBlock::new("line one\nline two");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert_eq!(lines.len(), 2);
        // Both lines should exclude their prefix/indent
        for line in &lines {
            match &line.selectable {
                Selectable::Spans(range) => {
                    assert_eq!(range.start, 1);
                }
                _ => panic!("Expected Selectable::Spans"),
            }
        }
    }

    // --- Fold behavior tests ---

    #[test]
    fn test_short_prompt_not_foldable() {
        let block = UserPromptBlock::new("hello");
        assert!(!block.is_foldable());
        assert_eq!(block.default_display_mode(), DisplayMode::Expanded);
    }

    #[test]
    fn test_three_line_prompt_not_foldable() {
        let block = UserPromptBlock::new("one\ntwo\nthree");
        assert!(!block.is_foldable());
        assert_eq!(block.default_display_mode(), DisplayMode::Expanded);
    }

    #[test]
    fn test_four_line_prompt_is_foldable() {
        let block = UserPromptBlock::new("one\ntwo\nthree\nfour");
        assert!(block.is_foldable());
        assert_eq!(block.default_display_mode(), DisplayMode::Collapsed);
    }

    #[test]
    fn test_long_single_line_is_foldable() {
        // A single line long enough to wrap past 3 visual lines at 60-char width
        let long_line = "a ".repeat(120); // 240 chars → 4 visual lines at 60
        let block = UserPromptBlock::new(long_line);
        assert!(block.is_foldable());
        assert_eq!(block.default_display_mode(), DisplayMode::Collapsed);
    }

    #[test]
    fn test_short_single_line_not_foldable() {
        // A single line that fits in 3 visual lines at 60-char width
        let short_line = "a ".repeat(60); // 120 chars → 2 visual lines at 60
        let block = UserPromptBlock::new(short_line);
        assert!(!block.is_foldable());
    }

    #[test]
    fn test_fold_toggle_collapsed_to_expanded() {
        let block = UserPromptBlock::new("one\ntwo\nthree\nfour");
        assert_eq!(
            block.next_fold_mode(DisplayMode::Collapsed, false),
            DisplayMode::Expanded
        );
    }

    #[test]
    fn test_fold_toggle_expanded_to_collapsed() {
        let block = UserPromptBlock::new("one\ntwo\nthree\nfour");
        assert_eq!(
            block.next_fold_mode(DisplayMode::Expanded, false),
            DisplayMode::Collapsed
        );
    }

    #[test]
    fn test_fold_toggle_truncated_to_expanded() {
        let block = UserPromptBlock::new("one\ntwo\nthree\nfour");
        assert_eq!(
            block.next_fold_mode(DisplayMode::Truncated, false),
            DisplayMode::Expanded
        );
    }

    #[test]
    fn user_prompt_bold_only_in_minimal() {
        let theme = Theme::current();
        let (prefix, body, skill) = UserPromptBlock::prompt_styles(&theme, true);
        assert!(prefix.add_modifier.contains(Modifier::BOLD));
        assert!(body.add_modifier.contains(Modifier::BOLD));
        assert!(skill.add_modifier.contains(Modifier::BOLD));

        let (prefix, body, skill) = UserPromptBlock::prompt_styles(&theme, false);
        assert!(!prefix.add_modifier.contains(Modifier::BOLD));
        assert!(!body.add_modifier.contains(Modifier::BOLD));
        assert!(!skill.add_modifier.contains(Modifier::BOLD));

        // Default unit-test env is fullscreen (lock off).
        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        let spans = &lines[0].content.spans;
        assert!(!spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(!spans[1].style.add_modifier.contains(Modifier::BOLD));
    }

    /// Pure band logic (no global `terminal_native_lock` — that races other
    /// tests that call `Theme::current()` without the theme test mutex).
    #[test]
    fn prompt_band_color_native_vs_rgb() {
        use ratatui::style::Color;

        let theme = Theme::groknight();
        assert_eq!(
            UserPromptBlock::prompt_band_color_for(&theme, false, true),
            Some(Color::DarkGray)
        );
        assert_eq!(
            UserPromptBlock::prompt_band_color_for(&theme, true, true),
            Some(Color::Gray)
        );
        // RGB theme: band follows bg_light.
        assert_eq!(
            UserPromptBlock::prompt_band_color_for(&theme, false, false),
            Some(theme.bg_light)
        );
        // Terminal-native palette: bg_light is Reset → no RGB band when not
        // in native mode; native mode still uses the ANSI elevated slots.
        let native = Theme::terminal_default();
        assert_eq!(
            UserPromptBlock::prompt_band_color_for(&native, false, false),
            None
        );
        assert_eq!(
            UserPromptBlock::prompt_band_color_for(&native, false, true),
            Some(Color::DarkGray)
        );
    }

    /// Applied band is semantic (not a panel) so minimal `flat_background`
    /// keeps it. Does not toggle process-global native lock.
    #[test]
    fn user_prompt_band_is_semantic_not_panel() {
        let block = UserPromptBlock::new("scan me");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        assert!(
            !lines[0].background_is_panel,
            "band must be semantic so flat_background keeps it"
        );
        // Whatever Theme::current() is this moment, wrap used the same source
        // for band selection (same process, no lock toggle in this test).
        let theme = Theme::current();
        assert_eq!(
            lines[0].background,
            UserPromptBlock::prompt_band_color_for(
                &theme,
                false,
                crate::theme::cache::terminal_native_locked(),
            )
        );
    }

    fn accent_test_ctx() -> BlockContext {
        BlockContext {
            mode: DisplayMode::Expanded,
            is_running: false,
            width: 80,
            raw: false,
            max_lines: None,
            appearance: crate::appearance::AppearanceConfig::default(),
            is_selected: false,
            cwd: None,
        }
    }

    /// Human prompts always paint a static left rail (`┃` via EntryRenderer),
    /// same geometry as Recap. Colour is the Human token (`accent_user`).
    #[test]
    fn user_prompt_block_accent_is_static_human_rail() {
        use ratatui::style::Color;

        let ctx = accent_test_ctx();
        let theme = Theme::current();
        let expected = match theme.accent_user {
            Color::Reset => Color::Cyan,
            c => c,
        };
        let blocks: Vec<UserPromptBlock> = vec![
            UserPromptBlock::new("hello"),
            UserPromptBlock::bash("ls -la"),
            UserPromptBlock::skill("/pr-babysit check"),
            UserPromptBlock::cron("scheduled"),
            UserPromptBlock::interjection("mid-turn"),
            UserPromptBlock::with_skill_tokens("see /todo later", vec![4..9]),
        ];
        for block in blocks {
            let accent = block
                .accent(&ctx)
                .expect("Human prompts must return a left accent rail");
            assert!(
                !accent.animated,
                "Human rail is static (not wave-animated like running tools)"
            );
            assert_eq!(
                accent.color, expected,
                "rail must share the Human accent_user token (Reset→Cyan like pointer)"
            );
            assert!(
                !matches!(accent.color, Color::Gray | Color::DarkGray),
                "Human rail must not paint ANSI gray: {:?}",
                accent.color
            );
        }
    }

    /// DOGE theme table: Human token is pure green. Live `accent()` follows
    /// `Theme::current()` after pinning `ThemeKind::Doge` (not `pin_theme()`,
    /// which forces GrokNight). Under `NO_COLOR`, slots are Reset and the
    /// product falls back to Cyan (same as the pointer).
    #[test]
    fn user_prompt_block_accent_is_green_rail_under_doge_default() {
        use ratatui::style::Color;

        let doge = Theme::doge();
        assert_eq!(
            doge.accent_user,
            Color::Rgb(0, 255, 0),
            "DOGE accent_user must be pure green for Human chrome"
        );
        assert_eq!(doge.accent_success, Color::Rgb(0, 255, 0));
        assert_ne!(doge.accent_user, Color::Rgb(255, 255, 255));

        let _lock = crate::theme::cache::test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::theme::cache::reset_for_test();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);

        let accent = UserPromptBlock::new("hi")
            .accent(&accent_test_ctx())
            .expect("rail on");
        assert!(!accent.animated, "Human rail is static");
        // Not gray and not the old white Human chrome.
        assert!(
            !matches!(
                accent.color,
                Color::Gray | Color::DarkGray | Color::Rgb(255, 255, 255)
            ),
            "Human rail must not be gray/white: {:?}",
            accent.color
        );
        let live = Theme::current().accent_user;
        let expected = match live {
            Color::Reset => Color::Cyan,
            c => c,
        };
        assert_eq!(accent.color, expected);
        if live == Color::Rgb(0, 255, 0) {
            assert_eq!(accent.color, Color::Rgb(0, 255, 0));
        }
    }

    /// Prefix pointer and left rail share the same Human colour token.
    #[test]
    fn user_prompt_prefix_matches_human_rail_color() {
        use ratatui::style::Color;

        let block = UserPromptBlock::new("hello");
        let lines = block.wrap_prompt_lines(80, None, true, false);
        let prefix_fg = lines[0].content.spans[0].style.fg;
        let theme = Theme::current();
        let expected = match theme.accent_user {
            Color::Reset => Some(Color::Cyan),
            c => Some(c),
        };
        assert_eq!(prefix_fg, expected, "pointer uses Human accent_user");
        let rail = block.accent(&accent_test_ctx()).expect("rail on").color;
        assert_eq!(Some(rail), expected, "rail matches pointer Human colour");
    }

    /// Fullscreen paint: Human prompt left cell is `┃` in `accent_user`.
    /// `accent()` returning green is not enough if EntryRenderer never paints it.
    #[test]
    fn user_prompt_entry_renderer_paints_green_rail() {
        use crate::render::Renderable;
        use crate::scrollback::RenderBlock;
        use crate::scrollback::entry::ScrollbackEntry;
        use crate::scrollback::wrappers::EntryRenderer;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::Color;

        let _lock = crate::theme::cache::test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::theme::cache::reset_for_test();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);

        let theme = Theme::current();
        let expected = match theme.accent_user {
            Color::Reset => Color::Cyan,
            c => c,
        };
        let entry = ScrollbackEntry::new(RenderBlock::user_prompt("hello from the human"));
        let renderer = EntryRenderer::new(&entry, &theme);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        renderer.render(area, &mut buf);

        let bar = crate::glyphs::accent_bar();
        let rail = (0..area.height)
            .find_map(|y| {
                let c = buf.cell((0, y))?;
                (c.symbol() == bar).then_some(c.clone())
            })
            .expect("Human prompt must paint a left ┃ rail");
        assert_eq!(
            rail.fg, expected,
            "painted rail must be Human accent_user, got {:?}",
            rail.fg
        );
        assert_ne!(rail.fg, theme.accent_running);
        assert_ne!(rail.fg, Color::Rgb(255, 255, 255));
    }
}
