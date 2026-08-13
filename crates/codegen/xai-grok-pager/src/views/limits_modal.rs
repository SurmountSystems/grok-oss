//! `/limits` popup modal — live meters with countdown, not a scrollback dump.
//!
//! Opens via slash `/limits` or status-bar meter click. Dismiss with Esc.
//! While open, the countdown ticks in place (days / hours / minutes / seconds).
//! When the countdown hits zero, the modal arms a silent billing refresh so
//! meters re-sample after period reset.

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::Theme;
use crate::views::limits_snapshot::{
    AllowanceMeterTone, LimitsSnapshot, countdown_is_zero, earliest_reset_at, format_limits_detail,
    format_reset_countdown,
};
use crate::views::modal_window::{
    self, ModalSizing, ModalWindowConfig, ModalWindowOutcome, ModalWindowState, Shortcut,
};
use crate::views::progress_bar::progress_bar_tracked_spans;

/// Title on the modal chrome.
pub const MODAL_TITLE: &str = "Limits";

/// View-state for the limits popup.
#[derive(Debug, Clone)]
pub struct LimitsModalState {
    pub window: ModalWindowState,
    /// Cached snapshot (rebuilt on billing refresh while open).
    pub snapshot: LimitsSnapshot,
    /// Vertical scroll of content lines.
    pub scroll: u16,
    /// When true, countdown already hit zero and a refresh was requested for
    /// this zero period (avoid spamming FetchBilling every tick).
    pub zero_refresh_sent: bool,
    /// Wall-clock of last snapshot apply (for tests / dogfood).
    pub last_updated_at: DateTime<Utc>,
}

impl LimitsModalState {
    pub fn new(snapshot: LimitsSnapshot) -> Self {
        Self {
            window: ModalWindowState::new(),
            snapshot,
            scroll: 0,
            zero_refresh_sent: false,
            last_updated_at: Utc::now(),
        }
    }

    /// Replace meters after a billing re-fetch (keeps window scroll chrome).
    pub fn apply_snapshot(&mut self, snapshot: LimitsSnapshot) {
        self.snapshot = snapshot;
        self.last_updated_at = Utc::now();
        // New period may have a future reset — allow another zero-refresh later.
        if let Some(reset) = earliest_reset_at(&self.snapshot) {
            if !countdown_is_zero(Utc::now(), reset) {
                self.zero_refresh_sent = false;
            }
        } else {
            self.zero_refresh_sent = false;
        }
    }

    /// Pure: should this modal request a silent billing refresh right now?
    ///
    /// True once when countdown reaches zero and we have not yet armed a
    /// refresh for this zero period.
    pub fn should_request_zero_refresh(&self, now: DateTime<Utc>) -> bool {
        if self.zero_refresh_sent {
            return false;
        }
        match earliest_reset_at(&self.snapshot) {
            Some(reset) => countdown_is_zero(now, reset),
            None => false,
        }
    }

    /// Mark that zero-refresh was requested (call after queuing FetchBilling).
    pub fn mark_zero_refresh_sent(&mut self) {
        self.zero_refresh_sent = true;
    }

    /// Content lines for render / tests (includes live countdown when known).
    pub fn content_lines(&self, now: DateTime<Utc>) -> Vec<String> {
        let mut body = format_limits_detail(&self.snapshot);
        if let Some(reset) = earliest_reset_at(&self.snapshot) {
            let countdown = format_reset_countdown(now, reset);
            // Inject countdown under the first "Next reset:" line.
            body = inject_countdown_line(&body, &countdown);
        }
        body.lines().map(str::to_owned).collect()
    }
}

/// Insert `Resets in: …` after the first `Next reset:` line.
fn inject_countdown_line(body: &str, countdown: &str) -> String {
    let mut out = String::with_capacity(body.len() + 40);
    let mut inserted = false;
    for line in body.lines() {
        out.push_str(line);
        out.push('\n');
        if !inserted && line.trim_start().starts_with("Next reset:") {
            out.push_str(&format!("  Resets in: {countdown}\n"));
            inserted = true;
        }
    }
    // Trim trailing newline to match format_limits_detail style.
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Key handling: Esc closes; arrows scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitsModalOutcome {
    Close,
    Changed,
    Unchanged,
}

pub fn handle_limits_key(state: &mut LimitsModalState, key: &KeyEvent) -> LimitsModalOutcome {
    let chrome_cfg = ModalWindowConfig {
        title: MODAL_TITLE,
        tabs: None,
        shortcuts: &[],
        sizing: ModalSizing::medium(),
        fold_info: None,
    };
    match modal_window::handle_modal_key(&mut state.window, key, &chrome_cfg) {
        ModalWindowOutcome::CloseRequested => return LimitsModalOutcome::Close,
        ModalWindowOutcome::Handled => return LimitsModalOutcome::Changed,
        ModalWindowOutcome::Unhandled => {}
        _ => return LimitsModalOutcome::Changed,
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => LimitsModalOutcome::Close,
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll = state.scroll.saturating_add(1);
            LimitsModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll = state.scroll.saturating_sub(1);
            LimitsModalOutcome::Changed
        }
        KeyCode::PageDown => {
            state.scroll = state.scroll.saturating_add(10);
            LimitsModalOutcome::Changed
        }
        KeyCode::PageUp => {
            state.scroll = state.scroll.saturating_sub(10);
            LimitsModalOutcome::Changed
        }
        KeyCode::Home => {
            state.scroll = 0;
            LimitsModalOutcome::Changed
        }
        _ => LimitsModalOutcome::Unchanged,
    }
}

