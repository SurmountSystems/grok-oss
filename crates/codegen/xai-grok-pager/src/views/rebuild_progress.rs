//! `/rebuild` progress strip: tracked bar + percent + human stage text.
//!
//! Pure helpers so unit tests can assert bar characters without a full TUI.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::progress_bar::progress_bar_tracked_spans;

/// Build the single status line painted during `/rebuild`.
///
/// Layout (left → right):
/// ` [████░░░░]  42%  Compiling xai-grok-pager (12 packages)`
///
/// When `width` is too narrow, the bar shrinks first, then the detail is
/// truncated. Always stays one row.
pub fn rebuild_progress_line(
    width: u16,
    fraction: f32,
    detail: &str,
    bar_fg: Color,
    track_fg: Color,
    bg: Color,
    text_fg: Color,
) -> Line<'static> {
    let fraction = xai_grok_update::clamp_rebuild_fraction(fraction);
    let pct = (fraction * 100.0).round() as u32;
    let pct_text = format!("  {pct:>3}%  ");
    let label = "Rebuild ";
    // Reserve: label + pct + at least a tiny bar + space for detail.
    let fixed = (label.chars().count() + pct_text.chars().count()) as u16;
    let bar_width = width
        .saturating_sub(fixed)
        .saturating_sub(12) // room for a short detail
        .clamp(6, 28);
    let after_bar = width.saturating_sub(fixed.saturating_add(bar_width));

    let mut spans = Vec::new();
    spans.push(Span::styled(
        label.to_string(),
        Style::default()
            .fg(text_fg)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    ));
    spans.extend(progress_bar_tracked_spans(
        bar_width, fraction, bar_fg, track_fg, bg,
    ));
    spans.push(Span::styled(pct_text, Style::default().fg(text_fg).bg(bg)));

    let detail = detail.trim();
    if !detail.is_empty() && after_bar > 0 {
        let max = after_bar as usize;
        let shown = if detail.chars().count() > max {
            let take = max.saturating_sub(3);
            detail.chars().take(take).collect::<String>() + "..."
        } else {
            detail.to_string()
        };
        spans.push(Span::styled(shown, Style::default().fg(bar_fg).bg(bg)));
    }

    Line::from(spans)
}

/// Pure text form of the rebuild strip (no styles) for unit tests and CLI-like
/// snapshots.
pub fn rebuild_progress_plain(width: u16, fraction: f32, detail: &str) -> String {
    let line = rebuild_progress_line(
        width,
        fraction,
        detail,
        Color::Magenta,
        Color::DarkGray,
        Color::Black,
        Color::White,
    );
    line.spans
        .into_iter()
        .map(|s| s.content.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_progress_plain_paints_bar_percent_and_stage() {
        let text = rebuild_progress_plain(48, 0.42, "Compiling xai-grok-pager (12 packages)");
        assert!(
            text.contains('█') || text.contains('░'),
            "must paint bar glyphs: {text}"
        );
        assert!(text.contains('%'), "must show percent: {text}");
        assert!(
            text.contains("Compiling") || text.contains("xai-grok-pager"),
            "must show stage: {text}"
        );
        assert!(text.contains("Rebuild"), "must label the strip: {text}");
        // 42% of progress
        assert!(text.contains("42%"), "expected 42% in {text}");
    }

    #[test]
    fn rebuild_progress_empty_and_full_bounds() {
        let empty = rebuild_progress_plain(40, 0.0, "Starting rebuild");
        assert!(empty.contains("0%"), "{empty}");
        assert!(empty.contains('░'), "empty track: {empty}");

        let full = rebuild_progress_plain(40, 1.0, "Rebuild complete");
        assert!(full.contains("100%"), "{full}");
        assert!(full.contains('█'), "full fill: {full}");
    }

    #[test]
    fn rebuild_progress_clamps_out_of_range_fraction() {
        let text = rebuild_progress_plain(32, 2.5, "overflow");
        assert!(text.contains("100%"), "{text}");
        let text = rebuild_progress_plain(32, -0.5, "underflow");
        assert!(text.contains("0%"), "{text}");
    }
}
