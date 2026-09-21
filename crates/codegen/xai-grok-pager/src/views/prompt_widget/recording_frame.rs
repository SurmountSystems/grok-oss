//! Grok OSS: While recording, the composer frame (the white line around the
//! prompt box) paints red. When not recording, it is not red.
//! Grok OSS: Recording red is `accent_error`, not `accent_running`.
//! Grok OSS: While recording, the prompt box grows with incoming transcript /
//! audio activity. It must not clip or cut off the spoken text.

use ratatui::style::Color;

use crate::theme::Theme;

/// True iff the composer is recording (the frame must paint red).
pub fn composer_frame_is_red(recording: bool) -> bool {
    recording
}

/// Recording frame color is `theme.accent_error`. Idle keeps `idle_white`.
pub fn composer_frame_color(recording: bool, theme: &Theme, idle_white: Color) -> Color {
    if recording {
        theme.accent_error
    } else {
        idle_white
    }
}

/// Rows needed to show committed prompt text plus live interim at `width`.
///
/// Does not ellipsize: the recording box grows instead of cutting spoken text.
pub fn wrapped_transcript_rows(committed: &str, interim: Option<&str>, width: u16) -> u16 {
    let mut text = String::new();
    let committed = committed.trim();
    if !committed.is_empty() {
        text.push_str(committed);
    }
    if let Some(interim) = interim.map(str::trim).filter(|t| !t.is_empty()) {
        if !text.is_empty() && !text.ends_with(char::is_whitespace) {
            text.push(' ');
        }
        text.push_str(interim);
    }
    if text.is_empty() {
        return 0;
    }
    // No row cap: grow the box rather than truncate with an ellipsis.
    let lines = super::wrap_voice_interim(&text, width.max(1) as usize, usize::MAX);
    (lines.len() as u16).max(1)
}

/// True when wrapping `text` at `width` needs more rows than `available_rows`.
pub fn recording_box_clips_spoken_text(text: &str, width: u16, available_rows: u16) -> bool {
    wrapped_transcript_rows("", Some(text), width) > available_rows
}