fn tone_color(tone: AllowanceMeterTone, theme: &Theme) -> ratatui::style::Color {
    match tone {
        AllowanceMeterTone::Success => theme.accent_success,
        AllowanceMeterTone::Warning => theme.warning,
        AllowanceMeterTone::Danger => theme.accent_error,
    }
}

/// Render the limits modal into `area`.
pub fn render_limits_modal(
    buf: &mut Buffer,
    area: Rect,
    state: &mut LimitsModalState,
    theme: &Theme,
    compact: bool,
    now: DateTime<Utc>,
) {
    let shortcuts = [Shortcut {
        label: "Esc close",
        clickable: true,
        id: 1,
    }];
    let sizing = ModalSizing {
        width_pct: 0.55,
        max_width: 88,
        min_width: 48,
        v_margin: 3,
        h_pad: 2,
        v_pad: 1,
        footer_lines: 2,
    }
    .with_compact(compact);
    let config = ModalWindowConfig {
        title: MODAL_TITLE,
        tabs: None,
        shortcuts: &shortcuts,
        sizing,
        fold_info: None,
    };
    let Some(mca) = modal_window::render_modal_window(buf, area, &mut state.window, &config, theme)
    else {
        return;
    };
    let content = mca.content;
    if content.width == 0 || content.height == 0 {
        return;
    }

    // Word-wrap plain content to content width so long notes do not mid-word
    // truncate at the chrome edge (dogfood: shared-pool note cut at "person").
    let width = content.width as usize;
    let mut display_lines: Vec<String> = Vec::new();
    for raw in state.content_lines(now) {
        display_lines.extend(wrap_plain_line(&raw, width));
    }

    // Progress bar under primary included allowance when known. Inject a
    // sentinel into the display stream so scroll max and viewport layout
    // reserve a row for the bar (painting only after the text line left the
    // bar off-screen when the allowance line was the last content row).
    let primary_bar = state.snapshot.primary.included.as_ref().map(|inc| {
        let rem = inc.remaining_fraction();
        let tone = AllowanceMeterTone::from_used_pct(inc.used_pct);
        (rem, tone)
    });
    if primary_bar.is_some() {
        let mut with_bar = Vec::with_capacity(display_lines.len() + 1);
        let mut injected = false;
        for line in display_lines {
            let is_allowance_meter = is_included_allowance_used_line(&line);
            with_bar.push(line);
            if !injected && is_allowance_meter {
                with_bar.push(REMAINING_BAR_SENTINEL.to_string());
                injected = true;
            }
        }
        display_lines = with_bar;
    }

    let max_scroll = display_lines.len().saturating_sub(content.height as usize) as u16;
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }
    let start = state.scroll as usize;
    let end = (start + content.height as usize).min(display_lines.len());

    let mut y = content.y;
    for text in display_lines[start..end].iter() {
        if y >= content.y + content.height {
            break;
        }
        if text.as_str() == REMAINING_BAR_SENTINEL {
            if let Some((rem, tone)) = primary_bar {
                // Tracked bar: brackets + ░ empty so remaining extent is obvious.
                let bar_w = content.width.saturating_sub(2).min(34);
                if bar_w >= 4 {
                    let fg = tone_color(tone, theme);
                    let spans =
                        progress_bar_tracked_spans(bar_w, rem, fg, theme.gray_dim, theme.bg_dark);
                    let mut bar_line =
                        vec![Span::styled("  ", Style::default().fg(theme.text_primary))];
                    bar_line.extend(spans);
                    buf.set_line(content.x, y, &Line::from(bar_line), content.width);
                }
            }
            y = y.saturating_add(1);
            continue;
        }
        let style = line_style(text, theme);
        let line = Line::from(Span::styled(text.clone(), style));
        buf.set_line(content.x, y, &line, content.width);
        y = y.saturating_add(1);
    }
}

/// Sentinel row for the primary remaining bar (not user-visible text).
const REMAINING_BAR_SENTINEL: &str = "\u{0}limits-remaining-bar";

/// True when a wrapped display line is the SuperGrok included allowance meter
/// (has used %), not the "no data yet" placeholder.
fn is_included_allowance_used_line(text: &str) -> bool {
    text.contains("Included") && text.contains("allowance:") && text.contains("% used")
}

/// Word-wrap a single plain line to `width` columns (space breaks preferred).
fn wrap_plain_line(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.chars().count() <= width {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.chars().count() <= width {
            out.push(rest.to_string());
            break;
        }
        // Prefer last space within width; else hard-break.
        // `cols` is char count (matches `.chars().count()` above), not unicode display width.
        let mut end_byte = rest.len();
        let mut last_space: Option<usize> = None;
        for (cols, (i, ch)) in rest.char_indices().enumerate() {
            if cols >= width {
                end_byte = i;
                break;
            }
            if ch == ' ' {
                last_space = Some(i);
            }
        }
        let break_at = last_space.filter(|&s| s > 0).unwrap_or(end_byte);
        let (chunk, next) = rest.split_at(break_at);
        let chunk = chunk.trim_end();
        if !chunk.is_empty() {
            out.push(chunk.to_string());
        }
        rest = next.trim_start();
        if rest.is_empty() {
            break;
        }
        // Preserve indent on continuation when original was indented.
        if text.starts_with("  ") && !rest.starts_with(' ') {
            // Continuation of an indented field / note — keep flush under text.
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn line_style(text: &str, theme: &Theme) -> Style {
    if text.ends_with(':') && !text.starts_with(' ') {
        Style::default()
            .fg(theme.accent_system)
            .add_modifier(Modifier::BOLD)
    } else if text.contains("Resets in:") {
        Style::default().fg(theme.accent_system)
    } else if text.contains("% used") {
        // Pick tone from used % if we can parse it.
        let used = text
            .split('%')
            .next()
            .and_then(|s| s.rsplit(' ').next())
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or(0.0);
        Style::default().fg(tone_color(AllowanceMeterTone::from_used_pct(used), theme))
    } else if text.contains("Note:")
        || text.contains("share one SuperGrok weekly pool")
        || text.contains("shared consumer pool")
        || text.contains("unified billing")
        || text.contains("Grok Business")
    {
        // Note line and wrap continuations of the shared-pool note.
        Style::default().fg(theme.text_secondary)
    } else {
        Style::default().fg(theme.text_primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::credit_bar::{ConsoleTeamPrepaidGap, CreditBalance, SamplingIdentityKind};
    use crate::views::limits_snapshot::LimitsSnapshot;

    fn weekly_bal(pct: f64, reset_at: DateTime<Utc>) -> CreditBalance {
        CreditBalance {
            usage_pct: pct,
            effective_usage_pct: pct,
            period_end_display: Some(
                reset_at
                    .with_timezone(&chrono::Local)
                    .format("%B %-d, %H:%M")
                    .to_string(),
            ),
            period_end_at: Some(reset_at),
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: Some(1250),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
            is_unified_billing_user: None,
            grok_build_usage_pct: None,
            included_usage_known: true,
        }
    }

    #[test]
    fn modal_content_includes_countdown_d_h_m_s() {
        let reset = DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(24.0, reset);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let state = LimitsModalState::new(snap);
        let lines = state.content_lines(now);
        let joined = lines.join("\n");
        assert!(joined.contains("Resets in: 2d 7h 25m 0s"), "{joined}");
        // Body has no second "Limits" title (chrome owns MODAL_TITLE).
        assert!(!joined.starts_with("Limits\n"), "{joined}");
        assert!(joined.contains("Live sampling:"), "{joined}");
    }

    #[test]
    fn wrap_plain_line_breaks_on_spaces_not_mid_word() {
        let long = "Note: personal + business share one SuperGrok weekly pool and Extra Usage Credits (not console team prepaid).";
        let wrapped = wrap_plain_line(long, 40);
        assert!(wrapped.len() > 1, "{wrapped:?}");
        for line in &wrapped {
            assert!(
                line.chars().count() <= 40,
                "line too long: {line:?} ({} chars)",
                line.chars().count()
            );
            // No mid-word hard break of "personal" into "person" + "al" when spaces exist.
            assert!(
                !line.ends_with("person") || line.contains("personal"),
                "must not truncate mid-word to 'person': {line}"
            );
        }
        let joined = wrapped.join(" ");
        assert!(joined.contains("personal"), "{joined}");
        assert!(joined.contains("weekly pool"), "{joined}");
    }

    #[test]
    fn zero_refresh_triggers_once_then_arms_down() {
        let reset = DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = reset; // exactly zero
        let bal = weekly_bal(99.0, reset);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        assert!(state.should_request_zero_refresh(now));
        state.mark_zero_refresh_sent();
        assert!(!state.should_request_zero_refresh(now));
    }

    #[test]
    fn zero_refresh_not_before_deadline() {
        let reset = DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(50.0, reset);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let state = LimitsModalState::new(snap);
        assert!(!state.should_request_zero_refresh(now));
    }

    #[test]
    fn console_key_requests_supergrok_in_modal_body_when_supergrok_live() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession)
            .with_console_key_available(true)
            .with_console_prepaid_gap(ConsoleTeamPrepaidGap::MissingManagementKey);
        let state = LimitsModalState::new(snap);
        let joined = state.content_lines(Utc::now()).join("\n");
        assert!(
            joined.contains("Requests: SuperGrok"),
            "key on file must not read as missing: {joined}"
        );
        assert!(
            !joined.contains("no key"),
            "key on file must not say no key: {joined}"
        );
        assert!(
            !joined.contains("saved"),
            "omit saved; presence is implicit: {joined}"
        );
        assert!(!joined.contains("Path:"), "Path: wording retired: {joined}");
        // Short Balance gap only — no Management Key lecture wall.
        assert!(
            joined.contains("Balance: no management key"),
            "short balance gap: {joined}"
        );
        assert!(
            !joined.contains("Management API key")
                && !joined.contains("Management Keys")
                && !joined.contains("team prepaid needs"),
            "must not lecture Management Key for chat-key honesty: {joined}"
        );
        let requests_line = joined
            .lines()
            .find(|l| l.trim_start().starts_with("Requests:"))
            .expect("requests line");
        assert!(
            !requests_line.to_ascii_lowercase().contains("management"),
            "Requests line must not mention management: {requests_line}"
        );
    }

    #[test]
    fn inject_countdown_after_next_reset() {
        let body = "Live sampling: SuperGrok session\n\nSuperGrok:\n  Next reset: August 3, 19:25\n  SuperGrok dollar extras: $1";
        let out = inject_countdown_line(body, "1d 2h 3m 4s");
        assert!(out.contains("Next reset: August 3, 19:25\n  Resets in: 1d 2h 3m 4s\n"));
    }

    #[test]
    fn esc_closes_limits_modal() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let key = KeyEvent::from(KeyCode::Esc);
        assert_eq!(
            handle_limits_key(&mut state, &key),
            LimitsModalOutcome::Close
        );
    }

    #[test]
    fn q_closes_limits_modal() {
        let snap = LimitsSnapshot::from_billing(None, None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let key = KeyEvent::from(KeyCode::Char('q'));
        assert_eq!(
            handle_limits_key(&mut state, &key),
            LimitsModalOutcome::Close
        );
    }

    /// Named contract: remaining bar paints track end bounds (`[` `]`) and
    /// visible empty track cells (`░`), not space-only fill that hides max extent.
    #[test]
    fn render_paints_tracked_remaining_bar_with_bounds() {
        let reset = DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(25.0, reset); // 75% remaining
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        render_limits_modal(&mut buf, area, &mut state, &theme, false, now);

        // Scan buffer for a tracked bar row: starts with `[` and has `░` or `█`.
        let mut found_track = false;
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                row.push_str(buf[(x, y)].symbol());
            }
            if row.contains('[') && row.contains(']') && (row.contains('█') || row.contains('░'))
            {
                found_track = true;
                assert!(
                    row.contains('░') || row.matches('█').count() >= 2,
                    "track must show empty or filled cells inside brackets: {row}"
                );
                break;
            }
        }
        assert!(
            found_track,
            "limits modal must paint a tracked remaining bar with [ ] bounds"
        );
    }

    /// Named contract: click on dimmed backdrop (outside popup) closes Limits.
    #[test]
    fn click_outside_popup_closes_limits_modal() {
        use crate::views::modal_window::{self as mw, ModalWindowOutcome};
        use crossterm::event::{MouseButton, MouseEventKind};

        let reset = DateTime::parse_from_rfc3339("2026-08-03T19:25:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bal = weekly_bal(50.0, reset);
        let snap =
            LimitsSnapshot::from_billing(Some(&bal), None, SamplingIdentityKind::SuperGrokSession);
        let mut state = LimitsModalState::new(snap);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        // Render sets popup_area / close_button_rect on window state.
        render_limits_modal(&mut buf, area, &mut state, &theme, false, now);
        let popup = state.window.popup_area.expect("render must set popup_area");
        // Corner of full area, outside centered popup.
        assert!(
            popup.x > 0 && popup.y > 0,
            "popup should be inset so outside click is possible: {popup:?}"
        );
        let outcome = mw::handle_modal_mouse(
            &mut state.window,
            MouseEventKind::Down(MouseButton::Left),
            0,
            0,
        );
        assert_eq!(
            outcome,
            ModalWindowOutcome::CloseRequested,
            "click outside Limits chrome must request close"
        );
    }
}
